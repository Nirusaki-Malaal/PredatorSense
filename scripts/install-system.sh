#!/usr/bin/env bash
# Privileged final phase. Cargo dependencies are built before entering this script.
set -euo pipefail
[[ $(cat /sys/class/dmi/id/sys_vendor) == Acer && $(cat /sys/class/dmi/id/product_name) == 'Predator PHN16-71' ]] || { echo 'Unsupported hardware: PredatorSense is specialized exclusively for Acer Predator PHN16-71.' >&2; exit 1; }
((EUID==0)) || { echo 'Root is required for the final installation phase.' >&2; exit 1; }
source_dir=${1:?Source directory required}
mode=${2:?Mode required}
account=${3:?User required}
kernel=${4:?Kernel required}
bindgen=${5:-}
[[ "$mode" == userspace || "$mode" == kernel ]] || exit 2
[[ "$kernel" =~ ^[[:alnum:]_.+-]+$ ]] || exit 2
getent passwd "$account" >/dev/null
version=0.6.1
state_dir=/var/lib/predator-sense
install -d -m 700 "$state_dir" "$state_dir/install-backup"
backup() {
    local target=$1
    local destination=$state_dir/install-backup${target}
    if [[ -e "$target" || -L "$target" ]] && [[ ! -e "$destination" && ! -L "$destination" ]]; then
        install -d "$(dirname -- "$destination")"
        cp -a -- "$target" "$destination"
    fi
}
if [[ "$mode" == kernel ]]; then
    dkms_version=$(dkms --version)
    [[ "$dkms_version" =~ dkms-([3-9]|[1-9][0-9]+)\. ]] || { echo 'DKMS 3 or newer is required.' >&2; exit 1; }
    [[ -x "$bindgen" ]] || { echo 'Missing bindgen executable.' >&2; exit 1; }
    install -d /usr/local/lib/predator-sense
    if ! cmp -s "$bindgen" /usr/local/lib/predator-sense/bindgen; then
        install -m 755 "$bindgen" /usr/local/lib/predator-sense/bindgen
    fi
    signing_report=$(umask 077; dkms generate_mok)
    printf '%s\n' "$signing_report"
    cert=$(printf '%s\n' "$signing_report" | sed -n 's/^Public certificate (MOK): //p' | tail -n 1)
    key=$(printf '%s\n' "$signing_report" | sed -n 's/^Signing key: //p' | tail -n 1)
    [[ "$cert" == /* && -s "$cert" ]] || { echo 'Cannot locate the native DKMS certificate. Check /etc/dkms/framework.conf.' >&2; exit 1; }
    if [[ "$key" == /* && -f "$key" ]]; then chmod 600 "$key"; fi
    printf '%s\n' "$cert" > "$state_dir/signing-certificate.path"
    module_source=/usr/src/predatrix_sense-$version
    if [[ -d "$module_source" ]]; then
        for file in "$source_dir/kernel/"*.rs "$source_dir/kernel/ffi.h" "$source_dir/kernel/Makefile" "$source_dir/kernel/Kbuild" "$source_dir/kernel/dkms.conf"; do
            cmp -s "$file" "$module_source/$(basename -- "$file")" || { echo 'Registered source differs. Increment the release version or uninstall this release before replacing it.' >&2; exit 1; }
        done
    else
        install -d -m 755 "$module_source"
        install -m 644 "$source_dir/kernel/"*.rs "$source_dir/kernel/ffi.h" "$source_dir/kernel/Makefile" "$source_dir/kernel/Kbuild" "$source_dir/kernel/dkms.conf" "$module_source/"
        install -m 755 "$source_dir/scripts/generate-bindings.sh" "$source_dir/tests/abi_check.py" "$module_source/"
    fi
    if ! dkms status -m predatrix_sense -v "$version" | grep -q .; then dkms add -m predatrix_sense -v "$version"; fi
    dkms build -m predatrix_sense -v "$version" -k "$kernel"
    dkms install -m predatrix_sense -v "$version" -k "$kernel"
    signer=$(modinfo -k "$kernel" -F signer predatrix_sense)
    [[ -n "$signer" ]] || { echo 'DKMS did not sign the module. Check its signing policy before activation.' >&2; exit 1; }
    printf 'Installed module signer: %s\n' "$signer"
    if command -v mokutil >/dev/null && mokutil --sb-state 2>/dev/null | grep -q 'SecureBoot enabled'; then
        if ! mokutil --test-key "$cert" >/dev/null 2>&1; then
            if [[ -t 0 ]]; then mokutil --import "$cert"; fi
            printf 'Enrollment needed: sudo mokutil --import %q\nComplete MOK enrollment during reboot.\n' "$cert"
        fi
    fi
fi
getent group predatrix_sense >/dev/null || groupadd --system predatrix_sense
usermod -aG predatrix_sense "$account"
# Stop competing firmware writers before starting the native daemon.
for service in predatrix-daemon.service predator-sense.service Predatrix-Daemon.service; do
    if [[ "$service" != predator-sense.service ]] && systemctl is-enabled --quiet "$service"; then
        printf '%s\n' "$service" >> "$state_dir/enabled-services"
        systemctl disable "$service"
    fi
    if systemctl is-active --quiet "$service"; then
        printf '%s\n' "$service" >> "$state_dir/stopped-services"
        systemctl stop "$service"
    fi
done
install -d /usr/local/bin /usr/local/share/applications /usr/local/share/icons/hicolor/scalable/apps /usr/local/lib/predator-sense
for binary in predator-daemon predatorctl predator-sense predator-tray; do
    backup "/usr/local/bin/$binary"
    install -m 755 "$source_dir/target/release/$binary" "/usr/local/bin/$binary"
done
backup /etc/systemd/system/predator-sense.service
if [[ -L /etc/systemd/system/predator-sense.service ]]; then unlink /etc/systemd/system/predator-sense.service; fi
install -m 644 "$source_dir/packaging/predator-sense.service" /etc/systemd/system/predator-sense.service
backup /usr/local/share/applications/predator-sense.desktop
install -m 644 "$source_dir/packaging/predator-sense.desktop" /usr/local/share/applications/
backup /usr/local/share/icons/hicolor/scalable/apps/predator-sense.svg
install -m 644 "$source_dir/packaging/predator-sense.svg" /usr/local/share/icons/hicolor/scalable/apps/
install -m 755 "$source_dir/scripts/activate-kernel.sh" "$source_dir/scripts/uninstall.sh" /usr/local/lib/predator-sense/
account_home=$(getent passwd "$account" | cut -d: -f6)
account_group=$(id -gn "$account")
install -d -o "$account" -g "$account_group" "$account_home/.config/autostart"
backup "$account_home/.config/autostart/predator-sense-tray.desktop"
install -m 644 -o "$account" -g "$account_group" "$source_dir/packaging/predator-sense-tray.desktop" "$account_home/.config/autostart/"
systemctl daemon-reload
systemctl enable --now predator-sense.service
printf '%s\n' 'Installed. Log out and back in for group permissions and tray autostart.' 'Open Predator Sense from your application menu. GNOME requires a StatusNotifier/AppIndicator extension.'
if [[ "$mode" == kernel ]]; then
    echo 'No running module was unloaded. The installed predatrix_sense replacement takes effect on the next module load/reboot.'
    echo 'If migrating from acer_wmi, use scripts/activate-kernel.sh --next-boot and read README.md.'
fi
