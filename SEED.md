# BoOS — Seed

BoOS 是运行在 Linux 之上的 AI 原生控制层。它不重做内核，也不试图取代
Hermes、OpenClaw 之类的 planner/runtime；它研究 planner 之下本应由系统
负责的部分：AI 身份、状态所有权、能力边界、持久请求与结果，以及未来的
skill 共享和隔离。

这份文档描述当前可运行的系统，不把历史实验或 roadmap 写成已实现能力。

## 当前边界

产品镜像启动后，不等待外部客户端：

```text
/init
  └─ boos-supervisor
      ├─ boos-agent resident
      │   └─ 建立 session，原子发布 ready/heartbeat
      └─ 内建 request processor
```

默认 resident 进程是一个 AI principal 的常驻运行槽，不是完整 LLM planner。
它证明系统能在没有 TCP、宿主机客户端或模型 API 的情况下建立 AI 所有的身份
和状态。`ready` 只表示本地生命周期与接口存活。

TCP gateway 仍被打包，但产品配置默认 `enabled=0`。CI 使用临时 overlay
开启 gateway，并把它绑定到独立的 `debug` principal；gateway 不是启动依赖。

## 物理形态

```text
QEMU 虚拟机
├── 与模块匹配的 Linux 内核
├── initramfs
│   ├── /init
│   ├── /bin/boos                    Rust 静态 multicall 二进制
│   ├── /bin/boos-{agent,exec,...}   指向 boos 的符号链接
│   ├── /bin/busybox
│   └── /etc/boos/
│       ├── commands/*.cmd
│       ├── capabilities.conf
│       ├── principals/*.principal
│       └── daemons/*.daemon
└── 持久 /var
    ├── boos/principals/resident/
    ├── boos/principals/debug/
    └── log/boos.log
```

每个 principal 的运行目录为：

```text
/var/boos/principals/<id>/
├── status.kv
├── memory/
│   ├── working.kv
│   ├── recent/
│   └── archive/
├── requests/
└── results/
```

旧的全局 `/var/boos/{memory,requests,results}` 不会被静默迁移。系统无法可靠
判断旧共享数据属于谁，自动归属会破坏隔离语义。

## 身份与信任

principal 定义位于 `/etc/boos/principals/*.principal`：

```text
id=resident
user=boos-agent
uid=101
gid=101
enabled=1
```

`BOOS_PRINCIPAL_ID` 只是选择器，不是授权凭证。Rust runtime 同时验证：

1. ID 语法合法；
2. 定义存在且启用；
3. 当前进程的 effective UID 与定义相符，且不同 principal 不能配置重复 UID；
4. 配置 GID 用于可信 supervisor/processor 的降权和结果组权限；
5. processor 子进程会清除 root 的 supplementary groups。

验证失败时，所有有状态命令 fail closed。兼容变量 `BOOS_AGENT_ID` 仅在主变量
缺失时使用，也必须通过相同的 effective UID 验证。

请求记录中的 `requester` 只用于追踪。请求属于哪个 principal，由它所在的
spool 决定；root processor 可以遍历所有已启用 spool，但结果仍写回原
principal 的目录。

## 多调用二进制

`main.rs` 按 `argv[0]` 分发：

| 入口 | 责任 |
|---|---|
| `boos-agent` | resident 生命周期；显式进入历史 agent 实验 |
| `boos-supervisor` | 启动、监控 workload；内建队列轮询 |
| `boos-exec` | 命令注册、能力检查与执行 |
| `boos-submit` | 向当前 principal 原子发布请求 |
| `boos-process` | 处理所有启用 principal 的 spool |
| `boos-shell` | 本地交互适配器 |
| `boos-gateway` | 可选 TCP 调试和模型代理适配器 |

代码按责任拆分：principal、memory namespace、queue record、atomic publish、
locking、gateway policy 等是独立边界，flow 模块只组合这些能力。

## 一次命令的路径

直接执行：

```text
resident workload / local shell / optional gateway
  → boos-exec
  → command registry
  → immutable deny + capability policy
  → builtin 或白名单外部程序
  → stdout/stderr + audit log
```

持久队列：

```text
current PrincipalContext
  → boos-submit
  → /var/boos/principals/<id>/requests/req-*
  → root supervisor 内建 processor
  → boos-exec
  → /var/boos/principals/<id>/results/req-*.out
```

发布请求、结果和 resident 状态均采用临时文件加 rename。processor 使用锁
避免同一请求被并发重复处理。result 文件只对 root 与所属 principal group
可读。

## AI 运行方式

### 产品默认：resident

```text
boos-agent
boos-agent resident
```

两者都进入相同常驻生命周期，不启动 gateway，不自动调用模型。

### 可选适配器：gateway

gateway 用于调试、历史客户端和模型 API 代理。没有 token 时只绑定 loopback；
配置非空 token 后，远程客户端必须先 `AUTH`。协议本身是明文 TCP，跨不可信
网络必须再使用加密隧道。`FETCH` 默认关闭，开启时仅允许精确列出的 HTTPS
公共地址。

### 历史实验

`boos-agent explore`、`loop`、`develop` 仍可显式调用，用于复现实验。它们
不是产品启动路径，也不代表当前 BoOS 的架构中心。

## 安全边界

- Linux effective UID 是 principal 身份锚点；配置 GID 用于降权和文件组权限。
- 每个 principal 的 memory、requests、results 目录互相隔离。
- 编译期 immutable deny 封锁不可逆能力。
- `/etc/boos` 的 capability policy 决定命令可用性。
- 文件访问会规范化路径并保护系统目录。
- 外部进程执行受白名单和资源边界约束。
- 写入有大小上限，持久记录采用原子发布。
- gateway secret 由独立 `boos-gateway` 用户持有。
- product rootfs 不启动网络 gateway。

这些机制缩小攻击面，但不等于形式化安全证明。攻击回归与研究结论必须分开：
普通测试证明已定义的边界没有回归；它不能证明 BoOS 的假设充分或优于其他
系统。

## 构建与验证

本机资源不足时应在远程 builder 或一次性容器中完成编译，避免把 target
目录写回开发机。

```bash
cd src/rust
cargo test
cargo build --release --target x86_64-unknown-linux-musl

cd ../..
commit=$(git rev-parse HEAD)
source_date_epoch=$(git show -s --format=%ct "$commit")
scripts/assemble-initramfs.sh \
  /path/to/matching/initramfs-virt \
  rootfs \
  src/rust/target/x86_64-unknown-linux-musl/release/boos \
  build/initramfs.cpio.gz \
  "$commit" \
  "$source_date_epoch"
tests/boot/verify-initramfs.sh \
  build/initramfs.cpio.gz \
  rootfs/init \
  src/rust/target/x86_64-unknown-linux-musl/release/boos \
  /path/to/matching/vmlinuz-virt \
  "$commit"
```

验证分三层：

1. Rust 单元/行为测试：身份解析、越权失败、namespace、并发与错误边界；
2. initramfs artifact verifier：必需文件、配置、内核与 module tree 一致性；
3. 真实 QEMU：resident ready、持久 `/var`、产品无 gateway、CI debug
   overlay 的认证与隔离。

不要用 `nc -z` 证明 guest gateway 存活：QEMU host forwarding 可能在 guest
没有 listener 时仍接受 host TCP。必须发送真实协议并收到预期响应。

## 开发约定

1. 配置和内部 wire record 使用有边界的 `key=value` 格式。
2. 状态先写同目录临时文件，再原子 rename。
3. 外部输入、环境变量和文件内容都不可信。
4. 授权来自 trusted boundary，不来自请求正文。
5. 修改安全行为前先建立能失败的行为测试。
6. 不用预设 BoOS 优势的 benchmark 代替充分性验证。
7. API key、token 和生产配置不进入仓库。
8. product 与 CI overlay 分离；测试便利配置不能渗入产品 rootfs。

## 代码导航

| 文件 | 责任 |
|---|---|
| `src/rust/src/principal.rs` | principal 定义、effective UID 验证、运行路径 |
| `src/rust/src/resident_agent.rs` | ready 与 heartbeat 生命周期 |
| `src/rust/src/memory.rs` | working/recent/archive 行为 |
| `src/rust/src/memory_namespace.rs` | principal memory 路径边界 |
| `src/rust/src/submit.rs` | 当前 principal 的请求提交 flow |
| `src/rust/src/process.rs` | 跨 spool 处理、owner 传播 |
| `src/rust/src/queue_record.rs` | request/result wire record |
| `src/rust/src/request_publish.rs` | 原子请求发布 |
| `src/rust/src/queue_lock.rs` | 并发处理锁 |
| `src/rust/src/supervisor.rs` | workload 生命周期与内建 processor |
| `src/rust/src/gateway.rs` | 可选 TCP/model adapter |
| `src/rust/src/gateway_policy.rs` | gateway 认证与网络策略 |
| `src/rust/src/world*.rs` | semantic object 只读实验 |
| `rootfs/init` | mount、用户和目录权限、启动 |

## 下一阶段：skill 共享与隔离

当前代码只建立了可信 principal 边界，尚未实现共享 skill pool。下一阶段应把
skill 视为带版本和来源的不可变对象，为每个 principal 提供一个 view：

- private overlay；
- 显式挂载的 shared collection；
- 每个任务固定 snapshot，避免执行中热更新；
- publish/promote、provenance、dependency、rollback；
- opt-in subscription，而不是全局强制同步。

这既允许多个 AI 共通一部分 skill，也允许项目、身份和任务按需隔离。更完整
设计见
[resident principal boundary design](docs/superpowers/specs/2026-07-30-boos-resident-principal-boundary-design.md)。
