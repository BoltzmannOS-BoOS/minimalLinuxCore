#!/bin/sh
set -e

# Source cargo environment
. "$HOME/.cargo/env"

PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ROOTFS="$PROJECT_ROOT/rootfs"
RUST_DIR="$PROJECT_ROOT/src/rust"

cd "$RUST_DIR"

echo "Building BoOS Rust components..."
cargo build --release --target x86_64-unknown-linux-musl

BIN="target/x86_64-unknown-linux-musl/release/boos"
cp "$BIN" "$BIN.tmp" && mv "$BIN.tmp" "$BIN"  # force mtime update

SIZE=$(stat -c%s "$BIN" 2>/dev/null || stat -f%z "$BIN" 2>/dev/null)
echo "  -> $BIN ($SIZE bytes)"

# Install to rootfs
cp "$BIN" "$ROOTFS/bin/boos"

# Create symlinks for multi-call dispatch
cd "$ROOTFS/bin"
for name in boos-exec boos-process boos-submit boos-gateway boos-supervisor; do
    ln -sf boos "$name"
done

echo "  -> installed to rootfs/bin/ (boos + 4 symlinks)"
echo "Done."
