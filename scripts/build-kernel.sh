#!/usr/bin/env bash
set -euo pipefail
source_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
kernel=${1:-$(uname -r)}
[[ "$kernel" =~ ^[[:alnum:]_.+-]+$ ]] || { echo 'Invalid kernel release' >&2; exit 2; }
headers=/lib/modules/$kernel/build
[[ -f "$headers/rust/libkernel.rmeta" ]] || { echo "Kernel $kernel lacks prepared Rust metadata. Install matching CONFIG_RUST=y headers." >&2; exit 1; }
[[ -f "$headers/include/linux/wmi.h" ]] && grep -q wmidev_invoke_method "$headers/include/linux/wmi.h" || { echo 'Modern buffer-based WMI API is required.' >&2; exit 1; }
if grep -Eq '^CONFIG_(GCC_PLUGIN_RANDSTRUCT|RANDSTRUCT_FULL|RANDSTRUCT_PERFORMANCE)=y' "$headers/.config"; then echo 'Randomized C struct layouts are not supported by these generated bindings.' >&2; exit 1; fi
stage=$(mktemp -d /tmp/predatrix-kbuild.XXXXXXXX)
trap 'find "$stage" -depth -delete' EXIT
install -m 644 "$source_dir/kernel/"*.rs "$source_dir/kernel/ffi.h" "$source_dir/kernel/Makefile" "$source_dir/kernel/Kbuild" "$source_dir/kernel/dkms.conf" "$stage/"
install -m 755 "$source_dir/scripts/generate-bindings.sh" "$source_dir/tests/abi_check.py" "$stage/"
make -C "$stage" KVER="$kernel"
install -d "$source_dir/artifacts/kernel/$kernel"
install -m 644 "$stage/predatrix_sense.ko" "$source_dir/artifacts/kernel/$kernel/"
modinfo "$source_dir/artifacts/kernel/$kernel/predatrix_sense.ko"
