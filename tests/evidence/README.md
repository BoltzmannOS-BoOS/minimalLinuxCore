# Living evidence system

This living evidence system keeps structured, reviewable records of observed
incidents, bounded claims, regression evidence, and frontier metadata so that
research evidence remains auditable as the implementation changes.

- [Incidents](incidents/) record observed problems and their normalized current
  forms.
- [Claims](claims/) bound what current evidence supports and preserves known
  counterevidence.
- [Regression](regression/) records evidence from already-exposed tests and
  wiring.
- [Frontier metadata](frontier/) governs sealed, not-yet-plaintext evaluation
  material and exposure retirement.

Run the focused-validator suite with `./test-all.sh`. Validate publishable
current records, excluding invalid fixtures, with `./validate-tree.sh`.

`check-frontier-eligibility.sh` retains its compatibility name, but it checks
only whether an exact tuple matches one of the supplied append-only
contamination records. Exit 0 does not establish target membership or frontier
eligibility: registration records contain no case, family, or metric roster,
and the guard cannot detect undisclosed exposure. An exact match exits 1 and
retires that registered tuple.

Primary outcomes use a strict versioned schema. Result verification checks
their byte digest, validates their structure, and reconciles the declared
`result_id`, `status`, and `failure_class` with the result summary. This checks
declared summary consistency, not evaluator truth or outcome sufficiency.
Passing these validators checks bounded structure and relationships, not BoOS
itself, construct validity, benchmark sufficiency, or semantic neutrality.
