# Rejected: Feature-Derived System-Boundary Benchmark

**Status:** Rejected
**Date rejected:** 2026-07-30

## Reason

This design started with capabilities BoOS was expected to provide—selective
sharing, isolation, trusted enforcement, and recovery—and then constructed a
benchmark around those capabilities.

Symmetric scoring did not remove the deeper construct bias. A test can avoid
declaring a preferred winner while still selecting tasks that encode one
implementation's assumptions and strengths. Passing such a test would not
supply sufficient evidence for the broader system claim.

## Decision

Do not implement this benchmark.

Future evaluation must:

- derive tasks from observed goals, failures, and consequences outside the
  feature design;
- preserve known tests as regression coverage without treating them as fresh
  evidence;
- use unexposed, rotating frontier cases for directional evaluation;
- verify that tests reject intentionally broken implementations;
- limit every conclusion to the claim and problem distribution actually
  covered.

The replacement design is
[`2026-07-30-boos-living-evidence-system-design.md`](2026-07-30-boos-living-evidence-system-design.md).

The rejected proposal remains available in Git history through commits
`38fd9e9` and `c79a87c` so the reasoning error is auditable.
