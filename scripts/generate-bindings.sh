#!/usr/bin/env bash
set -euo pipefail
headers=${1:?Kernel headers required}
output=${2:?Output path required}
script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
if [[ -f "$script_dir/ffi.h" ]]; then input=$script_dir/ffi.h; else input=$script_dir/../kernel/ffi.h; fi
bindgen=${BINDGEN:-bindgen}
arch=${SRCARCH:-x86}
RUST_LOG=error "$bindgen" "$input" \
    --use-core --no-layout-tests --no-doc-comments --no-derive-debug \
    --no-prepend-enum-name --generate functions,types,vars \
    --allowlist-type 'kernel_param|kernel_param_ops|device_attribute|attribute_group|wmi_.*|input_dev|key_entry|platform_.*|hwmon_.*|dmi_header|acpi_.*|device' \
    --allowlist-function '(__wmi_driver_register|wmi_driver_unregister|wmidev_.*|platform_.*|__platform_.*|input_.*|__input_.*|sparse_keymap_.*|hwmon_.*|devm_hwmon_.*|dmi_.*|acpi_.*|kthread_.*|ktime_get_mono_fast_ns|wake_up_process|msleep|msleep_interruptible|schedule_timeout_interruptible|param_.*|sysfs_.*|kobject_uevent_env|kfree|bus_find_device|device_match_name|put_device|get_device)' \
    --allowlist-var 'KEY_.*|EV_.*|ABS_.*|SW_.*|BUS_HOST|SYN_REPORT|HWMON_.*|PLATFORM_.*|DMI_.*|ACPI_.*|platform_bus_type|param_ops_.*|CONFIG_HZ' \
    --output "$output" -- \
    -D__KERNEL__ -DMODULE -D__BINDGEN__ -funsigned-char -fshort-wchar -fms-extensions \
    -Wno-microsoft-anon-tag -Wno-ignored-attributes -Wno-address-of-packed-member -Wno-duplicate-decl-specifier \
    -I"$headers/arch/$arch/include" -I"$headers/arch/$arch/include/generated" \
    -I"$headers/include" -I"$headers/arch/$arch/include/uapi" \
    -I"$headers/arch/$arch/include/generated/uapi" -I"$headers/include/uapi" \
    -I"$headers/include/generated/uapi" -include "$headers/include/linux/kconfig.h" \
    -include "$headers/include/linux/compiler_types.h"
