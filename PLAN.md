# PLAN.md

This file is the working plan for `minimalLinuxCore`.

It is written so a new ChatGPT / Claude / agent session can continue the project without needing the previous conversation.

## Project Context

Repository:

```txt
minimalLinuxCore

Project role:

minimalLinuxCore is the minimal Linux experiment substrate for BoOS.

BoOS long-term idea:

An AI-owned computer environment where actions are registered, permissioned, submitted, executed, logged, and inspectable.

Important clarification:

minimalLinuxCore is not the entire BoOS project.
It is the low-level experiment ground.

The current repo should focus on building a tiny Linux environment that can be booted, controlled, audited, and later operated by an AI/runtime through a request interface.

Current State
Done
M1: Bootable minimal Linux

Done.

The repo can build an initramfs and boot in QEMU.

Current boot path:

QEMU
  ↓
Linux kernel
  ↓
initramfs
  ↓
/init
M2: Custom BoOS shell

Done.

/init successfully runs:

/bin/boos-shell

The system no longer directly drops into raw /bin/sh.

M3: Command logging

Done.

Commands are logged to:

/var/log/boos.log

Example log entries:

[boos-shell] input: status
[boos-exec] allowed: status - show system status
M4: Capability system

Done.

Capabilities are stored in:

/etc/boos/capabilities.conf

Example:

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

The key idea:

Commands are not automatically trusted.
Commands require explicit capabilities.
M5: Command registry

Done.

Commands are registered under:

/etc/boos/commands/

Example:

/etc/boos/commands/status.cmd

with content like:

name=status
capability=allow_status
description=show system status
exec=__builtin_status

The key idea:

The system can describe what actions exist.
Future AI/runtime should inspect this registry instead of guessing shell commands.
M6: Executor split

Done.

boos-shell is now mostly an interactive frontend.

Actual command execution goes through:

/bin/boos-exec

Architecture:

boos-shell
  ↓
boos-exec
  ↓
command registry
  ↓
capability check
  ↓
execution

The key idea:

Execution is separated from the human interface.
Future AI/programs can call boos-exec without owning the terminal.
M7: Request queue

Done.

The system supports action requests.

Request submission:

submit status

creates request files under:

/run/boos/requests/

Manual processing:

process

executes requests through:

boos-process
  ↓
boos-exec

Results are written to:

/run/boos/results/

Tested behavior:

submit status
process

works.

Also tested:

submit shell
process

and it is correctly denied when:

allow_shell=0

This confirms capability checks work through the request queue.

Current / In Progress
M8: BoOS daemon

Status:

In progress / next thing to verify.

Goal:

boos-daemon should automatically process pending requests.

Target architecture:

/init
  ↓
boos-daemon &
  ↓
boos-shell

Request flow after M8:

submit status
  ↓
request file appears in /run/boos/requests/
  ↓
boos-daemon automatically calls boos-process
  ↓
result appears in /run/boos/results/

Expected user interaction:

boos> submit status
Submitted request: /run/boos/requests/req-xxxx

boos> results
BoOS request results:
----- /run/boos/results/req-xxxx.out -----
BoOS substrate status:
  kernel: ...
  uptime: ...
  pid: ...

The user should not need to manually run:

process

although process can remain available for debugging.

What Is Not Done
Not done: real AI runtime

There is currently no local LLM or AI agent connected.

The current system only creates the system-side control interface that a future AI could use.

Not done: persistent filesystem

The current initramfs is ephemeral.

Logs and results exist only during the QEMU boot session unless explicitly exported.

Not done: real authentication

The system has capabilities, but no real users, identities, tokens, signatures, or principals.

Current permissions are simple config flags.

Not done: strong security

This is a prototype.

The current system is not secure against hostile code or malicious users.

Not done: real daemon supervision

boos-daemon is currently just a background shell loop.

There is no service manager, restart policy, health check, or crash recovery yet.

Not done: structured request format

Requests are currently simple files like:

command=status

There is no JSON, no requester identity, no timestamp, no nonce, no reason field, and no approval flow.

Not done: structured result format

Results are currently plain command output files.

There is no metadata such as exit code, timestamp, duration, requester, or status.

Not done: multi-argument commands

Current command handling mostly supports:

command

or:

submit <command>

It does not yet support rich arguments safely.

Not done: C/Rust rewrite

Current implementation is shell-script based.

This is intentional for fast iteration.

Later, core components may be rewritten in C or Rust.

Next Step
Immediate Next Step: Finish and verify M8

Implement or verify:

/bin/boos-daemon

and make sure /init starts it in the background:

/bin/boos-daemon &
exec /bin/boos-shell

Then test:

submit status
results

without manually running:

process

Expected result:

The daemon automatically processes the request.

Also test denied command path:

submit shell
results

Expected result:

Permission denied: missing capability 'shell'
Suggested M8 Implementation Checklist
Files that should exist
rootfs/bin/boos-daemon
rootfs/bin/boos-process
rootfs/bin/boos-submit
rootfs/bin/boos-exec
rootfs/bin/boos-shell
rootfs/init
rootfs/etc/boos/capabilities.conf
rootfs/etc/boos/commands/results.cmd
BusyBox symlinks likely needed

Inside:

rootfs/bin/

ensure links exist for:

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

They should point to:

busybox
M8 test commands inside BoOS

After boot:

commands
submit status
results
results
submit shell
results
log

If daemon works, results should show output without requiring manual process.

After M8
M9 candidate: structured request/result metadata

Upgrade request files from:

command=status

to something like:

id=req-123
requester=human
command=status
created_at=<uptime>
reason=manual test
status=pending

Upgrade result files to include:

id=req-123
command=status
status=allowed/denied/failed/success
exit_code=0
started_at=<uptime>
finished_at=<uptime>
output=...

Reason:

Future AI actions need attribution, traceability, and replayability.
M10 candidate: approval gate

Add a concept of commands that require approval.

Example:

approval=none
approval=human
approval=admin

So command registry entries can look like:

name=shell
capability=allow_shell
approval=human
description=enter raw BusyBox shell
exec=/bin/sh

Reason:

Some actions may be capability-allowed but still require explicit human approval.
M11 candidate: requester identity

Add requester fields:

requester=human
requester=ai
requester=system

Reason:

The same command may have different permission rules depending on who requested it.
M12 candidate: move shell scripts toward real programs

After the architecture stabilizes, consider rewriting core pieces:

boos-exec
boos-process
boos-submit
boos-daemon

in C or Rust.

Do not rush this before the design is clear.

Current Philosophy

Do not turn this into a normal Linux distro too early.

Do not chase package management, GUI, systemd replacement, or custom kernel yet.

The project should focus on this core loop:

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

That is the important BoOS idea.

Normal Linux gives you a shell.

BoOS should give you a controlled action substrate.
