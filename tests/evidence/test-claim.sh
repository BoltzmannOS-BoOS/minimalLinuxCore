#!/bin/sh
set -eu

evidence_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
validator="$evidence_dir/validate-record.sh"
verifier="$evidence_dir/verify-claim.sh"
registration="$evidence_dir/fixtures/valid/registration.kv"
result="$evidence_dir/fixtures/valid/result.kv"
supported_claim="$evidence_dir/fixtures/valid/claim-supported.kv"
temporary_dir="$(mktemp -d)"
trap 'rm -rf "$temporary_dir"' EXIT HUP INT TERM

expect_failure_message() {
    description="$1"
    expected_message="$2"
    shift 2
    if "$@" >"$temporary_dir/stdout" 2>"$temporary_dir/stderr"; then
        echo "expected failure: $description" >&2
        exit 1
    fi
    if ! grep -F -x "$expected_message" "$temporary_dir/stderr" >/dev/null; then
        echo "expected failure message: $description" >&2
        cat "$temporary_dir/stderr" >&2
        exit 1
    fi
}

"$validator" "$evidence_dir/fixtures/valid/claim.kv" >/dev/null
expect_failure_message "missing claim scope" \
    "missing required field: scope" \
    "$validator" "$evidence_dir/fixtures/invalid/claim-missing-scope.kv"
expect_failure_message "invalid claim status" \
    "invalid status: proven" \
    "$validator" "$evidence_dir/fixtures/invalid/claim-invalid-status.kv"

sed \
    's/^problem_distribution=.*/problem_distribution=unregistered/' \
    "$supported_claim" >"$temporary_dir/supported-unregistered.kv"
expect_failure_message "supported claim uses an unregistered distribution" \
    "supported_within_scope requires a registered problem_distribution" \
    "$validator" "$temporary_dir/supported-unregistered.kv"

sed \
    's/^benchmark_versions=.*/benchmark_versions=none/' \
    "$supported_claim" >"$temporary_dir/supported-no-benchmark.kv"
expect_failure_message "supported claim omits benchmark versions" \
    "supported_within_scope requires benchmark_versions" \
    "$validator" "$temporary_dir/supported-no-benchmark.kv"

sed \
    's/^implementation_versions=.*/implementation_versions=none/' \
    "$supported_claim" >"$temporary_dir/supported-no-implementation.kv"
expect_failure_message "supported claim omits implementation versions" \
    "supported_within_scope requires implementation_versions" \
    "$validator" "$temporary_dir/supported-no-implementation.kv"

sed \
    's/^status=.*/status=contradicted/' \
    "$supported_claim" >"$temporary_dir/contradicted-no-counterevidence.kv"
expect_failure_message "contradicted claim omits counterevidence" \
    "contradicted claim requires named counterevidence" \
    "$validator" "$temporary_dir/contradicted-no-counterevidence.kv"

sed -e 's/^status=.*/status=stale/' \
    -e 's/^benchmark_versions=.*/benchmark_versions=none/' \
    "$supported_claim" >"$temporary_dir/stale-no-benchmark.kv"
expect_failure_message "stale claim omits benchmark versions" \
    "stale claim requires benchmark_versions" \
    "$validator" "$temporary_dir/stale-no-benchmark.kv"

sed -e 's/^status=.*/status=stale/' \
    -e 's/^implementation_versions=.*/implementation_versions=none/' \
    "$supported_claim" >"$temporary_dir/stale-no-implementation.kv"
expect_failure_message "stale claim omits implementation versions" \
    "stale claim requires implementation_versions" \
    "$validator" "$temporary_dir/stale-no-implementation.kv"

"$verifier" "$supported_claim" "$registration" "$result" \
    >"$temporary_dir/verified-claim.stdout"
expected_verification="verified claim links; this does not establish evidentiary sufficiency or justify a supported status"
if ! grep -F -x "$expected_verification" \
    "$temporary_dir/verified-claim.stdout" >/dev/null
then
    echo "claim verification omitted its epistemic limit" >&2
    cat "$temporary_dir/verified-claim.stdout" >&2
    exit 1
fi

expect_failure_message "registration cannot stand in for the claim role" \
    "claim input must use schema boos.evidence.claim.v1" \
    "$verifier" "$registration" "$registration" "$result"
expect_failure_message "result cannot stand in for the registration role" \
    "registration input must use schema boos.evidence.registration.v1" \
    "$verifier" "$supported_claim" "$result" "$result"
expect_failure_message "registration cannot stand in for the result role" \
    "result input must use schema boos.evidence.result.v1" \
    "$verifier" "$supported_claim" "$registration" "$registration"

sed \
    's/^problem_distribution=.*/problem_distribution=problem-distribution.v2/' \
    "$supported_claim" >"$temporary_dir/mismatched-distribution.kv"
expect_failure_message "claim distribution does not match registration" \
    "claim/registration mismatch: problem_distribution" \
    "$verifier" "$temporary_dir/mismatched-distribution.kv" "$registration" "$result"

sed \
    's/^benchmark_versions=.*/benchmark_versions=registration-example-other/' \
    "$supported_claim" >"$temporary_dir/mismatched-benchmark.kv"
expect_failure_message "claim benchmark version does not match registration" \
    "claim/registration mismatch: benchmark_versions" \
    "$verifier" "$temporary_dir/mismatched-benchmark.kv" "$registration" "$result"

sed \
    's/^implementation_versions=.*/implementation_versions=ffffffffffffffffffffffffffffffffffffffff/' \
    "$supported_claim" >"$temporary_dir/mismatched-implementation.kv"
expect_failure_message "claim implementation version does not match registration" \
    "claim/registration mismatch: implementation_versions" \
    "$verifier" "$temporary_dir/mismatched-implementation.kv" "$registration" "$result"

echo "evidence claim validator tests passed"
