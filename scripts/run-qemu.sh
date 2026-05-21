#!/bin/sh
set -e

KERNEL=build/vmlinuz

qemu-system-x86_64 \
  -kernel "$KERNEL" \
  -initrd build/initramfs.cpio.gz \
  -append "console=ttyS0 rdinit=/init" \
  -nographic
