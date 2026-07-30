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

"$validator" "$evidence_dir/fixtures/valid/claim.kv" >/dev/null
expect_failure "missing claim scope" \
    "$validator" "$evidence_dir/fixtures/invalid/claim-missing-scope.kv"
expect_failure "invalid claim status" \
    "$validator" "$evidence_dir/fixtures/invalid/claim-invalid-status.kv"

echo "evidence claim validator tests passed"
