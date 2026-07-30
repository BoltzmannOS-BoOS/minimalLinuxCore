#!/bin/sh
set -eu

evidence_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
publishability_checker="$evidence_dir/check-incident-publishable.sh"
tree_validator="$evidence_dir/validate-tree.sh"
private_incident="$evidence_dir/fixtures/invalid/incident-private.kv"
public_incident="$evidence_dir/incidents/current/2026-07-30-parallel-project-context-friction.kv"
temporary_dir="$(mktemp -d)"
temporary_incident=""
cleanup() {
    if [ -n "$temporary_incident" ]; then
        rm -f "$temporary_incident"
    fi
    rm -rf "$temporary_dir"
}
trap cleanup EXIT HUP INT TERM

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

"$evidence_dir/validate-record.sh" "$private_incident" >/dev/null
expect_failure_message "private incident is structurally valid but not publishable" \
    "incident is not publishable: privacy must be public" \
    "$publishability_checker" "$private_incident"

"$publishability_checker" "$public_incident" >"$temporary_dir/stdout"
if ! grep -F -x "incident is publishable" "$temporary_dir/stdout" >/dev/null; then
    echo "current public incident did not pass publishability checking" >&2
    cat "$temporary_dir/stdout" >&2
    exit 1
fi
"$tree_validator" >/dev/null

temporary_incident="$(mktemp "$evidence_dir/incidents/current/private-publication-test.XXXXXX.kv")"
cp "$private_incident" "$temporary_incident"
expect_failure_message "publishable tree rejects a private current incident" \
    "incident is not publishable: privacy must be public" \
    "$tree_validator"

rm -f "$temporary_incident"
temporary_incident=""

echo "incident publishability tests passed"
