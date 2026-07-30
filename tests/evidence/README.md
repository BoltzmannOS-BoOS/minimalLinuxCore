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

Frontier eligibility is derived only from the supplied append-only
contamination records; the guard cannot detect undisclosed exposure. Passing
these validators checks record structure and relationships, not BoOS itself:
it does not validate BoOS.
