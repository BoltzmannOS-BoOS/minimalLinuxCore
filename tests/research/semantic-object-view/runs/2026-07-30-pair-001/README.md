# Test 0: Interface and Wiring Probe — Pair 001

This is the first exploratory paired run of the frozen v1 protocol. Both
variants completed all five tasks, but the hypothesis was only partially
supported.

## Raw results

| Metric | Baseline | Object | Object change |
|---|---:|---:|---:|
| Completed tasks | 5/5 | 5/5 | equal |
| Environment interactions | 12 | 7 | -5 (-41.7%) |
| Observation bytes | 6,013 | 15,555 | +9,542 (+158.7%) |
| Incorrect capability assumptions | 1 | 0 | -1 |
| Invalid command attempts | 1 | 0 | -1 |
| Skipped verifications | 0 | 0 | equal |

The baseline's incorrect assumption was an intermediate statement that there
were 23 enabled flags; its final answer corrected the count to 25. The invalid
attempt was `read-file` without its required argument. Policy denials were not
counted as invalid commands.

Token usage is ancillary because it is not a v1 protocol metric:

| Usage | Baseline | Object | Object change |
|---|---:|---:|---:|
| Input tokens | 285,239 | 186,985 | -34.4% |
| Cached input tokens | 231,680 | 156,928 | -32.3% |
| Output tokens | 3,612 | 2,534 | -29.8% |
| Reasoning output tokens | 1,216 | 972 | -20.1% |

## Interpretation

The semantic view made discovery more coherent: the model needed fewer
round-trips, made no unsupported capability claim, and did not learn parameter
syntax by intentionally issuing an invalid command. This shows that the tested
model could consume the implemented object protocol under an
interface-specific prompt. Because the tasks were constructed from that
interface, the result supplies wiring evidence but no general evidence that a
semantic ABI improves AI operation.

The current enumeration shape is not compact. A single unfiltered `world list`
returned every field for 39 capabilities plus the system object, so the object
variant consumed about 2.6 times as many observation bytes. The next experiment
should test filtered, projected and paginated discovery, such as listing only
`id` and `state` before showing selected objects.

This pair is exploratory evidence, not an aggregate conclusion.

## Validity limits

- This is one paired run with one model and no repetition.
- The interfaces expose different capability universes. Baseline `caps`
  reports 26 policy flags, while `world list` reports 39 registered semantic
  capabilities. Task 001 therefore compares interface-native maps, not an
  identical set of entities.
- The object run determined disabled invocation from inspected state and the
  absence of an `invoke` affordance. It did not attempt an unavailable semantic
  invoke command.
- Identical experiment-only boot adaptations were applied to both guests
  because the local build environment could not produce a bootable image
  directly. Both used the same kernel and base initramfs; neither adaptation is
  part of the product commit.
- Both guests logged module-probe warnings, although the required network and
  gateway started and every recorded command completed.
- The model sampling temperature was not exposed by the CLI and is recorded as
  provider-default-unset.

## Evidence

- `baseline-prompt.txt` and `object-prompt.txt` are the exact submitted prompts,
  including the shared transport appendix.
- `baseline-trace.jsonl` and `object-trace.jsonl` are the raw Codex event
  streams. Their SHA-256 values are recorded in the result files.
- `baseline-final.txt` and `object-final.txt` preserve the submitted final
  answers.
- `baseline-result.kv` and `object-result.kv` contain protocol metrics.
- `environment.kv` records the paired controls and tested artifact identities.
