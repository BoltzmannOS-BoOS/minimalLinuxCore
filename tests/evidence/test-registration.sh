#!/bin/sh
set -eu

evidence_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
validator="$evidence_dir/validate-record.sh"
temporary_dir="$(mktemp -d)"
trap 'rm -rf "$temporary_dir"' EXIT HUP INT TERM

expect_failure() {
    description="$1"
    shift
    if "$@" >"$temporary_dir/stdout" 2>"$temporary_dir/stderr"; then
        echo "expected failure: $description" >&2
        exit 1
    fi
}

"$validator" "$evidence_dir/fixtures/valid/registration.kv" >/dev/null
expect_failure "registration is not sealed" \
    "$validator" "$evidence_dir/fixtures/invalid/registration-unsealed.kv"
expect_failure "registration case digest is malformed" \
    "$validator" "$evidence_dir/fixtures/invalid/registration-bad-digest.kv"
expect_failure "registration has no cases" \
    "$validator" "$evidence_dir/fixtures/invalid/registration-zero-cases.kv"
expect_failure "registration has a malformed line" \
    "$validator" "$evidence_dir/fixtures/invalid/registration-malformed-line.kv"
expect_failure "registration has an unknown field" \
    "$validator" "$evidence_dir/fixtures/invalid/registration-unknown-field.kv"
expect_failure "registration is missing a required field" \
    "$validator" "$evidence_dir/fixtures/invalid/registration-missing-analysis-method.kv"
expect_failure "registration has a duplicate field" \
    "$validator" "$evidence_dir/fixtures/invalid/registration-duplicate-generator-version.kv"
expect_failure "registration case count is malformed" \
    "$validator" "$evidence_dir/fixtures/invalid/registration-invalid-case-count.kv"
expect_failure "registration retry budget is negative" \
    "$validator" "$evidence_dir/fixtures/invalid/registration-negative-retry-budget.kv"
expect_failure "registration family weights digest is malformed" \
    "$validator" "$evidence_dir/fixtures/invalid/registration-invalid-family-weights-digest.kv"
expect_failure "registration environment digest is malformed" \
    "$validator" "$evidence_dir/fixtures/invalid/registration-invalid-environment-digest.kv"
expect_failure "registration implementation commit is malformed" \
    "$validator" "$evidence_dir/fixtures/invalid/registration-invalid-implementation-commit.kv"
expect_failure "registration token budget is malformed" \
    "$validator" "$evidence_dir/fixtures/invalid/registration-invalid-token-budget.kv"
expect_failure "registration interaction budget is zero" \
    "$validator" "$evidence_dir/fixtures/invalid/registration-zero-interaction-budget.kv"
expect_failure "registration wall clock seconds are malformed" \
    "$validator" "$evidence_dir/fixtures/invalid/registration-invalid-wall-clock-seconds.kv"
expect_failure "registration retry budget is malformed" \
    "$validator" "$evidence_dir/fixtures/invalid/registration-invalid-retry-budget.kv"

echo "evidence registration validator tests passed"
