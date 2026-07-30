#!/bin/sh
set -eu

if [ "$#" -ne 6 ]; then
    echo "usage: $0 <base-initramfs> <rootfs> <boos-binary> <output> <commit> <source-date-epoch>" >&2
    exit 2
fi

base_initramfs=$1
rootfs_dir=$2
boos_binary=$3
output=$4
build_commit=$5
source_date_epoch=$6

absolute_path() {
    case "$1" in
        /*) printf '%s\n' "$1" ;;
        *) printf '%s/%s\n' "$(pwd -P)" "$1" ;;
    esac
}

base_initramfs=$(absolute_path "$base_initramfs")
rootfs_dir=$(absolute_path "$rootfs_dir")
boos_binary=$(absolute_path "$boos_binary")
output=$(absolute_path "$output")

for required_file in "$base_initramfs" "$rootfs_dir/init" "$boos_binary"; do
    if [ ! -e "$required_file" ]; then
        echo "missing build input: $required_file" >&2
        exit 1
    fi
done

case "$build_commit" in
    *[!0-9a-f]* | "")
        echo "build commit must be a lowercase hexadecimal Git object ID" >&2
        exit 1
        ;;
esac
case "$source_date_epoch" in
    *[!0-9]* | "")
        echo "source date epoch must be a non-negative integer" >&2
        exit 1
        ;;
esac

work_dir=$(mktemp -d "${TMPDIR:-/tmp}/boos-assemble.XXXXXX")
cleanup() {
    case "$work_dir" in
        "${TMPDIR:-/tmp}"/boos-assemble.*)
            rm -rf -- "$work_dir"
            ;;
    esac
}
trap cleanup EXIT HUP INT TERM

merged_root="$work_dir/root"
mkdir "$merged_root"
(
    cd "$merged_root"
    gzip -dc "$base_initramfs" | cpio -idm 2>/dev/null
)

cp -a "$rootfs_dir/." "$merged_root/"
install -m 0755 "$boos_binary" "$merged_root/bin/boos"

# The project rootfs intentionally supplies its own static BusyBox. Its modprobe
# cannot pass Alpine's compressed modules to the kernel, so normalize the
# modules at the image boundary and discard the now-stale binary dependency
# index. BusyBox modprobe reads the updated text index.
if [ -d "$merged_root/lib/modules" ]; then
    find "$merged_root/lib/modules" -type f -name '*.ko.gz' -exec gzip -d {} \;
    find "$merged_root/lib/modules" -type f \
        \( -name modules.dep -o -name modules.order \) \
        -exec sed -i 's/\.ko\.gz/.ko/g' {} \;
    find "$merged_root/lib/modules" -type f -name modules.dep.bin -delete
fi

for command_name in exec process submit gateway supervisor shell agent; do
    ln -sfn boos "$merged_root/bin/boos-$command_name"
done

sha256_file() {
    sha256sum "$1" | awk '{print $1}'
}

mkdir -p "$merged_root/etc/boos"
{
    printf 'commit=%s\n' "$build_commit"
    printf 'binary_sha256=%s\n' "$(sha256_file "$boos_binary")"
    printf 'init_sha256=%s\n' "$(sha256_file "$rootfs_dir/init")"
} > "$merged_root/etc/boos/build-info"

chmod 0755 "$merged_root/init"
find "$merged_root" -xdev -exec touch -h -d "@$source_date_epoch" {} +

output_dir=$(dirname "$output")
mkdir -p "$output_dir"
temporary_output="$output.tmp"
(
    cd "$merged_root"
    find . -xdev -print0 |
        LC_ALL=C sort -z |
        cpio --null --reproducible --owner=0:0 -H newc -o 2>/dev/null
) | gzip -n > "$temporary_output"
mv "$temporary_output" "$output"

echo "assembled $output for commit $build_commit"
