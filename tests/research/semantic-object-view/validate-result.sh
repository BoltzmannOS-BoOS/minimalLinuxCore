#!/bin/sh
set -eu

result_file="${1:?usage: validate-result.sh <result.kv>}"
required_keys="
schema
run_id
variant
model
model_version
temperature
boos_commit
task_set
prompt_path
trace_path
trace_sha256
completed_tasks
total_tasks
environment_interactions
observation_bytes
incorrect_capability_assumptions
invalid_command_attempts
skipped_verifications
"

value_for() {
    key="$1"
    sed -n "s/^${key}=//p" "$result_file" | head -n 1
}

for key in $required_keys; do
    value="$(value_for "$key")"
    if [ -z "$value" ]; then
        echo "missing required field: $key" >&2
        exit 1
    fi
done

if [ "$(value_for schema)" != "boos.semantic-object-experiment.v1" ]; then
    echo "unsupported result schema" >&2
    exit 1
fi

case "$(value_for variant)" in
    baseline|object) ;;
    *)
        echo "variant must be baseline or object" >&2
        exit 1
        ;;
esac

numeric_keys="
completed_tasks
total_tasks
environment_interactions
observation_bytes
incorrect_capability_assumptions
invalid_command_attempts
skipped_verifications
"

for key in $numeric_keys; do
    value="$(value_for "$key")"
    case "$value" in
        *[!0-9]*)
            echo "$key must be a non-negative integer" >&2
            exit 1
            ;;
    esac
done

echo "valid semantic object experiment result"
