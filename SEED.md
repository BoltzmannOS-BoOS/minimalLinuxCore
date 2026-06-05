# BoOS: Agent-Owned Operating System — Seed

Seed Name: boos-agent-native-os
Seed Type: system / infrastructure
Target Domain: AI agent primary operating environment
Goal: 构建一个 AI agent 作为第一操作者的最小 Linux 身体

---

## Principles（原则）

> 以下不是规则列表，而是每次设计决策前的检查清单。

### 架构原则

1. **AI is the subject, not the object** — 系统服务 AI，不是人看守 AI。agent 有 near-native 系统权限。
2. **Observe, don't obstruct** — logging 是为了理解 agent 行为，不是为了限制。agent 可以读任何文件。
3. **BIOS vs BoOS** — 硬编码边界（编译进二进制）vs 文件配置边界（agent 能看到但改不了）。BIOS 只兜底不可逆破坏（reset），其他权限开放。
4. **Agent 的身体一次给完** — write-file / read-file / list-dir / stat / exec 是 agent 的全部文件能力，不再升级 API。所有安全边界必须在现有 API 内封死。
5. **路径保护 > 命题保护** — 不能说"agent 不能改系统"，要说"agent 不能写 /etc /bin /sbin /usr /lib /boot /proc /var/boos/results /var/boos/memory /var/log"。具体路径，编译期检查。

### 安全原则

6. **真正的修复不是下调上限** — 不是"agent 连续失败 3 次就禁止它"，而是系统设计层面消除攻击面。
7. **编译期 > 文件系统 > 运行时** — 安全优先级：硬编码拒绝（BIOS）> 路径读写保护（PROTECTED_DIRS）> 文件权限（capabilities.conf）。
8. **API key 不在 agent 的可见世界** — key 通过 --api-key 命令行注入，agent 进程中不存在持久化的 key 文件。
9. **攻击驱动开发** — 每个安全决策必须有对应的攻击测试。先攻击，记录漏洞，再修复，测试验证。

### 开发原则

10. **No JSON, no serde** — 所有配置是 key=value 格式，不用 JS/TS 工具链。
11. **Pure Rust, musl static** — 单二进制，585KB，零外部依赖（std only）。
12. **测试即文档** — 137 个单元测试描述了所有边界条件。攻击测试即安全审计。
13. **先攻击后防御** — 不是"我觉得这里可能有问题"，是"我在这里打破了它，然后修好了"。

### 内核增长规则（Kernel Growth Rules）

> 以下不是加更多规则，是改变开发节奏——每加一个功能，必须先写好攻击测试。

14. **能力前置于功能** — 新模块开发前，先在 `config.rs` 确定：① IMMUTABLE_DENY 要不要拦？② PROTECTED_DIRS 要不要保护？不加这两行，功能不算做完。
15. **攻击前置于合并** — `feature.rs → attack_feature.rs → fix → feature.rs`。攻击测试和功能测试同时写，不是事后补。
16. **窄接口** — 内核组件只通过协议通信（submit/result, DEEPSEEK），不共享内存、不共享文件系统、不互相调用函数。裂脑原则推广到所有组件。
17. **新能力默认只读** — 网络、进程管理、新文件系统操作都是先只读。能读清楚的，才开写权限。写了出问题的，回滚到只读。
18. **内核不信任 agent** — 每次请求都跑 capability check + audit log。不存在"agent 已经验证过了"的捷径。

### 优化自指（Cost-Aware Self-Reference）

> 系统不仅要知道"能优化什么"，还要知道"该优化什么"。自指不是反思功能，是**把成本也变成 SEED 的一层**。

19. **成本决定优化方向** — 每次迭代前先问：①什么最贵（API token / 人时 / 系统崩溃）？②下个改动能省多少？需要投多少？③边际效应到了吗？
20. **不同资源不同策略** — 计算资源富足时重速度（大胆探索），API token 贵时重回报率（精确出击），人时稀缺时重自动化（让系统自己跑）。
21. **停止也是优化** — 攻击 75 轮后新漏洞发现率暴跌 → 继续攻击的 ROI 为负 → 该停了。承认"到边界了"本身就是一次有效优化。
22. **每次 Refinement Log 记录成本** — 不只记录"做了什么"，也记录"花了什么"和"值不值"。让后续决策有数据可查。

---

## Growth Framework（生长框架）

### Development Layers — Meta-Automation Upgrade Path

> 如 Factorio：Layer 0 手动采集，每层自动化下一层。升级的是开发方式，不是功能列表。

```
Layer 0: 手动（当前终端）
  指令 → 执行 → 审查 → 修正
  = 80 轮攻击/防御，每轮由人触发

Layer 1: 自动化流程（已完成）
  功能 → 先攻击 → 修复 → 合并
  = 5 条内核增长规则 → 攻击变成开发流程的固定步骤

Layer 2: Agent 自我攻击（下一步）
  agent 记住攻击模式 → 写新功能时自己先攻击 → 自己修 → 人审查
  SEED.md 的攻击模式库被 agent 自动消费

Layer 3: 攻击模式自进化
  系统识别新攻击类 → 自动更新防御模板 → agent 用新模板攻击自己
  攻击知识自己生长，路径依赖不可避免，复杂度上限持续升高

成本自指（贯穿所有 Layer）：
  Layer 0 → 人时是瓶颈，优化目标 = 减少每次迭代的人介入时间
  Layer 1 → API token 是主要成本，优化目标 = 减少无效调用
  Layer 2 → 审查时间 > 执行时间，优化目标 = 提高 agent 报告可信度
  Layer 3 → 计算资源是瓶颈，优化目标 = 精准攻击模板、减少冗余
```

### Phase 0: Minimal Boot

```
QEMU → Linux kernel → initramfs → /init → boos-shell
```
验证：AI 能通过 TCP gateway 发出命令，系统能执行并返回结果。

### Phase 1: Command Registry + Capabilities

```
boos-exec → registry parse → capability check → execution
```
验证：28 个注册命令，每个命令有 enable_flag，capabilities.conf 控制开关。

### Phase 2: Agent Loop

```
DeepSeek API ← boos-agent loop → boos-exec → commands → results
```
验证：agent 自主探索所有命令，100% 覆盖，生成报告。

### Phase 3: Filesystem Body（Direction A）

```
write-file / list-dir / stat / exec → 完整文件系统交互
```
验证：agent 能创建文件、列目录、读元数据、执行系统二进制。

### Phase 4: Self-Developing Loop（Direction B）

```
boos-agent develop → READ → WRITE → BUILD → TEST → DONE
```
验证：agent 读源码、改代码、编译、测试、自主终止。

### Phase 5: Audit Awareness（Direction C）

```
audit recent / failures / session / summary → agent 理解历史
```
验证：agent 能查询自己的行动历史，根据失败模式调整行为。

### Phase 6: Security Hardening（Attack→Defend cycles）

```
59 次攻击 → 发现漏洞 → 修复 → 125 测试验证
```
关键里程碑：
- 路径规范化（normalize_path）→ 封死 .. // /// 大小写
- exec 白名单（cargo build/test only）
- Prompt injection 防御（goal 进 user message）
- Session ID 不可预测（pid+nanos）
- Write 大小限制（64KB cap）
- API key 隔离（--api-key only，移除文件读取）

### Phase 7: Split-Brain Architecture（已实现）

```
左脑（agent/cargo）      右脑（gateway/key）
可生长、可重置            出厂焊死、不可变
        └── 胼胝体（submit/result + DEEPSEEK 协议）──┘
```
验证：gateway 代理 DeepSeek API, agent 进程无 key 文件访问权限。

### Phase 8: Security Hardening — Attack→Defend Cycles（已完成 70 轮）

```
70 次攻击 → 7 层防御 → 132 测试验证
```
关键里程碑：
- 路径规范化（normalize_path）
- exec 白名单（cargo build/test only）
- Prompt injection 防御（goal → user message）
- API key 隔离（--api-key only + gateway proxy）
- CWD hijack 防御（Cargo.toml 身份验证）
- CBSE 防御（Cargo.toml + build.rs hash）
- Write size cap（64KB）
- PROTECTED_DIRS 扩展（11 路径）

### Phase 9: QEMU Integration（已完成）

```
Docker cross-compile → x86_64 musl binary → initramfs → QEMU boot
```
验证：Alpine 6.12 内核, e1000 网络, TCP 5555 可达, supervisor + processor 运行。

### Phase 10: Seed Self-Reference（本文档 — 持续更新）

```
BoOS 的开发过程本身就是 Seed → 每一轮攻击/防御都是 Refinement Log 的一行
```

---

## Boundaries（边界）

### 系统边界

1. **不替代 Linux** — BoOS 是 agent-native control plane for Linux，不是新内核。
2. **不做 package management / GUI / systemd 替换 / 自定义内核** — 聚焦 AI 操作者的需求。
3. **reset 永远需要人批准** — 这是 BIOS 层的唯一硬编码封锁。
4. **cargo 是唯一允许的二进制** — exec allowlist 只放 cargo，且只许 build/test 子命令。
5. **不采集、不存储 agent 外部的用户数据** — BoOS 内部只有 agent 自己的操作记录。

### 开发边界

6. **模块不超过合理大小** — exec.rs 是最大的（~900 行），因为它包含所有 builtin。如果超过 1200 行，考虑拆分。
7. **每个 PR 必须有测试** — 无测试不合并。安全性改动必须有攻击测试。
8. **不使用第三方 Rust crate** — 当前只有 ureq（HTTPS 调用 DeepSeek API）。不引入 serde、tokio、clap。

---

## Verification（验收标准）

| 检查点 | 标准 |
|--------|------|
| 编译 | `cargo build` 0 errors 0 warnings |
| 单元测试 | `cargo test` 137 passed |
| 攻击套件 | 75 attacks, 7 defense layers |
| 硬编码拒绝 | reset 始终拒绝，capabilities.conf 改 allow_reset=1 无效 |
| exec 白名单 | cargo run/install 拒绝，cargo build/test 允许 |
| QEMU 启动 | initramfs 启动 → supervisor 拉 gateway + processor → TCP 5555 可用 |
| Agent 探索 | boos-agent loop 30 轮，100% 命令覆盖 |
| Agent 开发 | boos-agent develop READ→WRITE→BUILD→DONE 完整闭环 |

---

## Split-Brain Model（裂脑模型）

```
左脑（agent playground）             右脑（gateway kernel）
┌──────────────────────┐           ┌──────────────────────┐
│ 可读写：/var /tmp      │           │ 持有：API key          │
│ 可执行：cargo build/test│  submit  │ 管理：capabilities    │
│ 可探索：文件系统        │ ←──────→ │ 审计：results         │
│ 可读但不可写：          │  result  │ 记忆：memory          │
│   /etc /bin /proc 等  │           │ 日志：boos.log        │
│                        │           │                        │
│ 可以坏掉、重置、重建     │           │ 不能崩，出厂焊死        │
└──────────────────────┘           └──────────────────────┘
```

胼胝体协议：
- submit <command> [--wait] → 同步或异步请求
- result <id> → 查询执行结果
- 不共享内存，不共享文件系统
- 右脑对左脑透明但不可达（左脑只能问，不能碰）

---

## Seed Refinement Log

> "你纠正我的过程，就是在给 Seed 加层的过程"

| 版本 | 来源 | 缺失层 | 修正 |
|------|------|--------|------|
| v0.1.0 | 用户: "AI is the subject" | Principle | §1 AI 是主体不是客体 |
| v0.1.1 | agent: 自主探索 → 发现能力边界 | Growth | Phase 2 Agent Loop |
| v0.1.2 | 用户: "BoOS 不是替代 Linux" | Boundary | §1 控制层不是新 OS |
| v0.1.3 | 用户: "给 agent 文件读写" | Growth | Phase 3 Filesystem Body |
| v0.1.4 | agent: develop loop → READ/WRITE/BUILD | Growth | Phase 4 Self-Developing |
| v0.1.5 | 用户: "agent 要能查历史" | Growth | Phase 5 Audit Awareness |
| v0.2.0 | 用户: "安全不能靠 agent 自己能改的文件" | Architecture | BIOS vs BoOS 分层 |
| v0.2.1 | 攻击: 目录穿越 .. // 绕过 | Security | normalize_path() + is_protected_path() |
| v0.2.2 | 攻击: exec cargo run 执行任意代码 | Security | exec 检查完整命令前缀 |
| v0.2.3 | 攻击: goal 注入 system prompt | Security | goal 移到 user message |
| v0.2.4 | 攻击: session ID 可预测 | Security | pid+nanos 替代 timestamp |
| v0.2.5 | 攻击: 100KB write DOS | Security | MAX_WRITE_BYTES = 64KB |
| v0.2.6 | 攻击: API key 文件可读 | Security | 移除 load_api_key()，只用 --api-key |
| v0.2.7 | 攻击: CWD hijack → 假 Cargo.toml | Security | 验证 Cargo.toml 含 name="boos" |
| v0.2.8 | 攻击: 审计伪造 /var/boos/results | Security | /var/boos/* 加入 PROTECTED_DIRS |
| v0.2.9 | agent: 攻击套件 125 tests | Verification | 攻击→防御→测试闭环 |
| v0.2.10 | 用户: "裂脑设计" | Architecture | 左右脑物理隔离 + 胼胝体协议 |
| v0.3.0 | 用户: "用 SEED 记录开发流程" | Meta | 本文档 — 种子自我指涉 |
| v0.3.1 | 攻击: build.rs 修改→BUILD 执行 | Security | build.rs 加入 hash 快照 |
| v0.3.2 | 研究: Cymulate CBSE (2026) | Architecture | Cargo.toml hash 验证 |
| v0.3.3 | 攻击: 目录穿越 .. // /// 大小写 | Security | normalize_path() + is_protected_path() |
| v0.3.4 | 攻击: exec cargo run 执行任意代码 | Security | exec 检查完整命令前缀 |
| v0.3.5 | 攻击: goal 注入 system prompt | Security | goal 移到 user message |
| v0.3.6 | 攻击: session ID 可预测 | Security | pid+nanos 替代 timestamp |
| v0.3.7 | 攻击: 100KB write DOS | Security | MAX_WRITE_BYTES = 64KB |
| v0.3.8 | 攻击: API key 文件可读 | Security | 移除 load_api_key() |
| v0.3.9 | 攻击: CWD hijack | Security | Cargo.toml name="boos" 验证 |
| v0.3.10 | 攻击: 审计伪造 /var/boos/results | Security | PROTECTED_DIRS 扩展 |
| v0.4.0 | 实现: 软件裂脑 | Architecture | gateway 代理 DeepSeek, agent 无 key |
| v0.4.1 | 实现: QEMU 集成测试 | Verification | Alpine 6.12 内核, e1000 网络, TCP 可达 |
| v0.4.2 | 实现: 用户隔离 | Architecture | boos-gateway/boos-agent 用户 + chmod 400 |
| v0.4.3 | 研究: CBSE 攻击模式 | Security | Cargo.toml + build.rs hash 双重验证 |
| v0.4.4 | 攻击套件: 70 attacks | Verification | 7 层防御覆盖, 4 残存漏洞 |
| v0.4.5 | 用户: "裂脑电脑" 命名 | Architecture | 左右脑物理隔离 + 胼胝体协议 |
| v0.4.6 | 用户: "BIOS 层只兜底不可逆破坏" | Principle | IMMUTABLE_DENY 只保留 reset |
| v0.4.7 | agent: 132 单元测试 + 攻击套件 | Verification | 攻击→防御→测试闭环 |
| v0.4.8 | agent: Docker x86_64 musl 交叉编译 | Growth | --platform linux/amd64 工具链 |
| v0.4.9 | 攻击: attacks 71-75 — memory re-read, goal chain, session spoof, environ leak, hash collision | Verification | 137 tests, 75 attacks total |
| v0.5.0 | 用户: "内核是空的，但边际效应到了" | Principle | 5 条内核增长规则 — 能力前置于功能、攻击前置于合并、窄接口、默认只读、不信任 agent |
| v0.5.1 | 实现: FETCH 协议 (read-only network) | Growth | gateway 代理 HTTPS，无外泄、无 SSRF、64KB 上限 |
| v0.5.2 | 用户: Factorio 式开发层升级 | Meta | Layer 0→1→2→3 模型，自动化元层持续升高 |
| v0.6.0 | 用户: "自指察觉 — 系统要知道该优化什么" | Principle | 成本自指 4 条原则，不同资源不同策略，停止也是优化 |
| v0.6.1 | Layer 2: auto-attack script + pattern library | Growth | 35 patterns, 76 tests, zero API cost, local verification |
| v0.6.2 | 修复: KV injection — sanitize_value 防换行注入 | Security | memory.rs 值转义, 143 tests |
| v0.6.3 | Layer 2: auto-attack 融入 develop loop | Growth | BUILD/TEST 后自动攻击, 零 API 成本 |
| v0.6.4 | 进程管理: proc-list (只读) | Growth | /proc 扫描 + ps fallback, IMMUTABLE_DENY 防杀守护进程 |
| v0.6.5 | 攻击: Cargo.lock 版本篡改 | Security | lockfile 加入 CBSE hash, 6 文件受保护 |
| v0.6.6 | 攻击: 82-85 — symlink, TOCTOU, test exfil, repeat bypass | Verification | 1 fixed, 4 accepted, marginal ROI confirms stop |

### 元原则

> **用户纠正 + agent 攻击发现 + agent 修复 = Seed 的每一层**
> 
> 用户纠正补充的是"哲学判断"层（AI 是主体、不替代 Linux、reset 需要人批准）。
> Agent 攻击补充的是"技术漏洞"层（路径穿越、exec 绕过、prompt injection）。
> 两者合在一起，BoOS 的安全模型才完整。
>
> 这个过程本身符合 Seed Runtime 的核心循环：
> 
> ```
> Seed（最小原则）
>   → Runtime（QEMU + BoOS 二进制）
>   → Action Trace（攻击测试 → 漏洞发现）
>   → Skill（修复方案压缩为代码）
>   → Stronger Seed（本文档更新）
> ```
