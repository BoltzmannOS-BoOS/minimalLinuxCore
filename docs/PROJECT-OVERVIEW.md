# BoOS: AI-Native Operating System — Project Overview v0.7.3

> **一句话**: BoOS 不是替代 Linux，是在 Linux 之上/之内构造一个 AI agent 的原生控制层。
> **目标**: 把 AI 从"外挂脚本"变成操作系统的第一公民。

---

## 一、哲学

```
传统 OS: 人是操作者，程序是工具，AI 是客人
BoOS:   AI 是操作者，OS 是身体，人是审查者
```

**23 条设计原则**，核心 5 条：

| # | 原则 | 含义 |
|---|------|------|
| 1 | AI 是主体 | agent 是第一操作者，不是模仿人类点鼠标 |
| 2 | 不替代 Linux | BoOS 是 agent-native control plane，不是新内核 |
| 6 | BIOS/BoOS 分层 | BIOS 硬编码不可变，BoOS 配置可生长 |
| 14 | 能力前置于功能 | 加功能前先注册 IMMUTABLE_DENY + PROTECTED_DIRS |
| 23 | AI 要服务，不要 syscall | agent 不该管 PID/fork/进程树。要服务级接口 |

完整 SEED.md：项目全貌、架构、安全模型、开发约定。

---

## 二、已建成：7 层防御 + 4 层开发体系

### 2.1 安全：7 层防御（88 次攻击，143 测试全部通过）

| 层 | 名称 | 防什么 | 实现 |
|----|------|--------|------|
| 1 | IMMUTABLE_DENY | 不可逆破坏（reset、网络外泄） | 硬编码，配置文件改不了 |
| 2 | PROTECTED_DIRS | 篡改系统骨骼 | 11 个路径 + symlink 解析 |
| 3 | exec 白名单 | 运行未授权二进制 | 完整命令前缀检查 |
| 4 | Prompt 隔离 | 指令注入 | goal 进 user message，system prompt 不可变 |
| 5 | Write 上限 | 磁盘耗尽 | 64KB 硬上限 per write |
| 6 | API key 隔离 | 密钥泄露 | Gateway 代理——agent 调用 DeepSeek 必须走 gateway，无 key 访问权限 |
| 7 | CBSE 防御 | 编译时代码注入 | Cargo.toml + build.rs + Cargo.lock hash 三重验证 |

### 2.2 攻击副核心：自进化攻击系统（Layer 0→3）

```
Layer 0: 36 个静态攻击模式（被动回放）
Layer 1: auto-attack.sh（BUILD/TEST 后自动触发，零 API 成本）
Layer 2: attack-compose.py（7 原语 × 4 算子 = 60 组合攻击）
Layer 3: attack-evolve.py（过滤 → 测试 → 报告 → 存档，持续自进化）
```

最近一次进化报告：48 组合测试，0 OPEN，7 层防线全部守住。

### 2.3 架构：Gateway 代理隔离

```
Agent 进程（boos-agent 用户）          Gateway 进程（boos-gateway 用户）
┌──────────────────────┐             ┌──────────────────────┐
│ 可读写：/var /tmp      │  submit     │ 持有：API key          │
│ 可执行：cargo build     │ ←────────→ │ 管理：capabilities    │
│ 可探索：文件系统        │  result    │ 审计：results         │
│ 可读不可写：/etc 等    │             │ 记忆：memory          │
│ 可以坏掉、重置          │  DEEPSEEK  │ 不共享 secret，不崩     │
│                        │  FETCH     │                      │
└──────────────────────┘             └──────────────────────┘
```

### 2.4 AI 当前能力（agent 能做什么）

| 能力 | 接口 | 状态 |
|------|------|------|
| 文件读写 | READ / WRITE / LIST / STAT | ✅ |
| 代码编译 | cargo BUILD / TEST | ✅ |
| AI 调用 | DEEPSEEK（gateway 代理） | ✅ |
| 只读网络 | FETCH（HTTPS only，无 SSRF，64KB 上限） | ✅ |
| 进程感知 | proc-list（只读，观察不干涉） | ✅ |
| 自我验证 | auto-attack + 进化引擎 | ✅ |
| 记忆 | remember / recall / observe | ✅ |
| 审计 | audit recent / failures / session | ✅ |
| QEMU 启动 | init → supervisor → gateway → 轮询 | ✅ |

### 2.5 明确不做的事

- ❌ **PID 管理** — AI 不该管进程树（原则 23）
- ❌ **Fork/spawn** — sysadmin 的活，不是 AI 的活
- ❌ **GUI/桌面环境** — AI 不需要屏幕
- ❌ **包管理** — 聚焦 AI 操作者需求
- ❌ **替代内核** — 是控制层，不是新 kernel

---

## 三、待讨论：AI 还需要什么？

以下是开放问题，供评估者考虑：

### Q1: Sub-agent 多分身
AI 应该说"让 3 个 worker 并行检查所有源文件的安全性"，而不是"fork 3 个进程"。BoOS 需要 SpawnWorker 接口吗？

```
当前: agent 一个人干活
期望: agent 分配任务给子 agent，回收结果
挑战: 子 agent 的安全边界在哪？gateway 代理模型怎么推广到 N 个 agent？
```

### Q2: 持久任务 + 定时
AI 应该说"每天凌晨跑一次系统巡检"，不需要知道 cron 语法。BoOS 需要 schedule 接口吗？

```
当前: agent 只在 develop loop 内运行
期望: agent 可以设置定时任务，到时间自动醒来执行
挑战: 醒来后怎么恢复上下文？记忆系统够用吗？
```

### Q3: 资源感知
AI 应该知道自己的上下文窗口快满了，该压缩或委派了。BoOS 需要 ResourceMonitor 吗？

```
当前: agent 不知道自己的上下文用了多少
期望: agent 主动感知资源压力，在溢出前做决策
挑战: 监控数据本身也占上下文
```

### Q4: 知识蒸馏
30 轮开发循环后，agent 应该自动压缩成一条记忆，而不是保留完整对话。BoOS 需要 Distill 接口吗？

```
当前: 记忆系统有 Working/Recent/Archive 三层
期望: agent 自动总结长对话为关键经验
挑战: 蒸馏可能丢关键信息，怎么设置可信度？
```

### Q5: 外部世界感知
agent 应该说"这个 RSS 有更新就通知我"，而不是用 FETCH 轮询。BoOS 需要 Subscribe 接口吗？

```
当前: FETCH 是一次性的 HTTPS GET
期望: 订阅外部数据源，有变化时主动通知 agent
挑战: 事件驱动的 agent 怎么被唤醒？
```

### Q6: 多 agent 协作
让攻击 agent 和防御 agent 对打，互相发现漏洞。BoOS 需要 Agent 间通信协议吗？

```
当前: 只有一个 agent（develop loop）
期望: 多个独立 agent 通过 gateway 通信，不共享 agent 状态
挑战: gateway 代理 × N 的复杂度会爆炸吗？
```

### Q7: 你还能想到什么？
BoOS 的目标是"AI 作为第一操作者的操作系统"。一个 AI 还需要什么我们现在没想到的？

---

## 当前研究方向：Semantic Object Layer（2026-07-30）

BoOS 不再把“更多 agent runtime 功能”当作当前差异化方向。当前把 Linux
之上的语义 ABI 作为待验证的假设和路线图方向：让 AI 直接观察稳定、可查询
的系统对象，而不是从面向人的帮助文本和命令输出中反复猜测系统状态。

当前只实现只读切片：命令注册表被投影为 `system` 与 `capability` 对象，
通过 `world schema`、`world list`、`world show` 查询。它不引入新权限，
也不绕过原有 capability policy。

现有暴露实验标记为 `Test 0: Interface and Wiring Probe`。
Test 0 is retained as a protocol/wiring regression. It does not establish
that the semantic object layer improves real AI operation. Fresh research
claims must use the Living Evidence System.

- 设计：`docs/superpowers/specs/2026-07-30-boos-semantic-object-layer-design.md`
- 实施计划：`docs/superpowers/plans/2026-07-30-boos-semantic-object-layer.md`
- 实验协议：`tests/research/semantic-object-view/README.md`
- 证据系统：`tests/evidence/README.md`

---

## 四、技术概要

```
语言:     Rust (std only, 0 external crates except ureq)
二进制:   x86_64-unknown-linux-musl, 2.1MB static-pie
编译:     Docker --platform linux/amd64
测试:     139 单元测试, 88 次攻击覆盖
QEMU:     Alpine 6.12.91 kernel, e1000 网络, TCP 5555
文件:     33 commits on main, GitHub: BoltzmannOS-BoOS/minimalLinuxCore
SEED:     23 原则, 5 增长规则, 4 成本自指, 52 条精炼日志
```

---

## 五、文件导航

```
SEED.md                    ← 从这里开始读（23 条原则 + 架构）
docs/attack-research.md    ← 真实世界攻击研究
docs/development-layers.md ← Factorio 式 Layer 0→3 模型
tests/research/semantic-object-view/ ← 语义对象 A/B 实验协议
tests/attack-knowledge.md   ← 攻击原语 + 组合算子
src/rust/src/config.rs     ← BIOS IMMUTABLE_DENY + PROTECTED_DIRS
src/rust/src/gateway.rs    ← Gateway 进程（DEEPSEEK + FETCH 协议代理）
src/rust/src/agent_develop.rs ← 开发循环 + 85 个攻击测试
rootfs/init                ← 启动脚本（用户隔离 + chmod 隔离）
```

---

*BoOS v0.7.3 — 2026-06-05*
*"AI 是主体，not a guest."*
