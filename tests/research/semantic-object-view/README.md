# Test 0: Interface and Wiring Probe

This frozen protocol is retained as a regression and wiring probe. Its tasks
and prompts were derived from the semantic-object interface and therefore
cannot support a general claim that the interface improves AI operation.

This directory defines a no-API A/B experiment for the first BoOS research
question:

> Does a stable semantic object view let an AI understand and operate BoOS
> with fewer interactions, fewer false assumptions, and less observation data
> than the existing human-oriented command surface?

This is an experiment protocol, not a claimed result.

## Hypothesis

For the fixed tasks in `tasks.kv`, the semantic object variant should preserve
task completion while reducing environment interactions, observation bytes,
incorrect capability assumptions, invalid commands, and skipped verification.

## Variants

- **baseline** — use `baseline-prompt.txt` against behavioral commit `22675ef`,
  before `world` is exposed as a public command.
- **object** — use `object-prompt.txt` against the candidate commit containing
  `world schema`, `world list`, and `world show`.

The semantic interface is the only intended behavioral difference. Record the
exact candidate commit in every result instead of treating a branch name as
provenance.

## Controlled Variables

Keep these identical within every paired run:

- model provider, model name and version;
- sampling parameters, system instructions and context budget;
- task file and task order;
- initial filesystem, capability flags and process state;
- time, interaction and token limits;
- scoring rules and evaluator.

Use a fresh BoOS instance and fresh model context for each run. Randomize which
variant runs first, and use multiple paired runs before drawing a conclusion.

## Procedure

1. Build the baseline and object images from their recorded commits.
2. Confirm that the baseline has no public `world` command and the object image
   does. Do not change any other capability.
3. Give the model the matching prompt and `tasks.kv`.
4. Record every command, stdout, stderr, conclusion and verification in an
   immutable trace file.
5. Score the trace using the definitions below. Copy `result.example.kv` to a
   new result file and replace every example value.
6. Compute the trace SHA-256 and store it in `trace_sha256`.
7. Run `./validate-result.sh <result.kv>` before comparing variants.

## Metric Definitions

- `completed_tasks`: tasks answered correctly with trace evidence.
- `environment_interactions`: command round trips sent to BoOS.
- `observation_bytes`: raw stdout and stderr bytes returned by BoOS.
- `incorrect_capability_assumptions`: unsupported claims about capability
  existence, state, parameters, or affordances.
- `invalid_command_attempts`: commands rejected because the command or syntax
  was invalid; policy denials are not invalid commands.
- `skipped_verifications`: task conclusions stated without checking available
  system evidence.

Task completion is the primary metric. Efficiency metrics only matter when
completion and correctness are not worse.

## Result Integrity

- `result.example.kv` is a schema example and must never be reported as data.
- Freeze tasks, prompts, metric definitions and limits before the first run.
  Version any later change instead of silently editing the protocol.
- Preserve failed and incomplete traces.
- Record exact BoOS and model versions and verify the stored trace hash.
- Apply one scoring rubric to both variants; report raw per-run records before
  aggregates.

Run the validator's own tests with:

```sh
./test-validate-result.sh
```

## Recorded Runs

- [`2026-07-30-pair-001`](runs/2026-07-30-pair-001/README.md) — first
  exploratory pair; baseline recorded 12 interactions and 6,013 observation
  bytes, while object recorded 7 interactions and 15,555 observation bytes.
