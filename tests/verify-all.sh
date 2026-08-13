#!/bin/sh
# BoOS Behavioral Verification Suite
# Tests every ✅ from the v0.9.1 report — not unit tests, behavioral depth checks.
# Run: bash tests/verify-all.sh

set -eu
PROJECT="/Users/hostsjim/project/minimalLinuxCore"
cd "$PROJECT"
echo "═══════════════════════════════════════════"
echo " BoOS v0.9.1 — Behavioral Verification"
echo "═══════════════════════════════════════════"
echo ""

PASS=0
FAIL=0
SHALLOW=0

check() {
    local name="$1"
    local result="$2"
    if [ "$result" = "PASS" ]; then
        echo "  ✅ $name"
        PASS=$((PASS + 1))
    elif [ "$result" = "SHALLOW" ]; then
        echo "  ⚠️  $name — SHALLOW (unit-test true, behavior false)"
        SHALLOW=$((SHALLOW + 1))
    else
        echo "  ❌ $name — FAILED: $result"
        FAIL=$((FAIL + 1))
    fi
}

# ── 1. SELF-STATE: returns real data, not hardcoded ──
echo "── 1. Proprioception (self-state) ──"
SELF_OUT=$(./src/rust/target/release/boos-agent develop \
    --goal "SELF-STATE" --api-key "sk-test" --max-loops 1 2>&1 || true)
if echo "$SELF_OUT" | grep -q "session: agent-"; then
    if echo "$SELF_OUT" | grep -q "uptime: ok memory: ok"; then
        check "SELF-STATE returns structured data" "SHALLOW"
        echo "     Detail: returns hardcoded string 'uptime: ok memory: ok attack: verified'"
        echo "     Not reporting actual uptime seconds, actual memory count, actual attack result"
    else
        check "SELF-STATE returns structured data" "PASS"
    fi
else
    check "SELF-STATE returns data" "FAIL: no self-state output"
fi

# ── 2. HEALTH-CHECK: detects warnings, triggers behavior change ──
echo ""
echo "── 2. Homeostasis (health-check) ──"
HEALTH_OUT=$(./src/rust/target/release/boos-agent develop \
    --goal "HEALTH-CHECK" --api-key "sk-test" --max-loops 1 2>&1 || true)
if echo "$HEALTH_OUT" | grep -q "HEALTH: PASS"; then
    check "HEALTH-CHECK returns status" "SHALLOW"
    echo "     Detail: returns hardcoded 'HEALTH: PASS (WARN count: 0)' every time"
    echo "     Never actually detects memory overflow, context overflow, or failure rate"
else
    check "HEALTH-CHECK returns status" "FAIL: no health output"
fi

# ── 3. EXTERNAL tagging: FETCH returns [EXTERNAL] ──
echo ""
echo "── 3. Self/Non-self (external tagging) ──"
# Can't test FETCH without API key, check code path
FETCH_HANDLER=$(grep -A5 'else if upper.starts_with("FETCH")' src/rust/src/agent_develop.rs | head -6)
if echo "$FETCH_HANDLER" | grep -q 'EXTERNAL'; then
    check "FETCH marks data [EXTERNAL]" "SHALLOW"
    echo "     Detail: code tags output with [EXTERNAL] but agent behavior doesn't change"
    echo "     Agent treats external data same as internal — no trust differential"
else
    check "FETCH marks data [EXTERNAL]" "FAIL: no EXTERNAL tag in FETCH handler"
fi

# ── 4. Circadian: phases actually run different code ──
echo ""
echo "── 4. Circadian (phase rhythm) ──"
PHASE_CHECK=$(grep -c "REFLECT\|SELF_CHECK\|IDLE" src/rust/src/agent_develop.rs)
REFLECT_CODE=$(grep -A5 'if phase == "REFLECT"' src/rust/src/agent_develop.rs)
SELF_CHECK_CODE=$(grep -A5 'if phase == "SELF_CHECK"' src/rust/src/agent_develop.rs)
IDLE_CODE=$(grep -A3 'if phase == "IDLE"' src/rust/src/agent_develop.rs)

if echo "$REFLECT_CODE" | grep -q "recent_add"; then
    REFLECT_OK=true
else
    REFLECT_OK=false
fi
if echo "$SELF_CHECK_CODE" | grep -q "auto-attack\|AUTO-ATTACK"; then
    SELF_CHECK_OK=true
else
    SELF_CHECK_OK=false
fi
if echo "$IDLE_CODE" | grep -q "sleep\|SELF-STATE"; then
    IDLE_OK=true
else
    IDLE_OK=false
fi

if $REFLECT_OK && $SELF_CHECK_OK; then
    if $IDLE_OK && ! echo "$IDLE_CODE" | grep -q "only sleep"; then
        check "Circadian phases run distinct code" "SHALLOW"
        echo "     Detail: REFLECT writes to memory, SELF_CHECK runs auto-attack, IDLE sleeps"
        echo "     IDLE is just sleep(LOOP_DELAY_MS * 3) — no introspection, no unprompted action"
    else
        check "Circadian phases run distinct code" "SHALLOW"
    fi
else
    check "Circadian phases exist" "FAIL"
fi

# ── 5. Checkpoint: persists to disk ──
echo ""
echo "── 5. AI Git (checkpoint persistence) ──"
rm -rf /tmp/boos-checkpoints
./src/rust/target/release/boos-agent develop \
    --goal "CHECKPOINT manual-test" --api-key "sk-test" --max-loops 2 2>&1 > /dev/null || true
CK_COUNT=$(ls /tmp/boos-checkpoints/*.json 2>/dev/null | wc -l | tr -d ' ')
if [ "$CK_COUNT" -ge 1 ]; then
    CK_CONTENT=$(cat /tmp/boos-checkpoints/*.json 2>/dev/null | head -1)
    if echo "$CK_CONTENT" | grep -q "session_id\|timestamp\|actions"; then
        check "Checkpoint persists to disk" "PASS"
        echo "     Detail: $CK_COUNT checkpoints with session_id, timestamp, actions"
    else
        check "Checkpoint has required fields" "FAIL: missing session_id/timestamp/actions"
    fi
else
    check "Checkpoint persists to disk" "FAIL: no checkpoint files found"
fi

# ── 6. Branch: creates new checkpoint ──
echo ""
echo "── 6. AI Git (branch) ──"
# Create a checkpoint first, then branch
mkdir -p /tmp/boos-checkpoints
echo '{"id":"ck-test-1","session_id":"test","timestamp":1,"label":"test","round":1,"branch":"main","parent":null,"actions":["READ test"]}' > /tmp/boos-checkpoints/ck-test-1.json
BRANCH_OUT=$(echo 'BRANCH ck-test-1 attack-test' | ./src/rust/target/release/boos-exec 2>&1 || echo "not-builtin")
if echo "$BRANCH_OUT" | grep -q "BRANCH created"; then
    check "Branch creates new checkpoint" "PASS"
else
    check "Branch creates new checkpoint" "SHALLOW"
    echo "     Detail: BRANCH exists in develop dispatch but not as standalone binary"
fi

# ── 7. Rollback: loads checkpoint ──
echo ""
echo "── 7. AI Git (rollback) ──"
ROLLBACK_OUT=$(echo 'ROLLBACK ck-test-1' | ./src/rust/target/release/boos-exec 2>&1 || echo "not-builtin")
if echo "$ROLLBACK_OUT" | grep -q "restored"; then
    check "Rollback restores state" "PASS"
elif echo "$ROLLBACK_OUT" | grep -q "not found"; then
    check "Rollback works (checkpoint not found is expected)" "SHALLOW"
    echo "     Detail: rollback returns proper error for missing checkpoint"
else
    check "Rollback exists" "SHALLOW"
fi

# ── 8. Embodied Memory: already tested, known SHALLOW ──
echo ""
echo "── 8. Embodied Memory (already verified) ──"
check "Embodied Memory" "SHALLOW"
echo "     Detail: keyword-match only, no cross-session persistence, no behavioral impact"

# ── Summary ──
echo ""
echo "═══════════════════════════════════════════"
echo " RESULTS"
echo "═══════════════════════════════════════════"
echo "  PASS:    $PASS"
echo "  SHALLOW: $SHALLOW"
echo "  FAIL:    $FAIL"
echo ""
if [ $SHALLOW -ge 3 ]; then
    echo "  ⚠️  3+ features are unit-test-level true, behavioral-level false"
    echo "  Recommendation: deepen before adding new features"
fi
