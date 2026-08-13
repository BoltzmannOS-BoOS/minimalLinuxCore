#!/bin/sh
set -eu

evidence_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
. "$evidence_dir/lib/record.sh"
validator="$evidence_dir/validate-record.sh"

usage() {
    cat >&2 <<'EOF'
usage: check-frontier-eligibility.sh <registration.kv> <target_level> <target_id> [contamination.kv ...]

Exits 0 only when no supplied valid contamination record matches the exact
registration/target tuple, 1 when an exact supplied record matches and retires
that tuple, or 2 for invalid input or an invalid contamination record (fail
closed). Exit 0 does not establish target membership or frontier eligibility.
EOF
}

if [ "$#" -lt 3 ]; then
    usage
    exit 2
fi

registration_file="$1"
target_level="$2"
target_id="$3"
shift 3

if [ -z "$target_id" ]; then
    echo "target ID must not be empty" >&2
    usage
    exit 2
fi

case "$target_level" in
    case|family|metric) ;;
    *)
        echo "invalid target level: $target_level" >&2
        usage
        exit 2
        ;;
esac

if ! "$validator" "$registration_file" >/dev/null; then
    echo "invalid registration reference: $registration_file" >&2
    exit 2
fi
if [ "$(record_value "$registration_file" schema)" != "boos.evidence.registration.v1" ]; then
    echo "registration reference must be a registration record: $registration_file" >&2
    exit 2
fi

registration_id="$(record_value "$registration_file" registration_id)"
for contamination_file in "$@"; do
    if ! "$validator" "$contamination_file" >/dev/null; then
        echo "invalid contamination record: $contamination_file" >&2
        exit 2
    fi
    if [ "$(record_value "$contamination_file" schema)" != "boos.evidence.contamination.v1" ]; then
        echo "contamination input must be a contamination record: $contamination_file" >&2
        exit 2
    fi
done

for contamination_file in "$@"; do
    if [ "$(record_value "$contamination_file" registration_id)" = "$registration_id" ] && \
        [ "$(record_value "$contamination_file" target_level)" = "$target_level" ] && \
        [ "$(record_value "$contamination_file" target_id)" = "$target_id" ]; then
        echo "matching supplied contamination record retires $registration_id:$target_level:$target_id" >&2
        exit 1
    fi
done

echo "no matching supplied contamination record: $registration_id:$target_level:$target_id; target membership and frontier status are not established"
