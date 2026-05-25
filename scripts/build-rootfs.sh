#!/bin/sh
set -e

# Build Rust components if source exists
if [ -f src/rust/Cargo.toml ]; then
    echo "=== Building Rust components ==="
    sh scripts/build-rust.sh
fi

# Clean up shell scripts that have been replaced by Rust symlinks
# (boos-exec, boos-process, boos-submit, boos-gateway are now symlinks → boos)
for name in boos-exec boos-process boos-submit boos-gateway; do
    target="rootfs/bin/$name"
    if [ -L "$target" ]; then
        echo "  $name → Rust (symlink)"
    fi
done

# Remove boos-handler (replaced by boos-gateway TcpListener)
rm -f rootfs/bin/boos-handler

mkdir -p build

# Build initramfs
cd rootfs
find . | cpio -H newc -o | gzip > ../build/initramfs.cpio.gz
cd ..

echo "Built build/initramfs.cpio.gz"

# Create persistent /var disk image (one-time)
DISK_IMG=build/var.img
if [ ! -f "$DISK_IMG" ]; then
    echo "Creating persistent disk image ($DISK_IMG)..."
    dd if=/dev/zero of="$DISK_IMG" bs=1M count=64 2>/dev/null
    mkfs.ext2 -F "$DISK_IMG" 2>/dev/null
    echo "Created: $DISK_IMG (64MB ext2)"
else
    echo "Persistent disk exists: $DISK_IMG"
fi
