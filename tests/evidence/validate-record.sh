#!/bin/sh
set -eu

evidence_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
. "$evidence_dir/lib/record.sh"
. "$evidence_dir/validators/incident.sh"
. "$evidence_dir/validators/claim.sh"

record_file="${1:?usage: validate-record.sh <record.kv>}"
test -f "$record_file" || {
    echo "record not found: $record_file" >&2
    exit 1
}

schema="$(record_value "$record_file" schema)"
case "$schema" in
    boos.evidence.incident.v1)
        validate_incident "$record_file"
        echo "valid evidence incident"
        ;;
    boos.evidence.claim.v1)
        validate_claim "$record_file"
        echo "valid evidence claim"
        ;;
    *)
        echo "unsupported evidence schema: $schema" >&2
        exit 1
        ;;
esac
