#!/bin/sh
set -eu

if [ "$#" -ne 5 ]; then
    echo "usage: $0 <initramfs> <expected-init> <expected-boos-binary> <expected-kernel> <expected-commit>" >&2
    exit 2
fi

archive=$1
expected_init=$2
expected_binary=$3
expected_kernel=$4
expected_commit=$5

absolute_file() {
    case "$1" in
        /*) printf '%s\n' "$1" ;;
        *) printf '%s/%s\n' "$(pwd -P)" "$1" ;;
    esac
}

archive=$(absolute_file "$archive")
expected_init=$(absolute_file "$expected_init")
expected_binary=$(absolute_file "$expected_binary")
expected_kernel=$(absolute_file "$expected_kernel")

for required_file in "$archive" "$expected_init" "$expected_binary" "$expected_kernel"; do
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
resident_principal="$work_dir/root/etc/boos/principals/resident.principal"
debug_principal="$work_dir/root/etc/boos/principals/debug.principal"
agent_daemon="$work_dir/root/etc/boos/daemons/agent.daemon"
gateway_daemon="$work_dir/root/etc/boos/daemons/gateway.daemon"

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

require_config_line() {
    config_file=$1
    expected_line=$2
    if [ ! -f "$config_file" ] || ! grep -Fqx "$expected_line" "$config_file"; then
        echo "initramfs config $(basename "$config_file") is missing: $expected_line" >&2
        exit 1
    fi
}

require_config_line "$resident_principal" "id=resident"
require_config_line "$resident_principal" "user=boos-agent"
require_config_line "$resident_principal" "uid=101"
require_config_line "$resident_principal" "gid=101"
require_config_line "$resident_principal" "enabled=1"

require_config_line "$debug_principal" "id=debug"
require_config_line "$debug_principal" "user=boos-gateway"
require_config_line "$debug_principal" "uid=100"
require_config_line "$debug_principal" "gid=100"
require_config_line "$debug_principal" "enabled=1"

require_config_line "$agent_daemon" "name=agent"
require_config_line "$agent_daemon" "exec=/bin/boos-agent resident"
require_config_line "$agent_daemon" "user=boos-agent"
require_config_line "$agent_daemon" "principal=resident"
require_config_line "$agent_daemon" "restart=always"
require_config_line "$agent_daemon" "enabled=1"

require_config_line "$gateway_daemon" "name=gateway"
require_config_line "$gateway_daemon" "user=boos-gateway"
require_config_line "$gateway_daemon" "principal=debug"
require_config_line "$gateway_daemon" "enabled=0"

kernel_release=$(
    strings "$expected_kernel" |
        sed -n 's/^\([0-9][^ ]*-virt\) .*/\1/p' |
        head -1
)
if [ -z "$kernel_release" ]; then
    echo "cannot identify the expected kernel release" >&2
    exit 1
fi
kernel_modules="$work_dir/root/lib/modules/$kernel_release"
if [ ! -f "$kernel_modules/modules.dep" ] ||
   [ ! -f "$kernel_modules/modules.builtin" ]; then
    echo "initramfs has no module metadata for kernel $kernel_release" >&2
    exit 1
fi
for required_module in virtio_blk ext4 virtio_net; do
    if ! grep -Eq "/${required_module}\\.ko(\\.gz)?:" "$kernel_modules/modules.dep" &&
       ! grep -Eq "/${required_module}\\.ko(\\.gz)?$" "$kernel_modules/modules.builtin"; then
        echo "initramfs cannot provide required module $required_module for $kernel_release" >&2
        exit 1
    fi
done

enabled_daemon_count=0
for daemon_config in "$work_dir"/root/etc/boos/daemons/*.daemon; do
    if grep -Fqx "enabled=1" "$daemon_config"; then
        enabled_daemon_count=$((enabled_daemon_count + 1))
    fi
done
if [ "$enabled_daemon_count" -ne 1 ]; then
    echo "product initramfs must enable exactly one daemon; found $enabled_daemon_count" >&2
    exit 1
fi

echo "verified initramfs for commit $expected_commit"
