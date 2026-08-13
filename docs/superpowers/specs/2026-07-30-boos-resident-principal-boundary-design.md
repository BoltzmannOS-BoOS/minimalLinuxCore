# BoOS Resident Principal Boundary Design

**Status:** Approved for implementation on 2026-07-30

## Purpose

BoOS currently boots a TCP command gateway and waits for an external client.
The repository describes an AI as the primary user, but no AI-owned process is
part of the boot lifecycle. The existing `boos-agent` default mode also starts
a second gateway, while the supervisor already owns that service.

This design establishes the smallest architecture-consistent boundary needed
before multi-AI skill sharing can be implemented:

1. BoOS boots a resident AI principal independently of remote access.
2. Linux process identity anchors the principal identity used by BoOS.
3. Memory, request queues, and results belong to a principal rather than a
   caller-provided label.
4. The TCP gateway becomes an optional adapter with its own isolated debug
   principal.

This phase creates the trustworthy substrate. It does not implement an LLM
planner, multi-agent scheduling, or the skill registry itself.

## Product invariants

- BoOS must reach a resident-principal-ready state without any host connection.
- A remote adapter must not be required for boot, state, or command execution.
- A principal identifier supplied in an environment variable is accepted only
  when the current Linux effective UID matches the configured UID.
- A queued request's owner is derived from its principal spool, never from a
  `requester` string inside the request record.
- Each principal can read and mutate only its own memory and result namespace
  through normal BoOS commands.
- The root processor may traverse every principal spool to execute requests.
- Existing single-principal command behavior remains available through a
  configured `debug` principal when the gateway adapter is explicitly enabled.
- Product rootfs disables the TCP gateway by default.

## Layered architecture

### L0: Principal identity

`principal.rs` owns:

- `PrincipalId`, including syntax validation;
- `PrincipalDefinition { id, user, uid, enabled }`;
- parsing `/etc/boos/principals/*.principal`;
- resolving a claimed principal against the effective UID;
- deriving principal-owned runtime paths.

The Linux UID is the trust anchor. `BOOS_PRINCIPAL_ID` selects a configured
principal, but cannot grant another principal's identity because resolution
also checks the effective UID.

Definitions:

```text
/etc/boos/principals/
  resident.principal  id=resident user=boos-agent   uid=101 enabled=1
  debug.principal     id=debug    user=boos-gateway uid=100 enabled=1
```

Runtime layout:

```text
/var/boos/principals/<principal>/
  status.kv
  memory/
    working.kv
    recent/
    archive/
  requests/
  results/
```

### L1: Principal-owned state and queue storage

Memory resolves its root through the current `PrincipalContext`. The legacy
`BOOS_AGENT_ID` variable remains accepted only as a compatibility alias when
`BOOS_PRINCIPAL_ID` is absent, and it is subject to the same configured UID
check.

Submission writes to the current principal's request spool. Request records no
longer grant identity through their `requester` field. The processor loads each
enabled principal definition, scans its request directory, and publishes into
the corresponding result directory.

Result, audit, and result-list commands use the current principal's result
directory. Cross-principal administrative inspection is outside this phase.

### L2: Resident principal lifecycle

`resident_agent.rs` owns the resident process lifecycle:

- resolve and validate its principal context;
- create a session if none exists;
- atomically publish `status.kv` with `state=ready`, PID, and start time;
- refresh a heartbeat without starting a gateway;
- log startup and fatal resolution errors.

The ready state means that the AI runtime slot and its local OS interfaces are
alive. It does not claim that a model provider is configured or that reasoning
has occurred.

### L3: Boot and optional adapters

The supervisor starts:

- the resident agent under `boos-agent`, with
  `BOOS_PRINCIPAL_ID=resident`;
- no TCP listener in the product rootfs.

The gateway daemon remains packaged but disabled. CI creates a disposable
overlay that enables it, assigns `BOOS_PRINCIPAL_ID=debug`, and adds an
authentication token. This tests the adapter without making it part of the
product's boot contract.

## Compatibility and migration

- The multicall binary names remain unchanged.
- Existing command registry files remain unchanged.
- Existing `/var/boos/memory`, `/var/boos/requests`, and
  `/var/boos/results` are not automatically copied. Silent migration would
  risk assigning old shared data to the wrong principal.
- If no valid principal context exists, stateful commands fail closed with a
  clear error instead of falling back to global shared state.
- `requester` may remain in serialized records for trace compatibility, but
  processors overwrite its semantic meaning with the spool principal.

## Verification

Unit tests must prove:

- malformed principal IDs and duplicate definitions are rejected;
- a matching ID with a mismatched UID is rejected;
- runtime paths cannot escape the configured root;
- queue requests inherit the spool principal even if their body claims another
  requester;
- memory paths are principal-scoped;
- resident mode never owns or starts a gateway.

QEMU verification must prove:

- the product rootfs reaches `resident_ready`;
- the product rootfs has no externally listening gateway by default;
- a CI-only overlay can enable the authenticated debug adapter;
- commands sent through that adapter write only to the debug principal;
- the resident principal remains alive while the adapter is exercised.

## Deferred phases

### Phase 2: Skill views

Add immutable skill versions and per-principal views:

- private overlay;
- explicitly mounted shared collections;
- pinned snapshots;
- provenance and dependency metadata;
- publish/promote operations.

### Phase 3: Skill subscriptions and multi-AI coordination

Add:

- opt-in hot-update subscriptions;
- task-level snapshot pinning;
- revocation and rollback;
- event delivery;
- concurrency-safe publication;
- evidence based on real cross-project skill incidents rather than a
  benchmark constructed around BoOS interfaces.

## Rejected alternatives

### Keep the gateway as the core and add authentication

Rejected because it preserves an external client as the system's first active
user and conflates transport, model secrets, and execution.

### Trust `BOOS_AGENT_ID`

Rejected because a process can choose its own environment. The Linux UID must
match a configured principal before the identifier is accepted.

### Implement a planner in BoOS

Rejected because Hermes, OpenClaw, and other runtimes already own planning.
BoOS owns identity, state, resources, events, and isolation; planners attach
as resident workloads.

