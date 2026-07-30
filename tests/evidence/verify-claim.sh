#!/bin/sh
set -eu

evidence_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
. "$evidence_dir/lib/record.sh"

claim_file="${1:?usage: verify-claim.sh <claim.kv> <registration.kv> <result.kv>}"
registration_file="${2:?usage: verify-claim.sh <claim.kv> <registration.kv> <result.kv>}"
result_file="${3:?usage: verify-claim.sh <claim.kv> <registration.kv> <result.kv>}"

"$evidence_dir/validate-record.sh" "$claim_file" >/dev/null

if [ "$(record_value "$claim_file" schema)" != "boos.evidence.claim.v1" ]; then
    echo "claim input must use schema boos.evidence.claim.v1" >&2
    exit 1
fi

"$evidence_dir/verify-result.sh" "$registration_file" "$result_file" >/dev/null

if [ "$(record_value "$claim_file" problem_distribution)" != \
    "$(record_value "$registration_file" distribution_version)" ]
then
    echo "claim/registration mismatch: problem_distribution" >&2
    exit 1
fi
if [ "$(record_value "$claim_file" benchmark_versions)" != \
    "$(record_value "$registration_file" registration_id)" ]
then
    echo "claim/registration mismatch: benchmark_versions" >&2
    exit 1
fi
if [ "$(record_value "$claim_file" implementation_versions)" != \
    "$(record_value "$registration_file" implementation_commit)" ]
then
    echo "claim/registration mismatch: implementation_versions" >&2
    exit 1
fi

echo "verified claim links; this does not establish evidentiary sufficiency or justify a supported status"
