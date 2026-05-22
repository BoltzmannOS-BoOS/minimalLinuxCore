# minimalLinuxCore

`minimalLinuxCore` is the experimental Linux substrate for **BoOS**.

BoOS is not just a chatbot, not just a normal Linux distro, and not just an agent script. The long-term idea is to explore an **AI-owned computer environment**: a small, inspectable, permission-controlled system where an AI can observe, request actions, execute allowed commands, and leave auditable traces.

This repository is currently used as the low-level experiment ground for that idea.

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

/init
  ↓
boos-shell    ← human interactive interface

The system has two main paths:

Human interactive path
human
  ↓
boos-shell
  ↓
boos-exec
  ↓
capability check
  ↓
execution
Request queue path
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
Completed Milestones
M1: Bootable minimal Linux

Status: complete.

The repo can build an initramfs and boot it in QEMU using a Linux kernel.

M2: Custom BoOS shell

Status: complete.

Instead of directly entering /bin/sh, the system boots into:

/bin/boos-shell
M3: Command logging

Status: complete.

BoOS commands are logged into:

/var/log/boos.log
M4: Capability system

Status: complete.

Commands are controlled by:

/etc/boos/capabilities.conf

Example:

allow_help=1
allow_status=1
allow_log=1
allow_shell=0
allow_poweroff=0

This means commands are not naturally trusted. They must be explicitly allowed.

M5: Command registry

Status: complete.

Commands are registered under:

/etc/boos/commands/

Example command file:

name=status
capability=allow_status
description=show system status
exec=__builtin_status

This makes BoOS commands discoverable and describable instead of being hidden inside shell case branches.

M6: Executor split

Status: complete.

Execution logic was separated from the interactive shell.

boos-shell
  ↓
boos-exec

This is important because future AI or programmatic callers should not need to control a TTY directly.

M7: Request queue

Status: complete.

The system supports request submission and processing:

submit status
process
results

A request is written to:

/run/boos/requests/

A result is written to:

/run/boos/results/
M8: BoOS daemon

Status: in progress / being tested.

The intended daemon design is:

/init
  ↓
boos-daemon &
  ↓
boos-shell

boos-daemon should automatically scan and process request files so that the user does not need to manually run process.

Important Design Principle

The point of this repo is not to make another general-purpose Linux distro.

The point is to gradually build a minimal system where actions are:

registered
described
permissioned
submitted
executed
logged
inspectable

That is the core difference between this and a normal shell.

A normal shell asks:

Can this command run?

BoOS asks:

Is this action registered, authorized, logged, and attributable?

Current Commands

Expected BoOS commands include:

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

Some commands may be blocked depending on the capability file.

For example, if:

allow_shell=0

then:

submit shell

should eventually produce:

Permission denied: missing capability 'shell'
Development Environment

Current development setup:

Windows
  ↓
WSL2 Ubuntu
  ↓
minimalLinuxCore repo
  ↓
QEMU

The repo should live inside WSL, for example:

~/projects/minimalLinuxCore

not inside /mnt/c/....

Build

Build the initramfs:

./scripts/build-rootfs.sh

Run QEMU:

./scripts/run-qemu.sh

Exit QEMU:

Ctrl+A, then X
Repository Direction

This repo is not the whole BoOS project.

A better long-term structure may be:

BoOS
├── minimalLinuxCore    # minimal bootable Linux experiment
├── boos-runtime        # future agent/runtime layer
├── boos-shell          # controlled human/system command interface
├── boos-exec           # action execution layer
├── boos-memory         # future persistent memory
├── boos-policy         # permission and capability logic
└── docs                # architecture, roadmap, threat model

For now, this repo remains the experimental substrate.