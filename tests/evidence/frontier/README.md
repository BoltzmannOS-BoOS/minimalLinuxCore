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

The compatibility-named eligibility guard checks only for an exact match among
the supplied append-only contamination records. A match retires that registered
tuple. No match does not establish that the requested target belongs to the
registration or remains frontier-eligible: registration manifests contain no
case, family, or metric roster, and the guard cannot detect exposure absent
from the supplied records.
