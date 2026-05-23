#!/bin/sh
set -e

KERNEL=build/vmlinuz

SHARED_DIR="${BOOS_SHARED_DIR:-/tmp/boos-shared}"
mkdir -p "$SHARED_DIR/requests"
mkdir -p "$SHARED_DIR/results"

qemu-system-x86_64 \
  -kernel "$KERNEL" \
  -initrd build/initramfs.cpio.gz \
  -append "console=ttyS0 rdinit=/init" \
  -virtfs local,path="$SHARED_DIR",mount_tag=shared,security_model=none \
  -nographic
