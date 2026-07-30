#!/bin/sh
set -eu

evidence_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
validator="$evidence_dir/validate-record.sh"
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

"$validator" "$evidence_dir/fixtures/valid/registration.kv" >/dev/null
expect_failure_message "registration is not sealed" \
    "invalid exposure_status: revealed" \
    "$validator" "$evidence_dir/fixtures/invalid/registration-unsealed.kv"
expect_failure_message "registration case digest is malformed" \
    "case_bundle_sha256 must be a lowercase SHA-256" \
    "$validator" "$evidence_dir/fixtures/invalid/registration-bad-digest.kv"
expect_failure_message "registration has no cases" \
    "case_count must be a positive integer" \
    "$validator" "$evidence_dir/fixtures/invalid/registration-zero-cases.kv"
expect_failure_message "registration has a malformed line" \
    "malformed record line" \
    "$validator" "$evidence_dir/fixtures/invalid/registration-malformed-line.kv"
expect_failure_message "registration has an unknown field" \
    "unknown field: unexpected" \
    "$validator" "$evidence_dir/fixtures/invalid/registration-unknown-field.kv"
expect_failure_message "registration is missing a required field" \
    "missing required field: analysis_method" \
    "$validator" "$evidence_dir/fixtures/invalid/registration-missing-analysis-method.kv"
expect_failure_message "registration has a duplicate field" \
    "duplicate field: generator_version" \
    "$validator" "$evidence_dir/fixtures/invalid/registration-duplicate-generator-version.kv"
expect_failure_message "registration case count is malformed" \
    "case_count must be a non-negative integer" \
    "$validator" "$evidence_dir/fixtures/invalid/registration-invalid-case-count.kv"
expect_failure_message "registration retry budget is negative" \
    "retry_budget must be a non-negative integer" \
    "$validator" "$evidence_dir/fixtures/invalid/registration-negative-retry-budget.kv"
expect_failure_message "registration family weights digest is malformed" \
    "family_weights_sha256 must be a lowercase SHA-256" \
    "$validator" "$evidence_dir/fixtures/invalid/registration-invalid-family-weights-digest.kv"
expect_failure_message "registration environment digest is malformed" \
    "environment_sha256 must be a lowercase SHA-256" \
    "$validator" "$evidence_dir/fixtures/invalid/registration-invalid-environment-digest.kv"
expect_failure_message "registration implementation commit is malformed" \
    "implementation_commit must be a lowercase 40-character Git commit" \
    "$validator" "$evidence_dir/fixtures/invalid/registration-invalid-implementation-commit.kv"
expect_failure_message "registration token budget is malformed" \
    "token_budget must be a non-negative integer" \
    "$validator" "$evidence_dir/fixtures/invalid/registration-invalid-token-budget.kv"
expect_failure_message "registration interaction budget is zero" \
    "interaction_budget must be a positive integer" \
    "$validator" "$evidence_dir/fixtures/invalid/registration-zero-interaction-budget.kv"
expect_failure_message "registration wall clock seconds are malformed" \
    "wall_clock_seconds must be a non-negative integer" \
    "$validator" "$evidence_dir/fixtures/invalid/registration-invalid-wall-clock-seconds.kv"
expect_failure_message "registration retry budget is malformed" \
    "retry_budget must be a non-negative integer" \
    "$validator" "$evidence_dir/fixtures/invalid/registration-invalid-retry-budget.kv"

echo "evidence registration validator tests passed"
