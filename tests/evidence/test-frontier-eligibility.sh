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

expect_exit_message() {
    expected_status="$1"
    description="$2"
    expected_message="$3"
    shift 3
    expect_exit "$expected_status" "$description" "$@"
    if ! grep -F -x "$expected_message" "$temporary_dir/stderr" >/dev/null; then
        echo "expected failure message for $description" >&2
        cat "$temporary_dir/stderr" >&2
        exit 1
    fi
}

expect_no_match() {
    target_level="$1"
    target_id="$2"
    shift 2
    expected_message="no matching supplied contamination record: registration-example-valid:$target_level:$target_id; target membership and frontier status are not established"
    if ! "$eligibility_checker" "$registration" "$target_level" "$target_id" "$@" \
        >"$temporary_dir/stdout" 2>"$temporary_dir/stderr"
    then
        echo "expected no supplied contamination match for $target_level:$target_id" >&2
        cat "$temporary_dir/stderr" >&2
        exit 1
    fi
    if ! grep -F -x "$expected_message" "$temporary_dir/stdout" >/dev/null; then
        echo "unexpected no-match wording for $target_level:$target_id" >&2
        cat "$temporary_dir/stdout" >&2
        exit 1
    fi
    if grep -F "eligible" "$temporary_dir/stdout" >/dev/null; then
        echo "no-match output described an unregistered target as eligible" >&2
        cat "$temporary_dir/stdout" >&2
        exit 1
    fi
}

expect_exit_message 1 "matching contamination retires the exact tuple" \
    "matching supplied contamination record retires registration-example-valid:family:problem-family-example" \
    "$eligibility_checker" "$registration" family problem-family-example \
    "$matching_contamination"
expect_exit_message 2 "invalid contamination after a matching record fails closed" \
    "missing required field: registration_id" \
    "$eligibility_checker" "$registration" family problem-family-example \
    "$matching_contamination" "$invalid_contamination"
expect_no_match family problem-family-example "$unrelated_contamination"
expect_no_match family problem-family-example \
    "$unrelated_registration_contamination"
expect_no_match family problem-family-example "$unrelated_level_contamination"
expect_no_match family unknown-family
expect_no_match family problem-famliy-example
expect_exit_message 2 "empty target ID is invalid input" \
    "target ID must not be empty" \
    "$eligibility_checker" "$registration" family "" "$unrelated_contamination"
expect_exit_message 2 "invalid contamination fails closed" \
    "missing required field: registration_id" \
    "$eligibility_checker" "$registration" family problem-family-example \
    "$invalid_contamination"

echo "contamination guard tests passed"
