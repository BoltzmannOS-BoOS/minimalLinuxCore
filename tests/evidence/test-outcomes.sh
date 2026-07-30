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

cat >"$temporary_dir/valid.kv" <<'EOF'
schema=boos.evidence.primary-outcomes.v1
result_id=result-example-valid
status=inconclusive
failure_class=infrastructure
EOF
"$validator" "$temporary_dir/valid.kv" >/dev/null

sed '/^failure_class=/d' \
    "$temporary_dir/valid.kv" >"$temporary_dir/missing-failure-class.kv"
expect_failure_message "primary outcomes omit failure class" \
    "missing required field: failure_class" \
    "$validator" "$temporary_dir/missing-failure-class.kv"

sed -e 's/^status=.*/status=pass/' \
    "$temporary_dir/valid.kv" >"$temporary_dir/inconsistent-summary.kv"
expect_failure_message "primary outcomes status and failure class disagree" \
    "status and failure_class are inconsistent" \
    "$validator" "$temporary_dir/inconsistent-summary.kv"

echo "primary outcomes validator tests passed"
