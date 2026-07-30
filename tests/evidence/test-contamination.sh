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

"$validator" "$evidence_dir/fixtures/valid/contamination.kv" >/dev/null
expect_failure "invalid contamination level" \
    "$validator" "$evidence_dir/fixtures/invalid/contamination-invalid-level.kv"
expect_failure "contamination retained as frontier evidence" \
    "$validator" "$evidence_dir/fixtures/invalid/contamination-frontier-disposition.kv"
expect_failure "case contamination requests a new evaluator" \
    "$validator" "$evidence_dir/fixtures/invalid/contamination-case-new-evaluator.kv"
expect_failure "case contamination requests claim review" \
    "$validator" "$evidence_dir/fixtures/invalid/contamination-case-claim-review.kv"
expect_failure "contamination omits the retired registration" \
    "$validator" "$evidence_dir/fixtures/invalid/contamination-missing-registration-id.kv"

echo "evidence contamination validator tests passed"
