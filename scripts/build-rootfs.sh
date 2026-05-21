#!/bin/sh
set -e

mkdir -p build

cd rootfs
find . | cpio -H newc -o | gzip > ../build/initramfs.cpio.gz
cd ..

echo "Built build/initramfs.cpio.gz"
