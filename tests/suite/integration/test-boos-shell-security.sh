#!/bin/sh
#
# tests/test-boos-shell-security.sh
# Security tests for boos-shell
#
# Verifies:
#   Test 1 (S1): Glob injection — `*` and `?` are NOT expanded to filenames
#   Test 2 (S2): Backslash preservation — backslash sequences preserved literally
#   Test 3:      Argument preservation — multi-word args not mangled
#
# Strategy:
#   Temporarily replaces /bin/boos-exec with a mock that captures its
#   arguments to a file.  Feeds input to boos-shell via pipe, then checks
#   the captured arguments for evidence of expansion or mangling.
#
# Returns exit 0 if all tests pass, non-zero if any fail.

set -u

PASS=0
FAIL=0

# ── helpers ──────────────────────────────────────────────────────────────

# Timeout helper: run a command, kill it after N seconds
# Usage: run_with_timeout <seconds> <cmd> [args...]
run_with_timeout() {
    timeout_secs="$1"; shift
    "$@" &
    pid=$!
    (
        sleep "$timeout_secs"
        kill "$pid" 2>/dev/null
    ) &
    watcher=$!
    wait "$pid" 2>/dev/null
    kill "$watcher" 2>/dev/null
    wait "$watcher" 2>/dev/null
    return 0
}

# Feed one line of input to boos-shell, let it process, then kill it.
# Reads captured args from /tmp/boos-test-args after.
feed_and_capture() {
    input="$1"
    rm -f /tmp/boos-test-args

    # Write input via pipe; run boos-shell in background, kill after delay.
    # We use a temp fifo so we can write a line and then close stdin,
    # which causes boos-shell's `read` to get EOF and exit the loop,
    # killing the shell naturally.
    fifo="/tmp/boos-test-fifo-$$"
    rm -f "$fifo"
    mkfifo "$fifo" 2>/dev/null || {
        # Fallback: no fifo support — pipe and kill after delay
        printf '%s\n' "$input" | run_with_timeout 2 /bin/boos-shell 2>/dev/null
        sleep 1
        cat /tmp/boos-test-args 2>/dev/null
        return
    }

    # Open fifo for writing in background, then run boos-shell reading from it
    {
        printf '%s\n' "$input"
    } > "$fifo" &
    writer=$!

    /bin/boos-shell < "$fifo" 2>/dev/null &
    shell_pid=$!

    # Give it time to process the one line and die on EOF
    sleep 2
    kill "$shell_pid" 2>/dev/null
    kill "$writer" 2>/dev/null
    wait "$shell_pid" 2>/dev/null
    wait "$writer" 2>/dev/null
    rm -f "$fifo"

    cat /tmp/boos-test-args 2>/dev/null
}

pass() {
    echo "PASS: $1"
    PASS=$((PASS + 1))
}

fail() {
    echo "FAIL: $1"
    FAIL=$((FAIL + 1))
}

# ── setup ────────────────────────────────────────────────────────────────

echo "=== boos-shell Security Tests ==="
echo ""

TESTDIR=$(mktemp -d)

ORIG_BOOS_EXEC_SAVED=""

cleanup() {
    # Restore original boos-exec
    if [ -n "$ORIG_BOOS_EXEC_SAVED" ] && [ -f "$TESTDIR/orig-boos-exec" ]; then
        cp "$TESTDIR/orig-boos-exec" /bin/boos-exec 2>/dev/null || true
    fi
    rm -rf "$TESTDIR"
    rm -f /tmp/boos-test-args /tmp/boos-test-fifo-*
}
trap cleanup EXIT INT TERM

# Save original /bin/boos-exec if it exists
if [ -f /bin/boos-exec ]; then
    cp /bin/boos-exec "$TESTDIR/orig-boos-exec"
    ORIG_BOOS_EXEC_SAVED=1
fi

# Install mock /bin/boos-exec that captures its arguments
cat > /bin/boos-exec << 'MOCKEOF'
#!/bin/sh
printf '%s\n' "$*" > /tmp/boos-test-args
MOCKEOF
chmod +x /bin/boos-exec

# Ensure /var/log exists (boos-shell logs there)
mkdir -p /var/log

# ── Test 1: Glob Injection (S1) ─────────────────────────────────────────

echo "--- Test 1: Glob Injection ---"

# Create a temp directory with marker files so that `*` would expand
# if globbing is active.
GLOBDIR="$TESTDIR/glob-test"
mkdir -p "$GLOBDIR"

# Create files with very distinctive names that would show up if expanded
touch "$GLOBDIR/___marker_a___"
touch "$GLOBDIR/___marker_b___"
touch "$GLOBDIR/___marker_c___"

# Run the test from inside the glob directory so `*` expands there
# if set -f is NOT active.
captured=$(cd "$GLOBDIR" && feed_and_capture "run *")

# Check: if glob expansion happened, the captured args will contain
# the marker filenames.  If the fix is in place (set -f or quoting),
# the args will contain the literal asterisk.
if echo "$captured" | grep -q '___marker_'; then
    fail "Test 1: Glob expansion detected — * expanded to filenames (got: $captured)"
else
    # Also verify we actually got output (not empty due to test infra failure)
    if [ -z "$captured" ]; then
        fail "Test 1: No output captured (test infrastructure issue)"
    else
        pass "Test 1: No glob injection — * preserved literally (got: $captured)"
    fi
fi

# ── Test 1b: Question-mark glob ─────────────────────────────────────────

echo "--- Test 1b: Question-mark Glob ---"

# Create files that would match ? patterns
touch "$GLOBDIR/X1" "$GLOBDIR/X2" "$GLOBDIR/Y1"

captured=$(cd "$GLOBDIR" && feed_and_capture "run X?")

if echo "$captured" | grep -q 'X1' || echo "$captured" | grep -q 'X2'; then
    fail "Test 1b: Glob expansion detected — ? expanded to filenames (got: $captured)"
else
    if [ -z "$captured" ]; then
        fail "Test 1b: No output captured (test infrastructure issue)"
    else
        pass "Test 1b: No glob injection — ? preserved literally (got: $captured)"
    fi
fi

# ── Test 2: Backslash Preservation (S2) ──────────────────────────────────

echo "--- Test 2: Backslash Preservation ---"

# Feed input containing a backslash before a non-special character.
# With `read` (no -r), the backslash would be consumed as an escape.
# With `read -r`, the backslash is preserved.
#
# We send: status hello\\world
# The printf sends two literal chars: \ and w (the shell sees \\\\ → \\)
# If read -r is used, the arg is: hello\\world (two backslashes? no...)
#
# Let's be very explicit.  printf '%s\n' sends the literal bytes.
# We send: status BShelloBSworld  where BS = literal backslash (0x5C)
# The printf format string: 'status \\hello\\world\n'  —
#   in printf, \\ in the format produces one literal \.
# So the actual bytes sent: status \hello\world newline
#
# boos-shell reads: status \hello\world
#   read -r:  $line = status \hello\world   (backslashes preserved)
#   read:     $line = status hello\world     (first \ consumed escaping 'h';
#                                             h is not special so it stays h;
#                                             second \ consumed escaping 'w')
# Wait — that's not right either.
#
# read without -r: \<char> produces <char> (backslash stripped).
# So \h → h, \w → w.
# Result: $line = status helloworld   (backslashes gone!)
#
# To make the test crystal clear, we use a backslash before a colon
# which is unambiguous: status foo\:bar
# read -r:  $line = status foo\:bar
# read:     $line = status foo:bar   (\: → :)
#
# We check: the captured args contain the literal backslash.

# Use a colon after the backslash for unambiguous detection
captured=$(feed_and_capture "status foo\\:bar")

if echo "$captured" | grep -q 'foo:bar' && ! echo "$captured" | grep -q 'foo\\:bar'; then
    fail "Test 2: Backslash was consumed — read is interpreting escapes (got: $captured)"
elif echo "$captured" | grep -q 'foo\\\\:bar' || echo "$captured" | grep -q 'foo\\:bar'; then
    pass "Test 2: Backslash preserved — no escape interpretation (got: $captured)"
else
    if [ -z "$captured" ]; then
        fail "Test 2: No output captured (test infrastructure issue)"
    else
        # We got something but it's not what we expected — likely the
        # backslash was stripped.  Treat as FAIL.
        fail "Test 2: Backslash NOT preserved — got: $captured"
    fi
fi

# ── Test 2b: Backslash-newline (line continuation) ──────────────────────

echo "--- Test 2b: Backslash-newline (continuation) ---"

# With `read` without -r, a trailing backslash means "continue on next line",
# which could be used to inject commands across line boundaries.
# Feed two lines: first ends with \, second has extra content.
# write the two lines to the fifo
rm -f /tmp/boos-test-args

fifo="/tmp/boos-test-fifo-$$"
rm -f "$fifo"
if mkfifo "$fifo" 2>/dev/null; then
    {
        printf '%s\n' 'status line1\'
        printf '%s\n' 'line2'
    } > "$fifo" &
    writer=$!

    /bin/boos-shell < "$fifo" 2>/dev/null &
    shell_pid=$!

    sleep 2
    kill "$shell_pid" 2>/dev/null
    kill "$writer" 2>/dev/null
    wait "$shell_pid" 2>/dev/null
    wait "$writer" 2>/dev/null
    rm -f "$fifo"
else
    # Fallback: just skip if fifo unavailable
    :
fi

captured=$(cat /tmp/boos-test-args 2>/dev/null)

# If backslash-newline continuation is active, the two lines get joined.
# "status line1\" + "line2" → "status line1line2"
# With read -r, they stay separate, and only "status line1\" is processed.
if echo "$captured" | grep -q 'line1line2'; then
    fail "Test 2b: Backslash-newline continuation active — lines were joined (got: $captured)"
elif echo "$captured" | grep -q 'line1\\'; then
    pass "Test 2b: Backslash-newline NOT interpreted as continuation — backslash preserved (got: $captured)"
else
    if [ -z "$captured" ]; then
        # No output is ambiguous but not a clear fail for this test
        echo "SKIP: Test 2b — no output captured (infra limitation)"
    else
        pass "Test 2b: Lines not joined — no continuation behavior (got: $captured)"
    fi
fi

# ── Test 3: Argument Preservation ───────────────────────────────────────

echo "--- Test 3: Multi-word Argument Preservation ---"

# Feed: submit status verbose
# Expect: all three words preserved as arguments
captured=$(feed_and_capture "submit status verbose")

if echo "$captured" | grep -q 'status.*verbose' || echo "$captured" | grep -q 'status verbose'; then
    pass "Test 3: Multi-word args preserved (got: $captured)"
else
    if [ -z "$captured" ]; then
        fail "Test 3: No output captured (test infrastructure issue)"
    else
        fail "Test 3: Arguments not preserved — got: $captured (expected: submit status verbose)"
    fi
fi

# ── Test 3b: Quoted-style args ──────────────────────────────────────────

echo "--- Test 3b: Arguments with spaces via word boundaries ---"

# Feed: run echo hello world
# Expect: echo hello world  (all args passed)
captured=$(feed_and_capture "run echo hello world")

if echo "$captured" | grep -q 'hello world' || echo "$captured" | grep -q 'hello.*world'; then
    pass "Test 3b: Multi-word args to run preserved (got: $captured)"
else
    if [ -z "$captured" ]; then
        fail "Test 3b: No output captured (test infrastructure issue)"
    else
        fail "Test 3b: Arguments not fully preserved — got: $captured"
    fi
fi

# ── results ─────────────────────────────────────────────────────────────

echo ""
echo "=== Results ==="
echo "Passed: $PASS"
echo "Failed: $FAIL"

if [ "$FAIL" -eq 0 ]; then
    echo ""
    echo "ALL TESTS PASSED"
    exit 0
else
    echo ""
    echo "SOME TESTS FAILED"
    exit 1
fi
