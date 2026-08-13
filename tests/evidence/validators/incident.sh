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
