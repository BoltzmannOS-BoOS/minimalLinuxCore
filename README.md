# minimalLinuxCore

`minimalLinuxCore` is the experimental Linux substrate for **BoOS**.

BoOS is not just a chatbot, not just a normal Linux distro, and not just an agent script.

The long-term idea is to explore an **AI-owned computer environment**: a small, inspectable, permission-controlled system where an AI can observe, request actions, execute allowed commands, and leave auditable traces.

This repository is currently used as the low-level experimental ground for that idea.

---

## Current Purpose

The purpose of this repo is to answer a narrow question first:

> What is the smallest Linux-based environment that can act as a controlled body for BoOS?

So this repo currently focuses on:

- booting a minimal Linux system in QEMU
- using an initramfs-based root filesystem
- starting a custom `/init`
- replacing the raw Linux shell with a BoOS-controlled shell
- registering commands as explicit system capabilities
- logging system actions
- routing actions through a request/execution pipeline

---

## Current Architecture

The current boot chain is:

```txt
QEMU
  ↓
Linux kernel
  ↓
initramfs
  ↓
/init
  ↓
boos-daemon   ← background request processor
  ↓
boos-process
  ↓
boos-exec
  ↓
command registry
  ↓
capability check
  ↓
audit log + result file
```

The system also starts the interactive shell:

```txt
/init
  ↓
boos-shell    ← human interactive interface
```

The system currently has two main execution paths.

### Human Interactive Path

```txt
human
  ↓
boos-shell
  ↓
boos-exec
  ↓
capability check
  ↓
execution
```

### Request Queue Path

```txt
human / future AI / future program
  ↓
boos-submit
  ↓
/run/boos/requests/req-*
  ↓
boos-daemon
  ↓
boos-process
  ↓
boos-exec
  ↓
capability check
  ↓
/run/boos/results/*.out
```

---

## Completed Milestones

### M1: Bootable Minimal Linux

**Status:** complete.

The repo can build an initramfs and boot it in QEMU using a Linux kernel.

---

### M2: Custom BoOS Shell

**Status:** complete.

Instead of directly entering `/bin/sh`, the system boots into:

```txt
/bin/boos-shell
```

---

### M3: Command Logging

**Status:** complete.

BoOS commands are logged into:

```txt
/var/log/boos.log
```

---

### M4: Capability System

**Status:** complete.

Commands are controlled by:

```txt
/etc/boos/capabilities.conf
```

Example:

```conf
allow_help=1
allow_status=1
allow_log=1
allow_shell=0
allow_poweroff=0
```

This means commands are not naturally trusted. They must be explicitly allowed.

---

### M5: Command Registry

**Status:** complete.

Commands are registered under:

```txt
/etc/boos/commands/
```

Example command file:

```ini
name=status
capability=allow_status
description=show system status
exec=__builtin_status
```

This makes BoOS commands discoverable and describable instead of being hidden inside shell case branches.

---

### M6: Executor Split

**Status:** complete.

Execution logic was separated from the interactive shell.

```txt
boos-shell
  ↓
boos-exec
```

This is important because future AI or programmatic callers should not need to control a TTY directly.

---

### M7: Request Queue

**Status:** complete.

The system supports request submission and processing:

```txt
submit status
process
results
```

A request is written to:

```txt
/run/boos/requests/
```

A result is written to:

```txt
/run/boos/results/
```

---

### M8: BoOS Daemon

**Status:** in progress / being tested.

The intended daemon design is:

```txt
/init
  ↓
boos-daemon &
  ↓
boos-shell
```

`boos-daemon` should automatically scan and process request files so that the user does not need to manually run `process`.

---

## Important Design Principle

The point of this repo is not to make another general-purpose Linux distro.

The point is to gradually build a minimal system where actions are:

- registered
- described
- permissioned
- submitted
- executed
- logged
- inspectable

That is the core difference between BoOS and a normal shell.

A normal shell asks:

> Can this command run?

BoOS asks:

> Is this action registered, authorized, logged, and attributable?

---

## Current Commands

Expected BoOS commands include:

```txt
help
commands
status
caps
log
submit <command>
process
results
shell
poweroff
```

Some commands may be blocked depending on the capability file.

For example, if:

```conf
allow_shell=0
```

then:

```txt
submit shell
```

should eventually produce something like:

```txt
Permission denied: missing capability 'allow_shell'
```

---

## Development Environment

Current development setup:

```txt
Windows
  ↓
WSL2 Ubuntu
  ↓
minimalLinuxCore repo
  ↓
QEMU
```

The repo should live inside WSL, for example:

```txt
~/projects/minimalLinuxCore
```

It should not live inside:

```txt
/mnt/c/...
```

Keeping the repo inside the Linux filesystem avoids slow filesystem behavior and path-related issues.

---

## Build

Build the initramfs:

```sh
./scripts/build-rootfs.sh
```

Run QEMU:

```sh
./scripts/run-qemu.sh
```

Exit QEMU:

```txt
Ctrl+A, then X
```

---

## Repository Direction

This repo is not the whole BoOS project.

A better long-term structure may be:

```txt
BoOS
├── minimalLinuxCore    # minimal bootable Linux experiment
├── boos-runtime        # future agent/runtime layer
├── boos-shell          # controlled human/system command interface
├── boos-exec           # action execution layer
├── boos-memory         # future persistent memory
├── boos-policy         # permission and capability logic
└── docs                # architecture, roadmap, threat model
```

For now, this repo remains the experimental substrate.

---

## Current Scope

This repo currently focuses on the low-level Linux substrate only.

It does not yet try to solve the full AI runtime problem.

### What This Repo Is

This repo is:

- a minimal Linux boot experiment
- a controlled command environment
- a capability-based execution prototype
- an audit/logging experiment
- a future body for BoOS runtime experiments

### What This Repo Is Not Yet

This repo is not yet:

- a full AI operating system
- a production Linux distribution
- a complete agent runtime
- a security-hardened sandbox
- a replacement for normal Linux
- a finished permission model

---

## Next Steps

The next likely steps are:

1. Finish and test `boos-daemon`.
2. Make request processing automatic after boot.
3. Improve result inspection with a stable `results` command.
4. Add request IDs and better audit metadata.
5. Separate human-submitted requests from future AI-submitted requests.
6. Add clearer policy failure messages.
7. Start writing architecture docs for the future BoOS runtime layer.

---

## Long-Term Goal

The long-term goal is to build toward a system where an AI does not merely chat about actions, but operates inside a controlled computer environment.

That environment should make every action:

- explicit
- inspectable
- permissioned
- logged
- attributable
- reversible where possible
- understandable to humans

`minimalLinuxCore` is the first small step toward that system.