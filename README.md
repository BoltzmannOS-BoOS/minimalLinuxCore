# BoOS — AI-Owned Operating System Substrate

> "AI is the subject, not the object."

BoOS is a minimal Linux environment designed to be **explored, operated, and improved by AI**.
Not a tool for AI — an environment where AI lives.

## Philosophy

Standard AI development: human writes spec → AI writes code → human verifies.

BoOS flips this: **human builds a world → AI enters and explores → AI discovers what's missing → human fixes → repeat.**

Like placing a brain in a vat and watching it figure out the shape of its container.

```
┌─────────────────────────────────────────┐
│  BoOS (Linux initramfs / Docker)        │
│                                         │
│  ┌───────────────────────┐              │
│  │  boos-agent loop       │              │
│  │  ┌───────────────────┐ │   HTTPS      │
│  │  │ DeepSeek inside   │─┼──────────────┼──→ DeepSeek API
│  │  │ "What IS this?"   │ │              │
│  │  └───────────────────┘ │              │
│  │  ↓ think ↓ act ↓ remember             │
│  │  boos-exec → commands → results       │
│  │  boos-memory (working/recent/archive)  │
│  └───────────────────────┘              │
│                                         │
│  Commands: help status caps log debug   │
│            submit remember recall observe│
│            read-file exec session-*     │
└─────────────────────────────────────────┘
```

## The v1 → v4 Story

This is the real value of the project — not the code, but the process.

### v1: Hermes Drives BoOS
DeepSeek (via Hermes) connects to BoOS's TCP gateway and manually explores.
11 rounds, zero prior knowledge. Discovers commands through `help → status → caps → ...`
Produces first exploration report.

→ [v1-deepseek-exploration.md](docs/v1-deepseek-exploration.md)

### v2: DeepSeek Lives Inside BoOS
`boos-agent loop` — an agent loop running inside BoOS that calls DeepSeek API itself.
No external Hermes. 26/26 commands explored. 100% coverage. Auto-generates report.

→ [v2-full-log.txt](docs/v2-full-log.txt) | [v2-deepseek-report.txt](docs/v2-deepseek-report.txt)

### v3: Memory Across Sessions
DeepSeek receives its own v2 report as prior knowledge.
Now thinks in terms of architecture: "BoOS is a multi-layer system — Linux → BoOS runtime → AI interface."
Identifies 4 unsolved mysteries.

→ [v3-full-log.txt](docs/v3-full-log.txt)

### v4: AI-Discovered Gaps, Human-Fixed
DeepSeek's reports revealed real gaps. This version adds:
- `read-file` — file system access
- `exec` — execute system binaries
- Fixed `daemons` (was returning exit=2 silently)
- Fixed `submit` pipeline

DeepSeek will re-explore and discover these new capabilities.

## Development Method

BoOS 有两份副本，对应两种开发模式：

```
minimalLinuxCore/     ← 主线。你开发，稳定。
boos-playground/      ← 沙箱。AI 随便造，随时可以删掉重建。
```

AI 驱动的开发循环：

```
1. 复制主线到 playground
2. AI 进入 playground 探索
3. AI 发现缺口 → 写代码 → cargo build → 测试
4. AI 报告: "改动有效，merge？"
5. 人审查 diff → merge 进主线
6. 重置 playground → 下一轮
```

这个循环由 [Tulpa](../tulpa) 框架驱动。

## Quick Start

```bash
# Build
cd src/rust
docker run --rm -v $PWD:/work -w /work/src/rust rust:alpine \
  cargo build --release

# Set API key (file, not CLI arg)
echo "api_key=sk-xxx" > /etc/boos/agent.conf

# Run autonomous exploration
boos-agent loop --goal "探索BoOS" --max-loops 30

# With prior knowledge from previous runs
boos-agent loop \
  --goal "基于历史经验深入分析" \
  --prior-knowledge docs/v2-deepseek-report.txt \
  --max-loops 50
```

## Architecture

```
/init (shell) → boos-supervisor (Rust, pid1)
  ├── boos-gateway (TCP :5555)     ← AI entry point
  ├── boos-process (poll loop)      ← request execution
  ├── boos-agent (autonomous)       ← self-exploring agent
  └── boos-exec                     ← command dispatcher

Storage:
  /etc/boos/commands/*.cmd          ← command registry
  /etc/boos/capabilities.conf       ← permission model
  /var/boos/memory/working.kv       ← session state
  /var/boos/memory/recent/*.kv      ← observation stream
  /var/boos/memory/archive/*.mem    ← persistent knowledge
```

## Key Design Decisions

- **No JSON, no serde** — all config is key=value format
- **No JS/TS toolchain** — pure Rust, musl static binary
- **Capability-based security** — each command has allow_* flag
- **3-tier memory** — working (session) / recent (ring buffer) / archive (persistent)

## Sub-projects

### boos-harness (planned)
QEMU-based end-to-end testing framework for BoOS.

### boos-autopilot (planned)
Fully automated development cycle:
build → deploy → agent explores → report → analyze → apply fixes → rebuild → repeat.

A self-growing system.
