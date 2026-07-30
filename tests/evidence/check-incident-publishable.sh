#!/bin/sh
set -eu

evidence_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
. "$evidence_dir/lib/record.sh"

incident_file="${1:?usage: check-incident-publishable.sh <incident.kv>}"

"$evidence_dir/validate-record.sh" "$incident_file" >/dev/null

if [ "$(record_value "$incident_file" schema)" != "boos.evidence.incident.v1" ]; then
    echo "incident input must use schema boos.evidence.incident.v1" >&2
    exit 1
fi
if [ "$(record_value "$incident_file" privacy)" != public ]; then
    echo "incident is not publishable: privacy must be public" >&2
    exit 1
fi

echo "incident is publishable"
