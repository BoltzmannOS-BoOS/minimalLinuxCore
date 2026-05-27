#!/bin/sh
# BoOS Integration Test Suite
# Run against a running BoOS gateway (QEMU or native).
# Usage: bash tests/integration-test.sh [host] [port]
#
# Covers: gateway protocol (AUTH, SESSION), submit --wait,
#         session tracking, all commands, security denials.

set -e

HOST="${1:-localhost}"
PORT="${2:-5555}"
PASS=0
FAIL=0

send() {
    # Send one or more lines, read response
    printf '%s\n' "$@" | nc -w5 "$HOST" "$PORT" 2>/dev/null || echo "CONNECTION_FAILED"
}

check() {
    local label="$1" expected="$2"
    shift 2
    local out
    out=$(send "$@")
    if echo "$out" | grep -q "$expected"; then
        echo "  PASS: $label"
        PASS=$((PASS + 1))
    else
        echo "  FAIL: $label"
        echo "    expected: '$expected'"
        echo "    got: $(echo "$out" | head -3 | tr '\n' ' ')"
        FAIL=$((FAIL + 1))
    fi
}

echo "=== BoOS Integration Tests ==="
echo "Target: $HOST:$PORT"
echo ""

# --- 1. Gateway connectivity ---
echo "--- 1. Gateway Connectivity ---"
out=$(send "help")
if echo "$out" | grep -q "BoOS commands"; then
    echo "  PASS: gateway responds"
    PASS=$((PASS + 1))
else
    echo "  FAIL: no response from gateway (is BoOS running?)"
    FAIL=$((FAIL + 1))
    echo ""
    echo "Make sure QEMU is running: bash scripts/run-qemu.sh"
    exit 1
fi

# --- 2. Basic Commands ---
echo ""
echo "--- 2. Basic Commands ---"
check "help"       "BoOS commands"    "help"
check "commands"   "Available"        "commands"
check "status"     "BoOS substrate"   "status"
check "caps"       "Capabilities"     "caps"
check "daemons"    "gateway"          "daemons"

# --- 3. Debug / Trace Toggle ---
echo ""
echo "--- 3. Debug/Trace ---"
check "debug show"    "Trace level"              "debug"
check "debug quiet"   "Trace level set to: quiet" "debug quiet"
check "debug normal"  "Trace level set to: normal" "debug normal"

# --- 4. Submit Pipeline (async) ---
echo ""
echo "--- 4. Submit/Result Pipeline ---"
SUB_ID=$(send "submit status" | grep -o 'req-[0-9a-z-]*' | head -1)
if [ -n "$SUB_ID" ]; then
    echo "  PASS: request ID: $SUB_ID"
    PASS=$((PASS + 1))
else
    echo "  FAIL: no request ID returned"
    FAIL=$((FAIL + 1))
fi

if [ -n "$SUB_ID" ]; then
    sleep 2  # wait for daemon to process
    check "result in list" "$SUB_ID"  "results"
    check "verdict allowed" "verdict=allowed" "result $SUB_ID"
fi

# --- 5. Submit --wait (synchronous) ---
echo ""
echo "--- 5. Submit --wait ---"
check "submit --wait" "BoOS substrate" "submit" "--wait" "status"

# --- 6. Multi-arg Submit ---
echo ""
echo "--- 6. Multi-arg Submit ---"
ID2=$(send "submit debug verbose" | grep -o 'req-[0-9a-z-]*' | head -1)
if [ -n "$ID2" ]; then
    echo "  PASS: multi-arg request ID: $ID2"
    PASS=$((PASS + 1))
    sleep 2
    out=$(send "result $ID2")
    if echo "$out" | grep -q "args=verbose"; then
        echo "  PASS: args preserved in result"
        PASS=$((PASS + 1))
    else
        echo "  FAIL: args not in result"
        FAIL=$((FAIL + 1))
    fi
fi

# --- 7. Session Tracking ---
echo ""
echo "--- 7. Session Tracking ---"
SESSION_ID="test-session-$(date +%s)"
SID=$(send "SESSION $SESSION_ID" "submit status" | grep -o 'req-[0-9a-z-]*' | head -1)
if [ -n "$SID" ]; then
    echo "  PASS: session request ID: $SID"
    PASS=$((PASS + 1))
    sleep 2
    out=$(send "result $SID")
    if echo "$out" | grep -q "session_id=$SESSION_ID"; then
        echo "  PASS: session_id in result"
        PASS=$((PASS + 1))
    else
        echo "  FAIL: session_id not in result"
        FAIL=$((FAIL + 1))
    fi
fi

# --- 8. Security: Denied Commands ---
echo ""
echo "--- 8. Security Denials ---"
check "shell denied"    "Permission denied" "shell"
check "poweroff denied" "Permission denied" "poweroff"

# --- 9. Unknown Command ---
echo ""
echo "--- 9. Unknown Command ---"
check "unknown cmd" "Unknown command" "nonexistent_cmd_xyz"

# --- 10. Exit Code Semantics ---
echo ""
echo "--- 10. Exit Code Semantics ---"
if [ -n "$SUB_ID" ]; then
    out=$(send "result $SUB_ID")
    EC=$(echo "$out" | grep 'exit_code=' | head -1 | cut -d= -f2)
    VD=$(echo "$out" | grep 'verdict=' | head -1 | cut -d= -f2)
    if [ "$EC" = "0" ] && [ "$VD" = "allowed" ]; then
        echo "  PASS: exit=0 -> allowed"
        PASS=$((PASS + 1))
    else
        echo "  FAIL: exit=$EC verdict=$VD"
        FAIL=$((FAIL + 1))
    fi
fi

# --- 11. Result File Completeness ---
echo ""
echo "--- 11. Result Completeness ---"
if [ -n "$SUB_ID" ]; then
    out=$(send "result $SUB_ID")
    missing=0
    for field in "id=" "requester=" "command=" "verdict=" "exit_code=" "started_at=" "finished_at=" "duration_ms="; do
        if ! echo "$out" | grep -q "$field"; then
            echo "    MISSING: $field"
            missing=$((missing + 1))
        fi
    done
    if ! echo "$out" | grep -q -e '^---$' -e '^---'; then
        echo "    MISSING: output delimiter"
        missing=$((missing + 1))
    fi
    if [ "$missing" -eq 0 ]; then
        echo "  PASS: all fields present"
        PASS=$((PASS + 1))
    else
        echo "  FAIL: $missing fields missing"
        FAIL=$((FAIL + 1))
    fi
fi

# --- 12. Requester Attribution ---
echo ""
echo "--- 12. Requester Attribution ---"
if [ -n "$SUB_ID" ]; then
    out=$(send "result $SUB_ID")
    if echo "$out" | grep -q "requester=ai"; then
        echo "  PASS: requester=ai"
        PASS=$((PASS + 1))
    else
        echo "  FAIL: wrong requester"
        FAIL=$((FAIL + 1))
    fi
fi

# --- 13. Config Hot Reload ---
echo ""
echo "--- 13. Config Hot Reload ---"
# This is a smoke test — the actual reload happens in supervisor loop
check "log shows entries" "component" "log"

# --- Results ---
echo ""
echo "=== Integration Test Results ==="
echo "Passed: $PASS"
echo "Failed: $FAIL"
echo ""
if [ "$FAIL" -eq 0 ]; then
    echo "ALL TESTS PASSED"
else
    echo "SOME TESTS FAILED"
fi
exit $FAIL
