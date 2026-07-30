#!/bin/sh
set -eu

evidence_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
. "$evidence_dir/lib/record.sh"

registration_file="${1:?usage: verify-result.sh <registration.kv> <result.kv>}"
result_file="${2:?usage: verify-result.sh <registration.kv> <result.kv>}"

"$evidence_dir/validate-record.sh" "$registration_file" >/dev/null
"$evidence_dir/validate-record.sh" "$result_file" >/dev/null

for field in registration_id case_bundle_sha256 evaluator_version; do
    registration_value="$(record_value "$registration_file" "$field")"
    result_value="$(record_value "$result_file" "$field")"
    if [ "$registration_value" != "$result_value" ]; then
        echo "registration/result mismatch: $field" >&2
        exit 1
    fi
done

repo_root="$(CDPATH= cd -P -- "$evidence_dir/../.." && pwd)"

resolve_regular_artifact() {
    artifact_label="$1"
    artifact_path="$2"
    candidate_path="$repo_root/$artifact_path"
    artifact_directory="$(dirname -- "$candidate_path")"
    physical_directory="$(CDPATH= cd -P -- "$artifact_directory" 2>/dev/null && pwd)" || {
        echo "$artifact_label file not found: $artifact_path" >&2
        return 1
    }
    physical_path="$physical_directory/$(basename -- "$candidate_path")"

    # This local validator assumes no concurrent untrusted mutation between these path checks and the SHA-256 reads below.
    case "$physical_path" in
        "$repo_root"/*) ;;
        *)
            echo "$artifact_label path is unsafe: $artifact_path" >&2
            return 1
            ;;
    esac
    if [ -L "$physical_path" ]; then
        echo "$artifact_label path is unsafe: $artifact_path" >&2
        return 1
    fi
    if [ ! -f "$physical_path" ]; then
        echo "$artifact_label file not found: $artifact_path" >&2
        return 1
    fi
    printf '%s\n' "$physical_path"
}

trace_path="$(record_value "$result_file" trace_path)"
outcomes_path="$(record_value "$result_file" primary_outcomes_path)"
trace_file="$(resolve_regular_artifact trace "$trace_path")"
outcomes_file="$(resolve_regular_artifact "primary outcomes" "$outcomes_path")"

expected_trace_sha256="$(record_value "$result_file" trace_sha256)"
actual_trace_sha256="$(sha256_file "$trace_file")"
if [ "$actual_trace_sha256" != "$expected_trace_sha256" ]; then
    echo "trace SHA-256 mismatch" >&2
    exit 1
fi

expected_outcomes_sha256="$(record_value "$result_file" primary_outcomes_sha256)"
actual_outcomes_sha256="$(sha256_file "$outcomes_file")"
if [ "$actual_outcomes_sha256" != "$expected_outcomes_sha256" ]; then
    echo "primary outcomes SHA-256 mismatch" >&2
    exit 1
fi

echo "verified evidence result"
