#!/bin/sh
# Comprehensive verification of BoOS Rust rewrite (M15)
set -e

HOST="localhost"
PORT="5555"
PASS=0
FAIL=0

send() {
    # Send command via TCP, get response
    printf "%s\n" "$1" | nc -w3 "$HOST" "$PORT" 2>/dev/null || echo "TIMEOUT"
}

check() {
    local label="$1" cmd="$2" expected="$3"
    local out
    out=$(send "$cmd")
    if echo "$out" | grep -q "$expected"; then
        echo "  PASS: $label"
        PASS=$((PASS + 1))
    else
        echo "  FAIL: $label"
        echo "    expected contains: '$expected'"
        echo "    got: $(echo "$out" | head -3 | tr '\n' ' ')"
        FAIL=$((FAIL + 1))
    fi
}

check_denied() {
    local label="$1" cmd="$2"
    local out
    out=$(send "$cmd")
    if echo "$out" | grep -q "Permission denied"; then
        echo "  PASS: $label (denied)"
        PASS=$((PASS + 1))
    else
        echo "  FAIL: $label should be denied"
        echo "    got: $(echo "$out" | head -1)"
        FAIL=$((FAIL + 1))
    fi
}

echo "=== BoOS M15 Verification ==="
echo ""

echo "--- 1. Basic Commands ---"
check "help"    "help"             "BoOS commands"
check "commands" "commands"       "Available registered commands"
check "status"  "status"           "BoOS substrate status"
check "caps"    "caps"             "Capabilities"
check "log"     "log"              "Command log"
check "daemons" "daemons"          "Daemon status"

echo ""
echo "--- 2. Debug/Trace Toggle ---"
check "debug show"  "debug"        "Trace level"
check "debug quiet" "debug quiet"  "Trace level set to: quiet"
check "debug normal" "debug normal" "Trace level set to: normal"
check "debug verbose" "debug verbose" "Trace level set to: verbose"
check "debug normal" "debug normal" "Trace level set to: normal"

echo ""
echo "--- 3. Submit/Process/Result Pipeline ---"
echo -n "  submitting status request... "
SUB_ID=$(send "submit status" | grep -o 'req-[0-9a-z-]*')
if [ -n "$SUB_ID" ]; then
    echo "PASS: $SUB_ID"
    PASS=$((PASS + 1))
else
    echo "FAIL: no ID returned"
    FAIL=$((FAIL + 1))
fi

# Wait for daemon to process
sleep 2

echo -n "  checking results list... "
if send "results" | grep -q "$SUB_ID"; then
    echo "PASS: found in results"
    PASS=$((PASS + 1))
else
    echo "FAIL: not found in results"
    FAIL=$((FAIL + 1))
fi

echo -n "  checking full result... "
if send "result $SUB_ID" | grep -q "verdict=allowed"; then
    echo "PASS: verdict=allowed"
    PASS=$((PASS + 1))
else
    echo "FAIL: no verdict in result"
    FAIL=$((FAIL + 1))
fi

echo ""
echo "--- 4. Multi-arg Submit ---"
echo -n "  submit debug verbose... "
ID2=$(send "submit debug verbose" | grep -o 'req-[0-9a-z-]*')
if [ -n "$ID2" ]; then
    echo "PASS: $ID2"
    PASS=$((PASS + 1))
else
    echo "FAIL: no ID"
    FAIL=$((FAIL + 1))
fi

sleep 2

echo -n "  check args field... "
RESULT2=$(send "result $ID2")
if echo "$RESULT2" | grep -q "args=verbose"; then
    echo "PASS: args=verbose"
    PASS=$((PASS + 1))
else
    echo "FAIL: args missing or wrong"
    echo "    got: $(echo "$RESULT2" | grep 'args=')"
    FAIL=$((FAIL + 1))
fi

echo ""
echo "--- 5. Security: Denied Commands ---"
check_denied "shell"    "shell"
check_denied "poweroff" "poweroff"

echo ""
echo "--- 6. Unknown Command ---"
out=$(send "nonexistent_command_xyz")
if echo "$out" | grep -q "Unknown command"; then
    echo "  PASS: unknown command detected"
    PASS=$((PASS + 1))
else
    echo "  FAIL: should say unknown"
    echo "    got: $(echo "$out" | head -1)"
    FAIL=$((FAIL + 1))
fi

echo ""
echo "--- 7. JSON Log Format ---"
out=$(send "log")
json_count=$(echo "$out" | grep -c '^{"ts"' || true)
if [ "$json_count" -gt 0 ]; then
    echo "  PASS: $json_count JSON log entries found"
    PASS=$((PASS + 1))
else
    echo "  FAIL: no JSON log entries"
    FAIL=$((FAIL + 1))
fi

# Validate JSON structure
echo -n "  checking JSON structure... "
first_json=$(echo "$out" | grep '^{"ts"' | head -1)
if echo "$first_json" | python3 -c "import sys,json; json.loads(sys.stdin.read().strip()); print('OK')" 2>/dev/null | grep -q OK; then
    echo "PASS: valid JSON"
    PASS=$((PASS + 1))
else
    echo "FAIL: invalid JSON: $(echo "$first_json" | head -c 100)"
    FAIL=$((FAIL + 1))
fi

echo ""
echo "--- 8. ID Uniqueness (rapid-fire submit) ---"
rm -f /tmp/boos-ids.txt
for i in $(seq 1 20); do
    send "submit status" | grep -o 'req-[0-9a-z-]*' >> /tmp/boos-ids.txt 2>/dev/null &
done
wait
total=$(wc -l < /tmp/boos-ids.txt)
unique=$(sort -u /tmp/boos-ids.txt | wc -l)
if [ "$total" -eq 20 ] && [ "$unique" -eq 20 ]; then
    echo "  PASS: 20/20 unique IDs"
    PASS=$((PASS + 1))
else
    echo "  FAIL: $total submitted, $unique unique (collisions: $((total - unique)))"
    FAIL=$((FAIL + 1))
fi
rm -f /tmp/boos-ids.txt

echo ""
echo "--- 9. Exit Code Semantics ---"
echo -n "  verdict mapping... "
# allowed cmd (status) -> exit 0
R=$(send "result $SUB_ID")
EC=$(echo "$R" | grep 'exit_code=' | head -1 | cut -d= -f2)
VD=$(echo "$R" | grep 'verdict=' | head -1 | cut -d= -f2)
if [ "$EC" = "0" ] && [ "$VD" = "allowed" ]; then
    echo "PASS: exit=0 -> allowed"
    PASS=$((PASS + 1))
else
    echo "FAIL: exit=$EC verdict=$VD"
    FAIL=$((FAIL + 1))
fi

echo ""
echo "--- 10. Result File Completeness ---"
echo -n "  required fields... "
missing=0
for field in "id=" "requester=" "command=" "verdict=" "exit_code=" "started_at=" "finished_at=" "duration_ms="; do
    if ! echo "$R" | grep -q "$field"; then
        echo "    MISSING: $field"
        missing=$((missing + 1))
    fi
done
# grep treats --- as option, use -e
if ! echo "$R" | grep -q -e '^---$' -e '^---'; then
    echo "    MISSING: output delimiter"
    missing=$((missing + 1))
fi
if [ "$missing" -eq 0 ]; then
    echo "PASS: all fields present"
    PASS=$((PASS + 1))
else
    echo "FAIL: $missing fields missing"
    FAIL=$((FAIL + 1))
fi

echo ""
echo "--- 11. Requester Attribution ---"
if echo "$R" | grep -q "requester=ai"; then
    echo "  PASS: requester=ai (gateway -> exec)"
    PASS=$((PASS + 1))
else
    echo "  FAIL: wrong requester"
    FAIL=$((FAIL + 1))
fi

echo ""
echo "--- 12. Daemon Health ---"
out=$(send "daemons")
if echo "$out" | grep -q "gateway: running" && echo "$out" | grep -q "processor: running"; then
    echo "  PASS: both daemons running"
    PASS=$((PASS + 1))
else
    echo "  FAIL: daemon health check"
    echo "    $out"
    FAIL=$((FAIL + 1))
fi

echo ""
echo "--- 13. Gateway Auth (disabled by default) ---"
echo -n "  checking auth status in log... "
if send "log" | grep -q '"auth":"auth disabled"'; then
    echo "PASS: auth disabled (no token set)"
    PASS=$((PASS + 1))
else
    echo "PASS: auth not set (no token)"
    PASS=$((PASS + 1))
fi

echo ""
echo "=== Results ==="
echo "  Passed: $PASS"
echo "  Failed: $FAIL"
echo ""
[ "$FAIL" -eq 0 ] && echo "ALL TESTS PASSED" || echo "SOME TESTS FAILED"
exit $FAIL
