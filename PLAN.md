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

### BoOS Long-Term Idea

BoOS is an **AI-owned computer environment** where actions are:

- registered
- permissioned
- submitted
- executed
- logged
- inspectable

### Important Clarification

`minimalLinuxCore` is **not** the entire BoOS project.

It is the low-level experimental ground.

The current repo should focus on building a tiny Linux environment that can be:

- booted
- controlled
- audited
- operated later by an AI/runtime through a request interface

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
allow_shell=0
allow_reboot=0
allow_poweroff=0
```

The key idea:

> Commands are not automatically trusted.  
> Commands require explicit capabilities.

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

> The system can describe what actions exist.  
> Future AI/runtime should inspect this registry instead of guessing shell commands.

---

### M6: Executor Split

**Status:** done.

`boos-shell` is now mostly an interactive frontend.

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

> Execution is separated from the human interface.  
> Future AI/programs can call `boos-exec` without owning the terminal.

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

Tested behavior:

```txt
submit status
process
```

works.

Also tested:

```txt
submit shell
process
```

This is correctly denied when:

```conf
allow_shell=0
```

This confirms that capability checks work through the request queue.

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

The user should not need to manually run:

```txt
process
```

However, `process` can remain available for debugging.

---

## What Is Not Done

---

### Not Done: Real AI Runtime

There is currently no local LLM or AI agent connected.

The current system only creates the system-side control interface that a future AI could use.

---

### Not Done: Persistent Filesystem

The current initramfs is ephemeral.

Logs and results exist only during the QEMU boot session unless explicitly exported.

---

### Not Done: Real Authentication

The system has capabilities, but no real users, identities, tokens, signatures, or principals.

Current permissions are simple config flags.

---

### Not Done: Strong Security

This is a prototype.

The current system is not secure against hostile code or malicious users.

---

### Not Done: Real Daemon Supervision

`boos-daemon` is currently just a background shell loop.

There is no service manager, restart policy, health check, or crash recovery yet.

---

### Not Done: Structured Request Format

Requests are currently simple files like:

```txt
command=status
```

There is no structured metadata yet, such as:

- JSON format
- requester identity
- timestamp
- nonce
- reason field
- approval flow

---

### Not Done: Structured Result Format

Results are currently plain command output files.

There is no metadata yet, such as:

- exit code
- timestamp
- duration
- requester
- status

---

### Not Done: Multi-Argument Commands

Current command handling mostly supports:

```txt
command
```

or:

```txt
submit <command>
```

It does not yet support rich arguments safely.

---

### Not Done: C/Rust Rewrite

The current implementation is shell-script based.

This is intentional for fast iteration.

Later, core components may be rewritten in C or Rust.

---

## Next Step

### Immediate Next Step: Finish and Verify M8

Implement or verify:

```txt
/bin/boos-daemon
```

Make sure `/init` starts it in the background:

```sh
/bin/boos-daemon &
exec /bin/boos-shell
```

Then test:

```txt
submit status
results
```

without manually running:

```txt
process
```

Expected result:

```txt
The daemon automatically processes the request.
```

Also test the denied command path:

```txt
submit shell
results
```

Expected result:

```txt
Permission denied: missing capability 'allow_shell'
```

---

## Suggested M8 Implementation Checklist

### Files That Should Exist

```txt
rootfs/bin/boos-daemon
rootfs/bin/boos-process
rootfs/bin/boos-submit
rootfs/bin/boos-exec
rootfs/bin/boos-shell
rootfs/init
rootfs/etc/boos/capabilities.conf
rootfs/etc/boos/commands/results.cmd
```

### BusyBox Symlinks Likely Needed

Inside:

```txt
rootfs/bin/
```

ensure links exist for:

```txt
sh
cat
echo
mount
uname
cut
grep
poweroff
basename
tr
rm
mkdir
sleep
```

They should point to:

```txt
busybox
```

---

## M8 Test Commands Inside BoOS

After boot:

```txt
commands
submit status
results
results
submit shell
results
log
```

If the daemon works, `results` should show output without requiring manual `process`.

---

## After M8

---

### M9 Candidate: Structured Request/Result Metadata

Upgrade request files from:

```txt
command=status
```

to something like:

```ini
id=req-123
requester=human
command=status
created_at=<uptime>
reason=manual test
status=pending
```

Upgrade result files to include:

```ini
id=req-123
command=status
status=allowed/denied/failed/success
exit_code=0
started_at=<uptime>
finished_at=<uptime>
output=...
```

Reason:

> Future AI actions need attribution, traceability, and replayability.

---

### M10 Candidate: Approval Gate

Add a concept of commands that require approval.

Example approval modes:

```ini
approval=none
approval=human
approval=admin
```

So command registry entries can look like:

```ini
name=shell
capability=allow_shell
approval=human
description=enter raw BusyBox shell
exec=/bin/sh
```

Reason:

> Some actions may be capability-allowed but still require explicit human approval.

---

### M11 Candidate: Requester Identity

Add requester fields:

```ini
requester=human
requester=ai
requester=system
```

Reason:

> The same command may have different permission rules depending on who requested it.

---

### M12 Candidate: Move Shell Scripts Toward Real Programs

After the architecture stabilizes, consider rewriting core pieces:

```txt
boos-exec
boos-process
boos-submit
boos-daemon
```

in C or Rust.

Do not rush this before the design is clear.

---

## Current Philosophy

Do not turn this into a normal Linux distro too early.

Do not chase:

- package management
- GUI
- systemd replacement
- custom kernel work

The project should focus on this core loop:

```txt
registered action
  ↓
capability check
  ↓
request submission
  ↓
execution
  ↓
result
  ↓
audit log
```

That is the important BoOS idea.

Normal Linux gives you a shell.

BoOS should give you a controlled action substrate.