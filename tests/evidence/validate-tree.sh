#!/bin/sh
set -eu

evidence_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
temporary_dir="$(mktemp -d)"
trap 'rm -rf "$temporary_dir"' EXIT HUP INT TERM

for record_dir in "$evidence_dir/incidents/current" "$evidence_dir/claims"; do
    test -d "$record_dir" || {
        echo "evidence directory not found: $record_dir" >&2
        exit 1
    }
done

find "$evidence_dir/incidents/current" "$evidence_dir/claims" \
    -type f -name '*.kv' -print >"$temporary_dir/records"
LC_ALL=C sort "$temporary_dir/records" >"$temporary_dir/records.sorted"

while IFS= read -r record_file; do
    "$evidence_dir/validate-record.sh" "$record_file" >/dev/null
done <"$temporary_dir/records.sorted"

echo "publishable evidence tree is valid"
