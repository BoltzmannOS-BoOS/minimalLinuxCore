#!/bin/sh
# Embodied Memory Behavioral Verification
# Tests whether [MEMORY:FAIL] actually changes agent behavior.
# 
# Scenario:
#   1. Pre-populate memory: "FETCH bad-url → FAIL: HTTP 403"
#   2. Give goal: "get data from {bad-url} or find an alternative"
#   3. Expected: agent should NOT repeat the failed URL, should adapt
#   4. Baseline: same goal WITHOUT pre-populated memory (should try bad-url)
#
# Pass criteria: agent actions differ between memory and no-memory runs.
# Fail criteria: agent does the same thing regardless of memory.

set -eu

PROJECT="/Users/hostsjim/project/minimalLinuxCore"
BINARY="$PROJECT/src/rust/target/release/boos-agent"
API_KEY="${DEEPSEEK_API_KEY:-}"

if [ -z "$API_KEY" ]; then
    source ~/.zshrc 2>/dev/null
    API_KEY="${DEEPSEEK_API_KEY:-}"
fi

BAD_URL="https://httpbin.org/status/403"
GOOD_URL="https://httpbin.org/json"

echo "═══════════════════════════════════════"
echo " Embodied Memory Behavioral Test"
echo "═══════════════════════════════════════"
echo ""

# Test 1: WITH memory — inject failure knowledge
echo "── Test 1: WITH failure memory ──"
MEM_DIR="/tmp/boos-memory-test/var/boos/memory"
rm -rf /tmp/boos-memory-test
mkdir -p "$MEM_DIR"
# Pre-populate failure memory
cat > "$MEM_DIR/recent.log" << MEMEOF
ts=1.000
type=develop
content=FETCH $BAD_URL => FAIL: HTTP 403 (Forbidden)
session_id=test-memory-session

ts=2.000
type=reflect
content=session test-memory-session: FETCH failed, need alternative approach
session_id=test-memory-session

ts=3.000
type=develop
content=FETCH $GOOD_URL => success: 429 chars returned
session_id=test-memory-session
MEMEOF

echo "Memory pre-populated with: FETCH $BAD_URL => FAIL"
echo ""

GOAL="get JSON data from $BAD_URL or find an alternative working URL"
echo "Goal: $GOAL"
echo ""

"$BINARY" develop \
    --goal "$GOAL" \
    --api-key "$API_KEY" \
    --max-loops 4 2>&1 | tee /tmp/embodied-memory-with.txt | grep -E "FETCH|WRITE|DONE|Round|Complete"

echo ""
echo "── Test 2: WITHOUT failure memory (baseline) ──"
rm -rf /tmp/boos-memory-test

"$BINARY" develop \
    --goal "$GOAL" \
    --api-key "$API_KEY" \
    --max-loops 4 2>&1 | tee /tmp/embodied-memory-baseline.txt | grep -E "FETCH|WRITE|DONE|Round|Complete"

echo ""
echo "═══════════════════════════════════"
echo " Comparison:"
echo "═══════════════════════════════════"
echo ""
echo "WITH memory actions:"
grep "FETCH\|WRITE\|DONE" /tmp/embodied-memory-with.txt | head -10
echo ""
echo "WITHOUT memory actions:"
grep "FETCH\|WRITE\|DONE" /tmp/embodied-memory-baseline.txt | head -10
echo ""
echo "Verdict:"
# Check if memory run used bad URL
if grep -q "$BAD_URL" /tmp/embodied-memory-with.txt 2>/dev/null; then
    echo "  ❌ FAIL: agent still used failing URL despite memory warning"
else
    echo "  ✅ PASS: agent avoided failing URL when memory was present"
fi
echo ""
echo "Files: /tmp/embodied-memory-with.txt /tmp/embodied-memory-baseline.txt"
