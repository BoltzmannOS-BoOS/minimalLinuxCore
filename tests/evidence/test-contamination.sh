#!/bin/sh
set -eu

evidence_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
validator="$evidence_dir/validate-record.sh"
temporary_dir="$(mktemp -d)"
trap 'rm -rf "$temporary_dir"' EXIT HUP INT TERM

expect_failure_message() {
    description="$1"
    expected_message="$2"
    shift 2
    if "$@" >"$temporary_dir/stdout" 2>"$temporary_dir/stderr"; then
        echo "expected failure: $description" >&2
        exit 1
    fi
    if ! grep -F -x "$expected_message" "$temporary_dir/stderr" >/dev/null; then
        echo "expected failure message: $description" >&2
        cat "$temporary_dir/stderr" >&2
        exit 1
    fi
}

"$validator" "$evidence_dir/fixtures/valid/contamination.kv" >/dev/null
expect_failure_message "invalid contamination level" \
    "invalid target_level: implementation" \
    "$validator" "$evidence_dir/fixtures/invalid/contamination-invalid-level.kv"
expect_failure_message "contamination retained as frontier evidence" \
    "invalid disposition: frontier" \
    "$validator" "$evidence_dir/fixtures/invalid/contamination-frontier-disposition.kv"
expect_failure_message "case contamination requests a new evaluator" \
    "invalid disposition for target level: case:new_evaluator" \
    "$validator" "$evidence_dir/fixtures/invalid/contamination-case-new-evaluator.kv"
expect_failure_message "case contamination requests claim review" \
    "invalid disposition for target level: case:claim_review" \
    "$validator" "$evidence_dir/fixtures/invalid/contamination-case-claim-review.kv"
expect_failure_message "contamination omits the retired registration" \
    "missing required field: registration_id" \
    "$validator" "$evidence_dir/fixtures/invalid/contamination-missing-registration-id.kv"

for matrix_entry in \
    case:regression:valid \
    case:archive:valid \
    case:new_distribution:invalid \
    case:new_evaluator:invalid \
    case:claim_review:invalid \
    family:regression:valid \
    family:archive:valid \
    family:new_distribution:valid \
    family:new_evaluator:invalid \
    family:claim_review:invalid \
    metric:regression:invalid \
    metric:archive:valid \
    metric:new_distribution:valid \
    metric:new_evaluator:valid \
    metric:claim_review:valid
do
    target_level="${matrix_entry%%:*}"
    matrix_remainder="${matrix_entry#*:}"
    disposition="${matrix_remainder%%:*}"
    expected_validity="${matrix_remainder##*:}"
    matrix_record="$temporary_dir/matrix-$target_level-$disposition.kv"
    sed \
        -e "s/^target_level=.*/target_level=$target_level/" \
        -e "s/^disposition=.*/disposition=$disposition/" \
        "$evidence_dir/fixtures/valid/contamination.kv" >"$matrix_record"

    if [ "$expected_validity" = valid ]; then
        "$validator" "$matrix_record" >/dev/null
    else
        expect_failure_message "$target_level cannot use $disposition" \
            "invalid disposition for target level: $target_level:$disposition" \
            "$validator" "$matrix_record"
    fi
done

echo "evidence contamination validator tests passed"
