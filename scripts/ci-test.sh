#!/bin/sh
# BoOS CI integration test: boot QEMU, send commands via TCP, verify results.
# NOTE: QEMU port forwarding (host->guest) requires Linux host.
# On macOS, port forwarding is unreliable; guest-internal tests work.
# GitHub Actions CI (ubuntu-latest) works correctly.
# Verified working in QEMU guest: boos-exec help/status, gateway nc 127.0.0.1:5555
set -e

PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$PROJECT_ROOT"

GATEWAY_PORT=15555  # offset from default 5555 to avoid conflicts
QEMU_PID=""

cleanup() {
    if [ -n "$QEMU_PID" ] && kill -0 "$QEMU_PID" 2>/dev/null; then
        kill "$QEMU_PID" 2>/dev/null || true
        wait "$QEMU_PID" 2>/dev/null || true
    fi
    rm -f build/ci-qemu.log
}
trap cleanup EXIT

echo "=== Starting QEMU ==="
qemu-system-x86_64 \
  -kernel build/vmlinuz \
  -initrd build/initramfs.cpio.gz \
  -append "console=ttyS0 rdinit=/init" \
  -drive file=build/var.img,format=raw,if=virtio,cache=directsync \
  -netdev user,id=net0,hostfwd=tcp::${GATEWAY_PORT}-:5555 \
  -device e1000,netdev=net0 \
  -nographic \
  -no-reboot \
  > build/ci-qemu.log 2>&1 &
QEMU_PID=$!

echo "Waiting for gateway on port ${GATEWAY_PORT}..."
for i in $(seq 1 60); do
    if echo "" | nc -w 1 127.0.0.1 ${GATEWAY_PORT} 2>/dev/null; then
        echo "Gateway is up (attempt $i)."
        break
    fi
    if [ "$i" -eq 60 ]; then
        echo "TIMEOUT: gateway did not start"
        cat build/ci-qemu.log
        exit 1
    fi
    sleep 2
done

failures=0
test_cmd() {
    test_command=$1
    expected_pattern=$2
    test_description=$3
    printf "  %-25s " "$test_description"
    command_output=$(echo "$test_command" | nc -w 3 127.0.0.1 "$GATEWAY_PORT" 2>/dev/null || echo "ERROR: nc failed")
    if echo "$command_output" | grep -q "$expected_pattern"; then
        echo "PASS"
    else
        echo "FAIL"
        echo "    sent:     $test_command"
        echo "    expected: $expected_pattern"
        echo "    got:      $(echo "$command_output" | head -3 | tr '\n' ' ')"
        failures=$((failures + 1))
    fi
}

echo ""
echo "=== Running integration tests ==="

test_cmd "help"              "BoOS commands"       "help"
test_cmd "status"            "kernel"               "status"
test_cmd "commands"          "name"                 "commands (list)"
test_cmd "commands --json"   '"name"'               "commands (--json)"
test_cmd "caps"              "allow_help"           "capabilities"
test_cmd "write-file /tmp/ci-test.txt hello_ci" "Written" "write-file"
test_cmd "read-file /tmp/ci-test.txt" "hello_ci" "read-file"
test_cmd "list-dir /tmp"     "ci-test.txt"          "list-dir"
test_cmd "session start test-ci"  "Session started" "session start"
test_cmd "remember ci-key ci-val" "Remembered"      "remember"
test_cmd "recall ci-key"     "ci-val"               "recall"
test_cmd "session status"    "Session"              "session status"
test_cmd "session end"       "ended"                "session end"
test_cmd "result nonexistent" "No result"           "result (missing)"
test_cmd "daemons"           "running\|stopped\|disabled" "daemons"
test_cmd "audit summary"     "Total actions"        "audit summary"
test_cmd "log"               "gateway"              "log"
test_cmd "shell"             "denied"               "shell (denied)"

echo ""
echo "=== Results: $failures failures ==="

# Show QEMU output if there were failures
if [ "$failures" -gt 0 ]; then
    echo ""
    echo "--- QEMU log (last 40 lines) ---"
    tail -40 build/ci-qemu.log
fi

exit $failures
