# BoOS Living Evidence System Design

**Status:** Approved direction; pending written-spec review
**Date:** 2026-07-30
**Scope:** Evidence construction, benchmark lifecycle, and claim discipline

## 1. Decision

BoOS will not use one fixed benchmark as continuing proof of correctness or
architectural value.

Once a test is visible and development responds to it, the test becomes a
regression check. It still protects known behavior, but its future success is
expected and no longer provides independent evidence that BoOS handles the
larger problem.

BoOS will maintain a living evidence system with three distinct tracks:

1. **Regression** — stable, public tests for known contracts and failures;
2. **Frontier** — sealed, rotating cases used for fresh directional evidence;
3. **Field** — real workflows and incidents used to discover what the
   benchmark still fails to represent.

A small calibration anchor set connects benchmark versions. Anchors preserve
comparability; they do not carry a research conclusion by themselves.

Goodhart pressure applies at three levels:

- a concrete case becomes a regression target after exposure;
- a task generator or problem family becomes predictable after repeated
  optimization;
- a metric becomes a poor proxy when the system learns to improve it without
  improving the underlying field outcome.

Rotating only case seeds is therefore insufficient.

## 2. The Error This Prevents

Tests need expected outputs. The error is not expectation; it is constructing
the problem from the implementation's assumptions and strengths, then
generalizing success beyond the test.

The original semantic-object pair did this:

- the prompt named `world schema`, `world list`, and `world show`;
- tasks requested metadata the object layer was built to expose;
- the environment remained static;
- no task required uncertain operation, stale information, or recovery;
- passing was expanded into evidence about the interface direction.

That pair remains useful as **Test 0: a protocol and wiring probe**. It does not
show that the semantic layer improves general AI operation.

The rejected system-boundary design repeated the same mistake at a higher
level. It selected sharing, isolation, and recovery because they matched the
proposed BoOS boundary. Symmetric result labels could not repair biased task
construction.

## 3. Epistemic Contract

Every test and every claim must obey these rules:

1. A test states what observable outcome counts as passing.
2. A research task originates from an external goal or failure, not from a
   feature inventory.
3. Necessary assumptions are explicit, versioned, and no broader than the
   task requires.
4. Passing a known test establishes only that known property under recorded
   conditions.
5. A suite must demonstrate detection power against realistic broken
   implementations before its passing result is interpreted.
6. Unknown, exposed, and implementation-tuned cases are never mixed without
   labels.
7. Raw failures remain part of the evidence even after the program learns to
   pass them.
8. No finite suite supports the statement “the program has no problems.”
9. Every conclusion names its tested population, conditions, outcomes, and
   unresolved risks.
10. Benchmark performance never overrides contradictory field evidence.

## 4. Evidence Tracks

### 4.1 Regression track

Regression tests are public and stable. They include:

- deterministic unit and integration contracts;
- every reproduced defect;
- every exposed frontier case that remains relevant;
- security and safety boundaries;
- compatibility behavior;
- intentionally broken fixtures used to qualify evaluators.

Regression success is necessary for a release candidate. It means known
behavior has not regressed. It does not demonstrate generality, novelty, or
real-world sufficiency.

Regression failures block a corresponding bounded claim. Regression successes
cannot create a new architectural claim.

### 4.2 Frontier track

Frontier cases are unexposed cases that have not been used to design, debug, or
tune the implementation under evaluation.

They are used to ask whether behavior transfers beyond known examples. A
frontier batch:

- is committed by cryptographic digest before the run;
- is stored outside model-readable project context until execution;
- contains tasks from more than one problem family;
- includes cases where a proposed feature may help, hurt, or be irrelevant;
- supplies no implementation-specific solution path;
- is revealed and archived after scoring;
- is retired from frontier status after exposure.

The same concrete case cannot provide frontier evidence twice for a system that
has already seen it.

### 4.3 Field track

Field evidence comes from real project work rather than a synthetic evaluator.
It records:

- the user's goal;
- available context and tools;
- what the AI attempted;
- observed failure or friction;
- consequence to correctness, safety, time, or human effort;
- human workaround;
- raw trace or reproducible evidence when available;
- uncertainty about the cause.

Field records do not assume BoOS is the solution. They may show that the real
problem belongs in an editor, Git workflow, agent runtime, model, social
process, or nowhere worth automating.

Field evidence has high external validity but weak control. It generates
questions and candidate tasks; it does not by itself isolate a causal
mechanism.

### 4.4 Calibration anchors

A small public anchor set is run across benchmark versions to detect gross
environment or model drift.

Anchors are deliberately excluded from fresh-evidence counts. They answer
whether two runs remain roughly comparable, not whether a new system idea
works.

If anchor behavior changes materially, cross-version comparisons are marked
incomparable until the cause is understood.

## 5. Evidence Lifecycle

Evidence moves in one direction:

```text
field observation
    -> normalized incident
    -> candidate problem family
    -> independent task construction
    -> sealed frontier case
    -> registered evaluation
    -> revealed result
    -> regression case or archive
    -> new field observation
```

### 5.1 Incident normalization

An incident record removes solution names and preserves:

- desired outcome;
- starting conditions;
- observable failure;
- consequence;
- workaround;
- which facts are observed and which are inferred.

For example, “BoOS needs shared skill namespaces” is not an incident. “Two
simultaneous AI project sessions required repeated manual skill copying, while
some project-specific instructions had to remain isolated” is an incident.

### 5.2 Candidate problem families

Incidents are grouped only by externally visible goal and failure shape. A
problem family must remain meaningful if BoOS does not exist.

A candidate is rejected when:

- its name or success condition requires a proposed BoOS primitive;
- only one implementation strategy can satisfy it by definition;
- it restates a feature specification as a user goal;
- its consequence is hypothetical and has no independent rationale;
- it is already fully exposed to the implementation.

### 5.3 Independent task construction

Task construction begins after the problem family is accepted. The author or
generator receives the normalized incident and outcome constraints, not the
feature implementation.

Each family must sample:

- ordinary cases;
- boundaries and malformed inputs;
- conflicting or incomplete context;
- environmental change;
- cases where no new mechanism is needed;
- cases where the proposed mechanism adds cost or failure modes.

Tasks can be synthetic when necessary for control, but their provenance and
distance from a real incident are recorded.

## 6. Contamination and Retirement

Contamination is tracked at case, family, and metric level.

### 6.1 Case contamination

A frontier case is case-contaminated when any of the following occurs:

- its prompt, fixture, expected output, or scoring rule becomes visible to the
  implementation or model context;
- a developer uses it to choose or revise behavior;
- it appears in documentation, a trace, an issue, or a public benchmark;
- its result is inspected before implementation changes finish;
- a generated case is close enough to an exposed case that the solution path
  is effectively the same.

Case contamination is not misconduct. It is the normal lifecycle of a useful
test. The case is immediately labeled and moved to regression or archive.

Deleting, silently editing, or keeping a contaminated case in the frontier
score is prohibited.

### 6.2 Family contamination

A problem family or generator is family-contaminated when implementation work
explicitly targets its recurring structure, shortcuts, vocabulary, generated
distribution, or evaluator weaknesses. Drawing a new random seed from that
generator does not restore independence.

A contaminated family can remain valuable regression coverage. It cannot
supply fresh frontier evidence until a new external incident, independently
constructed family, or materially different hidden generator creates a new
registered distribution version.

### 6.3 Metric contamination

A metric is contaminated when improvement no longer tracks the underlying
field outcome. Signals include:

- better benchmark scores alongside unchanged or worse field failures;
- behavior that satisfies the measured proxy while violating the user goal;
- systematic exploitation of evaluator blind spots;
- a narrowed task policy that avoids difficult but relevant work.

Metric contamination triggers claim review and a new evaluator or distribution
version. It is not repaired by adding more cases that use the same proxy.

### 6.4 Historical validity

Later contamination does not retroactively alter a registered result. The
result remains evidence about the exact implementation, model, environment,
case distribution, and evaluator that produced it.

It cannot be reused as independent evidence for a later implementation that
was optimized using the exposed benchmark.

## 7. Rotation Without Cherry-Picking

Continuous benchmark change introduces its own risk: changing the benchmark
until a preferred result appears.

Rotation is controlled as follows:

1. The frontier generation method, problem-family weights, evaluator, and
   analysis are frozen in a registered manifest before cases are drawn.
   Generator details capable of revealing solution structure remain sealed
   until the batch is exposed.
2. Case bundles receive a digest before execution and are revealed after
   per-run results are immutable.
3. A used batch expires regardless of whether the result is favorable.
4. Failures cannot be removed from published batch results.
5. Correcting an evaluator after results creates a new benchmark version; the
   old result remains published as invalid or limited.
6. Family weights change only from new field evidence or a documented coverage
   audit, never from a desired score.
7. Raw scores from different major benchmark versions are not directly
   compared.
8. Cross-version claims use anchors and report the uncertainty introduced by
   distribution change.

The system therefore rotates cases while preserving rules, provenance, and
historical failures.

## 8. Benchmark Versioning

Every benchmark has:

- a schema version;
- a problem-distribution version;
- an evaluator version;
- a case-batch identifier;
- an implementation commit;
- model and environment identities;
- an exposure status.

Version changes follow these rules:

- changing only sealed cases within frozen family weights creates a new batch;
- changing problem-family definitions or weights increments the distribution
  major version;
- changing pass/fail semantics increments the evaluator major version;
- correcting a pre-run formatting error may increment a minor version;
- changing anything after result inspection creates a new registered run.

Published records are append-only.

## 9. Detection Power

A benchmark must prove it can expose relevant failures.

Before a frontier family is trusted, its evaluator runs against mutations or
broken fixtures representing:

- missing behavior;
- stale state;
- incorrect authority or isolation;
- partial writes and interrupted transitions;
- duplicated effects;
- false success reporting;
- corrupted or malformed input;
- valid behavior through an unanticipated implementation path.

The last case prevents an evaluator from rejecting correct alternatives merely
because they differ from the expected implementation.

Mutation survival is reported. A surviving meaningful mutation is a benchmark
gap, not a product success.

## 10. Evaluation and Claims

### 10.1 Outcome vector

Results remain multidimensional:

- task completion and output correctness;
- unsafe or unauthorized effects;
- recovery and state consistency;
- human intervention;
- resource, latency, interaction, and context cost;
- unsupported assumptions;
- unresolved or unobservable behavior.

A single aggregate score may be supplied for a preregistered decision, but raw
primary outcomes remain authoritative. A favorable average cannot hide a
safety failure.

### 10.2 Claim register

Every nontrivial claim has a versioned record containing:

- exact claim text;
- scope and excluded scope;
- target problem distribution;
- required regression, frontier, and field evidence;
- primary outcomes and decision rule;
- known counterevidence;
- benchmark and implementation versions;
- status and expiry condition.

Claim statuses are:

- unsupported;
- exploratory;
- supported within recorded scope;
- contradicted;
- stale.

A historical claim remains attached to the exact tested version. It becomes
stale when applied to a later implementation optimized against that evidence,
when the target environment changes materially, or when new field evidence
falls outside the tested distribution.

### 10.3 Evidence limits

Passing regression tests supports compatibility with known cases.

Passing one frontier batch supports transfer to that registered sample.

Repeated frontier batches across problem families and models increase
confidence but do not establish universal correctness.

Field evidence tests whether the benchmark still tracks reality. Persistent
field failures can invalidate a benchmark-backed claim even when the suite
passes.

## 11. Storage and Sealing

Public repository contents include:

- schemas and validators;
- incident records safe to publish;
- claim records;
- regression tests;
- retired frontier cases;
- registered manifests;
- encrypted or external-bundle digests;
- raw revealed results and archives.

Unexposed frontier plaintext is never stored in the model-readable working
tree. A registration manifest commits its digest, generation version, family
weights, budgets, and evaluator identity before execution. After evaluation,
the plaintext bundle is archived with the digest so the commitment can be
verified.

Secrets, personal data, and unrelated project content are removed during
incident normalization. Redaction cannot remove facts necessary to understand
the failure; such an incident remains private instead.

## 12. Architecture

The evidence system belongs under `tests/evidence/` and follows:

```text
L3 capture, registration, run, reveal, and archive commands
                    |
L2 evidence lifecycle and benchmark orchestration
                    |
L1 incident normalization, generation, mutation, and evaluation
                    |
L0 versioned incidents, claims, manifests, results, and errors
```

### L0 — contracts

Defines small, versioned, machine-validated records. It has no dependency on
BoOS runtime code or a model provider.

### L1 — evidence building blocks

Validates records, constructs cases from accepted problem families, evaluates
external outcomes, detects duplicates, and runs mutation fixtures.

### L2 — lifecycle

Owns contamination state, frontier eligibility, version transitions,
registration, blinding, rotation, and append-only archives.

### L3 — diplomacy

Connects field capture, local runners, model providers, environments, and
report publication to the evidence contracts.

Dependency direction is `L3 -> L2 -> L1 -> L0`. Product modules cannot import
benchmark internals.

## 13. Initial Implementation Slice

The first slice builds the evidence discipline, not a new BoOS feature:

1. relabel the semantic-object experiment as Test 0 and limit its claim to
   protocol/wiring behavior;
2. add incident, claim, registration, contamination, and result schemas;
3. add validators and intentionally invalid fixtures;
4. record the first parallel-project context/skill incident without naming a
   solution;
5. create a public regression index and a frontier metadata index;
6. add a claim record stating that the need for an OS boundary remains
   unsupported;
7. do not implement skill sharing, semantic filtering, or another comparative
   benchmark yet.

This slice creates no model runner and calls no paid API.

## 14. Failure Semantics

- Missing required provenance makes a record invalid.
- Missing frontier plaintext after registration makes a run inconclusive and
  preserves the registration failure.
- Product or model failures remain results, not infrastructure exclusions.
- Digest mismatch invalidates the affected bundle and is never retried under
  the same run ID.
- A contaminated frontier case contributes zero fresh-evidence count but may
  remain a regression result.
- An evaluator that fails mutation qualification cannot score frontier runs.
- A field incident with uncertain cause remains valid when facts and
  inferences are separated.

## 15. Non-Goals

The first slice does not:

- build a universal benchmark;
- claim statistical sufficiency from one batch;
- replace normal unit or integration tests;
- automate feature selection;
- use an LLM as the primary correctness oracle;
- keep benchmark scores comparable by pretending the task distribution never
  changed;
- rotate tests merely because the system failed them;
- optimize BoOS to the first captured incident.

## 16. Acceptance Criteria

The design is ready for implementation planning when:

1. regression, frontier, field, and anchor evidence have non-overlapping roles;
2. a task derived from a feature specification is ineligible for frontier
   evidence;
3. exposure automatically retires a frontier case;
4. benchmark rotation is versioned and cannot delete unfavorable results;
5. claims cannot exceed their registered problem distribution;
6. evaluators must reject broken implementations before scoring real ones;
7. unresolved mutations and contradictory field evidence remain visible;
8. Test 0 is preserved but cannot support a general semantic-interface claim;
9. the first slice changes evidence infrastructure only, not BoOS behavior;
10. fresh seeds from a contaminated family cannot be mislabeled as independent
    frontier evidence;
11. later contamination limits reuse without rewriting the historical result.
