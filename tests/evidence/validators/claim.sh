validate_claim() {
    record_file="$1"
    keys="schema claim_id statement scope excluded_scope problem_distribution required_evidence primary_outcomes decision_rule known_counterevidence benchmark_versions implementation_versions status expiry_condition"
    require_keys "$record_file" "$keys"
    reject_unknown_keys "$record_file" "$keys"
    require_schema "$record_file" "boos.evidence.claim.v1"
    require_enum "$record_file" status \
        "unsupported exploratory supported_within_scope contradicted stale"
}
