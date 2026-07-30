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

echo "evidence registration validator tests passed"
