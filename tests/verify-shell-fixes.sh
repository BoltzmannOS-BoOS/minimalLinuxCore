#!/bin/sh
# Compatibility entry point for the former shell-security regression test.
# The runtime is now the Rust multicall binary, so verify the replacement
# boundary instead of grepping removed shell implementations.
set -eu

REPO_ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
PASS=0
FAIL=0

pass() {
    echo "  PASS: $1"
    PASS=$((PASS + 1))
}

fail() {
    echo "  FAIL: $1"
    FAIL=$((FAIL + 1))
}

check_text() {
    label=$1
    pattern=$2
    file=$3
    if grep -qF "$pattern" "$REPO_ROOT/$file"; then
        pass "$label"
    else
        fail "$label — $pattern not found in $file"
    fi
}

check_absent() {
    label=$1
    path=$2
    if [ ! -e "$REPO_ROOT/$path" ] && [ ! -L "$REPO_ROOT/$path" ]; then
        pass "$label"
    else
        fail "$label — obsolete path still exists: $path"
    fi
}

check_link() {
    label=$1
    path=$2
    expected=$3
    actual=$(readlink "$REPO_ROOT/$path" 2>/dev/null || true)
    if [ "$actual" = "$expected" ]; then
        pass "$label"
    else
        fail "$label — expected link to $expected, found ${actual:-not-a-link}"
    fi
}

echo "=== Rust runtime boundary verification ==="

check_link "boos-shell is a multicall link" "rootfs/bin/boos-shell" "boos"
check_link "boos-supervisor is a multicall link" "rootfs/bin/boos-supervisor" "boos"
check_absent "obsolete shell queue daemon removed" "rootfs/bin/boos-daemon"
check_absent "obsolete processor daemon config removed" \
    "rootfs/etc/boos/daemons/processor.daemon"

check_text "supervisor owns queue processing" \
    "crate::process::main();" "src/rust/src/supervisor.rs"
check_text "queue polling remains configurable" \
    "POLL_INTERVAL=1" "rootfs/etc/boos/daemon.conf"
check_text "resident principal starts by default" \
    "principal=resident" "rootfs/etc/boos/daemons/agent.daemon"
check_text "gateway remains an optional debug adapter" \
    "enabled=0" "rootfs/etc/boos/daemons/gateway.daemon"

echo
echo "Passed: $PASS"
echo "Failed: $FAIL"

if [ "$FAIL" -ne 0 ]; then
    exit 1
fi

echo "ALL CHECKS PASSED"
