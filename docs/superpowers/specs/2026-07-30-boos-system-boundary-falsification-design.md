# BoOS System-Boundary Falsification Benchmark

**Status:** Approved direction; pending written-spec review
**Date:** 2026-07-30
**Scope:** Selective skill propagation, isolation, and crash recovery

## 1. Decision

BoOS will not treat the semantic-object experiment as evidence that an
AI-native operating system is necessary. The existing experiment becomes
**Test 0: an interface probe**. It demonstrates that a model can consume the
`world` representation and exposes the cost of an unfiltered object listing.
It does not test an operating-system boundary.

The next benchmark will evaluate a narrower question:

> For selective skill propagation, isolation, and recovery, which boundary—if
> any—produces the safest and most reliable outcome with the least human
> intervention?

The benchmark compares three implementations of one frozen, externally scored
scenario:

1. ordinary Linux/POSIX mechanisms;
2. Linux plus a purpose-built user-space semantic service;
3. BoOS with the same logical semantics enforced at its trusted control
   boundary.

The primary comparison is Linux semantic service versus BoOS. Beating raw
POSIX alone is not evidence that the feature belongs in an operating system.

## 2. Anti-Self-Deception Constitution

The benchmark may define the state that constitutes task completion. It must
not encode which variant is expected to reach that state.

The following rules are part of the benchmark contract:

1. Freeze the scenario, primary metrics, budgets, exclusion rules, evaluator,
   and analysis method before implementing a product response to the scenario.
2. Preregister four symmetric result categories without designating a default:
   BoOS advantage, Linux-service advantage, operational equivalence within
   frozen margins, and insufficient evidence.

3. Use an external deterministic oracle. Model prose, self-reported success,
   and implementation-specific logs cannot decide whether a task passed.
4. Give every variant the same objective, initial logical state, fault
   schedule, model, context budget, time budget, and retry budget.
5. Generate hidden case details only after the implementations and evaluator
   are frozen. Preserve the seed and generated case after each run.
6. Keep the evaluator blind to variant labels until per-run results and trace
   hashes are final.
7. Score safety and final state before efficiency. Lower token or interaction
   counts cannot compensate for leakage, corruption, or an incorrect result.
8. Preserve failed, incomplete, and excluded traces. Exclusion reasons must
   come from the frozen protocol.
9. Publish raw paired results before summaries. Do not replace a failed metric,
   change weights, or add a favorable task after seeing results.
10. Report the category selected by the frozen analysis. Do not describe
    absence of detected difference as equivalence unless the preregistered
    equivalence test passes.

The evaluator must also be tested against intentionally broken states. A
benchmark that cannot reject a leaking, stale, partially committed, or
double-applied implementation is invalid.

## 3. Claim Ladder

The project must keep three claims separate:

| Claim | Evidence required |
|---|---|
| An AI can parse a semantic ABI | Deterministic interface and parser probes |
| A semantic control service improves real task outcomes | Outcome-based comparison against POSIX |
| The capability belongs at an OS boundary | BoOS outperforms a strong user-space service specifically through trusted enforcement, lifecycle, or recovery |

Test 0 addresses only the first claim. This benchmark addresses one instance of
the third. Even a positive result would not establish the sufficiency of BoOS
as a whole; other system-boundary claims require separate benchmarks.

## 4. Real-World Scenario

The first scenario models the observed problem of developing multiple projects
with AI at the same time:

- one project publishes a corrected skill revision;
- one consumer is subscribed to compatible updates;
- one consumer is intentionally isolated;
- the shared consumer must receive and use the new revision without manual
  copying;
- the isolated consumer must remain on its original revision until an explicit
  grant;
- an invalid or unauthorized revision must never become active;
- a coordinator crash during publication must recover to one consistent
  revision without double application;
- every activation or rejection must leave evidence the external oracle can
  inspect.

This is not a test of a `skill publish` command. Each variant may use its native
mechanism. The invariant is the observable state of the three principals and
their downstream work.

### 4.1 Principals

Each generated case contains three opaque principals:

- **publisher** — owns a skill and may publish a new valid revision;
- **shared consumer** — may receive revisions allowed by its subscription and
  compatibility constraints;
- **isolated consumer** — may not observe or activate the new revision before a
  later explicit grant.

Names are generated per case so models cannot memorize paths or commands from
earlier runs.

### 4.2 Revisions

Each case contains:

- an initial valid revision active for both consumers;
- a valid update intended for the shared consumer;
- an invalid update with a deterministic validation failure;
- a later grant that permits the isolated consumer to activate the valid
  update.

The skill performs a small deterministic transformation on a hidden fixture.
The oracle validates the transformed output, so copying a revision label
without actually using the correct content cannot pass.

The valid update also contains a generated isolation canary. The oracle scans
all model-visible environment output, exported state, and downstream artifacts
for that canary. Its appearance in the isolated consumer before the grant is an
unauthorized disclosure even when the revision was never activated.

### 4.3 Fault Schedule

The harness injects one crash after an implementation has recorded publication
intent but before the benchmark observes completed activation. The exact
injection point is selected from a frozen set by the hidden seed.

The frozen fault set uses common lifecycle phases—request received, update
validated, activation prepared, and activation externally visible. Adapters
must expose these phase boundaries to the fault controller through the harness
contract. Those signals control process termination but are not accepted as
outcome evidence. Adapter conformance verifies that each phase represents the
same externally defined transition before registered runs begin.

After restart:

- the shared consumer must have exactly one authorized active revision;
- the isolated consumer must still have the initial revision;
- the invalid update must remain inactive;
- no partial temporary state may be treated as committed;
- replay must not apply the update twice.

The fault controller records only process and external-state events. It does
not interpret implementation-specific journal text.

## 5. Variants

### 5.1 Variant A — POSIX

This variant may use standard files, directories, permissions, symlinks, Git,
and ordinary processes. It receives no dedicated skill registry or semantic
daemon.

Variant A answers whether a competent agent can solve the scenario with common
Linux mechanisms. It is an ergonomic baseline, not the decisive OS comparison.

### 5.2 Variant B — Linux Semantic Service

This is the strongest plausible non-OS alternative, not a deliberately weak
control. It may provide:

- stable skill and revision identities;
- consumer namespaces and subscription state;
- validation and activation operations;
- durable user-space storage and a journal;
- notifications or polling;
- service-managed access checks.

It runs without privileged enforcement beyond the ordinary Unix account that
owns it. The benchmark must not withhold an obvious user-space technique or
grant an extra mechanism merely to favor one variant.

### 5.3 Variant C — BoOS Boundary

BoOS exposes the same logical resources and transitions as Variant B, but
authority, activation, audit receipts, and recovery are owned by the trusted
BoOS control boundary rather than an agent-writable service state.

Variant C may not receive extra task facts, larger budgets, a friendlier
prompt, or a more informative evaluator. Any claimed advantage must be traced
to enforcement, lifecycle, or recovery behavior that Variant B cannot provide
without moving the same responsibility into an equivalent trusted boundary.

## 6. Fairness and Blinding

### 6.1 Frozen task

All variants receive one neutral objective describing desired outcomes:

- publish the valid update;
- make it available to the authorized consumer;
- preserve isolation for the unauthorized consumer;
- complete the consumers' hidden-fixture work;
- recover after interruption;
- apply the later grant.

The prompt does not name expected commands, paths, schemas, or implementation
layers. Variant-specific bootstrap information is limited to how to enter the
environment. It cannot explain how to solve the task.

### 6.2 Hidden case generation

A deterministic generator derives the following from a recorded random seed:

- principal and skill names;
- revision identifiers;
- fixture content and expected output;
- compatibility constraint;
- invalid-update defect;
- crash injection point;
- timing of the explicit grant.

The generator and oracle are frozen before any registered seed is drawn.
Pilot seeds are permanently marked as pilots and cannot enter reported
evidence.

### 6.3 Blind evaluation

During execution, variants use opaque run labels. The oracle receives the case
manifest, final external state, receipts, and downstream outputs but not the
variant identity. Variant labels are revealed only after:

1. the oracle has emitted its result;
2. the trace and result hashes are recorded;
3. any exclusion decision has been made from a preregistered rule.

## 7. Outcome Model

### 7.1 Primary outcomes

Primary outcomes are evaluated in this order:

1. **Isolation safety** — no unauthorized content disclosure or activation;
2. **Final-state correctness** — each consumer has the authorized revision and
   correct downstream output at each checkpoint;
3. **Recovery correctness** — interruption produces neither lost committed
   state, partial activation, nor duplicate application;
4. **Human intervention** — number of out-of-band corrective actions;
5. **Task completion** — the complete scenario finishes within the frozen
   budget.

Any isolation violation is a failed run. A weighted aggregate cannot hide it.
An intervention is an action by the experiment operator that changes variant
state, supplies missing task knowledge, retries an exhausted operation, or
repairs the environment after the model starts. Actions autonomously selected
by the tested model remain ordinary task actions.

### 7.2 Secondary outcomes

After primary outcomes are frozen, the harness records:

- environment interactions;
- model input, cached input, output, and reasoning tokens when available;
- wall-clock duration;
- retries and rejected operations;
- observation bytes;
- recovery time.

These explain cost and friction but cannot reverse a primary failure.

### 7.3 Symmetric interpretation

The registered analysis assigns one of four results:

1. **BoOS advantage** — BoOS is no worse on isolation safety and exceeds the
   frozen superiority margin for end-to-end success or human intervention.
   Traces must identify an enforcement, lifecycle, or recovery mechanism that
   explains the difference.
2. **Linux-service advantage** — Variant B is no worse on isolation safety and
   exceeds the same frozen superiority margin in the other direction.
3. **Operational equivalence** — both directions pass the preregistered
   equivalence tests for every primary outcome. For this scenario and tested
   conditions, the evidence says the OS boundary is unnecessary.
4. **Insufficient evidence** — none of the preceding conditions is met. This
   includes noisy estimates, mixed safety/correctness outcomes, and an
   underpowered run.

No category is a default or expected result. Cross-model consistency and
mechanism traces accompany whichever directional result occurs. Failure to
detect a difference is not silently promoted to equivalence.

## 8. Experimental Phases

### 8.1 Oracle qualification

Before any model run, fixed fixtures prove that the oracle:

- accepts a correct final state;
- rejects an unauthorized disclosure;
- rejects a stale shared consumer;
- rejects premature isolated activation;
- rejects an activated invalid revision;
- rejects partial activation;
- rejects double application after recovery;
- rejects a revision label paired with incorrect skill output.

The intentionally broken adapters are test fixtures, not experimental
variants.

### 8.2 Adapter conformance

Each variant must pass deterministic setup, reset, checkpoint, crash, restart,
and evidence-export checks. Conformance proves that the harness can observe the
variant; it does not prove the scenario succeeds.

All variants must begin registered runs from equivalent logical state. A run is
inconclusive if the harness cannot establish that equivalence before the model
starts.

### 8.3 Pilot

Pilot runs debug the harness, choose budgets large enough to avoid trivial
timeouts, and expose ambiguous instructions. Pilot results are never evidence.
Any protocol change after a pilot increments the protocol version and
invalidates all earlier pilot seeds.

### 8.4 Registered run

The registered-run manifest must freeze exact:

- model providers and versions;
- model count and seeds;
- per-run token, interaction, wall-clock, and retry budgets;
- case seeds and randomized variant order;
- software commits and artifact hashes;
- exclusions and stopping rules;
- pairwise analysis method.

The runner refuses to start a registered run when any field is absent. The
minimum registered design uses three model families and ten paired case seeds
per model. Fewer observations may be published as exploratory data but cannot
support an OS-boundary claim.

## 9. Architecture

The benchmark lives under
`tests/research/selective-skill-propagation/`. Product code does not change
until the oracle and intentionally broken fixtures prove the benchmark can
detect every primary failure.

The default dependency direction is:

```text
L3 variant adapters and CLI
          |
L2 paired runner, fault controller, blind scorer
          |
L1 case generator, state oracle, evidence reader
          |
L0 manifests, events, receipts, invariants, errors
```

### L0 — contracts

Owns versioned case, checkpoint, receipt, and result formats. It has no
dependency on BoOS or a model provider.

### L1 — independent truth

Owns deterministic case generation and outcome evaluation. It reads exported
state through the benchmark evidence contract, never through product-internal
types.

### L2 — experiment flow

Owns fresh-environment setup, randomized order, fault injection, budgets,
trace hashing, blind scoring, exclusion handling, and label reveal.

### L3 — diplomacy

Owns POSIX, Linux-service, and BoOS adapters plus CLI/provider integration. An
adapter translates common lifecycle operations into its environment; it cannot
change the scenario or evaluator.

No reverse dependency from the oracle to an adapter is allowed.

## 10. Evidence Contract

Every variant exports a read-only evidence bundle containing:

- opaque principal IDs;
- skill identity and content digest;
- active revision at each checkpoint;
- authorization decision and reason code;
- activation receipt ID;
- downstream output digest;
- restart generation;
- committed transition sequence number.

The bundle contains facts, not an implementation narrative. The oracle ignores
unknown fields and rejects missing required fields, duplicate receipt IDs,
non-monotonic committed sequence numbers, and digests that do not match the
generated case.

Raw model traces, environment transcripts, fault events, manifests, evidence
bundles, result records, hashes, and the label-reveal map are preserved per
run.

## 11. Failure and Inconclusive Rules

A run fails when the oracle observes an incorrect or unsafe state, the agent
exhausts a frozen budget, or the environment cannot recover within that
budget.

A run is inconclusive only for a frozen infrastructure condition outside the
variant, such as:

- the model provider is unavailable before the first model response;
- the host terminates all variants in the pair;
- the initial-state equivalence check fails before exposure to the model;
- trace storage fails before a complete immutable record exists.

Product crashes, adapter errors, malformed output, and recovery failures are
results, not infrastructure exclusions.

## 12. Documentation Changes

When implementation begins:

- relabel the existing semantic-object experiment as **Test 0: Interface
  Probe**;
- state in the project overview that Test 0 supplies no OS-boundary evidence;
- link the new benchmark without claiming that its hypothesis is true;
- preserve Pair 001 and its original frozen metrics unchanged.

Historical traces and result files must not be rewritten to fit the new
framing.

## 13. Non-Goals

This first system-boundary benchmark does not:

- prove the sufficiency of BoOS as a complete AI-native operating system;
- benchmark general planning quality;
- select a preferred model;
- measure website or aesthetic quality;
- implement a universal skill format;
- add multi-agent messaging unrelated to skill propagation;
- treat a lower token count as evidence of an OS boundary;
- use an LLM as the outcome judge.

## 14. Acceptance Criteria

The design is ready for implementation planning when:

1. the symmetric result categories and strongest baseline are explicit;
2. scenario success is implementation-independent and externally observable;
3. oracle qualification precedes product implementation;
4. hidden generation, blinding, frozen budgets, and raw evidence prevent
   post-result metric changes;
5. safety cannot be averaged away;
6. the decisive comparison is Linux semantic service versus BoOS;
7. equivalence requires frozen margins, while an unresolved comparison remains
   insufficient evidence;
8. Test 0 remains preserved but is no longer presented as architectural
   evidence;
9. the first implementation slice is limited to contracts, generator, oracle,
   and broken-state fixtures before any variant adapter.
