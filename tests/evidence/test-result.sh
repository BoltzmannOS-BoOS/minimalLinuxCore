#!/bin/sh
set -eu

evidence_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
validator="$evidence_dir/validate-record.sh"
verifier="$evidence_dir/verify-result.sh"
registration="$evidence_dir/fixtures/valid/registration.kv"
valid_result="$evidence_dir/fixtures/valid/result.kv"
temporary_dir="$(mktemp -d "$evidence_dir/result-test.XXXXXX")"
temporary_path="tests/evidence/$(basename -- "$temporary_dir")"
trap 'rm -rf "$temporary_dir"' EXIT HUP INT TERM

expect_failure() {
    description="$1"
    shift
    if "$@" >"$temporary_dir/stdout" 2>"$temporary_dir/stderr"; then
        echo "expected failure: $description" >&2
        exit 1
    fi
}

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

"$validator" "$valid_result" >/dev/null
expect_failure "result is not revealed" \
    "$validator" "$evidence_dir/fixtures/invalid/result-unrevealed.kv"
expect_failure "result trace hash is malformed" \
    "$validator" "$evidence_dir/fixtures/invalid/result-bad-trace-hash.kv"
expect_failure "passing result reports a failure class" \
    "$validator" "$evidence_dir/fixtures/invalid/result-pass-with-failure.kv"
"$verifier" "$registration" "$valid_result" >/dev/null
if ! LC_ALL=definitely-invalid "$verifier" "$registration" "$valid_result" \
    >"$temporary_dir/locale-stdout" 2>"$temporary_dir/locale-stderr"; then
    echo "expected verification to succeed under an invalid inherited locale" >&2
    exit 1
fi
if [ -s "$temporary_dir/locale-stderr" ]; then
    echo "artifact hashing emitted diagnostics under an invalid inherited locale" >&2
    cat "$temporary_dir/locale-stderr" >&2
    exit 1
fi

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

sed \
    "s%^trace_path=.*%trace_path=$temporary_path/missing-trace.txt%" \
    "$valid_result" >"$temporary_dir/missing-trace.kv"
expect_failure_message "safe repository-relative trace path does not exist" \
    "trace file not found: $temporary_path/missing-trace.txt" \
    "$verifier" "$registration" "$temporary_dir/missing-trace.kv"

ln -s /etc "$temporary_dir/unsafe-parent"
sed \
    "s%^trace_path=.*%trace_path=$temporary_path/unsafe-parent/hosts%" \
    "$valid_result" >"$temporary_dir/unsafe-trace-path.kv"
expect_failure_message "artifact paths must not resolve outside the repository" \
    "trace path is unsafe: $temporary_path/unsafe-parent/hosts" \
    "$verifier" "$registration" "$temporary_dir/unsafe-trace-path.kv"

echo "evidence result validator tests passed"
