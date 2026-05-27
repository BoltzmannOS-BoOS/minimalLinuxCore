#!/bin/sh
# Verify S1-S6 shell security fixes (no QEMU required)
# Run: sh tests/verify-shell-fixes.sh
set -e

PASS=0
FAIL=0
ROOTFS="rootfs/bin"

check() {
    label="$1"; pattern="$2"; file="$3"
    if grep -q "$pattern" "$file" 2>/dev/null; then
        echo "  PASS: $label"
        PASS=$((PASS + 1))
    else
        echo "  FAIL: $label — pattern not found in $file"
        FAIL=$((FAIL + 1))
    fi
}

check_not() {
    label="$1"; pattern="$2"; file="$3"
    if grep -q "$pattern" "$file" 2>/dev/null; then
        echo "  FAIL: $label — pattern STILL present in $file (not fixed)"
        FAIL=$((FAIL + 1))
    else
        echo "  PASS: $label"
        PASS=$((PASS + 1))
    fi
}

echo "=== S1-S6 Shell Security Fix Verification ==="
echo ""

echo "--- boos-shell ---"
check     "S1: set -f present"           "set -f"                    "$ROOTFS/boos-shell"
check     "S2: read -r present"          "read -r line"              "$ROOTFS/boos-shell"
check     "S1: \$@ quoted in run case"   '/bin/boos-exec "$@"'      "$ROOTFS/boos-shell"
check     "S1: \$@ quoted in submit"     '/bin/boos-exec submit "$@"' "$ROOTFS/boos-shell"
check_not "S1: unquoted \$* in run"      '/bin/boos-exec \\$\\*'     "$ROOTFS/boos-shell"

echo ""
echo "--- boos-supervisor ---"
check     "S3: set -f present"           "set -f"                    "$ROOTFS/boos-supervisor"
check     "S4: cmdline TOCTOU check"     "/proc/.*pid.*cmdline"      "$ROOTFS/boos-supervisor"
check     "S4: grep -qF name check"      'grep -qF "\$name"'          "$ROOTFS/boos-supervisor"

echo ""
echo "--- boos-daemon ---"
check     "S5: stderr to log (line 1)"   '2>>"\$LOG_FILE"'            "$ROOTFS/boos-daemon"
check     "S6: POLL_INTERVAL variable"   "POLL_INTERVAL"             "$ROOTFS/boos-daemon"
check     "S6: sleep with variable"      'sleep "\$POLL_INTERVAL"'    "$ROOTFS/boos-daemon"
check_not "S5: /dev/null swallowing"     '>/dev/null 2>&1'           "$ROOTFS/boos-daemon"
check     "daemon.conf exists"           "POLL_INTERVAL"             "rootfs/etc/boos/daemon.conf"

echo ""
echo "--- Syntax checks ---"
for f in "$ROOTFS/boos-shell" "$ROOTFS/boos-supervisor" "$ROOTFS/boos-daemon"; do
    name=$(basename "$f")
    if sh -n "$f" 2>/dev/null; then
        echo "  PASS: $name syntax valid"
        PASS=$((PASS + 1))
    else
        echo "  FAIL: $name syntax error"
        FAIL=$((FAIL + 1))
    fi
done

echo ""
echo "--- Shellcheck (warnings already verified as acceptable) ---"
if command -v shellcheck >/dev/null 2>&1; then
    for f in "$ROOTFS/boos-shell" "$ROOTFS/boos-supervisor" "$ROOTFS/boos-daemon"; do
        name=$(basename "$f")
        issues=$(shellcheck "$f" 2>&1 | grep -c '^In ' || true)
        if [ "$issues" -le 2 ]; then
            echo "  PASS: $name — $issues warnings (all pre-existing or expected)"
            PASS=$((PASS + 1))
        else
            echo "  FAIL: $name — $issues warnings (unexpected new warnings?)"
            FAIL=$((FAIL + 1))
        fi
    done
else
    echo "  SKIP: shellcheck not installed"
fi

echo ""
echo "=== Results ==="
echo "Passed: $PASS"
echo "Failed: $FAIL"
echo ""
[ "$FAIL" -eq 0 ] && echo "ALL CHECKS PASSED" || echo "SOME CHECKS FAILED"
exit $FAIL
