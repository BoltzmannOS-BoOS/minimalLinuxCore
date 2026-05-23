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

### M8: BoOS Daemon

**Status:** in progress / next thing to verify.

Goal:

`boos-daemon` should automatically process pending requests.

Target architecture:

```txt
/init
  ↓
boos-daemon &
  ↓
boos-shell
```

Request flow after M8:

```txt
submit status
  ↓
request file appears in /run/boos/requests/
  ↓
boos-daemon automatically calls boos-process
  ↓
result appears in /run/boos/results/
```

Expected user interaction:

```txt
boos> submit status
Submitted request: /run/boos/requests/req-xxxx

boos> results
BoOS request results:
----- /run/boos/results/req-xxxx.out -----
BoOS substrate status:
  kernel: ...
  uptime: ...
  pid: ...
```

The user should not need to manually run `process`. However, `process` can remain available for debugging.

---

## What Is Not Done

---

### Not Done: AI Connected to the System

There is currently no AI — local LLM, remote API, or agent — connected to BoOS.

The system has never been used by its intended primary user.

This is the most urgent gap.

---

### Not Done: Persistent Filesystem

The current initramfs is ephemeral.

Logs and results exist only during the QEMU boot session unless explicitly exported.

To observe AI behavior across sessions, persistence is needed.

---

### Not Done: Structured Observability

Current log format is plain text with minimal structure:

```txt
[boos-exec] allowed: status - show system status
```

The developer (human) is curious about:
- What exactly did the AI do? (full command with arguments)
- How did it do it? (timing, syscall patterns, resource usage)
- Why might it have done it? (sequence context, preceding commands)
- What was the result? (exit code, output, side effects)

Current logs cannot answer these questions well.

---

### Not Done: Multi-Argument Commands

Current command handling supports:

```txt
command
submit <command>
```

It does not yet support:

```txt
submit write /path/to/file "content here"
```

---

### Not Done: Requester Identity

Requests don't carry information about who submitted them:

```txt
requester=human
requester=ai
requester=system
```

This matters for curiosity: when reviewing logs, the developer wants to know which actions were AI-initiated vs human-initiated.

---

### Not Done: Real Daemon Supervision

`boos-daemon` is currently just a background shell loop.

There is no service manager, restart policy, health check, or crash recovery yet.

---

### Not Done: C/Rust Rewrite

The current implementation is shell-script based.

This is intentional for fast iteration.

Later, core components may be rewritten in C or Rust for reliability and performance.

---

## Replanned Milestones (After M8)

These replace the old M9–M12 which were written under the "human guards AI" assumption and no longer match the project philosophy.

---

### M9: AI Client Connection (NEW — top priority after M8)

**Goal:** The system gets used by its intended primary user for the first time.

Create a minimal AI client that:
- Runs on the host (not inside QEMU)
- Reads the command registry to discover available actions
- Submits requests to `/run/boos/requests/` via shared directory or serial port
- Reads results from `/run/boos/results/`

The AI can be a local LLM (Ollama) or a simple API call to any provider.

**Why this is urgent:** Without an AI using it, the system's design can only be validated by imagination. One real AI session will reveal more about what's wrong with the architecture than weeks of planning.

---

### M10: Structured Observability

**Goal:** The curious human can see what the AI did in rich detail.

Upgrade request files from:

```txt
command=status
```

to:

```ini
id=req-123
requester=ai
command=status
args=
submitted_at=<uptime>
status=pending
```

Upgrade result files to include:

```ini
id=req-123
command=status
verdict=allowed
exit_code=0
started_at=<uptime_ms>
finished_at=<uptime_ms>
duration_ms=12
output=...
```

Upgrade the log format to be structured (JSON lines or structured INI) so both human and AI can query it.

**Why:** Curiosity needs data. The developer wants to trace: what the AI requested → what happened → what the result was.

---

### M11: Persistent Overlay

**Goal:** AI behavior survives across reboots so the developer can study it.

Options:
- A disk image mounted at `/var` 
- A 9p/virtfs shared directory from the host
- Exporting `/var/log` and `/run/boos/results` to host on shutdown

**Why:** Ephemeral initramfs means every boot session is a blank slate. Curiosity about AI behavior patterns requires historical data.

---

### M12: Debug-Level Observability

**Goal:** Go beyond "what command ran" to "what did that command actually do to the system."

Add an optional debug mode (`/etc/boos/debug.conf` with `trace_level=verbose|normal|quiet`) that enables:
- Per-command timing (start/end/duration)
- Output capture and storage
- Filesystem changes tracking (what files were read/written)
- Command chaining context (what preceded this command)

This could later be implemented via eBPF (see logira), ptrace, or simple shell wrappers.

**Why:** When the AI does something surprising, the developer wants to zoom in and see the full execution trace, not just the command name.

---

### M13: Requester Identity

Add requester fields to requests:

```ini
requester=human
requester=ai
requester=system
```

**Why:** For curiosity, not for permission differentiation. The developer reviewing logs wants to distinguish AI-initiated actions from their own manual commands.

---

### M14: C/Rust Rewrite of Core Components

After the architecture stabilizes through real AI usage, rewrite core pieces:

```txt
boos-exec
boos-process
boos-submit
boos-daemon
```

in C or Rust.

Do not rush this before the design is validated by actual AI usage.

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
