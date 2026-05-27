#!/bin/sh
# BoOS Integration Test Suite
# Run against a running BoOS gateway (QEMU or native).
# Uses Rust tcp-client binary (BusyBox nc has race conditions).
# Usage: bash tests/integration-test.sh [host] [port]

HOST="${1:-localhost}"
PORT="${2:-5555}"
PASS=0
FAIL=0

# Build tcp-client if needed
TCP_CLIENT="${TCP_CLIENT:-./target/release/tcp-client}"
if [ ! -x "$TCP_CLIENT" ]; then
    TCP_CLIENT="./src/rust/target/release/tcp-client"
fi

send() {
    # Join all arguments into a single space-separated command line
    printf '%s\n' "$*" | "$TCP_CLIENT" "$HOST" "$PORT" 2>/dev/null
}

send_lines() {
    # Send multiple newline-separated lines (for SESSION protocol)
    printf '%s\n' "$@" | "$TCP_CLIENT" "$HOST" "$PORT" 2>/dev/null
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
    echo "  FAIL: no response from gateway"
    FAIL=$((FAIL + 1))
    echo "  Make sure BoOS gateway is running on $HOST:$PORT"
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
    sleep 2
    check "result in list" "$SUB_ID"  "results"
    check "verdict allowed" "verdict=allowed" "result $SUB_ID"
fi

# --- 5. Submit --wait ---
echo ""
echo "--- 5. Submit --wait ---"
check "submit --wait" "BoOS substrate" "submit" "--wait" "status"

# --- 7. Session Tracking ---
echo ""
echo "--- 7. Session Tracking ---"
SESSION_ID="test-session-$$"
SID=$(send_lines "SESSION $SESSION_ID" "submit status" | grep -o 'req-[0-9a-z-]*' | head -1)
if [ -n "$SID" ]; then
    echo "  PASS: session req ID: $SID"
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

# --- 8. Security Denials ---
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

echo ""
echo "=== Integration Test Results ==="
echo "Passed: $PASS"
echo "Failed: $FAIL"
echo ""
[ "$FAIL" -eq 0 ] && echo "ALL TESTS PASSED" || echo "SOME TESTS FAILED"
exit $FAIL
