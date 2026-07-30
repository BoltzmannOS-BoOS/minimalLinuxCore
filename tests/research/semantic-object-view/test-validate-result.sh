#!/bin/sh
set -eu

experiment_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
validator="$experiment_dir/validate-result.sh"
example="$experiment_dir/result.example.kv"
temporary_dir="$(mktemp -d)"
trap 'rm -rf "$temporary_dir"' EXIT HUP INT TERM

"$validator" "$example" >/dev/null

sed '/^variant=/d' "$example" > "$temporary_dir/missing-variant.kv"
if "$validator" "$temporary_dir/missing-variant.kv" >/dev/null 2>&1; then
    echo "validator accepted a result without variant" >&2
    exit 1
fi

sed '/^boos_commit=/d' "$example" > "$temporary_dir/missing-commit.kv"
if "$validator" "$temporary_dir/missing-commit.kv" >/dev/null 2>&1; then
    echo "validator accepted a result without boos_commit" >&2
    exit 1
fi

sed '/^trace_sha256=/d' "$example" > "$temporary_dir/missing-trace-hash.kv"
if "$validator" "$temporary_dir/missing-trace-hash.kv" >/dev/null 2>&1; then
    echo "validator accepted a result without trace_sha256" >&2
    exit 1
fi

sed 's/^completed_tasks=.*/completed_tasks=unknown/' \
    "$example" > "$temporary_dir/invalid-count.kv"
if "$validator" "$temporary_dir/invalid-count.kv" >/dev/null 2>&1; then
    echo "validator accepted a non-numeric completed_tasks value" >&2
    exit 1
fi

echo "semantic object result validator tests passed"
