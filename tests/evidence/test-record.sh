#!/bin/sh
set -eu

evidence_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
. "$evidence_dir/lib/record.sh"
temporary_dir="$(mktemp -d)"
trap 'rm -rf "$temporary_dir"' EXIT HUP INT TERM

expect_failure() {
    description="$1"
    shift
    if "$@" >"$temporary_dir/stdout" 2>"$temporary_dir/stderr"; then
        echo "expected failure: $description" >&2
        exit 1
    fi
}

cat >"$temporary_dir/valid.kv" <<'EOF'
schema=boos.evidence.test.v1
name=value=with=equals
enabled=true
count=0
digest=0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef
commit=0123456789abcdef0123456789abcdef01234567
path=tests/evidence/example.txt
EOF

test "$(record_value "$temporary_dir/valid.kv" name)" = "value=with=equals"
require_schema "$temporary_dir/valid.kv" "boos.evidence.test.v1"
require_keys "$temporary_dir/valid.kv" \
    "schema name enabled count digest commit path"
reject_unknown_keys "$temporary_dir/valid.kv" \
    "schema name enabled count digest commit path"
require_boolean "$temporary_dir/valid.kv" enabled
require_nonnegative_integer "$temporary_dir/valid.kv" count
require_sha256 "$temporary_dir/valid.kv" digest
require_git_commit "$temporary_dir/valid.kv" commit
require_relative_path_or_none "$temporary_dir/valid.kv" path
require_relative_path "$temporary_dir/valid.kv" path

cat >"$temporary_dir/duplicate.kv" <<'EOF'
schema=boos.evidence.test.v1
name=first
name=second
EOF
expect_failure "duplicate keys" \
    require_keys "$temporary_dir/duplicate.kv" "schema name"

cat >"$temporary_dir/missing-required.kv" <<'EOF'
schema=boos.evidence.test.v1
EOF
expect_failure "missing required key" \
    require_keys "$temporary_dir/missing-required.kv" "schema name"

cat >"$temporary_dir/unknown.kv" <<'EOF'
schema=boos.evidence.test.v1
name=value
surprise=value
EOF
expect_failure "unknown keys" \
    reject_unknown_keys "$temporary_dir/unknown.kv" "schema name"

cat >"$temporary_dir/invalid-boolean.kv" <<'EOF'
enabled=yes
EOF
expect_failure "invalid boolean" \
    require_boolean "$temporary_dir/invalid-boolean.kv" enabled

cat >"$temporary_dir/invalid-nonnegative-integer.kv" <<'EOF'
count=-1
EOF
expect_failure "invalid non-negative integer" \
    require_nonnegative_integer "$temporary_dir/invalid-nonnegative-integer.kv" count

cat >"$temporary_dir/invalid-sha256.kv" <<'EOF'
digest=0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdeF
EOF
expect_failure "invalid SHA-256" \
    require_sha256 "$temporary_dir/invalid-sha256.kv" digest

cat >"$temporary_dir/invalid-git-commit.kv" <<'EOF'
commit=0123456789abcdef0123456789abcdef0123456g
EOF
expect_failure "invalid Git commit" \
    require_git_commit "$temporary_dir/invalid-git-commit.kv" commit

cat >"$temporary_dir/absolute-path.kv" <<'EOF'
schema=boos.evidence.test.v1
path=/tmp/evidence
EOF
expect_failure "absolute evidence path" \
    require_relative_path_or_none "$temporary_dir/absolute-path.kv" path

cat >"$temporary_dir/zero.kv" <<'EOF'
schema=boos.evidence.test.v1
count=0
EOF
expect_failure "positive integer is zero" \
    require_positive_integer "$temporary_dir/zero.kv" count

cat >"$temporary_dir/no-path.kv" <<'EOF'
schema=boos.evidence.test.v1
path=none
EOF
expect_failure "required path is none" \
    require_relative_path "$temporary_dir/no-path.kv" path

cat >"$temporary_dir/malformed-line.kv" <<'EOF'
schema=boos.evidence.test.v1
not-a-record-line
EOF
expect_failure "line without equals" \
    reject_unknown_keys "$temporary_dir/malformed-line.kv" "schema"

expect_failure "unknown dispatcher schema" \
    "$evidence_dir/validate-record.sh" "$temporary_dir/valid.kv"

echo "evidence record foundation tests passed"
