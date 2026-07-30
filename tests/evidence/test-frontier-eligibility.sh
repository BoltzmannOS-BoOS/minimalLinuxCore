#!/bin/sh
set -eu

evidence_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
eligibility_checker="$evidence_dir/check-frontier-eligibility.sh"
registration="$evidence_dir/fixtures/valid/registration.kv"
matching_contamination="$evidence_dir/fixtures/valid/contamination.kv"
unrelated_contamination="$evidence_dir/fixtures/valid/contamination-unrelated.kv"
unrelated_registration_contamination="$evidence_dir/fixtures/valid/contamination-unrelated-registration.kv"
unrelated_level_contamination="$evidence_dir/fixtures/valid/contamination-unrelated-level.kv"
invalid_contamination="$evidence_dir/fixtures/invalid/contamination-missing-registration-id.kv"
temporary_dir="$(mktemp -d)"
trap 'rm -rf "$temporary_dir"' EXIT HUP INT TERM

expect_exit() {
    expected_status="$1"
    description="$2"
    shift 2
    if "$@" >"$temporary_dir/stdout" 2>"$temporary_dir/stderr"; then
        actual_status=0
    else
        actual_status=$?
    fi
    if [ "$actual_status" -ne "$expected_status" ]; then
        echo "expected exit $expected_status for $description, got $actual_status" >&2
        exit 1
    fi
}

expect_exit 1 "matching contamination retires the exact frontier tuple" \
    "$eligibility_checker" "$registration" family problem-family-example \
    "$matching_contamination"
expect_exit 2 "invalid contamination after a matching record fails closed" \
    "$eligibility_checker" "$registration" family problem-family-example \
    "$matching_contamination" "$invalid_contamination"
"$eligibility_checker" "$registration" family problem-family-example \
    "$unrelated_contamination" >/dev/null
"$eligibility_checker" "$registration" family problem-family-example \
    "$unrelated_registration_contamination" >/dev/null
"$eligibility_checker" "$registration" family problem-family-example \
    "$unrelated_level_contamination" >/dev/null
expect_exit 2 "empty target ID is invalid input" \
    "$eligibility_checker" "$registration" family "" "$unrelated_contamination"
expect_exit 2 "invalid contamination fails closed" \
    "$eligibility_checker" "$registration" family problem-family-example \
    "$invalid_contamination"

echo "frontier eligibility tests passed"
