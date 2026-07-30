# BoOS — AI-Owned Operating System Substrate

> AI is the subject, not the object.

BoOS is an experimental Linux control layer whose first operating identity is
an AI principal rather than a remote human client. It does not replace the
Linux kernel or compete with agent planners. It explores the lower boundary
that planners currently inherit from human-oriented systems: identity, owned
state, capability enforcement, durable requests, results, and eventually
shareable skills.

## What exists now

The product image boots a resident principal without waiting for a network
client:

```text
/init
  └─ boos-supervisor
      ├─ boos-agent resident        principal=resident, UID=101
      └─ built-in queue processor

/var/boos/principals/
  ├─ resident/
  │   ├─ status.kv
  │   ├─ memory/{working.kv,recent/,archive/}
  │   ├─ requests/
  │   └─ results/
  └─ debug/                         separate UID and state
```

The resident process currently establishes an authenticated runtime slot,
session, ready state, and heartbeat. It is not yet an LLM planner and
`state=ready` does not claim that a model has reasoned or completed work.

`boos-gateway` remains available as an optional debug adapter, but is disabled
in the product rootfs. CI enables it only through a disposable overlay and
binds it to the isolated `debug` principal.

## Why this boundary

Agent runtimes already plan, call tools, and coordinate work. BoOS is testing a
different question:

> If an AI were the native user of a system, what should the system own beneath
> any particular planner?

The first answer is deliberately small:

- an identity anchored in Linux UID/GID, not a caller-provided label;
- private memory, request, and result namespaces;
- local boot and progress independent of remote connectivity;
- stable capability and semantic-object interfaces;
- explicit adapters instead of making a TCP gateway the system core.

These are implemented boundaries, not evidence that BoOS is a superior OS.
Claims about necessity or advantage require experiments in the
[Living Evidence System](tests/evidence/README.md).

## Build and verify

BoOS builds one static Rust multicall binary and packages it with BusyBox in an
initramfs. See [SEED.md](SEED.md) for the complete runtime map.

```bash
cd src/rust
cargo test
cargo build --release --target x86_64-unknown-linux-musl

cd ../..
commit=$(git rev-parse HEAD)
source_date_epoch=$(git show -s --format=%ct "$commit")
scripts/assemble-initramfs.sh \
  /path/to/matching/initramfs-virt \
  rootfs \
  src/rust/target/x86_64-unknown-linux-musl/release/boos \
  build/initramfs.cpio.gz \
  "$commit" \
  "$source_date_epoch"
tests/boot/verify-initramfs.sh \
  build/initramfs.cpio.gz \
  rootfs/init \
  src/rust/target/x86_64-unknown-linux-musl/release/boos \
  /path/to/matching/vmlinuz-virt \
  "$commit"
```

Run the resulting image with the repository's matching kernel and persistent
disk:

```bash
qemu-system-x86_64 \
  -kernel build/vmlinuz \
  -initrd build/initramfs.cpio.gz \
  -append "console=ttyS0 rdinit=/init" \
  -drive file=build/var.img,format=raw,if=virtio,cache=directsync \
  -nographic -no-reboot
```

The explicit `boos-agent loop`, `develop`, and `explore` modes are retained as
historical experiments. The default `boos-agent` mode is `resident`; it does
not start a gateway or call a model provider.

## Runtime architecture

The `boos` binary dispatches by `argv[0]`:

| Entry point | Responsibility |
|---|---|
| `boos-supervisor` | Start configured workloads, enforce restart policy, process queues |
| `boos-agent` | Resident lifecycle and explicit experimental agent modes |
| `boos-exec` | Capability-checked command execution |
| `boos-submit` | Publish a request into the current principal's spool |
| `boos-process` | Process enabled principals' request spools |
| `boos-shell` | Local interactive adapter |
| `boos-gateway` | Optional authenticated TCP debug/model adapter |

Principal definitions live in `/etc/boos/principals/*.principal`.
`BOOS_PRINCIPAL_ID` selects a definition only when the effective UID matches
its configured UID. A request's `requester` field is trace metadata; the spool
owner determines authorization and result ownership.

The gateway fails closed for remote access. Without
`/etc/boos/gateway_token` or `BOOS_GATEWAY_TOKEN`, it binds only to loopback.
With a token, remote clients must authenticate. Its protocol is plain TCP, not
TLS, so external use still requires a trusted private network or encrypted
tunnel. `FETCH` is disabled unless an exact HTTPS hostname allowlist is
configured.

## Research slices

### Semantic object layer

`world schema`, `world list`, and `world show` project the command registry into
stable `system` and `capability` objects. This is **Test 0: Interface and Wiring
Probe** only. It verifies protocol shape and wiring, not usefulness,
sufficiency, or superiority.

- [Design](docs/superpowers/specs/2026-07-30-boos-semantic-object-layer-design.md)
- [Experiment](tests/research/semantic-object-view/README.md)

### Principal boundary

The resident-principal phase makes ownership real before BoOS attempts
multi-AI sharing.

- [Design](docs/superpowers/specs/2026-07-30-boos-resident-principal-boundary-design.md)
- [Implementation plan](docs/superpowers/plans/2026-07-30-boos-resident-principal-boundary.md)

## Roadmap

1. **Resident principal boundary** — UID-backed identity, private state and
   queues, resident boot, optional debug adapter. Implemented.
2. **Skill views** — immutable skill versions, private overlays, explicitly
   mounted shared collections, provenance, and task-pinned snapshots.
3. **Subscriptions and coordination** — opt-in hot updates, rollback,
   revocation, events, and concurrency-safe publication.
4. **Research validation** — changing benchmarks based on real cross-project
   incidents, including cases where sharing helps and where isolation is
   required.

Skill sharing and multi-AI scheduling are not implemented yet.

## Historical experiments

The v1–v4 logs record how earlier external-gateway and in-system DeepSeek
experiments exposed missing commands, memory, and auditability. They are
project history, not the current product architecture.

- [v1 external exploration](docs/v1-deepseek-exploration.md)
- [v2 full log](docs/v2-full-log.txt)
- [v2 report](docs/v2-deepseek-report.txt)
- [v3 full log](docs/v3-full-log.txt)

## Design constraints

- Linux remains the kernel and process-isolation substrate.
- Configuration and wire records use bounded `key=value` formats.
- State publication is atomic.
- Capability checks remain fail-closed.
- Product secrets stay outside the repository.
- Tests distinguish interface regressions from research evidence.
