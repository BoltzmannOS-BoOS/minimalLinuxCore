# PLAN.md

This file is the working plan for `minimalLinuxCore`.

It is written so a new ChatGPT, Claude, or agent session can continue the project without needing the previous conversation.

---

## Project Context

### Repository

```txt
minimalLinuxCore
```

### Project Role

`minimalLinuxCore` is the minimal Linux experimental substrate for **BoOS**.

### BoOS Core Idea

BoOS is an **AI-owned computer environment**.

The AI is the primary user — not a guest, not a threat to contain.

The system exists to give the AI a body:
- every action is **registered** (the AI knows what it can do)
- every action is **logged** (the AI can introspect on what it did)
- every action is **inspectable** (curiosity-driven, not security-driven)

### Design Philosophy (updated 2026-05)

> AI has near-native system rights. Capabilities describe what actions exist, not what actions are forbidden. The default is permissive.

Key principles:
- **AI is the subject, not the object** — the system serves the AI, not the human gatekeeper
- **Observe, don't obstruct** — log everything because we are curious, not because we are afraid
- **Rich observability, minimal restriction** — the audit layer exists to satisfy human curiosity during development, not to permanently jail the AI
- **Logs are a mirror, not a prison** — the AI reads them to understand its own behavior

### Important Clarification

`minimalLinuxCore` is **not** the entire BoOS project.

It is the low-level experimental ground.

The current repo should focus on building a tiny Linux environment that can be:
- booted
- operated by an AI through a request interface
- observed in detail (for developer curiosity)
- inspected and introspected (by both AI and human)

---

## Current State

## Done

---

### M1: Bootable Minimal Linux

**Status:** done.

The repo can build an initramfs and boot in QEMU.

Current boot path:

```txt
QEMU
  ↓
Linux kernel
  ↓
initramfs
  ↓
/init
```

---

### M2: Custom BoOS Shell

**Status:** done.

`/init` successfully runs:

```txt
/bin/boos-shell
```

The system no longer directly drops into raw `/bin/sh`.

---

### M3: Command Logging

**Status:** done.

Commands are logged to:

```txt
/var/log/boos.log
```

Example log entries:

```txt
[boos-shell] input: status
[boos-exec] allowed: status - show system status
```

---

### M4: Capability System

**Status:** done.

Capabilities are stored in:

```txt
/etc/boos/capabilities.conf
```

Example:

```conf
allow_help=1
allow_commands=1
allow_status=1
allow_log=1
allow_caps=1
allow_submit=1
allow_process=1
allow_results=1
allow_shell=1
allow_reboot=1
allow_poweroff=1
```

The key idea:

> The system can describe what actions exist through capability flags. Default is permissive — AI has near-native rights. Capabilities exist for discoverability and logging, not for restriction.

---

### M5: Command Registry

**Status:** done.

Commands are registered under:

```txt
/etc/boos/commands/
```

Example command file:

```txt
/etc/boos/commands/status.cmd
```

Example content:

```ini
name=status
capability=allow_status
description=show system status
exec=__builtin_status
```

The key idea:

> The AI can inspect the registry to discover what actions are available, rather than guessing shell commands or being told by a prompt.

---

### M6: Executor Split

**Status:** done.

`boos-shell` is mostly an interactive frontend.

Actual command execution goes through:

```txt
/bin/boos-exec
```

Architecture:

```txt
boos-shell
  ↓
boos-exec
  ↓
command registry
  ↓
capability check
  ↓
execution
```

The key idea:

> Execution is separated from the human interface. Future AI can call `boos-exec` without owning a terminal.

---

### M7: Request Queue

**Status:** done.

The system supports action requests.

Request submission:

```txt
submit status
```

This creates request files under:

```txt
/run/boos/requests/
```

Manual processing:

```txt
process
```

This executes requests through:

```txt
boos-process
  ↓
boos-exec
```

Results are written to:

```txt
/run/boos/results/
```

Tested behavior: `submit status` then `process` works. `submit shell` correctly passes capability check.

---

## Current / In Progress

---

### M10: Structured Observability

**Status:** done.

**Goal:** The curious human (and AI) can see what happened in rich, queryable detail.

**What was built:**
- **Structured request files**: `id`, `requester` (ai/shell/system), `command`, `args`, `submitted_at`, `status`
- **Structured result files**: `id`, `requester`, `command`, `verdict` (allowed/denied/error), `exit_code`, `started_at`, `finished_at`, `duration_ms`, full output after `---` delimiter
- **Structured log format**: `ts=<uptime> component=<name> event=<event> key=value ...` — grep-friendly and parseable by both shell and AI
- **Requester attribution**: `BOOS_REQUESTER` env var propagated through shell (shell), handler (ai), daemon (system)
- **New `result <id>` command**: view a single result's full metadata and output

**Log format example:**
```
ts=14.940 component=boos-exec event=allowed command=status desc="show system status"
ts=29.240 component=boos-submit event=submitted id=req-29220 requester=ai command=shell
ts=30.130 component=boos-process event=completed id=req-29220 verdict=denied exit_code=1 duration_ms=399
```

**Queryable with simple grep:**
- `grep 'event=denied' /var/log/boos.log` — all denials
- `grep 'requester=ai' /var/log/boos.log` — all AI-initiated actions
- `grep 'command=status' /var/log/boos.log` — all status commands

**Result file format:**
```ini
id=req-13840
requester=ai
command=status
verdict=allowed
exit_code=0
started_at=14.560
finished_at=14.960
duration_ms=400
---
BoOS substrate status:
  kernel: 7.0.0-15-generic
  uptime: 14.9 seconds
  pid: 230
```

**Verified 2026-05-25:**
- AI-submitted commands show `requester=ai` in logs and result files
- Denied commands correctly show `verdict=denied` and `exit_code=1`
- Timing captures execution with millisecond precision
- `result <id>` returns full structured result with metadata + output
- `results` shows summary of all results with metadata
- Log format is grep-friendly: filter by component, event, command, requester

---

### M9: AI Client Connection

**Status:** done.

**Goal:** The system gets used by its intended primary user for the first time.

**What was built:**
- **TCP Gateway** (`boos-gateway` + `boos-handler`): Listens on port 5555 inside the VM, accepts commands via TCP, and returns results synchronously.
- **AI Client** (`ai-client.py`): Runs on the host, converts the BoOS command registry into OpenAI tool definitions, and sends commands via TCP to the gateway.
- **Network setup**: QEMU user-mode networking with virtio-net + DHCP, host-to-guest port forwarding.

**Architecture:**

```txt
ai-client.py (host)
  ↓ TCP (localhost:5555 → QEMU port forward → guest:5555)
boos-gateway (guest)
  ↓ stdin/stdout per connection
boos-handler
  ↓
boos-exec
  ↓
command registry → capability check → execution
```

**Verified 2026-05-25:**
- Real AI session completed (DeepSeek V4 Pro, 4 turns, ~3233 tokens)
- AI successfully explored the system: discovered commands, read status/logs/capabilities
- `submit {"command": "status"}` worked — AI used the request pipeline (submit → daemon → results)
- AI correctly understood the capability system and identified which commands are allowed/blocked
- TCP gateway handles rapid-fire connections reliably using `nc -ll`
- `ai-client.py` uses only Python stdlib (no external dependencies)
- AI noted that `shell`, `poweroff`, `reboot` are locked down — as intended

**Note:** The 9p shared directory alternative is not available due to kernel lacking 9p filesystem support. TCP is the primary transport. virtio-net replaces e1000 (driver not in kernel).

---

### M11: Persistent Overlay

**Status:** done.

**Goal:** AI behavior (logs, results) survives across reboots so the developer can study it.

**What was built:**
- **64MB ext2 disk image** (`build/var.img`) created once during build, attached as virtio block device
- **Persistent `/var` mount**: init mounts `/dev/vda` on `/var` at boot
- **Moved all persistent state to `/var`**: `/var/boos/requests`, `/var/boos/results`, `/var/log`
- **QEMU directsync cache**: `cache=directsync` ensures writes hit the disk image immediately

**Architecture:**

```txt
build/var.img (host, 64MB ext2)
  ↓ -drive file=...,if=virtio,cache=directsync
/dev/vda (guest)
  ↓ mount -t ext2 /dev/vda /var
/var/log/boos.log       ← persistent
/var/boos/results/*.out ← persistent
/var/boos/requests/     ← persistent (cleared when processed)
```

**Verified 2026-05-25:**
- Log entries from session N are visible after reboot in session N+1
- Results from previous sessions are preserved and queryable
- New entries are appended (not overwritten) across sessions
- Disk image created once by build script, survives rebuilds (only initramfs rebuilt)
- `debugfs` confirms directory structure and file contents on the host side
- Clean shutdown with `sync` flushes all data

---

### M12: Debug-Level Observability

**Status:** done.

**Goal:** Go beyond "what command ran" to "what did that command actually do to the system."

**What was built:**
- **Trace level config** (`/etc/boos/debug.conf`): `trace_level=quiet|normal|verbose`
- **`debug` command**: show or set trace level (`debug`, `debug verbose`, `debug quiet`, `debug normal`)
- **Quiet mode**: only `event=denied` and `event=error` logged — reduces noise when observing specific behaviors
- **Verbose mode**: adds filesystem change tracking (files modified under `/var` during command execution) and command chaining context (`prev_command` in results and logs)
- **Normal mode**: all events logged (default, same as before)
- **Filesystem tracking**: creates marker before command execution, uses `find -newer` to detect modified files on the persistent `/var` disk

**Architecture:**

```txt
/etc/boos/debug.conf  ← trace_level=normal|verbose|quiet
        ↓
boos-exec (reads trace_level, filters log output)
boos-process (reads trace_level, enables fs tracking in verbose mode)
boos-shell (status shows current trace level)
        ↓
/var/log/boos.log    ← filtered by trace level
/var/boos/results/*  ← includes prev_command + files_touched in verbose mode
```

**Verified 2026-05-25:**
- Quiet mode: `status` not logged, `shell` denial IS logged
- Verbose mode: `/var/log/boos.log` detected as modified during command execution
- Command chaining: second request shows `prev: status` (first request shows `prev: none`)
- `status` command shows current trace level
- Debug config persists across sessions (on persistent /var... wait, it's in /etc which is tmpfs — config resets on reboot, which is arguably correct for debug mode)

**Note:** Filesystem tracking on tmpfs (`/etc`, `/tmp`) is unreliable due to timestamp granularity. Tracking focuses on `/var` (persistent disk) where it works reliably with ext2 + `cache=directsync`.

---

## What Is Not Done

---

### M13: Multi-Argument Commands — done (2026-05-25)

**What was built:**
- `boos-submit` separates first positional arg as `command` and remainder as `args`
- Request files store `command` and `args` as separate fields
- `boos-process` reads both fields and reconstructs `full_cmd="$cmd $args"`
- Result files include `args=$args` field
- `boos-shell` `run` and `submit` commands use unquoted `$*` to preserve word splitting
- `boos-exec` `__builtin_submit` uses unquoted `$args` to pass to `boos-submit`

Example: `submit debug verbose` correctly produces `command=debug`, `args=verbose`.

---

### M14: Real Daemon Supervision — done (2026-05-25)

**What was built:**
- **`boos-supervisor`**: starts, monitors, and restarts daemon processes
- **Daemon registry** (`/etc/boos/daemons/*.daemon`): daemon configs with name, exec, restart policy, enabled flag
- **Health monitoring**: checks daemon PIDs every 2 seconds via `kill -0`
- **Auto-restart**: restarts dead daemons with `restart=always` policy
- **Backoff**: stops restarting after 5 failures to prevent tight loops
- **Lifecycle logging**: `event=started`, `event=starting`, `event=restarting`, `event=stopped`, `event=failed`
- **`daemons` command**: shows daemon health (running/stopped, PID, restart count)
- **`status` command**: includes daemon health summary
- **`/init`**: uses supervisor instead of raw `&` background processes

**Architecture:**
```
/etc/boos/daemons/*.daemon    ← daemon configs (like command registry)
        ↓
boos-supervisor start         ← reads configs, starts daemons, monitors
        ↓
/var/boos/daemons/*.pid       ← PID tracking
/var/boos/daemons/*.restarts  ← restart counters
        ↓  
/var/log/boos.log             ← lifecycle events
```

**Supervised daemons:**
- `gateway` — TCP gateway (restart=always)
- `processor` — request queue processor (restart=always)

**Verified 2026-05-25:**
- Both daemons start via supervisor and show `running` status with PIDs
- `daemons` command shows health for both daemons
- `status` command includes daemon health information
- Lifecycle events are logged with `component=boos-supervisor`
- Disabled daemons are skipped; restart=never daemons are not restarted

---

### Not Done / Future

---

## Replanned Milestones (After M8)

These replace the old M9–M12 which were written under the "human guards AI" assumption and no longer match the project philosophy.

---

### M9: AI Client Connection — done (2026-05-25)

---

### M10: Structured Observability — done (2026-05-25)

**Note:** M13 (Requester Identity) was absorbed into M10 — requester attribution is part of the structured request/result/log format.

---

### M11: Persistent Overlay — done (2026-05-25)

---

### M12: Debug-Level Observability — done (2026-05-25)

**Note:** Per-command timing and output capture were already handled by M10. M12 adds trace levels (quiet/normal/verbose), filesystem change tracking, and command chaining context. Implemented via shell wrappers (not eBPF/ptrace — those remain future options for deeper syscall-level tracing).

---

### M15: Rust Rewrite of Core Components — done (2026-05-25)

**What was built:**

4 core components rewritten in Rust as a single multi-call binary (`boos`) with symlinks:

- **`boos-exec`** (`src/rust/src/exec.rs`): command dispatch, capability check, 14 builtins
- **`boos-process`** (`src/rust/src/process.rs`): request queue processor, output capture with 1MB limit
- **`boos-submit`** (`src/rust/src/submit.rs`): request file creation, unique ID generation with O_EXCL retry
- **`boos-gateway`** (`src/rust/src/gateway.rs`): TCP listener via `std::net::TcpListener`, optional AUTH token

Shared modules: `log.rs` (JSON structured logging), `registry.rs` (command/param parsing), `config.rs` (constants), `main.rs` (multi-call dispatch).

**Build:** `scripts/build-rust.sh` — static musl binary (~585KB stripped), integrated into `scripts/build-rootfs.sh`.

**All 20 M15 issues addressed:**

| # | Issue | Fix |
|---|-------|-----|
| 1 | ID collision | Microsecond timestamp + random suffix + O_EXCL retry |
| 2 | Glob injection | Rust has no implicit glob |
| 3 | Verdict by grep | Exit codes: 0=allowed, 1=denied. No string matching. |
| 4 | AI tool gaps | `params` field parsed from .cmd files |
| 5 | Gateway no auth | Optional `BOOS_GATEWAY_TOKEN` env var, `AUTH <token>` first line |
| 6 | External binary bypass | Only `__builtin_*` or registered `exec=` with explicit type |
| 7 | Requester forgery | Set by caller, no `-r` override |
| 8 | Log injection | JSON lines with `\n` → `\\n` escaping |
| 9 | No output limit | 1MB buffer hard cap with `[truncated N bytes]` marker |
| 10 | Error swallowing | All errors logged, empty queue vs. failure distinguished |
| 11 | Supervisor liveness | Shell supervisor with PID monitoring every 2s |
| 12 | PID TOCTOU | Fork child, hold handle; no PID files for Rust binaries |
| 13 | nc -ll unreliable | `std::net::TcpListener` with `handle_connection` |
| 14 | Non-atomic write | `write(tempfile) + rename()` on same filesystem |
| 15 | Capability naming | `capability` → `enable_flag` in .cmd files |
| 16 | No sessions | `session_id` field in schema, left as `none` |
| 17 | Config hot reload | Deferred (check mtime before dispatch — not yet implemented) |
| 18 | Log format | JSON lines: `{"ts":14.940,"component":"boos-exec","event":"allowed"}` |
| 19 | Queue polling | Daemon polls every 1s; `submit --wait` deferred |
| 20 | Don't over-engineer | No tokio, no serde, just `std` + manual JSON strings |

**Kept as shell scripts:**
- `boos-shell` — thin interactive wrapper
- `boos-daemon` — polling loop (calls Rust boos-process)
- `boos-supervisor` — restart logic (calls Rust binaries)

**Verified 2026-05-25:**
- Full boot in QEMU, all 14 commands work via TCP gateway
- `status` shows kernel, uptime, trace level, daemon health
- `submit` → `results` pipeline works end-to-end
- `shell` correctly denied (capability disabled)
- `debug quiet/normal/verbose` toggles work
- JSON log entries from Rust components: `{"ts":3.890,"component":"boos-gateway","event":"started"}`
- `caps`, `log`, `commands`, `daemons`, `help` all functional
- Binary: 585KB static musl, stripped

---

## Dropped Candidates

These were in the old M9-M12 list. They are dropped because they conflict with the AI-as-primary-user philosophy:

- **Old M10: Approval Gate** — "Some actions require explicit human approval." Dropped: the AI is not a child asking permission. Humans watch, they don't gate.
- **Old: Permission-by-requester differentiation** — "Same command, different rules for AI vs human." Dropped: AI has near-native rights. Attribution is useful (M13), differentiated restriction is not.

---

## Current Philosophy

Do not turn this into a normal Linux distro.

Do not chase:
- package management
- GUI
- systemd replacement
- custom kernel work
- anti-AI security hardening (sandboxes, mandatory access control, tamper-proofing)

The project should focus on this core loop:

```txt
AI discovers available actions (registry)
  ↓
AI submits request
  ↓
system executes with full capability (near-native rights)
  ↓
system records everything (for curiosity & introspection)
  ↓
AI reads results and logs (self-awareness)
```

Normal Linux gives you a shell.

BoOS should give the AI a body.
