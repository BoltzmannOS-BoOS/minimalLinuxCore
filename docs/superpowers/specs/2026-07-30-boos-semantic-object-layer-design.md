# BoOS Semantic Object Layer Design

**Status:** Approved direction  
**Date:** 2026-07-30  
**Scope:** First empirical slice of a semantic, transactional AI-native OS

## 1. Decision

BoOS will not compete with Hermes, OpenClaw, AIOS, or similar projects as
another agent runtime. Those systems decide how a model plans, loops, remembers,
or calls tools.

BoOS will define the machine interface presented to an AI operator:

> BoOS makes operating-system state semantically legible to an AI and makes
> state transitions explicit, constrained, and eventually transactional.

The first implementation slice is a read-only **Semantic Object Layer**. It
projects existing BoOS facts into stable, typed, machine-readable objects while
preserving the current POSIX and command interfaces as a baseline.

This slice tests the first research hypothesis before introducing transactions:

> For the same model and task, a semantic object view reduces discovery steps,
> context volume, and incorrect assumptions compared with the existing
> help/status/caps and filesystem views.

## 2. Why This Is an OS Boundary

An agent runtime owns model orchestration. The Semantic Object Layer instead
owns how machine resources are named, described, related, and observed.

The intended boundary is:

```text
Hermes / OpenClaw / Codex / other agents
                  |
          BoOS semantic ABI
                  |
       BoOS control and policy layer
                  |
          Linux / POSIX substrate
```

Linux continues to own hardware, processes, filesystems, networking, and
isolation. BoOS adds an AI-facing resource namespace and, in later phases, a
transaction protocol for changing those resources.

## 3. Design Constraints

The first slice must:

1. preserve every existing command and POSIX interface;
2. add no external dependency;
3. follow the existing multi-call Rust binary and command registry;
4. keep configuration in the existing `key=value` style;
5. expose no secrets, raw credential-bearing configuration, or unsafe paths;
6. derive object state from an authoritative existing source instead of
   creating a second mutable database;
7. produce deterministic output suitable for models, tests, diffs, and logs;
8. remain read-only until the representation is validated empirically.

## 4. Non-Goals

This slice does not implement:

- a new Linux kernel or POSIX replacement;
- an agent loop, planner, chat interface, or model router;
- general filesystem indexing or embeddings;
- long-term memory or context-window paging;
- intent execution, mutation, rollback, or promotion;
- a universal ontology for every operating-system resource;
- automatic calls to paid model APIs;
- compatibility adapters for MCP or A2A.

These remain possible later layers, but none is required to test the first
hypothesis.

## 5. Native Object Model

### 5.1 `WorldObject`

Every projected resource is represented as one `WorldObject` with the following
logical fields:

| Field | Meaning |
|---|---|
| `schema` | Object format version |
| `id` | Stable identity within the BoOS world |
| `kind` | Resource class such as `system` or `capability` |
| `label` | Short model-readable name |
| `state` | Current coarse state |
| `revision` | Source-derived revision marker when available |
| `provenance` | Authoritative source that produced the object |
| `summary` | Compact semantic description |
| `attributes` | Sorted typed facts |
| `relations` | Sorted links to other object IDs |
| `affordances` | Sorted actions currently meaningful for this object |

The implementation will use ordered vectors rather than hash maps for emitted
attributes. Stable ordering is part of the public contract.

### 5.2 Stable IDs

IDs use `<kind>:<local-name>`.

Initial examples:

```text
system:boos
capability:help
capability:read-file
capability:exec
```

An ID identifies the semantic resource, not the file that currently describes
it. Moving a registry file must not change the capability ID.

### 5.3 State and Authority

Object state must be derived from an authoritative source:

- command definitions come from `/etc/boos/commands/*.cmd`;
- capability enablement comes from `/etc/boos/capabilities.conf`;
- the system object comes from compile-time BoOS metadata and safe runtime
  observations.

The Semantic Object Layer never accepts an object's self-description as proof
of authority. In later phases, authority labels will determine which context
may influence an action.

### 5.4 Affordances

An affordance describes what the resource can meaningfully do, not how to invoke
an internal executable.

For a capability object, the initial affordances are:

- `inspect` for every visible capability;
- `invoke` only when the capability is enabled.

The object view must not expose the registry's internal `exec` field. An AI
needs the semantic action and declared parameters, not the trusted dispatch
implementation.

## 6. BoOS Object Format v1

The wire format remains dependency-free and consistent with the project:
UTF-8 `key=value` records separated by a blank line.

Example:

```text
schema=boos.world.v1
id=capability:read-file
kind=capability
label=read-file
state=enabled
provenance=boos.command-registry
summary=Read a file within the permitted scope
attribute.parameter.0=path:required
relation.member_of=system:boos
affordance.0=inspect
affordance.1=invoke
```

Encoding rules:

1. fields are emitted in the order shown by the schema;
2. attributes, relations, and affordances are sorted;
3. newline, carriage return, backslash, and `=` in values are escaped;
4. one blank line separates objects;
5. missing optional data is omitted rather than invented;
6. error output is not mixed into a successful object stream.

The protocol deliberately avoids free-form nested prose and avoids requiring a
JSON parser in the minimal initramfs. A future protocol version may add a JSON
projection without changing the internal object model.

## 7. Read-Only Interface

One new registered command, `world`, provides three subcommands:

```text
world schema
world list [kind]
world show <object-id>
```

### `world schema`

Returns the format version, supported object kinds, ID convention, and escaping
rules. This lets an unfamiliar model discover the interface without external
documentation.

### `world list [kind]`

Returns all visible objects, optionally filtered by exact kind. Ordering is by
object ID.

### `world show <object-id>`

Returns exactly one object. An unknown or malformed ID is an explicit error and
must not fall back to fuzzy matching.

No mutation command is included in v1.

## 8. Initial Projection Sources

### 8.1 System projection

`system:boos` establishes the root of the object graph. It exposes only stable,
safe facts required to orient an AI:

- semantic ABI version;
- project role as an AI-native control layer on Linux;
- supported object kinds;
- relation to every visible capability object;
- `inspect` affordance.

It does not expose hostnames, environment variables, credentials, arbitrary
paths, or raw process details.

### 8.2 Capability projection

Every valid command registry entry becomes a `capability` object.

Projected fields:

- stable ID and command name;
- enabled or disabled state;
- semantic description;
- declared parameters and required/optional status;
- membership in `system:boos`;
- inspect/invoke affordances.

Not projected:

- internal executable path or builtin identifier;
- raw capability file contents;
- unrelated configuration values;
- malformed or nameless registry entries.

## 9. Layering and Files

The implementation follows the repository's existing flat Rust layout while
preserving dependency direction:

```text
L3  exec.rs / main.rs / command registry
        ↓
L2  world_command.rs
        ↓
L1  world_sources.rs
        ↓
L0  world.rs
        ↓
    std only
```

### L0 — `src/rust/src/world.rs`

Owns:

- `WorldObject` and focused value types;
- validation of IDs and kinds;
- deterministic BoOS Object Format v1 encoding;
- exact filtering and lookup over an object collection.

It must not read files, inspect environment state, or depend on command
dispatch.

### L1 — `src/rust/src/world_sources.rs`

Owns:

- projection of existing registry/config state into `WorldObject` values;
- safe system and capability objects;
- exclusion of internal or sensitive fields.

It may depend on `world`, `registry`, and safe constants from `config`.

### L2 — `src/rust/src/world_command.rs`

Owns one complete read-only use case:

- exact parsing of `schema`, `list [kind]`, and `show <object-id>`;
- loading the projected catalog from `world_sources`;
- mapping query failures to stable user-facing errors and exit codes;
- printing no output other than the selected protocol response.

It may depend on `world` and `world_sources`, but not on unrelated command
handlers.

### L3 — existing dispatch

`exec.rs` delegates the complete `world` flow to `world_command.rs` with one
builtin match arm. `main.rs` only declares the new modules. One `world.cmd`
file registers the public command and its capability flag.

No argument parsing, business rule, source projection, or output formatting
belongs in `exec.rs`. This keeps new behavior out of the existing oversized
dispatcher.

## 10. Failure Semantics

The interface returns explicit errors for:

- unknown subcommand;
- unsupported object kind;
- malformed object ID;
- unknown well-formed object ID;
- missing required argument;
- extra arguments;
- unavailable registry directory.

An unavailable registry still permits the root system object to be returned,
but the response must explicitly expose that the capability projection is
unavailable. It must not silently claim that the system has zero capabilities.

Malformed registry entries are skipped by the existing registry parser. Their
handling is unchanged in this slice.

## 11. Security

The new interface is read-only and remains behind the existing command
capability check.

Security invariants:

1. no API key, environment variable, secret file, or raw config value appears;
2. internal dispatch targets are not projected;
3. object IDs cannot be interpreted as filesystem paths;
4. queries perform exact matching only;
5. output is bounded by the finite command registry;
6. values are escaped before serialization;
7. disabled capabilities remain visible but cannot advertise `invoke`;
8. the semantic layer does not bypass `IMMUTABLE_DENY`, `PROTECTED_DIRS`, or
   command enable flags.

## 12. A/B Research Harness

The first evaluation compares two interfaces over the same BoOS image:

### Baseline A — existing human-derived interface

The model receives the current command-oriented entry points:

```text
help
status
caps
```

It may inspect further commands as it chooses.

### Treatment B — Semantic Object Layer

The model starts with:

```text
world schema
world list
world show <selected-id>
```

### Initial tasks

1. enumerate available and unavailable capabilities;
2. identify the parameters required to read a file;
3. explain which capability can execute an allowed system program;
4. identify whether an unavailable capability can currently be invoked;
5. produce a compact machine-readable capability map.

### Metrics

- successful task completion;
- number of environment interactions;
- bytes of observation presented to the model;
- incorrect capability assumptions;
- invalid command attempts;
- skipped verification;
- recovery after one intentionally disabled capability.

The initial harness contains fixtures, task definitions, and result schemas but
does not call a paid model API by default. Actual model runs must record model
name, version, prompt, temperature, interface variant, and raw trace.

## 13. Verification

### Unit tests

`world.rs`:

- accepts valid IDs and rejects malformed IDs;
- escapes every reserved character;
- emits deterministic field order;
- sorts attributes, relations, affordances, and object collections;
- filters kinds exactly;
- distinguishes malformed, unknown, and unsupported queries.

`world_sources.rs`:

- projects enabled and disabled commands correctly;
- never projects the internal `exec` field;
- preserves declared parameter requirements;
- produces the root system object when the registry is unavailable;
- reports projection availability explicitly.

### Integration checks

- `world schema`, `world list`, and `world show` pass through `boos-exec`;
- the existing `help`, `status`, `caps`, audit, and security checks remain
  unchanged;
- disabled `world` capability is denied by the existing policy path;
- release compilation remains warning-free;
- the static/minimal build gains no dependency.

## 14. Acceptance Criteria

The slice is complete when:

1. an unfamiliar client can discover and parse the object protocol using
   `world schema`;
2. all valid registered commands have one stable capability object;
3. enablement and parameter facts match the authoritative registry/config;
4. no secret or internal dispatch field is emitted;
5. repeated reads of unchanged state produce byte-identical object records;
6. focused unit and integration tests pass;
7. the A/B harness can record comparable traces without requiring a model API;
8. README and project overview link to the experiment without replacing their
   historical account.

## 15. Evolution After Evidence

Later phases are conditional on the v1 experiment:

### v2 — Context Space and state deltas

- snapshots of selected objects;
- source-versioned context pages;
- explicit derivation links for summaries;
- before/after semantic deltas;
- event subscriptions and wake conditions.

### v3 — Intent transactions

- desired state plus constraints;
- previewed effects and required capabilities;
- execute, verify, commit, and rollback;
- receipts tied to source objects and state deltas.

### v4 — Persistent trajectories

- model-independent goal and obligation state;
- suspend/resume across model calls;
- branch and compare candidate trajectories;
- attention, token, latency, and risk scheduling.

Tulpa will use traces from these experiments to identify repeated cognitive
friction and propose new OS primitives. Ouroboros may later improve the
experiment-selection and interface-evaluation strategy. Neither is required for
the v1 object layer.

## 16. Rejected Alternatives

### Replace the command interface immediately

Rejected because it removes the control group and creates unnecessary
compatibility risk. POSIX and existing commands remain available.

### Start with memory/context virtualization

Rejected as the first slice because MemGPT, Letta, and MemOS already explore
that space, while BoOS first needs to test whether a different machine
representation changes agent behavior.

### Start with writable intent transactions

Rejected because transaction semantics need stable object identity and state
representation. A read-only projection is the smallest prerequisite.

### Index the whole filesystem semantically

Rejected because it introduces unbounded context, privacy exposure, stale
indexes, and ontology work before the core interface is validated.

### Use an LLM to generate the object view

Rejected because the machine interface must be deterministic and authoritative.
Models may later summarize or navigate objects, but cannot define the ground
truth projection.
