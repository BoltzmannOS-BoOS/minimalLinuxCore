validate_claim() {
    record_file="$1"
    keys="schema claim_id statement scope excluded_scope problem_distribution required_evidence primary_outcomes decision_rule known_counterevidence benchmark_versions implementation_versions status expiry_condition"
    require_keys "$record_file" "$keys"
    reject_unknown_keys "$record_file" "$keys"
    require_schema "$record_file" "boos.evidence.claim.v1"
    require_enum "$record_file" status \
        "unsupported exploratory supported_within_scope contradicted stale"

    status="$(record_value "$record_file" status)"
    problem_distribution="$(record_value "$record_file" problem_distribution)"
    benchmark_versions="$(record_value "$record_file" benchmark_versions)"
    implementation_versions="$(record_value "$record_file" implementation_versions)"
    known_counterevidence="$(record_value "$record_file" known_counterevidence)"
    case "$status" in
        supported_within_scope)
            if [ "$problem_distribution" = unregistered ]; then
                fail_record "supported_within_scope requires a registered problem_distribution"
                return 1
            fi
            if [ "$benchmark_versions" = none ]; then
                fail_record "supported_within_scope requires benchmark_versions"
                return 1
            fi
            if [ "$implementation_versions" = none ]; then
                fail_record "supported_within_scope requires implementation_versions"
                return 1
            fi
            ;;
        contradicted)
            if [ "$known_counterevidence" = none ]; then
                fail_record "contradicted claim requires named counterevidence"
                return 1
            fi
            ;;
        stale)
            if [ "$benchmark_versions" = none ]; then
                fail_record "stale claim requires benchmark_versions"
                return 1
            fi
            if [ "$implementation_versions" = none ]; then
                fail_record "stale claim requires implementation_versions"
                return 1
            fi
            ;;
    esac
}
