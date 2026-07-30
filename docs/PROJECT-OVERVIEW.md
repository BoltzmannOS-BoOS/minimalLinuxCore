# BoOS Project Overview

> BoOS 不替代 Linux；它研究 AI 作为原生操作者时，planner 之下应该存在什么
> 系统边界。

## 当前目标

把已经完成的 resident principal 基础收束成可信产品边界，然后进入
skill 共享与隔离：

```text
Linux UID/GID
  → BoOS principal
  → principal-owned memory / requests / results
  → per-principal skill view（下一阶段）
```

## 已完成

- 产品开机启动 `resident` principal，不依赖外部 TCP 客户端；
- principal 身份由配置和 effective UID/GID 共同验证；
- memory、request spool、result spool 按 principal 隔离；
- request 正文不能伪造 owner；
- supervisor 内建队列处理，不再依赖旧 shell daemon；
- gateway 退为可选 `debug` adapter，产品配置默认关闭；
- CI overlay 可以单独开启带认证的 debug gateway；
- initramfs verifier 检查产品配置和 kernel/module compatibility；
- semantic object layer 保留为只读 wiring experiment。

## 进行中

- 最终清理 gateway 时代的无调用产物；
- 用远程 Rust、artifact 和真实 QEMU 验证 release candidate；
- 审查文档不再把 roadmap、历史实验或测试通过写成产品能力证明。

## 下一步

### Phase 2：Skill views

skill 不应是一个所有 AI 自动读取的全局文件夹。建议模型是：

```text
immutable skill versions
  ├─ private overlay
  ├─ explicitly mounted shared collections
  └─ task-pinned snapshot
```

需要实现：

- skill content、版本、来源和依赖 metadata；
- principal 自己的 view；
- 显式 share/mount 和 publish/promote；
- task 开始时 pin snapshot，执行中不被热更新改变；
- 更新、回滚和冲突的证据链。

### Phase 3：Subscriptions and coordination

- opt-in skill hot-update；
- revocation 与 rollback；
- principal event delivery；
- 多 AI 并发发布的一致性；
- 共享有利和隔离必要的真实跨项目案例。

### 持续研究

- AI 需要的是哪些稳定 OS 对象，而不是更多拟人 CLI；
- context 压力、长任务恢复和 provenance 应由哪一层负责；
- 何时共享知识，何时复制或隔离；
- benchmark 如何持续变化，避免把实现假设写成获胜条件。

## 阻塞项

当前 principal boundary 无已知实现阻塞。Skill views 在进入编码前仍需要用
真实跨项目工作流确定最小对象模型，避免先造一个与现有 skill book 等价的
静态目录。

## 系统边界

### 产品启动

```text
/init
  └─ boos-supervisor
      ├─ boos-agent resident             UID/GID 101
      └─ built-in request processor      root-owned traversal
```

`resident` 发布 ready 和 heartbeat，表示本地 runtime slot 存活；不表示模型
已配置或产生了推理。

### 可选 adapter

```text
boos-gateway                        UID/GID 100
  → principal=debug
  → /var/boos/principals/debug/*
```

产品 rootfs 不启动 gateway。调试和 CI 可通过 overlay 显式开启。gateway
authentication、FETCH allowlist 和 secret ownership 仍然有效，但它们不再
定义 BoOS 的核心。

### 状态所有权

```text
/etc/boos/principals/<id>.principal
  → id + user + uid + gid + enabled

/var/boos/principals/<id>/
  → status.kv
  → memory/
  → requests/
  → results/
```

环境变量只选择 principal；UID/GID 才是信任锚。`requester` 是 trace 字段，
spool owner 才决定 request 和 result 属于谁。

## 当前能力与限度

| 能力 | 当前状态 | 限度 |
|---|---|---|
| resident 生命周期 | 已实现 | 不是 planner 或模型 |
| principal identity | 已实现 | 只覆盖本机 Linux identity |
| memory/queue 隔离 | 已实现 | 暂无跨 principal 管理接口 |
| capability-checked commands | 已实现 | 不构成形式化安全证明 |
| optional gateway | 已实现 | 明文 TCP，外网需加密隧道 |
| semantic objects | 只读实验 | 只证明 interface/wiring |
| skill sharing | 未实现 | Phase 2 |
| multi-AI scheduling | 未实现 | 不应与 skill sharing 混为一谈 |

## 研究纪律

普通测试回答“代码是否满足已经定义的 contract”；它不能回答：

- AI 是否真正需要这层 OS；
- 这个 abstraction 是否充分；
- BoOS 是否比 agent runtime 或普通 Linux 更好；
- benchmark 是否代表现实。

因此 semantic object 现有实验被明确标记为 **Test 0: Interface and Wiring
Probe**。新的比较实验必须从真实任务采样，包含 BoOS 可能无优势或失败的条件，
并进入 [Living Evidence System](../tests/evidence/README.md)。

## 设计取舍

- Linux 继续负责 process、filesystem、UID/GID 和资源隔离；
- BoOS 负责 principal、owned state、capability、durable flow 和语义接口；
- planner/runtime 负责模型推理、策略和任务编排；
- adapter 负责 TCP、CLI 或特定 provider，不反向成为 core；
- 不因为消除少量重复而建立大型 framework；
- product config 与 CI/debug overlay 分开。

## 相关文件

| 位置 | 内容 |
|---|---|
| [`../SEED.md`](../SEED.md) | 当前 runtime、信任边界、构建和代码地图 |
| [`superpowers/specs/2026-07-30-boos-resident-principal-boundary-design.md`](superpowers/specs/2026-07-30-boos-resident-principal-boundary-design.md) | 已批准 principal boundary 设计 |
| [`superpowers/plans/2026-07-30-boos-resident-principal-boundary.md`](superpowers/plans/2026-07-30-boos-resident-principal-boundary.md) | 分步实现与验证计划 |
| [`../src/rust/src/principal.rs`](../src/rust/src/principal.rs) | UID/GID principal identity |
| [`../src/rust/src/resident_agent.rs`](../src/rust/src/resident_agent.rs) | resident lifecycle |
| [`../src/rust/src/supervisor.rs`](../src/rust/src/supervisor.rs) | workload 与 queue orchestration |
| [`../tests/evidence/README.md`](../tests/evidence/README.md) | 研究证据规则 |

*Current snapshot: 2026-07-30*
