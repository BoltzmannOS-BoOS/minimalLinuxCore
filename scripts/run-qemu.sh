#!/bin/sh
set -e

KERNEL=build/vmlinuz

SHARED_DIR="${BOOS_SHARED_DIR:-/tmp/boos-shared}"
mkdir -p "$SHARED_DIR/requests"
mkdir -p "$SHARED_DIR/results"

GATEWAY_PORT="${BOOS_GATEWAY_PORT:-5555}"

qemu-system-x86_64 \
  -kernel "$KERNEL" \
  -initrd build/initramfs.cpio.gz \
  -append "console=ttyS0 rdinit=/init" \
  -drive file=build/var.img,format=raw,if=virtio,cache=directsync \
  -virtfs local,path="$SHARED_DIR",mount_tag=shared,security_model=none \
  -netdev user,id=net0,hostfwd=tcp::${GATEWAY_PORT}-:5555 \
  -device virtio-net,netdev=net0 \
  -nographic
