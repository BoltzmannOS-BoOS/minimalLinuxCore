#!/bin/sh
set -eu

if [ "$#" -ne 4 ]; then
    echo "usage: $0 <initramfs> <expected-init> <expected-boos-binary> <expected-commit>" >&2
    exit 2
fi

archive=$1
expected_init=$2
expected_binary=$3
expected_commit=$4

absolute_file() {
    case "$1" in
        /*) printf '%s\n' "$1" ;;
        *) printf '%s/%s\n' "$(pwd -P)" "$1" ;;
    esac
}

archive=$(absolute_file "$archive")
expected_init=$(absolute_file "$expected_init")
expected_binary=$(absolute_file "$expected_binary")

for required_file in "$archive" "$expected_init" "$expected_binary"; do
    if [ ! -f "$required_file" ]; then
        echo "missing required file: $required_file" >&2
        exit 1
    fi
done

case "$expected_commit" in
    *[!0-9a-f]* | "")
        echo "expected commit must be a lowercase hexadecimal Git object ID" >&2
        exit 1
        ;;
esac

work_dir=$(mktemp -d "${TMPDIR:-/tmp}/boos-initramfs.XXXXXX")
cleanup() {
    case "$work_dir" in
        "${TMPDIR:-/tmp}"/boos-initramfs.*)
            rm -rf -- "$work_dir"
            ;;
    esac
}
trap cleanup EXIT HUP INT TERM

gzip -dc "$archive" | cpio -it 2>/dev/null > "$work_dir/entries"
if grep -Eq '(^/|(^|/)\.\.(/|$))' "$work_dir/entries"; then
    echo "initramfs contains an unsafe path" >&2
    exit 1
fi

mkdir "$work_dir/root"
(
    cd "$work_dir/root"
    gzip -dc "$archive" | cpio -idm 2>/dev/null
)

actual_init="$work_dir/root/init"
actual_binary="$work_dir/root/bin/boos"
build_info="$work_dir/root/etc/boos/build-info"

if [ ! -x "$actual_init" ]; then
    echo "initramfs /init is missing or not executable" >&2
    exit 1
fi
if ! cmp -s "$expected_init" "$actual_init"; then
    echo "initramfs /init does not match the repository rootfs/init" >&2
    exit 1
fi

if [ ! -x "$actual_binary" ]; then
    echo "initramfs /bin/boos is missing or not executable" >&2
    exit 1
fi
if ! cmp -s "$expected_binary" "$actual_binary"; then
    echo "initramfs /bin/boos does not match the binary built by this job" >&2
    exit 1
fi

for command_name in exec process submit gateway supervisor shell agent; do
    command_path="$work_dir/root/bin/boos-$command_name"
    if [ ! -L "$command_path" ] || [ "$(readlink "$command_path")" != "boos" ]; then
        echo "invalid multicall link: /bin/boos-$command_name" >&2
        exit 1
    fi
done

if [ ! -f "$build_info" ]; then
    echo "initramfs is missing /etc/boos/build-info" >&2
    exit 1
fi

sha256_file() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | awk '{print $1}'
    else
        shasum -a 256 "$1" | awk '{print $1}'
    fi
}

binary_sha256=$(sha256_file "$expected_binary")
init_sha256=$(sha256_file "$expected_init")

grep -Fqx "commit=$expected_commit" "$build_info"
grep -Fqx "binary_sha256=$binary_sha256" "$build_info"
grep -Fqx "init_sha256=$init_sha256" "$build_info"

echo "verified initramfs for commit $expected_commit"
