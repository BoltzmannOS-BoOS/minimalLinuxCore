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
