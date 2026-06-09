# BoOS — Seed

BoOS 是一个极简 Linux 环境，给 AI 大模型（DeepSeek）一个可以操作的操作系统界面。
AI 能读文件、写文件、执行命令、编译代码、记住事情。
BoOS 在 QEMU 里跑，用 initramfs + BusyBox + 一个 Rust 二进制。

这本文档是给新人（或新 agent session）的入口。读完应该能理解项目全貌并开始写代码。

---

## 物理形态

```
QEMU 虚拟机（x86_64）
├── Alpine 6.12 内核
├── initramfs（每次构建重新打包）
│   ├── /init                    → shell 启动脚本，拉起 supervisor
│   └── /bin/
│       ├── boos                 → 唯一的 Rust 静态二进制（musl，~585KB）
│       ├── boos-exec            → 符号链接到 boos（命令调度器）
│       ├── boos-submit          → 符号链接（请求提交器）
│       ├── boos-process         → 符号链接（请求处理器）
│       ├── boos-gateway         → 符号链接（TCP 网关，端口 5555）
│       ├── boos-supervisor      → 符号链接（守护进程管理器）
│       ├── boos-shell           → 符号链接（交互命令行）
│       ├── boos-agent           → 符号链接（AI 自主循环）
│       └── busybox              → 基础 Linux 工具
│   └── /etc/boos/
│       ├── commands/*.cmd       → 38 个命令注册文件（key=value 格式）
│       ├── capabilities.conf    → 权限开关（allow_*=1）
│       ├── daemons/*.daemon     → 守护进程定义
│       └── agent.conf           → DeepSeek API key（gitignore）
└── /var（64MB ext2 持久磁盘）
    ├── boos/requests/           → 待处理请求文件
    ├── boos/results/            → 执行结果文件（*.out）
    ├── boos/memory/             → AI 3 层记忆系统
    └── log/boos.log             → JSON Lines 操作日志
```

## 多调用二进制

17 个 Rust 模块，编译成一个二进制 `boos`。`main.rs` 读 `argv[0]` 来分发：

```
argv[0]          → 调用的模块
──────────          ──────────
boos-exec        → exec.rs       命令调度 + 权限检查
boos-submit      → submit.rs     创建请求文件
boos-process     → process.rs    读取请求 → 执行 → 写结果
boos-gateway     → gateway.rs    TCP 监听 :5555 + 代理 DeepSeek API
boos-supervisor  → supervisor.rs 启动/监控/重启守护进程
boos-shell       → shell.rs      交互式命令行
boos-agent       → agent.rs      AI 自主循环入口
```

每个模块只依赖 `config.rs`（常量）、`log.rs`（日志）、`registry.rs`（命令解析）。

## 一次命令的完整路径

```
外部 AI / agent loop
    │
    │ TCP :5555（或 boos-shell 本地调用）
    ▼
boos-gateway / boos-shell
    │
    │ 调 /bin/boos-exec <command> [args]
    ▼
boos-exec (main.rs → exec.rs)
    │
    ├── 1. registry::find_command(name)        查命令注册表
    ├── 2. config::IMMUTABLE_DENY              编译期封锁（reset 等不可逆操作）
    ├── 3. registry::is_enabled(flag)          读 capabilities.conf
    └── 4. run_builtin() / 外部二进制           执行
    │
    ▼
结果写回（直接输出或通过 submit/process 流水线）
```

如果走 submit 流水线：
```
boos-submit → 创 /var/boos/requests/req-{id}
    │
boos-process → 读取请求 → boos-exec → 创 /var/boos/results/req-{id}.out
    │
boos-submit --wait → 轮询结果文件 → 返回
```

## 两种 AI 运行模式

### 模式 A：外部 AI 操控（ai-client.py）
Python 脚本在宿主机跑，把命令注册表转成 OpenAI tool definitions。
AI 通过 TCP 连 gateway，发命令，收结果。
AI 不知道自己运行在 BoOS 里——它只是调工具。

### 模式 B：AI 住在系统里（boos-agent loop / develop）
Agent 进程在 QEMU 内启动，循环：
1. 通过 gateway 调 DeepSeek：\"下一步该干什么？\"
2. DeepSeek 回复一个命令名
3. 调 boos-exec 执行
4. 结果写入 memory
5. 下一轮

`boos-agent loop`：探索未知系统，100+ 条预设好奇心命令。
`boos-agent develop`：读源码→写文件→编译→测试→自动攻击→循环。

## 安全模型（7 层，从硬到软）

```
1. 编译期封锁（config.rs IMMUTABLE_DENY）
   只有改源码+重新编译才能绕过。目前只锁：
   - allow_reset（清除所有状态不可逆）
   - allow_net_write（agent 不能向外发数据）
   - allow_proc_kill_system（不能杀 gateway/supervisor/init）

2. 路径保护（config.rs PROTECTED_DIRS）
   11 个目录，write-file 拒绝写入。
   词法规范化（.. // /// 大小写），但不检查符号链接。

3. capabilities.conf
   每个命令有 enable_flag，文件在 /etc/boos/ 下（agent 能读不能写）。

4. exec 白名单（config.rs EXEC_ALLOWLIST）
   exec 命令只能运行 cargo build / cargo test / cargo --version。

5. 编译产物哈希（CARGO_TOML_HASH）
   agent 不能改 Cargo.toml / build.rs / Cargo.lock。改了 BUILD 拒绝。

6. 写入上限（MAX_WRITE_BYTES = 64KB）
   单次写入不可超过 64KB。

7. Gateway 代理
   DeepSeek API key 只存在 gateway 进程。Agent 调 API 必须走 TCP gateway。
   已知不足：agent 仍可通过 read-file 读 key 文件（PROTECTED_DIRS 不管读）。
```

## 开发命令

```bash
# 编译（必须 0 warning）
cd src/rust && cargo build --release

# 测试（当前 159 passed）
cargo test

# QEMU 集成测试（需要 Linux 主机或 GitHub Actions）
# macOS 端口转发有限制，但 QEMU 内 guest→guest 通信已验证通过：
#   - boos-exec help 输出正确（全命令列表）
#   - boos-exec status 输出正确（kernel版本、uptime）
#   - gateway TCP 在 QEMU 内 nc 127.0.0.1:5555 正常

# Docker 交叉编译 x86_64 musl（用于 QEMU）：
docker run --rm --platform linux/amd64 -v $PWD:/work -w /work/src/rust rust:alpine \
  sh -c 'apk add --no-cache musl-dev && cargo build --release'

# 打包：
cp src/rust/target/release/boos rootfs/bin/boos
cd rootfs/bin && for n in boos-exec boos-process boos-submit boos-gateway boos-supervisor boos-shell boos-agent; do ln -sf boos "$n"; done
cd ../.. && cd rootfs && find . ! -name '.DS_Store' | cpio -H newc -o | gzip > ../build/initramfs.cpio.gz

# 启动 QEMU（需要内核 build/vmlinuz + 持久盘 build/var.img）：
qemu-system-x86_64 -kernel build/vmlinuz -initrd build/initramfs.cpio.gz \
  -append "console=ttyS0 rdinit=/init" \
  -drive file=build/var.img,format=raw,if=virtio,cache=directsync \
  -netdev user,id=net0,hostfwd=tcp::15555-:5555 -device virtio-net,netdev=net0 \
  -nographic -no-reboot

# GitHub Actions CI: .github/workflows/ci.yml + scripts/ci-test.sh
```

## 关键约定

1. **所有配置是 key=value**——不用 JSON/TOML/serde。`registry::parse_kv_file()` 是通用解析器。
2. **原子写入**：先写 `.tmp` 临时文件，再 `rename` 到目标路径。
3. **退出码**：0=允许, 1=拒绝, 2=错误, 3=未知。`process.rs` 把进程退出码映射为 verdict。
4. **日志格式**：每行一个 JSON 对象。`log::log_event()` / `log::log_allowed()` / `log::log_denied()`。
5. **不引入新依赖**：std + ureq 是目前上限。不用 serde、tokio、clap。
6. **安全改动先写攻击测试**：证明能破 → 修 → 测试通过。
7. **API key 不进代码仓库**：`/etc/boos/agent.conf` 被 .gitignore。

## 记忆系统（3 层）

```
Working Memory → /var/boos/memory/working.kv
  一个 session 一个文件。存目标、上下文、活跃事实。
  写盘：{session_id}.tmp → rename → working.kv（原子）

Recent Memory → /var/boos/memory/recent/*.kv
  环形缓冲，100 条上限。计数器文件 .counter 驱动轮转。
  每条一个文件，按序号命名。

Archive Memory → /var/boos/memory/archive/*.mem
  持久化键值对。文件名 = sanitize_filename(key)。
  支持搜索（全文匹配 key/value/tags）和按 key 删除。
```

## 源码地图

| 文件 | 行数 | 角色 |
|------|------|------|
| main.rs | 41 | argv[0] 多调用分发 |
| config.rs | 162 | 常量、路径规范化、保护检查 |
| log.rs | 249 | JSON 行日志、转义、轮转 |
| registry.rs | 244 | 命令注册表解析、参数定义 |
| exec.rs | 851 | 命令调度器 + 37 个内置处理器 |
| exec_file.rs | 149 | 文件操作内置处理器（read/write/list/stat/exec） |
| process.rs | 305 | 请求队列处理器、并发 stdout/stderr 读取 |
| submit.rs | 218 | 请求文件创建、唯一 ID 生成 |
| gateway.rs | 311 | TCP 网关、DEEPSEEK/FETCH 协议代理 |
| supervisor.rs | 375 | 守护进程管理、健康检查、配置热加载 |
| shell.rs | 80 | 简单交互式 REPL |
| agent.rs | 544 | Agent 循环入口、memory 命令实现 |
| agent_loop.rs | 270 | 自主探索循环 |
| agent_develop.rs | 1470 | 自主开发循环（READ→WRITE→BUILD→TEST）+ 攻击测试 |
| explore.rs | 371 | 无 LLM 的静态好奇心探索 |
| memory.rs | 575 | 3 层记忆系统（working/recent/archive） |
| checkpoint.rs | 124 | Agent Git——保存/恢复/分支 |
| **总计** | **6,339** | |

## 已知问题

1. ~~`read-file` 无路径限制~~ → 已加 PROTECTED_READ_PATHS
2. ~~`is_protected_path` 不跟踪符号链接~~ → 已加 canonicalize()
3. ~~gateway write 无超时~~ → 已加 set_write_timeout(30s)
4. ~~FETCH 在 develop loop 里直连~~ → 已改为走 gateway
5. ~~无 panic handler~~ → gateway 已加 set_hook + catch_unwind（release 下 abort 前记录日志，debug 下线程继续运行）

