#!/bin/sh

fail_record() {
    echo "$1" >&2
    return 1
}

record_value() {
    record_file="$1"
    record_key="$2"
    awk -v wanted="$record_key" '
        index($0, wanted "=") == 1 {
            print substr($0, length(wanted) + 2)
        }
    ' "$record_file"
}

require_keys() {
    record_file="$1"
    required_keys="$2"
    for record_key in $required_keys; do
        count="$(awk -F= -v wanted="$record_key" '$1 == wanted { count += 1 } END { print count + 0 }' "$record_file")"
        if [ "$count" -eq 0 ]; then
            fail_record "missing required field: $record_key"
            return 1
        fi
        if [ "$count" -ne 1 ]; then
            fail_record "duplicate field: $record_key"
            return 1
        fi
        if [ -z "$(record_value "$record_file" "$record_key")" ]; then
            fail_record "empty required field: $record_key"
            return 1
        fi
    done
}

reject_unknown_keys() {
    record_file="$1"
    allowed_keys=" $2 "
    while IFS= read -r line || [ -n "$line" ]; do
        case "$line" in
            ""|\#*) continue ;;
            *=*) ;;
            *)
                fail_record "malformed record line"
                return 1
                ;;
        esac
        record_key="${line%%=*}"
        case "$record_key" in
            ""|*[!a-z0-9_]*)
                fail_record "invalid field name: $record_key"
                return 1
                ;;
        esac
        case "$allowed_keys" in
            *" $record_key "*) ;;
            *)
                fail_record "unknown field: $record_key"
                return 1
                ;;
        esac
    done <"$record_file"
}

require_schema() {
    record_file="$1"
    expected_schema="$2"
    actual_schema="$(record_value "$record_file" schema)"
    if [ "$actual_schema" != "$expected_schema" ]; then
        fail_record "unsupported schema: $actual_schema"
        return 1
    fi
}

require_enum() {
    record_file="$1"
    record_key="$2"
    allowed_values=" $3 "
    actual_value="$(record_value "$record_file" "$record_key")"
    case "$allowed_values" in
        *" $actual_value "*) ;;
        *)
            fail_record "invalid $record_key: $actual_value"
            return 1
            ;;
    esac
}

require_boolean() {
    require_enum "$1" "$2" "true false"
}

require_nonnegative_integer() {
    actual_value="$(record_value "$1" "$2")"
    case "$actual_value" in
        ""|*[!0-9]*)
            fail_record "$2 must be a non-negative integer"
            return 1
            ;;
    esac
}

require_positive_integer() {
    require_nonnegative_integer "$1" "$2" || return 1
    actual_value="$(record_value "$1" "$2")"
    if [ "$actual_value" -eq 0 ]; then
        fail_record "$2 must be a positive integer"
        return 1
    fi
}

require_sha256() {
    actual_value="$(record_value "$1" "$2")"
    case "$actual_value" in
        *[!0-9a-f]*)
            fail_record "$2 must be a lowercase SHA-256"
            return 1
            ;;
    esac
    if [ "${#actual_value}" -ne 64 ]; then
        fail_record "$2 must be a lowercase SHA-256"
        return 1
    fi
}

require_git_commit() {
    actual_value="$(record_value "$1" "$2")"
    case "$actual_value" in
        *[!0-9a-f]*)
            fail_record "$2 must be a lowercase 40-character Git commit"
            return 1
            ;;
    esac
    if [ "${#actual_value}" -ne 40 ]; then
        fail_record "$2 must be a lowercase 40-character Git commit"
        return 1
    fi
}

require_relative_path_or_none() {
    actual_value="$(record_value "$1" "$2")"
    case "$actual_value" in
        none) ;;
        /*|../*|*/../*|*/..)
            fail_record "$2 must be a repository-relative path or none"
            return 1
            ;;
        tests/evidence/*|tests/research/*) ;;
        *)
            fail_record "$2 must stay inside an evidence directory"
            return 1
            ;;
    esac
}

require_relative_path() {
    require_relative_path_or_none "$1" "$2" || return 1
    if [ "$(record_value "$1" "$2")" = "none" ]; then
        fail_record "$2 must be a repository-relative path"
        return 1
    fi
}

sha256_file() {
    file_path="$1"
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$file_path" | awk '{ print $1 }'
    elif command -v shasum >/dev/null 2>&1; then
        shasum -a 256 "$file_path" | awk '{ print $1 }'
    else
        fail_record "no SHA-256 command available"
        return 1
    fi
}
