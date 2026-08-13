# BoOS Resident Principal Boundary Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Boot a UID-backed resident AI principal, scope memory/requests/results to that principal, and make the TCP gateway an optional debug adapter.

**Architecture:** A new L0 principal module validates configured principal IDs against Linux UIDs and derives runtime paths. Existing memory and queue components consume that context; a focused resident lifecycle publishes readiness without owning transport. Rootfs starts the resident principal and disables the gateway, while CI enables the debug adapter only in its disposable overlay.

**Tech Stack:** Rust 2021 standard library, BusyBox initramfs, QEMU, shell CI, key-value configuration.

## Status

### Current goal

Complete. The implementation is published in draft
[PR #8](https://github.com/BoltzmannOS-BoOS/minimalLinuxCore/pull/8)
against `feat/reality-check-runtime`.

### Completed

- Resident principal boots without a remote client or network device.
- Effective-UID-backed identity rejects duplicate principal IDs and UIDs.
- Memory, requests, results, status, and last-command state are
  principal-owned.
- The product gateway is disabled; CI enables an isolated authenticated debug
  adapter.
- Principal state cannot be forged through the generic file API.
- Obsolete gateway-era shell daemon and conflict artifacts were removed.

### In progress

None for this phase.

### Next steps

Implement Phase 2 immutable skill versions and per-principal skill views from
real cross-project sharing/isolation incidents.

### Blockers

None. GitHub Actions run
[`30546826013`](https://github.com/BoltzmannOS-BoOS/minimalLinuxCore/actions/runs/30546826013)
passed both Rust and real boot/QEMU jobs.

### Related files

- `docs/superpowers/specs/2026-07-30-boos-resident-principal-boundary-design.md`
- `src/rust/src/principal.rs`
- `src/rust/src/resident_agent.rs`
- `rootfs/init`
- `scripts/ci-test.sh`

### Verification record

- Remote `cargo test --locked`: 193 passed, 0 failed.
- Remote `cargo clippy --locked --all-targets -- -D warnings`: passed.
- Remote musl release build and Living Evidence System tests: passed.
- Initramfs artifact/provenance and kernel-module verification: passed.
- Real QEMU product boot with `-nic none`: resident ready.
- Real QEMU authenticated debug overlay: 23 checks, 0 failures.
- GitHub Actions exact candidate `24ed84a`: Rust and boot jobs passed.

## Global Constraints

- Do not compile in `/Users/hostsjim/project/minimalLinuxCore`; all Rust red/green verification runs on the `aliyun` SSH host under `/opt/boos-build/resident-principal`.
- Preserve the existing multicall binary and command registry contracts.
- Add no new Rust dependencies.
- Treat Linux UID plus configured principal ID as the identity boundary.
- Do not auto-migrate legacy shared state into a principal namespace.
- Product rootfs must not listen on TCP port 5555 by default.
- Each task ends with focused remote tests and a commit.

---

### Task 1: UID-backed principal definitions and context

**Files:**
- Create: `src/rust/src/principal.rs`
- Modify: `src/rust/src/main.rs`
- Modify: `src/rust/src/config.rs`

**Interfaces:**
- Produces: `PrincipalId::parse(&str) -> io::Result<PrincipalId>`
- Produces: `PrincipalDefinition { id: PrincipalId, user: String, uid: u32, gid: u32, enabled: bool }`
- Produces: `load_definitions_from(&Path) -> io::Result<Vec<PrincipalDefinition>>`
- Produces: `resolve_context(&[PrincipalDefinition], &str, u32, &Path) -> io::Result<PrincipalContext>`
- Produces: `PrincipalContext::{id(), runtime_root(), memory_root(), requests_dir(), results_dir(), status_path()}`
- Produces: `current_context() -> io::Result<PrincipalContext>`

- [x] **Step 1: Write failing unit tests**

Add table-driven tests using temporary directories:

```rust
#[test]
fn rejects_claim_when_effective_uid_does_not_match_definition() {
    let definitions = vec![definition("resident", 101)];
    let error = resolve_context(
        &definitions,
        "resident",
        100,
        Path::new("/runtime"),
    )
    .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
}

#[test]
fn derives_paths_below_the_validated_principal_root() {
    let definitions = vec![definition("resident", 101)];
    let context = resolve_context(
        &definitions,
        "resident",
        101,
        Path::new("/runtime"),
    )
    .unwrap();
    assert_eq!(context.memory_root(), Path::new("/runtime/resident/memory"));
    assert_eq!(context.requests_dir(), Path::new("/runtime/resident/requests"));
    assert_eq!(context.results_dir(), Path::new("/runtime/resident/results"));
}

#[test]
fn duplicate_principal_ids_are_rejected() {
    let directory = fixture_directory(&[
        ("a.principal", "id=resident\nuser=a\nuid=101\nenabled=1\n"),
        ("b.principal", "id=resident\nuser=b\nuid=102\nenabled=1\n"),
    ]);
    assert_eq!(
        load_definitions_from(directory.path()).unwrap_err().kind(),
        io::ErrorKind::InvalidData
    );
}
```

- [x] **Step 2: Synchronize and verify RED remotely**

Run:

```bash
rsync -a --delete --exclude target ./ aliyun:/opt/boos-build/resident-principal/
ssh aliyun 'cd /opt/boos-build/resident-principal/src/rust && cargo test --locked principal::tests'
```

Expected: compilation fails because `principal` and its interfaces do not
exist.

- [x] **Step 3: Implement the principal module**

Implement strict key-value parsing, duplicate detection, ID validation using
the existing runtime-ID character contract, `/proc/self/status` effective UID
parsing, and path derivation. `current_context` reads
`BOOS_PRINCIPAL_ID`, accepts `BOOS_AGENT_ID` only as a fallback, loads
`config::PRINCIPAL_CONFIG_DIR`, and uses `config::PRINCIPAL_RUNTIME_DIR`.

- [x] **Step 4: Verify GREEN remotely**

Run the same focused test, then:

```bash
ssh aliyun 'cd /opt/boos-build/resident-principal/src/rust && cargo test --locked principal::tests config::'
```

Expected: all selected tests pass.

- [x] **Step 5: Commit**

```bash
git add src/rust/src/principal.rs src/rust/src/main.rs src/rust/src/config.rs
git commit -m "feat: add uid-backed principal identity"
```

### Task 2: Principal-scoped memory

**Files:**
- Modify: `src/rust/src/memory_namespace.rs`
- Modify: `src/rust/src/memory.rs`

**Interfaces:**
- Consumes: `principal::current_context() -> io::Result<PrincipalContext>`
- Produces: `MemoryNamespace::from_context(&PrincipalContext) -> Self`
- Preserves: `MemoryNamespace::new(&Path, Option<&str>)` for focused legacy tests only

- [x] **Step 1: Write failing behavior tests**

Add tests proving that two valid principal contexts produce disjoint working,
recent, and archive paths, and that `from_environment` no longer silently
returns `/var/boos/memory` without an authenticated context.

```rust
#[test]
fn principal_contexts_never_share_memory_tiers() {
    let resident = context("resident", "/runtime/resident");
    let debug = context("debug", "/runtime/debug");
    let resident_memory = MemoryNamespace::from_context(&resident);
    let debug_memory = MemoryNamespace::from_context(&debug);
    assert_ne!(resident_memory.working_path(), debug_memory.working_path());
    assert_eq!(
        resident_memory.archive_dir(),
        Path::new("/runtime/resident/memory/archive")
    );
}
```

The production mutation this catches is reintroducing a global memory root.

- [x] **Step 2: Verify RED remotely**

Synchronize the worktree and run:

```bash
ssh aliyun 'cd /opt/boos-build/resident-principal/src/rust && cargo test --locked memory_namespace::tests memory::tests'
```

Expected: failure because `from_context` is absent and memory still derives a
global root.

- [x] **Step 3: Route memory through the principal context**

Make `MemoryNamespace::from_environment` call `principal::current_context`.
Keep tier-specific persistence behavior unchanged. Do not copy old global
memory.

- [x] **Step 4: Verify GREEN remotely**

Run the focused memory tests and Task 1 principal tests. Expected: all pass.

- [x] **Step 5: Commit**

```bash
git add src/rust/src/memory_namespace.rs src/rust/src/memory.rs
git commit -m "feat: scope memory to authenticated principals"
```

### Task 3: Principal-scoped request and result spools

**Files:**
- Modify: `src/rust/src/request_publish.rs`
- Modify: `src/rust/src/queue_record.rs`
- Modify: `src/rust/src/submit.rs`
- Modify: `src/rust/src/process.rs`
- Modify: `src/rust/src/exec.rs`

**Interfaces:**
- Consumes: `principal::current_context()`
- Consumes: `principal::load_definitions()`
- Produces: `process_principal_queue(&PrincipalContext) -> io::Result<u32>`
- Changes: `QueuedRequest` carries no authoritative requester identity
- Changes: results and audit commands use the current principal result path

- [x] **Step 1: Write failing queue ownership tests**

Construct a request body containing `requester=forged` under a resident spool.
Assert that processing metadata records `principal=resident` and never treats
`forged` as authority. Add a second test proving resident and debug requests
with the same filename publish into different result directories.

```rust
#[test]
fn spool_principal_overrides_untrusted_requester_field() {
    let request = load_request(&fixture_request(
        "id=req-1\nrequester=forged\ncommand=help\nstatus=pending\n",
    ))
    .unwrap();
    assert_eq!(request.claimed_requester.as_deref(), Some("forged"));
    let owned = request.with_principal(PrincipalId::parse("resident").unwrap());
    assert_eq!(owned.principal.as_str(), "resident");
}
```

The production mutation this catches is authorizing from serialized request
content.

- [x] **Step 2: Verify RED remotely**

Synchronize and run:

```bash
ssh aliyun 'cd /opt/boos-build/resident-principal/src/rust && cargo test --locked queue_record::tests request_publish::tests process::tests submit::tests'
```

Expected: failure because principal-owned spools and owned request records do
not exist.

- [x] **Step 3: Implement per-principal spools**

Submit into `current_context().requests_dir()`. Refactor the processor so its
public entry enumerates enabled definitions and a focused function processes
one principal directory. Publish `principal=<id>` in result metadata.
Result-list, result-show, prune, and audit readers resolve only the caller's
result directory. Retain `requester` only as non-authoritative trace text.

- [x] **Step 4: Verify GREEN remotely**

Run the focused tests, then the complete Rust test suite remotely:

```bash
ssh aliyun 'cd /opt/boos-build/resident-principal/src/rust && cargo test --locked --all-targets'
```

Expected: all tests pass.

- [x] **Step 5: Commit**

```bash
git add src/rust/src/request_publish.rs src/rust/src/queue_record.rs \
  src/rust/src/submit.rs src/rust/src/process.rs src/rust/src/exec.rs
git commit -m "feat: isolate request results by principal"
```

### Task 4: Resident lifecycle without gateway ownership

**Files:**
- Create: `src/rust/src/resident_agent.rs`
- Modify: `src/rust/src/agent.rs`
- Modify: `src/rust/src/main.rs`

**Interfaces:**
- Consumes: `principal::current_context()`
- Produces: `resident_agent::run() -> i32`
- Produces: `resident_agent::write_status(&PrincipalContext, ResidentState) -> io::Result<()>`
- Changes: no-argument `boos-agent` and `boos-agent resident` enter resident mode

- [x] **Step 1: Write failing lifecycle tests**

Use a temporary principal runtime root and assert:

- ready status contains the principal, process ID, state, and start time;
- status publication uses a temporary file and rename;
- an invalid principal context returns an error before readiness;
- resident lifecycle has no gateway child or port setting in its API.

```rust
#[test]
fn ready_status_identifies_the_resident_principal() {
    let context = context("resident", temporary_root.path());
    write_status(&context, ResidentState::ready(42, 100)).unwrap();
    assert_eq!(
        fs::read_to_string(context.status_path()).unwrap(),
        "principal=resident\nstate=ready\npid=42\nstarted_at=100\n"
    );
}
```

The production mutation this catches is publishing readiness for the wrong
principal or before identity resolution.

- [x] **Step 2: Verify RED remotely**

Synchronize and run:

```bash
ssh aliyun 'cd /opt/boos-build/resident-principal/src/rust && cargo test --locked resident_agent::tests'
```

Expected: compilation fails because the lifecycle module is absent.

- [x] **Step 3: Implement and wire resident mode**

Move the default daemon responsibility out of `agent.rs`. Delete its gateway
spawn/restart loop. Resident mode resolves context, prepares its memory
session, publishes ready status, logs `resident_ready`, and refreshes a
heartbeat at a bounded interval. Existing explicit `loop`, `develop`,
`explore`, and memory subcommands remain experimental CLI modes.

- [x] **Step 4: Verify GREEN remotely**

Run resident tests and all Rust tests remotely. Expected: all pass with no
gateway process spawned by agent tests.

- [x] **Step 5: Commit**

```bash
git add src/rust/src/resident_agent.rs src/rust/src/agent.rs src/rust/src/main.rs
git commit -m "feat: add resident principal lifecycle"
```

### Task 5: Product boot boundary and CI-only debug adapter

**Files:**
- Create: `rootfs/etc/boos/principals/resident.principal`
- Create: `rootfs/etc/boos/principals/debug.principal`
- Create: `rootfs/etc/boos/daemons/agent.daemon`
- Modify: `rootfs/etc/boos/daemons/gateway.daemon`
- Modify: `rootfs/init`
- Modify: `.github/workflows/ci.yml`
- Modify: `scripts/ci-test.sh`
- Modify: `tests/boot/verify-initramfs.sh`

**Interfaces:**
- Product rootfs: agent enabled, gateway disabled
- CI overlay: gateway enabled with `BOOS_PRINCIPAL_ID=debug`
- QEMU log contract: `resident_ready principal=resident`

- [x] **Step 1: Write failing boot verification**

Extend `verify-initramfs.sh` to inspect the extracted artifact behaviorally:
product daemon configuration must contain one enabled resident agent and no
enabled gateway. Extend `ci-test.sh` to require a `resident_ready` boot marker
before testing the CI-only gateway and to verify the resident daemon remains
running.

- [x] **Step 2: Verify RED without compiling locally**

Synchronize the repository and run the shell verifier on the existing
initramfs assembly inputs on `aliyun`. Expected: failure because the resident
principal config and agent daemon do not exist and gateway is enabled.

- [x] **Step 3: Implement rootfs ownership and daemon configuration**

Create principal runtime directories in init, owned by their configured Linux
users with mode `0700`. Configure `agent.daemon` with:

```text
name=agent
exec=/bin/boos-agent resident
user=boos-agent
principal=resident
restart=always
enabled=1
```

Extend supervisor daemon parsing to pass `BOOS_PRINCIPAL_ID` from the
`principal` field. Set product `gateway.daemon` to `enabled=0` and
`principal=debug`. In the CI overlay, change only the extracted gateway daemon
to `enabled=1` before repacking.

- [x] **Step 4: Verify GREEN remotely**

On `aliyun`, run all Rust tests and rootfs shell verifiers. Push the completed
candidate once and use GitHub Actions for the real musl/QEMU boot check.
Expected:

- Rust tests pass;
- initramfs provenance passes;
- product config asserts resident-on/gateway-off;
- QEMU observes resident readiness;
- authenticated debug gateway checks pass;
- resident daemon remains running.

- [x] **Step 5: Commit**

```bash
git add rootfs/etc/boos/principals rootfs/etc/boos/daemons \
  rootfs/init src/rust/src/supervisor.rs .github/workflows/ci.yml \
  scripts/ci-test.sh tests/boot/verify-initramfs.sh
git commit -m "feat: boot resident principal by default"
```

### Task 6: Remove misleading runtime artifacts and align current docs

**Files:**
- Delete: `src/rust/src/rust/Cargo.lock`
- Delete: `src/rust/src/rust/src/agent_develop.rs`
- Delete: `rootfs/init.orig`
- Delete: `rootfs/init.rej`
- Delete: `rootfs/bin/boos-daemon`
- Delete: `rootfs/etc/boos/daemons/processor.daemon`
- Modify: `README.md`
- Modify: `SEED.md`
- Modify: `docs/PROJECT-OVERVIEW.md`

**Interfaces:**
- Documentation describes implemented boundaries only.
- Deprecated files no longer appear in the shipped rootfs.

- [x] **Step 1: Verify each artifact has no live caller**

Use `rg` over source, rootfs assembly, CI, and tests. Remove or update stale
tests that target the deprecated shell daemon rather than preserving dead
code for those tests.

- [x] **Step 2: Delete dead artifacts and update documentation**

Document:

- resident principal is the default boot workload;
- gateway is an optional debug adapter;
- principal-scoped state exists;
- skill sharing remains Phase 2 and is not claimed as implemented;
- semantic objects remain an experiment rather than proof of AI-native
  operation.

- [x] **Step 3: Run complete remote verification**

Run:

```bash
rsync -a --delete --exclude target ./ aliyun:/opt/boos-build/resident-principal/
ssh aliyun 'cd /opt/boos-build/resident-principal/src/rust && cargo test --locked --all-targets'
ssh aliyun 'cd /opt/boos-build/resident-principal && bash tests/evidence/test-all.sh'
```

Expected: all selected checks pass. Do not claim QEMU verification until the
single release-candidate GitHub Actions run completes.

- [x] **Step 4: Commit**

```bash
git add -A README.md SEED.md docs/PROJECT-OVERVIEW.md rootfs src/rust/src/rust tests
git commit -m "chore: remove obsolete gateway-era artifacts"
```

### Task 7: Final architecture verification and handoff

**Files:**
- Modify if required by observed failures: only files already listed above
- Update: `docs/superpowers/plans/2026-07-30-boos-resident-principal-boundary.md`

**Interfaces:**
- Produces a release candidate with a traceable remote test record.

- [x] **Step 1: Review dependency direction**

Confirm `principal` depends only on config/std; memory, queue, and resident
flows depend on principal; gateway and CLI wiring depend on those flows.
Confirm no principal code imports gateway, agent loop, or command UI modules.

- [x] **Step 2: Review failure boundaries**

Verify malformed config, duplicate IDs/UIDs, UID mismatch, missing directories,
queue publication failure, status publication failure, and daemon restart are
observable and fail closed.

- [x] **Step 3: Push one release candidate and inspect CI**

Push `feat/boos-resident-principal` and inspect its single GitHub Actions run.
Do not repeatedly mutate tests to chase CI without reproducing the failure on
the remote build host.

- [x] **Step 4: Mark completed plan checkboxes and commit evidence-only updates**

Record exact commands and results without pasting full logs. Commit only if the
plan status changed.
