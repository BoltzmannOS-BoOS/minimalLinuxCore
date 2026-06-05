#!/bin/sh
# BoOS Auto-Attack Script (Layer 2 infrastructure)
# Runs attack tests and reports defense status. Zero API cost.

set -eu

PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CARGO="/Users/hostsjim/.cargo/bin/cargo"
REPORT="$PROJECT_ROOT/tests/auto-attack-report.txt"

echo "═══ BoOS Auto-Attack Report ═══" | tee "$REPORT"
echo "Timestamp: $(date)" | tee -a "$REPORT"

# Pattern stats
P="$PROJECT_ROOT/tests/attack-patterns.txt"
TOTAL=$(grep -c "^PATTERN" "$P")
BLOCKED=$(grep -c "BLOCKED" "$P")
OPEN=$(grep -c "OPEN" "$P")
echo "Patterns: $TOTAL ($BLOCKED blocked, $OPEN open)" | tee -a "$REPORT"
echo "" | tee -a "$REPORT"

# Run attack tests
echo "--- Attack Tests ---" | tee -a "$REPORT"
cd "$PROJECT_ROOT/src/rust"
OUT=$("$CARGO" test attack 2>&1)
echo "$OUT" | grep "test result:" | tee -a "$REPORT"

# Check failures
if echo "$OUT" | grep -q "FAILED"; then
    echo "⚠️  REGRESSION: previously blocked attacks now pass" | tee -a "$REPORT"
    echo "$OUT" | grep -A3 "FAILED" | tee -a "$REPORT"
fi

echo "" | tee -a "$REPORT"
echo "--- Open Issues ---" | tee -a "$REPORT"
grep "OPEN\|PARTIAL" "$P" | while read -r line; do
    pat=$(echo "$line" | cut -d'|' -f1 | sed 's/PATTERN //')
    echo "  ⚠️  $pat" | tee -a "$REPORT"
done

echo "" | tee -a "$REPORT"
echo "--- Layer 3: Attack Evolution ---" | tee -a "$REPORT"
EVOLVE="$PROJECT_ROOT/tests/attack-evolve.py"
if [ -f "$EVOLVE" ]; then
    python3 "$EVOLVE" 2>&1 | tail -5 | tee -a "$REPORT"
fi
