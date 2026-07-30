#!/bin/sh
set -eu

evidence_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"

# Keep the foundational schemas ahead of records that depend on them.
for test_script in \
    "$evidence_dir/test-record.sh" \
    "$evidence_dir/test-incident.sh" \
    "$evidence_dir/test-claim.sh" \
    "$evidence_dir/test-registration.sh" \
    "$evidence_dir/test-contamination.sh" \
    "$evidence_dir/test-frontier-eligibility.sh" \
    "$evidence_dir/test-result.sh"
do
    sh "$test_script"
done

echo "all evidence system tests passed"
