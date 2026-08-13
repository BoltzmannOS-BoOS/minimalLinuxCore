validate_result() {
    record_file="$1"
    keys="schema result_id registration_id subject_id status case_bundle_sha256 evaluator_version trace_path trace_sha256 primary_outcomes_path primary_outcomes_sha256 failure_class scored_at exposure_status notes"
    require_keys "$record_file" "$keys"
    reject_unknown_keys "$record_file" "$keys"
    require_schema "$record_file" "boos.evidence.result.v1"
    require_result_summary_consistency "$record_file"
    require_enum "$record_file" exposure_status "revealed"
    require_sha256 "$record_file" case_bundle_sha256
    require_sha256 "$record_file" trace_sha256
    require_sha256 "$record_file" primary_outcomes_sha256
    require_relative_path "$record_file" trace_path
    require_relative_path "$record_file" primary_outcomes_path
}
