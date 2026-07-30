# Frontier metadata

Plaintext frontier cases do not live in the working tree. This directory holds
only metadata needed to govern their use.

- A **registration digest** commits to a sealed registered case bundle before
  evaluation.
- **Exposure** is an event that makes a case, family, or metric known to an
  evaluator or another relevant audience.
- **Retirement** removes an exposed target from frontier eligibility for its
  registered evaluation.
- **Family contamination** records exposure that retires the affected problem
  family, rather than only one case.
- **Append-only reveal rules** require exposure and contamination records to be
  added without rewriting earlier records.

Eligibility is derived from the supplied append-only contamination records for
the exact registered target. The guard cannot detect exposure that was not
disclosed in those records.
