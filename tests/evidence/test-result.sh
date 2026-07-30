#!/bin/sh
set -eu

evidence_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
validator="$evidence_dir/validate-record.sh"
verifier="$evidence_dir/verify-result.sh"
registration="$evidence_dir/fixtures/valid/registration.kv"
valid_result="$evidence_dir/fixtures/valid/result.kv"
temporary_dir="$(mktemp -d)"
unsafe_trace_link="$evidence_dir/fixtures/valid/unsafe-trace-link"
trap 'rm -f "$unsafe_trace_link"; rm -rf "$temporary_dir"' EXIT HUP INT TERM

expect_failure() {
    description="$1"
    shift
    if "$@" >"$temporary_dir/stdout" 2>"$temporary_dir/stderr"; then
        echo "expected failure: $description" >&2
        exit 1
    fi
}

"$validator" "$valid_result" >/dev/null
expect_failure "result is not revealed" \
    "$validator" "$evidence_dir/fixtures/invalid/result-unrevealed.kv"
expect_failure "result trace hash is malformed" \
    "$validator" "$evidence_dir/fixtures/invalid/result-bad-trace-hash.kv"
expect_failure "passing result reports a failure class" \
    "$validator" "$evidence_dir/fixtures/invalid/result-pass-with-failure.kv"
"$verifier" "$registration" "$valid_result" >/dev/null

sed \
    's/^case_bundle_sha256=.*/case_bundle_sha256=ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff/' \
    "$valid_result" >"$temporary_dir/mismatched-bundle.kv"
expect_failure "registration and result bundle mismatch" \
    "$verifier" "$registration" "$temporary_dir/mismatched-bundle.kv"

sed \
    's/^trace_sha256=.*/trace_sha256=0000000000000000000000000000000000000000000000000000000000000000/' \
    "$valid_result" >"$temporary_dir/wrong-trace-content.kv"
expect_failure "recorded trace digest does not match trace bytes" \
    "$verifier" "$registration" "$temporary_dir/wrong-trace-content.kv"

sed \
    's/^primary_outcomes_sha256=.*/primary_outcomes_sha256=0000000000000000000000000000000000000000000000000000000000000000/' \
    "$valid_result" >"$temporary_dir/wrong-outcomes-content.kv"
expect_failure "recorded outcomes digest does not match outcome bytes" \
    "$verifier" "$registration" "$temporary_dir/wrong-outcomes-content.kv"

printf 'outside trace\n' >"$temporary_dir/outside-trace.txt"
ln -s "$temporary_dir/outside-trace.txt" "$unsafe_trace_link"
sed \
    -e 's%^trace_path=.*%trace_path=tests/evidence/fixtures/valid/unsafe-trace-link%' \
    -e 's/^trace_sha256=.*/trace_sha256=e5a1702caadf9242231a496c58f673bf5327eb20e2d8cbb16eadf1eafcda81e2/' \
    "$valid_result" >"$temporary_dir/unsafe-trace-path.kv"
expect_failure "artifact paths must not resolve outside the repository" \
    "$verifier" "$registration" "$temporary_dir/unsafe-trace-path.kv"

echo "evidence result validator tests passed"
