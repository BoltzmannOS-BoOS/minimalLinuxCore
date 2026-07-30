validate_outcomes() {
    record_file="$1"
    keys="schema result_id status failure_class"
    require_keys "$record_file" "$keys"
    reject_unknown_keys "$record_file" "$keys"
    require_schema "$record_file" "boos.evidence.primary-outcomes.v1"
    require_result_summary_consistency "$record_file"
}
