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
