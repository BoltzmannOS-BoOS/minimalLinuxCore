#!/bin/sh
# Keep the historical suite path while sharing the canonical regression check.
set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
exec "$SCRIPT_DIR/../../verify-shell-fixes.sh"
