# BoOS Living Evidence System Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the first, model-free evidence infrastructure slice: strict versioned records, validators that prove detection power against broken fixtures, one solution-neutral field incident, one bounded unsupported claim, public evidence indexes, and an explicit Test 0 classification.

**Architecture:** POSIX shell validators implement framework-light L0/L1 evidence contracts under `tests/evidence/`; each record type owns one validator and focused fixtures. A thin L3 dispatcher selects validators by schema, while repository documentation separates regression, frontier, field, and anchor evidence. No BoOS runtime module imports benchmark code, and no model runner or product feature is added.

**Tech Stack:** POSIX `sh`, `awk`, `sed`, `grep`, existing `key=value` repository conventions, Git, Markdown.

## Global Constraints

- Do not change BoOS runtime behavior, capability configuration, command registration, rootfs contents, or Rust source.
- Do not implement skill sharing, semantic filtering, a model runner, task generator, frontier plaintext storage, or a comparative benchmark.
- Tests need expected outcomes; frontier eligibility must not be derived from a feature specification.
- Regression success protects known behavior but supplies no fresh architectural evidence.
- No finite suite supports the conclusion that the program has no problems.
- Unexposed frontier plaintext must never be stored in the model-readable working tree.
- All validators must reject duplicate keys, unknown keys, missing required fields, malformed enums, and malformed hashes where relevant.
- All scripts must be POSIX `sh`, run from any working directory, and avoid new dependencies.
- Record values are single-line UTF-8 text. Parsers split only on the first `=`.
- Every intentionally invalid fixture must remain invalid for one named reason.
- Existing user-owned untracked files remain untouched.

---

## File Structure

### Shared validation

- Create `tests/evidence/lib/record.sh` — common L0 key/value parsing and validation assertions.
- Create `tests/evidence/validate-record.sh` — L3 schema dispatcher for one record file.
- Create `tests/evidence/test-record.sh` — behavior tests for common parser invariants and dispatch failures.

### Record validators

- Create `tests/evidence/validators/incident.sh` — field-incident schema and enums.
- Create `tests/evidence/validators/claim.sh` — bounded claim schema and evidence status.
- Create `tests/evidence/validators/registration.sh` — sealed frontier registration schema.
- Create `tests/evidence/validators/contamination.sh` — case/family/metric contamination schema.
- Create `tests/evidence/validators/result.sh` — revealed result schema.

### Focused tests and fixtures

- Create `tests/evidence/test-incident.sh`.
- Create `tests/evidence/test-claim.sh`.
- Create `tests/evidence/test-registration.sh`.
- Create `tests/evidence/test-contamination.sh`.
- Create `tests/evidence/test-result.sh`.
- Create `tests/evidence/test-all.sh` — runs every focused test from any directory.
- Create `tests/evidence/fixtures/valid/*.kv` — one valid example per schema.
- Create `tests/evidence/fixtures/invalid/*.kv` — named single-defect fixtures.

### Current evidence

- Create `tests/evidence/incidents/current/2026-07-30-parallel-project-context-friction.kv` — first solution-neutral field incident.
- Create `tests/evidence/claims/context-skill-system-boundary.kv` — explicitly unsupported system-boundary claim.
- Create `tests/evidence/validate-tree.sh` — validates publishable current records, never invalid fixtures.

### Reader indexes

- Create `tests/evidence/README.md` — short evidence-system index and claim limits.
- Create `tests/evidence/regression/README.md` — known/exposed test index.
- Create `tests/evidence/frontier/README.md` — metadata-only frontier policy; no plaintext tasks.
- Create `tests/evidence/incidents/README.md`.
- Create `tests/evidence/claims/README.md`.

### Existing documentation

- Modify `tests/research/semantic-object-view/README.md` — classify the protocol as Test 0 without changing frozen tasks, metrics, or prompts.
- Modify `tests/research/semantic-object-view/runs/2026-07-30-pair-001/README.md` — replace the overbroad direction claim with a bounded wiring conclusion.
- Modify `README.md` — link the Living Evidence System and bound Test 0.
- Modify `docs/PROJECT-OVERVIEW.md` — record that the OS-boundary claim remains unsupported.

---

## Spec Coverage

| Approved first-slice requirement | Plan task |
|---|---|
| Relabel semantic-object experiment as Test 0 | Task 8 |
| Add strict incident contract and first field incident | Task 2 |
| Add bounded claim contract and unsupported current claim | Task 3 |
| Add sealed registration contract | Task 4 |
| Add case/family/metric contamination contract | Task 5 |
| Add immutable result and artifact-integrity checks | Task 6 |
| Qualify validators with intentionally invalid fixtures | Tasks 1–6 |
| Add regression and frontier metadata indexes | Task 7 |
| Preserve raw Test 0 tasks, metrics, traces, and result records | Task 8 |
| Avoid product/runtime changes and paid model calls | Global constraints and Task 8 scope audit |

---

### Task 1: Strict key/value validation foundation

**Files:**
- Create: `tests/evidence/lib/record.sh`
- Create: `tests/evidence/validate-record.sh`
- Create: `tests/evidence/test-record.sh`

**Interfaces:**
- Produces: `record_value <file> <key>` prints the complete value after the first `=`.
- Produces: `require_schema <file> <schema>`.
- Produces: `require_keys <file> "<space separated keys>"`.
- Produces: `reject_unknown_keys <file> "<space separated keys>"`.
- Produces: `require_enum <file> <key> "<space separated values>"`.
- Produces: `require_boolean <file> <key>`.
- Produces: `require_nonnegative_integer <file> <key>`.
- Produces: `require_positive_integer <file> <key>`.
- Produces: `require_sha256 <file> <key>`.
- Produces: `require_git_commit <file> <key>`.
- Produces: `require_relative_path <file> <key>`.
- Produces: `require_relative_path_or_none <file> <key>`.
- Produces: `sha256_file <path>` with Linux and macOS command support.
- Produces: `fail_record <message>` writes one error to stderr and exits nonzero.
- Consumes: no product code or external dependency.

- [ ] **Step 1: Write the common-validator tests**

Create `tests/evidence/test-record.sh` with a temporary directory and these
cases:

```sh
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

cat >"$temporary_dir/unknown.kv" <<'EOF'
schema=boos.evidence.test.v1
name=value
surprise=value
EOF
expect_failure "unknown keys" \
    reject_unknown_keys "$temporary_dir/unknown.kv" "schema name"

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
```

- [ ] **Step 2: Run the test to verify RED**

Run:

```sh
sh tests/evidence/test-record.sh
```

Expected: FAIL because `tests/evidence/lib/record.sh` does not exist.

- [ ] **Step 3: Implement the common validation library**

Create `tests/evidence/lib/record.sh`:

```sh
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
```

Create the initial `tests/evidence/validate-record.sh` dispatcher:

```sh
#!/bin/sh
set -eu

evidence_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
. "$evidence_dir/lib/record.sh"

record_file="${1:?usage: validate-record.sh <record.kv>}"
test -f "$record_file" || {
    echo "record not found: $record_file" >&2
    exit 1
}

schema="$(record_value "$record_file" schema)"
case "$schema" in
    *)
        echo "unsupported evidence schema: $schema" >&2
        exit 1
        ;;
esac
```

Make both scripts executable.

- [ ] **Step 4: Run the focused test to verify GREEN**

Run:

```sh
sh tests/evidence/test-record.sh
```

Expected: `evidence record foundation tests passed`.

- [ ] **Step 5: Commit the foundation**

```sh
git add tests/evidence/lib/record.sh \
  tests/evidence/validate-record.sh \
  tests/evidence/test-record.sh
git commit -m "test: add strict evidence record foundation"
```

---

### Task 2: Field incident contract and first real incident

**Files:**
- Create: `tests/evidence/validators/incident.sh`
- Create: `tests/evidence/test-incident.sh`
- Create: `tests/evidence/fixtures/valid/incident.kv`
- Create: `tests/evidence/fixtures/invalid/incident-missing-consequence.kv`
- Create: `tests/evidence/fixtures/invalid/incident-invalid-source.kv`
- Create: `tests/evidence/incidents/current/2026-07-30-parallel-project-context-friction.kv`
- Create: `tests/evidence/incidents/README.md`
- Modify: `tests/evidence/validate-record.sh`

**Interfaces:**
- Consumes: Task 1 common assertions.
- Produces: schema `boos.evidence.incident.v1`.
- Produces required keys: `schema incident_id observed_on source_kind goal starting_conditions observed_failure consequence human_workaround observed_facts inferences evidence_path privacy status`.
- Does not claim that structural validation can establish construct validity or solution neutrality.

- [ ] **Step 1: Write incident RED tests and fixtures**

The valid fixture uses:

```text
schema=boos.evidence.incident.v1
incident_id=incident-example-valid
observed_on=2026-07-30
source_kind=field
goal=Keep useful project knowledge available across simultaneous AI-assisted work while preserving project-specific context.
starting_conditions=Two project sessions operate independently with separate context and skill state.
observed_failure=Useful changes must be repeated manually, while forced centralization risks leaking project-specific instructions.
consequence=Repeated work, inconsistent quality, and uncertainty about which knowledge should cross project boundaries.
human_workaround=Copy or reference shared files manually and inspect each project separately.
observed_facts=The user reported repeated work and tension between sharing and isolation across concurrent projects.
inferences=The responsible layer and required mechanism are not yet established.
evidence_path=none
privacy=public
status=normalized
```

`incident-missing-consequence.kv` removes `consequence`.

`incident-invalid-source.kv` copies the valid fixture but sets
`source_kind=feature_spec`, which is outside the accepted provenance enum.

Create `tests/evidence/test-incident.sh`:

```sh
#!/bin/sh
set -eu

evidence_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
validator="$evidence_dir/validate-record.sh"
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

valid_fixture="$evidence_dir/fixtures/valid/incident.kv"
missing_consequence="$evidence_dir/fixtures/invalid/incident-missing-consequence.kv"
invalid_source="$evidence_dir/fixtures/invalid/incident-invalid-source.kv"

"$validator" "$valid_fixture" >/dev/null
expect_failure "missing incident consequence" \
    "$validator" "$missing_consequence"
expect_failure "feature specification is not incident provenance" \
    "$validator" "$invalid_source"

echo "evidence incident validator tests passed"
```

- [ ] **Step 2: Run the incident test to verify RED**

Run:

```sh
sh tests/evidence/test-incident.sh
```

Expected: FAIL with `unsupported evidence schema`.

- [ ] **Step 3: Implement incident validation and dispatch**

Create `tests/evidence/validators/incident.sh`:

```sh
validate_incident() {
    record_file="$1"
    keys="schema incident_id observed_on source_kind goal starting_conditions observed_failure consequence human_workaround observed_facts inferences evidence_path privacy status"
    require_keys "$record_file" "$keys"
    reject_unknown_keys "$record_file" "$keys"
    require_schema "$record_file" "boos.evidence.incident.v1"
    require_enum "$record_file" source_kind "field external synthetic"
    require_enum "$record_file" privacy "public private"
    require_enum "$record_file" status "observed reproduced normalized"
    require_relative_path_or_none "$record_file" evidence_path
}
```

Add this source line after `record.sh` in `validate-record.sh`:

```sh
. "$evidence_dir/validators/incident.sh"
```

Add this exact case branch:

```sh
boos.evidence.incident.v1)
    validate_incident "$record_file"
    echo "valid evidence incident"
    ;;
```

Create the current incident with the same solution-neutral content as the valid
fixture but `incident_id=parallel-project-context-friction-2026-07-30`.

Create `tests/evidence/incidents/README.md` with four statements:

- incidents record observed problems, not requested features;
- facts and inferences remain separate;
- structural validation cannot prove solution neutrality; independent semantic
  review is still required;
- public records contain no raw private transcript;
- `current/` holds normalized incidents; later archives are dated.

- [ ] **Step 4: Run incident and foundation tests to verify GREEN**

Run:

```sh
sh tests/evidence/test-record.sh
sh tests/evidence/test-incident.sh
tests/evidence/validate-record.sh \
  tests/evidence/incidents/current/2026-07-30-parallel-project-context-friction.kv
```

Expected: both tests pass; current incident reports `valid evidence incident`.

- [ ] **Step 5: Commit the incident contract**

```sh
git add tests/evidence/validators/incident.sh \
  tests/evidence/test-incident.sh \
  tests/evidence/fixtures/valid/incident.kv \
  tests/evidence/fixtures/invalid/incident-missing-consequence.kv \
  tests/evidence/fixtures/invalid/incident-invalid-source.kv \
  tests/evidence/incidents \
  tests/evidence/validate-record.sh
git commit -m "test: capture solution-neutral field incidents"
```

---

### Task 3: Bounded claim contract

**Files:**
- Create: `tests/evidence/validators/claim.sh`
- Create: `tests/evidence/test-claim.sh`
- Create: `tests/evidence/fixtures/valid/claim.kv`
- Create: `tests/evidence/fixtures/invalid/claim-missing-scope.kv`
- Create: `tests/evidence/fixtures/invalid/claim-invalid-status.kv`
- Create: `tests/evidence/claims/context-skill-system-boundary.kv`
- Create: `tests/evidence/claims/README.md`
- Modify: `tests/evidence/validate-record.sh`

**Interfaces:**
- Consumes: Task 1 common assertions.
- Produces: schema `boos.evidence.claim.v1`.
- Produces statuses: `unsupported exploratory supported_within_scope contradicted stale`.
- Produces a current claim whose status is `unsupported`; the record does not name a winning implementation.

- [ ] **Step 1: Write claim RED tests**

Use this valid fixture shape:

```text
schema=boos.evidence.claim.v1
claim_id=claim-example-valid
statement=A dedicated system boundary improves outcomes for a defined external problem distribution.
scope=Only the registered problem distribution, models, environments, and implementation versions.
excluded_scope=Universal program correctness and unregistered workflows.
problem_distribution=unregistered
required_evidence=regression,frontier,field
primary_outcomes=correctness,safety,human_intervention
decision_rule=Remain unsupported until registered frontier and corroborating field evidence exist.
known_counterevidence=none
benchmark_versions=none
implementation_versions=none
status=unsupported
expiry_condition=Any environment or implementation version outside the registered evidence.
```

Create one fixture without `scope` and one with `status=proven`.

Create `tests/evidence/test-claim.sh`:

```sh
#!/bin/sh
set -eu

evidence_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
validator="$evidence_dir/validate-record.sh"
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

"$validator" "$evidence_dir/fixtures/valid/claim.kv" >/dev/null
expect_failure "missing claim scope" \
    "$validator" "$evidence_dir/fixtures/invalid/claim-missing-scope.kv"
expect_failure "invalid claim status" \
    "$validator" "$evidence_dir/fixtures/invalid/claim-invalid-status.kv"

echo "evidence claim validator tests passed"
```

- [ ] **Step 2: Run the claim test to verify RED**

Run:

```sh
sh tests/evidence/test-claim.sh
```

Expected: FAIL with `unsupported evidence schema`.

- [ ] **Step 3: Implement claim validation and current claim**

Create `tests/evidence/validators/claim.sh`:

```sh
validate_claim() {
    record_file="$1"
    keys="schema claim_id statement scope excluded_scope problem_distribution required_evidence primary_outcomes decision_rule known_counterevidence benchmark_versions implementation_versions status expiry_condition"
    require_keys "$record_file" "$keys"
    reject_unknown_keys "$record_file" "$keys"
    require_schema "$record_file" "boos.evidence.claim.v1"
    require_enum "$record_file" status \
        "unsupported exploratory supported_within_scope contradicted stale"
}
```

Add this source line after the other validator source lines:

```sh
. "$evidence_dir/validators/claim.sh"
```

Add this exact dispatcher branch:

```sh
boos.evidence.claim.v1)
    validate_claim "$record_file"
    echo "valid evidence claim"
    ;;
```

Create `tests/evidence/claims/context-skill-system-boundary.kv`:

```text
schema=boos.evidence.claim.v1
claim_id=context-skill-system-boundary
statement=An operating-system control boundary is necessary to solve cross-project context and skill coordination.
scope=Concurrent AI-assisted project work represented by future registered problem distributions.
excluded_scope=General multi-agent quality, all context management, and universal operating-system necessity.
problem_distribution=unregistered
required_evidence=regression,frontier,field
primary_outcomes=task_correctness,isolation,human_intervention,recovery
decision_rule=Remain unsupported until non-feature-derived frontier evidence and corroborating field evidence distinguish candidate boundaries.
known_counterevidence=Existing workflows use ordinary files and manual coordination; no comparative field evidence shows an operating-system boundary is necessary.
benchmark_versions=none
implementation_versions=none
status=unsupported
expiry_condition=Any conclusion applied beyond the exact registered distribution, models, environment, or implementation versions.
```

`tests/evidence/claims/README.md` must state that a claim record:

- bounds rather than proves a claim;
- cannot use regression success as fresh support;
- becomes stale outside registered versions;
- preserves counterevidence.

- [ ] **Step 4: Run claim and foundation tests to verify GREEN**

Run:

```sh
sh tests/evidence/test-record.sh
sh tests/evidence/test-claim.sh
tests/evidence/validate-record.sh \
  tests/evidence/claims/context-skill-system-boundary.kv
```

Expected: tests pass; current claim reports `valid evidence claim`.

- [ ] **Step 5: Commit the claim contract**

```sh
git add tests/evidence/validators/claim.sh \
  tests/evidence/test-claim.sh \
  tests/evidence/fixtures/valid/claim.kv \
  tests/evidence/fixtures/invalid/claim-missing-scope.kv \
  tests/evidence/fixtures/invalid/claim-invalid-status.kv \
  tests/evidence/claims \
  tests/evidence/validate-record.sh
git commit -m "test: add bounded evidence claims"
```

---

### Task 4: Sealed registration contract

**Files:**
- Create: `tests/evidence/validators/registration.sh`
- Create: `tests/evidence/test-registration.sh`
- Create: `tests/evidence/fixtures/valid/registration.kv`
- Create: `tests/evidence/fixtures/invalid/registration-unsealed.kv`
- Create: `tests/evidence/fixtures/invalid/registration-bad-digest.kv`
- Create: `tests/evidence/fixtures/invalid/registration-zero-cases.kv`
- Modify: `tests/evidence/validate-record.sh`

**Interfaces:**
- Consumes: Task 1 common assertions.
- Produces: schema `boos.evidence.registration.v1`.
- Produces: only preregistered `exposure_status=sealed`.
- Does not create or store a frontier plaintext bundle.

- [ ] **Step 1: Write registration RED tests**

The valid fixture contains:

```text
schema=boos.evidence.registration.v1
registration_id=registration-example-valid
registered_at=2026-07-30T12:00:00+08:00
distribution_version=problem-distribution.v1
evaluator_version=evaluator.v1
case_batch_id=batch-example-sealed
case_count=12
case_bundle_sha256=0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef
generator_version=generator.v1
family_weights_sha256=123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0
analysis_method=paired raw primary outcomes with no post-result reweighting
model_provider=example-provider
model_version=example-model
implementation_commit=0123456789abcdef0123456789abcdef01234567
environment_sha256=23456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef01
token_budget=100000
interaction_budget=100
wall_clock_seconds=3600
retry_budget=0
exposure_status=sealed
```

The unsealed fixture changes `exposure_status` to `revealed`. The bad-digest
fixture shortens `case_bundle_sha256`. The zero-cases fixture sets
`case_count=0`.

Create `tests/evidence/test-registration.sh`:

```sh
#!/bin/sh
set -eu

evidence_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
validator="$evidence_dir/validate-record.sh"
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

"$validator" "$evidence_dir/fixtures/valid/registration.kv" >/dev/null
expect_failure "registration is not sealed" \
    "$validator" "$evidence_dir/fixtures/invalid/registration-unsealed.kv"
expect_failure "registration case digest is malformed" \
    "$validator" "$evidence_dir/fixtures/invalid/registration-bad-digest.kv"
expect_failure "registration has no cases" \
    "$validator" "$evidence_dir/fixtures/invalid/registration-zero-cases.kv"

echo "evidence registration validator tests passed"
```

- [ ] **Step 2: Run the registration test to verify RED**

Run:

```sh
sh tests/evidence/test-registration.sh
```

Expected: FAIL with `unsupported evidence schema`.

- [ ] **Step 3: Implement registration validation**

Create `tests/evidence/validators/registration.sh`:

```sh
validate_registration() {
    record_file="$1"
    keys="schema registration_id registered_at distribution_version evaluator_version case_batch_id case_count case_bundle_sha256 generator_version family_weights_sha256 analysis_method model_provider model_version implementation_commit environment_sha256 token_budget interaction_budget wall_clock_seconds retry_budget exposure_status"
    require_keys "$record_file" "$keys"
    reject_unknown_keys "$record_file" "$keys"
    require_schema "$record_file" "boos.evidence.registration.v1"
    for record_key in case_count token_budget interaction_budget wall_clock_seconds; do
        require_positive_integer "$record_file" "$record_key"
    done
    for record_key in retry_budget; do
        require_nonnegative_integer "$record_file" "$record_key"
    done
    for record_key in case_bundle_sha256 family_weights_sha256 environment_sha256; do
        require_sha256 "$record_file" "$record_key"
    done
    require_git_commit "$record_file" implementation_commit
    require_enum "$record_file" exposure_status "sealed"
}
```

Add this source line after the other validator source lines:

```sh
. "$evidence_dir/validators/registration.sh"
```

Add this exact dispatcher branch:

```sh
boos.evidence.registration.v1)
    validate_registration "$record_file"
    echo "valid evidence registration"
    ;;
```

Do not create a `frontier/current` plaintext directory.

- [ ] **Step 4: Run registration tests to verify GREEN**

Run:

```sh
sh tests/evidence/test-record.sh
sh tests/evidence/test-registration.sh
```

Expected: both pass.

- [ ] **Step 5: Commit the registration contract**

```sh
git add tests/evidence/validators/registration.sh \
  tests/evidence/test-registration.sh \
  tests/evidence/fixtures/valid/registration.kv \
  tests/evidence/fixtures/invalid/registration-unsealed.kv \
  tests/evidence/fixtures/invalid/registration-bad-digest.kv \
  tests/evidence/fixtures/invalid/registration-zero-cases.kv \
  tests/evidence/validate-record.sh
git commit -m "test: add sealed evidence registrations"
```

---

### Task 5: Contamination lifecycle contract

**Files:**
- Create: `tests/evidence/validators/contamination.sh`
- Create: `tests/evidence/test-contamination.sh`
- Create: `tests/evidence/fixtures/valid/contamination.kv`
- Create: `tests/evidence/fixtures/invalid/contamination-invalid-level.kv`
- Create: `tests/evidence/fixtures/invalid/contamination-frontier-disposition.kv`
- Modify: `tests/evidence/validate-record.sh`

**Interfaces:**
- Consumes: Task 1 common assertions.
- Produces: schema `boos.evidence.contamination.v1`.
- Produces levels: `case family metric`.
- Produces dispositions: `regression archive new_distribution new_evaluator claim_review`.
- Enforces that contamination can never retain `frontier` disposition.

- [ ] **Step 1: Write contamination RED tests**

Use this valid fixture:

```text
schema=boos.evidence.contamination.v1
contamination_id=contamination-example-valid
target_level=family
target_id=problem-family-example
detected_at=2026-07-30T12:30:00+08:00
trigger=used_for_design
evidence_path=none
disposition=new_distribution
effective_version=problem-distribution.v2
notes=Fresh seeds from the exposed generator no longer count as independent evidence.
```

The invalid-level fixture uses `target_level=implementation`. The invalid
disposition fixture uses `disposition=frontier`.

Create `tests/evidence/test-contamination.sh`:

```sh
#!/bin/sh
set -eu

evidence_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
validator="$evidence_dir/validate-record.sh"
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

"$validator" "$evidence_dir/fixtures/valid/contamination.kv" >/dev/null
expect_failure "invalid contamination level" \
    "$validator" "$evidence_dir/fixtures/invalid/contamination-invalid-level.kv"
expect_failure "contamination retained as frontier evidence" \
    "$validator" "$evidence_dir/fixtures/invalid/contamination-frontier-disposition.kv"

echo "evidence contamination validator tests passed"
```

- [ ] **Step 2: Run the contamination test to verify RED**

Run:

```sh
sh tests/evidence/test-contamination.sh
```

Expected: FAIL with `unsupported evidence schema`.

- [ ] **Step 3: Implement contamination validation**

Create `tests/evidence/validators/contamination.sh`:

```sh
validate_contamination() {
    record_file="$1"
    keys="schema contamination_id target_level target_id detected_at trigger evidence_path disposition effective_version notes"
    require_keys "$record_file" "$keys"
    reject_unknown_keys "$record_file" "$keys"
    require_schema "$record_file" "boos.evidence.contamination.v1"
    require_enum "$record_file" target_level "case family metric"
    require_enum "$record_file" trigger \
        "exposed used_for_design published result_inspected near_duplicate proxy_decoupled"
    require_enum "$record_file" disposition \
        "regression archive new_distribution new_evaluator claim_review"
    require_relative_path_or_none "$record_file" evidence_path
}
```

Add this source line after the other validator source lines:

```sh
. "$evidence_dir/validators/contamination.sh"
```

Add this exact dispatcher branch:

```sh
boos.evidence.contamination.v1)
    validate_contamination "$record_file"
    echo "valid evidence contamination record"
    ;;
```

- [ ] **Step 4: Run contamination tests to verify GREEN**

Run:

```sh
sh tests/evidence/test-record.sh
sh tests/evidence/test-contamination.sh
```

Expected: both pass.

- [ ] **Step 5: Commit the contamination contract**

```sh
git add tests/evidence/validators/contamination.sh \
  tests/evidence/test-contamination.sh \
  tests/evidence/fixtures/valid/contamination.kv \
  tests/evidence/fixtures/invalid/contamination-invalid-level.kv \
  tests/evidence/fixtures/invalid/contamination-frontier-disposition.kv \
  tests/evidence/validate-record.sh
git commit -m "test: model evidence contamination lifecycle"
```

---

### Task 6: Revealed result contract and registration consistency

**Files:**
- Create: `tests/evidence/validators/result.sh`
- Create: `tests/evidence/test-result.sh`
- Create: `tests/evidence/verify-result.sh`
- Create: `tests/evidence/fixtures/valid/result.kv`
- Create: `tests/evidence/fixtures/invalid/result-unrevealed.kv`
- Create: `tests/evidence/fixtures/invalid/result-bad-trace-hash.kv`
- Create: `tests/evidence/fixtures/invalid/result-pass-with-failure.kv`
- Modify: `tests/evidence/validate-record.sh`

**Interfaces:**
- Consumes: Task 1 common assertions and Task 4 registration schema.
- Produces: schema `boos.evidence.result.v1`.
- Produces: `verify-result.sh <registration.kv> <result.kv>`.
- Verifies matching `registration_id`, `case_bundle_sha256`, and `evaluator_version`.
- Verifies the actual trace and primary-outcome bytes against their recorded
  SHA-256 values.

- [ ] **Step 1: Write result RED tests**

Use this valid fixture:

```text
schema=boos.evidence.result.v1
result_id=result-example-valid
registration_id=registration-example-valid
subject_id=opaque-subject-01
status=inconclusive
case_bundle_sha256=0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef
evaluator_version=evaluator.v1
trace_path=tests/evidence/fixtures/valid/trace.example.txt
trace_sha256=276d9829518f945c56158b37f9d63e86335c83ad76b661cf2460676a1e497d4c
primary_outcomes_path=tests/evidence/fixtures/valid/outcomes.example.kv
primary_outcomes_sha256=91e6fa9e3550d04c19f9c9c4a0c1fe1dc36dd0efe73e4ff2f2ff6edaaf6a6105
failure_class=infrastructure
scored_at=2026-07-30T13:00:00+08:00
exposure_status=revealed
notes=Fixture result used only to validate record structure.
```

Create `trace.example.txt` with the exact bytes `fixture trace\n`. Create
`outcomes.example.kv` with the exact bytes `status=inconclusive\n`. The recorded
trace hash above is the SHA-256 of the exact trace bytes.

The unrevealed fixture changes `exposure_status=sealed`. The bad-hash fixture
shortens `trace_sha256`. The pass-with-failure fixture sets `status=pass` while
retaining `failure_class=infrastructure`.

Create `tests/evidence/test-result.sh`:

```sh
#!/bin/sh
set -eu

evidence_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
validator="$evidence_dir/validate-record.sh"
verifier="$evidence_dir/verify-result.sh"
registration="$evidence_dir/fixtures/valid/registration.kv"
valid_result="$evidence_dir/fixtures/valid/result.kv"
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

"$validator" "$valid_result" >/dev/null
expect_failure "result is not revealed" \
    "$validator" "$evidence_dir/fixtures/invalid/result-unrevealed.kv"
expect_failure "result trace hash is malformed" \
    "$validator" "$evidence_dir/fixtures/invalid/result-bad-trace-hash.kv"
expect_failure "passing result reports a failure class" \
    "$validator" "$evidence_dir/fixtures/invalid/result-pass-with-failure.kv"
"$verifier" "$registration" "$valid_result" >/dev/null

sed \
    's/^case_bundle_sha256=.*/case_bundle_sha256=ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff/' \
    "$valid_result" >"$temporary_dir/mismatched-bundle.kv"
expect_failure "registration and result bundle mismatch" \
    "$verifier" "$registration" "$temporary_dir/mismatched-bundle.kv"

sed \
    's/^trace_sha256=.*/trace_sha256=0000000000000000000000000000000000000000000000000000000000000000/' \
    "$valid_result" >"$temporary_dir/wrong-trace-content.kv"
expect_failure "recorded trace digest does not match trace bytes" \
    "$verifier" "$registration" "$temporary_dir/wrong-trace-content.kv"

sed \
    's/^primary_outcomes_sha256=.*/primary_outcomes_sha256=0000000000000000000000000000000000000000000000000000000000000000/' \
    "$valid_result" >"$temporary_dir/wrong-outcomes-content.kv"
expect_failure "recorded outcomes digest does not match outcome bytes" \
    "$verifier" "$registration" "$temporary_dir/wrong-outcomes-content.kv"

echo "evidence result validator tests passed"
```

- [ ] **Step 2: Run the result test to verify RED**

Run:

```sh
sh tests/evidence/test-result.sh
```

Expected: FAIL with `unsupported evidence schema`.

- [ ] **Step 3: Implement result validation and cross-record verification**

Create `tests/evidence/validators/result.sh`:

```sh
validate_result() {
    record_file="$1"
    keys="schema result_id registration_id subject_id status case_bundle_sha256 evaluator_version trace_path trace_sha256 primary_outcomes_path primary_outcomes_sha256 failure_class scored_at exposure_status notes"
    require_keys "$record_file" "$keys"
    reject_unknown_keys "$record_file" "$keys"
    require_schema "$record_file" "boos.evidence.result.v1"
    require_enum "$record_file" status "pass fail inconclusive"
    require_enum "$record_file" failure_class \
        "none product model infrastructure evaluator"
    require_enum "$record_file" exposure_status "revealed"
    require_sha256 "$record_file" case_bundle_sha256
    require_sha256 "$record_file" trace_sha256
    require_sha256 "$record_file" primary_outcomes_sha256
    require_relative_path "$record_file" trace_path
    require_relative_path "$record_file" primary_outcomes_path
    status="$(record_value "$record_file" status)"
    failure_class="$(record_value "$record_file" failure_class)"
    case "$status:$failure_class" in
        pass:none|fail:product|fail:model|fail:infrastructure|fail:evaluator|inconclusive:product|inconclusive:model|inconclusive:infrastructure|inconclusive:evaluator) ;;
        *)
            fail_record "status and failure_class are inconsistent"
            return 1
            ;;
    esac
}
```

Add this source line after the other validator source lines:

```sh
. "$evidence_dir/validators/result.sh"
```

Add this exact dispatcher branch:

```sh
boos.evidence.result.v1)
    validate_result "$record_file"
    echo "valid evidence result"
    ;;
```

Create `tests/evidence/verify-result.sh`:

```sh
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

repo_root="$(CDPATH= cd -- "$evidence_dir/../.." && pwd)"
trace_path="$(record_value "$result_file" trace_path)"
outcomes_path="$(record_value "$result_file" primary_outcomes_path)"
test -f "$repo_root/$trace_path" || {
    echo "trace file not found: $trace_path" >&2
    exit 1
}
test -f "$repo_root/$outcomes_path" || {
    echo "primary outcomes file not found: $outcomes_path" >&2
    exit 1
}
expected_trace_sha256="$(record_value "$result_file" trace_sha256)"
actual_trace_sha256="$(sha256_file "$repo_root/$trace_path")"
if [ "$actual_trace_sha256" != "$expected_trace_sha256" ]; then
    echo "trace SHA-256 mismatch" >&2
    exit 1
fi

expected_outcomes_sha256="$(record_value "$result_file" primary_outcomes_sha256)"
actual_outcomes_sha256="$(sha256_file "$repo_root/$outcomes_path")"
if [ "$actual_outcomes_sha256" != "$expected_outcomes_sha256" ]; then
    echo "primary outcomes SHA-256 mismatch" >&2
    exit 1
fi

echo "verified evidence result"
```

Make it executable.

- [ ] **Step 4: Run result tests to verify GREEN**

Run:

```sh
sh tests/evidence/test-record.sh
sh tests/evidence/test-registration.sh
sh tests/evidence/test-result.sh
```

Expected: all pass.

- [ ] **Step 5: Commit the result contract**

```sh
git add tests/evidence/validators/result.sh \
  tests/evidence/test-result.sh \
  tests/evidence/verify-result.sh \
  tests/evidence/fixtures/valid/result.kv \
  tests/evidence/fixtures/valid/trace.example.txt \
  tests/evidence/fixtures/valid/outcomes.example.kv \
  tests/evidence/fixtures/invalid/result-unrevealed.kv \
  tests/evidence/fixtures/invalid/result-bad-trace-hash.kv \
  tests/evidence/fixtures/invalid/result-pass-with-failure.kv \
  tests/evidence/validate-record.sh
git commit -m "test: verify revealed evidence results"
```

---

### Task 7: Evidence indexes and repository-level validation

**Files:**
- Create: `tests/evidence/README.md`
- Create: `tests/evidence/regression/README.md`
- Create: `tests/evidence/frontier/README.md`
- Create: `tests/evidence/test-all.sh`
- Create: `tests/evidence/validate-tree.sh`

**Interfaces:**
- Consumes: all focused validators from Tasks 1–6.
- Produces: `tests/evidence/test-all.sh` as the local test entry point.
- Produces: `tests/evidence/validate-tree.sh` for publishable current records.
- Does not validate invalid fixtures as current evidence.

- [ ] **Step 1: Write the aggregate test entry point**

Create `tests/evidence/test-all.sh`:

```sh
#!/bin/sh
set -eu

evidence_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"

for test_script in \
    "$evidence_dir/test-record.sh" \
    "$evidence_dir/test-incident.sh" \
    "$evidence_dir/test-claim.sh" \
    "$evidence_dir/test-registration.sh" \
    "$evidence_dir/test-contamination.sh" \
    "$evidence_dir/test-result.sh"
do
    sh "$test_script"
done

echo "all evidence system tests passed"
```

Create `tests/evidence/validate-tree.sh`:

```sh
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
```

Make both executable.

- [ ] **Step 2: Run aggregate entry points before adding indexes**

Run:

```sh
tests/evidence/test-all.sh
tests/evidence/validate-tree.sh
```

Expected: both pass. This step verifies the entry points before prose changes.

- [ ] **Step 3: Add focused reader indexes**

`tests/evidence/README.md` must be an index with:

- one paragraph defining the living evidence system;
- links to incidents, claims, regression, and frontier metadata;
- one sentence for each evidence track;
- the command `./test-all.sh`;
- the command `./validate-tree.sh`;
- an explicit warning that passing these validators does not validate BoOS.

`tests/evidence/regression/README.md` must:

- link to `../../research/semantic-object-view/`;
- classify it as Test 0 after exposure;
- state that future passing results are regression/wiring evidence only.

`tests/evidence/frontier/README.md` must:

- state that plaintext frontier cases do not live in the working tree;
- define registration digest, exposure, retirement, family contamination, and
  append-only reveal rules;
- contain no example frontier task that could become an implementation target.

- [ ] **Step 4: Run indexes and tree validation**

Run:

```sh
tests/evidence/test-all.sh
tests/evidence/validate-tree.sh
git diff --check
```

Expected: all exit zero.

- [ ] **Step 5: Commit evidence entry points**

```sh
git add tests/evidence/README.md \
  tests/evidence/regression/README.md \
  tests/evidence/frontier/README.md \
  tests/evidence/test-all.sh \
  tests/evidence/validate-tree.sh
git commit -m "docs: index living evidence tracks"
```

---

### Task 8: Reclassify Test 0 and bound repository claims

**Files:**
- Modify: `tests/research/semantic-object-view/README.md`
- Modify: `tests/research/semantic-object-view/runs/2026-07-30-pair-001/README.md`
- Modify: `README.md`
- Modify: `docs/PROJECT-OVERVIEW.md`

**Interfaces:**
- Consumes: evidence indexes from Task 7.
- Produces: reader-facing claim boundaries.
- Preserves: frozen prompts, tasks, metric definitions, result records, raw
  traces, hashes, and numeric results.

- [ ] **Step 1: Add a documentation assertion test**

Before editing, run this command and confirm it fails:

```sh
rg -F "Test 0: Interface and Wiring Probe" \
  tests/research/semantic-object-view/README.md \
  tests/research/semantic-object-view/runs/2026-07-30-pair-001/README.md \
  README.md \
  docs/PROJECT-OVERVIEW.md
```

Expected: nonzero because the bounded label is not present in all four files.

- [ ] **Step 2: Reclassify without rewriting frozen evidence**

Make these exact conceptual changes:

1. In `tests/research/semantic-object-view/README.md`, change the title to:

   ```markdown
   # Test 0: Interface and Wiring Probe
   ```

   Add a retrospective note before the original research question:

   ```markdown
   This frozen protocol is retained as a regression and wiring probe. Its tasks
   and prompts were derived from the semantic-object interface and therefore
   cannot support a general claim that the interface improves AI operation.
   ```

   Leave the hypothesis, variants, tasks, metrics, and procedure unchanged as
   historical protocol content.

2. In Pair 001's README, add `Test 0` to the title. Replace:

   ```text
   That supports a semantic ABI as a useful BoOS direction.
   ```

   with:

   ```text
   This shows that the tested model could consume the implemented object
   protocol under an interface-specific prompt. Because the tasks were
   constructed from that interface, the result supplies wiring evidence but
   no general evidence that a semantic ABI improves AI operation.
   ```

   Do not change the metric tables or validity limits.

3. In the root README's current-research section, link
   `tests/evidence/README.md` and state that the OS-boundary claim remains
   unsupported.

4. In `docs/PROJECT-OVERVIEW.md`, replace any implication that the existing
   A/B pair validates the direction with:

   ```text
   Test 0 is retained as a protocol/wiring regression. It does not establish
   that the semantic object layer improves real AI operation. Fresh research
   claims must use the Living Evidence System.
   ```

- [ ] **Step 3: Run the documentation assertion to verify GREEN**

Run:

```sh
for file in \
  tests/research/semantic-object-view/README.md \
  tests/research/semantic-object-view/runs/2026-07-30-pair-001/README.md \
  README.md \
  docs/PROJECT-OVERVIEW.md
do
  rg -F -q "Test 0" "$file"
done
rg -F -q "no general evidence" \
  tests/research/semantic-object-view/runs/2026-07-30-pair-001/README.md
rg -F -q "Living Evidence System" README.md docs/PROJECT-OVERVIEW.md
```

Expected: all assertions exit zero.

- [ ] **Step 4: Run full verification**

Run:

```sh
tests/evidence/test-all.sh
tests/evidence/validate-tree.sh
tests/research/semantic-object-view/test-validate-result.sh
tests/research/semantic-object-view/validate-result.sh \
  tests/research/semantic-object-view/runs/2026-07-30-pair-001/baseline-result.kv
tests/research/semantic-object-view/validate-result.sh \
  tests/research/semantic-object-view/runs/2026-07-30-pair-001/object-result.kv
git diff --check
```

Expected:

- every evidence focused test passes;
- current evidence records validate;
- the original Test 0 validator self-test passes;
- both frozen Pair 001 result records remain valid;
- no whitespace errors.

- [ ] **Step 5: Audit scope before committing**

Run:

```sh
git status --short
git diff --stat
git diff -- src rootfs
```

Expected:

- only `tests/evidence/`, the four documentation files, and plan-authorized
  evidence fixtures are changed;
- `git diff -- src rootfs` is empty;
- user-owned untracked files remain unstaged.

- [ ] **Step 6: Commit claim-bound documentation**

```sh
git add README.md \
  docs/PROJECT-OVERVIEW.md \
  tests/research/semantic-object-view/README.md \
  tests/research/semantic-object-view/runs/2026-07-30-pair-001/README.md
git commit -m "docs: bound semantic object experiment claims"
```

---

## Completion Gate

Before calling the first slice complete:

1. Run every command in Task 8 Step 4 again from a clean shell.
2. Verify every invalid fixture is rejected by its named validator test.
3. Verify `validate-tree.sh` does not traverse `fixtures/invalid/`.
4. Verify no frontier plaintext task exists under `tests/evidence/frontier/`.
5. Verify the current claim status is `unsupported`.
6. Verify the first incident describes observed friction without naming BoOS,
   a namespace, registry, daemon, or operating-system solution.
7. Verify `git diff -- src rootfs` is empty.
8. Verify only intended files are staged for each commit.
9. Preserve `.superpowers/`, `boos-desktop-1440x1000.png`, and
   `boos-mobile-390x844.png` as user-owned untracked files.

The slice is complete only when these checks pass. Passing the slice proves
that evidence records are structurally enforced and known broken fixtures are
detected; it does not prove BoOS correctness or architectural necessity.
