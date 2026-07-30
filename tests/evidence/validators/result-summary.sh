require_result_summary_consistency() {
    record_file="$1"
    require_enum "$record_file" status "pass fail inconclusive"
    require_enum "$record_file" failure_class \
        "none product model infrastructure evaluator"

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
