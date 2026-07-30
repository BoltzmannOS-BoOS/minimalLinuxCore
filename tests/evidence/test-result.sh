#!/bin/sh
set -eu

evidence_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
validator="$evidence_dir/validate-record.sh"
verifier="$evidence_dir/verify-result.sh"
registration="$evidence_dir/fixtures/valid/registration.kv"
valid_result="$evidence_dir/fixtures/valid/result.kv"
temporary_dir="$(mktemp -d "$evidence_dir/result-test.XXXXXX")"
temporary_path="tests/evidence/$(basename -- "$temporary_dir")"
expected_trace_sha256="276d9829518f945c56158b37f9d63e86335c83ad76b661cf2460676a1e497d4c"
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

"$validator" "$valid_result" >/dev/null
expect_failure_message "result is not revealed" \
    "invalid exposure_status: sealed" \
    "$validator" "$evidence_dir/fixtures/invalid/result-unrevealed.kv"
expect_failure_message "result trace hash is malformed" \
    "trace_sha256 must be a lowercase SHA-256" \
    "$validator" "$evidence_dir/fixtures/invalid/result-bad-trace-hash.kv"
expect_failure_message "passing result reports a failure class" \
    "status and failure_class are inconsistent" \
    "$validator" "$evidence_dir/fixtures/invalid/result-pass-with-failure.kv"
"$verifier" "$registration" "$valid_result" >"$temporary_dir/verified-result.stdout"
expected_verification="verified evidence result; declared summary reconciliation does not establish evaluator truth or outcome sufficiency"
if ! grep -F -x "$expected_verification" \
    "$temporary_dir/verified-result.stdout" >/dev/null
then
    echo "result verification omitted its epistemic limit" >&2
    cat "$temporary_dir/verified-result.stdout" >&2
    exit 1
fi

expect_failure_message "result cannot stand in for the registration role" \
    "registration input must use schema boos.evidence.registration.v1" \
    "$verifier" "$valid_result" "$valid_result"
expect_failure_message "registration cannot stand in for the result role" \
    "result input must use schema boos.evidence.result.v1" \
    "$verifier" "$registration" "$registration"

if ! (
    . "$evidence_dir/lib/record.sh"
    awk() {
        if [ "${LC_ALL-}" != C ]; then
            echo "digest parser did not use LC_ALL=C" >&2
            return 1
        fi
        command awk "$@"
    }
    LC_ALL=definitely-invalid
    export LC_ALL
    sha256_file "$evidence_dir/fixtures/valid/trace.example.txt"
) >"$temporary_dir/hash-stdout" 2>"$temporary_dir/hash-stderr"; then
    echo "expected sha256_file to succeed under an invalid inherited locale" >&2
    cat "$temporary_dir/hash-stderr" >&2
    exit 1
fi
if [ "$(cat "$temporary_dir/hash-stdout")" != "$expected_trace_sha256" ]; then
    echo "sha256_file returned an unexpected digest" >&2
    cat "$temporary_dir/hash-stdout" >&2
    exit 1
fi
if [ -s "$temporary_dir/hash-stderr" ]; then
    echo "artifact hashing emitted diagnostics under an invalid inherited locale" >&2
    cat "$temporary_dir/hash-stderr" >&2
    exit 1
fi

sed \
    's/^registration_id=.*/registration_id=registration-example-other/' \
    "$valid_result" >"$temporary_dir/mismatched-registration-id.kv"
expect_failure_message "registration and result ID mismatch" \
    "registration/result mismatch: registration_id" \
    "$verifier" "$registration" "$temporary_dir/mismatched-registration-id.kv"

sed \
    's/^case_bundle_sha256=.*/case_bundle_sha256=ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff/' \
    "$valid_result" >"$temporary_dir/mismatched-bundle.kv"
expect_failure_message "registration and result bundle mismatch" \
    "registration/result mismatch: case_bundle_sha256" \
    "$verifier" "$registration" "$temporary_dir/mismatched-bundle.kv"

sed \
    's/^evaluator_version=.*/evaluator_version=evaluator.v2/' \
    "$valid_result" >"$temporary_dir/mismatched-evaluator.kv"
expect_failure_message "registration and result evaluator mismatch" \
    "registration/result mismatch: evaluator_version" \
    "$verifier" "$registration" "$temporary_dir/mismatched-evaluator.kv"

sed \
    's/^trace_sha256=.*/trace_sha256=0000000000000000000000000000000000000000000000000000000000000000/' \
    "$valid_result" >"$temporary_dir/wrong-trace-content.kv"
expect_failure_message "recorded trace digest does not match trace bytes" \
    "trace SHA-256 mismatch" \
    "$verifier" "$registration" "$temporary_dir/wrong-trace-content.kv"

sed \
    's/^primary_outcomes_sha256=.*/primary_outcomes_sha256=0000000000000000000000000000000000000000000000000000000000000000/' \
    "$valid_result" >"$temporary_dir/wrong-outcomes-content.kv"
expect_failure_message "recorded outcomes digest does not match outcome bytes" \
    "primary outcomes SHA-256 mismatch" \
    "$verifier" "$registration" "$temporary_dir/wrong-outcomes-content.kv"

cat >"$temporary_dir/incomplete-outcomes.kv" <<'EOF'
schema=boos.evidence.primary-outcomes.v1
result_id=result-example-valid
status=inconclusive
EOF
incomplete_outcomes_sha256="$(
    . "$evidence_dir/lib/record.sh"
    sha256_file "$temporary_dir/incomplete-outcomes.kv"
)"
sed \
    -e "s%^primary_outcomes_path=.*%primary_outcomes_path=$temporary_path/incomplete-outcomes.kv%" \
    -e "s/^primary_outcomes_sha256=.*/primary_outcomes_sha256=$incomplete_outcomes_sha256/" \
    "$valid_result" >"$temporary_dir/incomplete-outcomes-result.kv"
expect_failure_message "hashed primary outcomes must satisfy their schema" \
    "missing required field: failure_class" \
    "$verifier" "$registration" "$temporary_dir/incomplete-outcomes-result.kv"

wrong_role_outcomes_sha256="$(
    . "$evidence_dir/lib/record.sh"
    sha256_file "$valid_result"
)"
sed \
    -e 's%^primary_outcomes_path=.*%primary_outcomes_path=tests/evidence/fixtures/valid/result.kv%' \
    -e "s/^primary_outcomes_sha256=.*/primary_outcomes_sha256=$wrong_role_outcomes_sha256/" \
    "$valid_result" >"$temporary_dir/wrong-outcomes-role.kv"
expect_failure_message "hashed primary outcomes must use the outcomes role" \
    "primary outcomes artifact must use schema boos.evidence.primary-outcomes.v1" \
    "$verifier" "$registration" "$temporary_dir/wrong-outcomes-role.kv"

sed \
    's/^result_id=.*/result_id=result-example-other/' \
    "$valid_result" >"$temporary_dir/contradictory-result-id.kv"
expect_failure_message "result ID contradicts the unchanged primary outcomes" \
    "primary outcomes/result mismatch: result_id" \
    "$verifier" "$registration" "$temporary_dir/contradictory-result-id.kv"

sed \
    's/^status=inconclusive$/status=fail/' \
    "$valid_result" >"$temporary_dir/contradictory-status.kv"
expect_failure_message "result status contradicts the unchanged primary outcomes" \
    "primary outcomes/result mismatch: status" \
    "$verifier" "$registration" "$temporary_dir/contradictory-status.kv"

sed \
    's/^failure_class=infrastructure$/failure_class=evaluator/' \
    "$valid_result" >"$temporary_dir/contradictory-failure-class.kv"
expect_failure_message "result failure class contradicts the unchanged primary outcomes" \
    "primary outcomes/result mismatch: failure_class" \
    "$verifier" "$registration" "$temporary_dir/contradictory-failure-class.kv"

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
