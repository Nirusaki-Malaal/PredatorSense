#!/usr/bin/env bash
# Configure exclusive WMI ownership for next boot; never unload live hardware.
set -euo pipefail
[[ $(cat /sys/class/dmi/id/sys_vendor) == Acer && $(cat /sys/class/dmi/id/product_name) == 'Predator PHN16-71' ]] || { echo 'Unsupported hardware: PredatorSense is specialized exclusively for Acer Predator PHN16-71.' >&2; exit 1; }
if ((EUID!=0)); then exec sudo bash "$0" "$@"; fi
[[ ${1:-} == --next-boot ]] || { echo 'Usage: sudo bash scripts/activate-kernel.sh --next-boot' >&2; exit 2; }
[[ $(modinfo -F version predatrix_sense) == 0.6.1 ]] || { echo 'The Rust predatrix_sense 0.6.1 module is not installed for this kernel.' >&2; exit 1; }
[[ -n $(modinfo -F signer predatrix_sense) ]] || { echo 'Module is unsigned; refusing activation.' >&2; exit 1; }
if [[ -d /sys/firmware/efi ]]; then
    command -v mokutil >/dev/null || { echo 'Install mokutil to establish Secure Boot state.' >&2; exit 1; }
    secure_boot=$(mokutil --sb-state) || { echo 'Cannot establish Secure Boot state.' >&2; exit 1; }
    if [[ "$secure_boot" == *'SecureBoot enabled'* ]]; then
        certificate=$(cat /var/lib/predator-sense/signing-certificate.path)
        mokutil --test-key "$certificate" >/dev/null || { echo 'Complete MOK enrollment before activation.' >&2; exit 1; }
    fi
fi
backup=/var/lib/predator-sense/activation-backup
install -d -m 700 "$backup"
for file in /etc/modprobe.d/predator-sense-exclusive.conf /etc/modules-load.d/predator-sense.conf; do
    if [[ -e "$file" ]] && ! grep -q '^# Managed by Predator Sense' "$file"; then echo "Existing configuration is not owned by this installer: $file" >&2; exit 1; fi
done
for name in linuwu_sense.conf linuwu-sense.conf; do
    file=/etc/modules-load.d/$name
    if [[ -e "$file" ]]; then
        [[ ! -e "$backup/$name" ]] || { echo "Backup already exists: $name" >&2; exit 1; }
        mv "$file" "$backup/$name"
    fi
done
if systemctl is-enabled --quiet linuwu_sense.service; then printf '%s\n' linuwu_sense.service > "$backup/enabled-service"; systemctl disable linuwu_sense.service; fi
install -d /etc/modprobe.d /etc/modules-load.d
cat > /etc/modprobe.d/predator-sense-exclusive.conf <<'EXCLUSIVE'
# Managed by Predator Sense
blacklist acer_wmi
blacklist linuwu_sense
EXCLUSIVE
cat > /etc/modules-load.d/predator-sense.conf <<'LOAD'
# Managed by Predator Sense
predatrix_sense
LOAD
printf '%s\n' 'Rust predatrix_sense scheduled for next boot. No live module was changed.' 'If an Acer WMI driver is embedded in initramfs, regenerate it using your distribution tool before reboot.' 'Rollback: sudo bash /usr/local/lib/predator-sense/uninstall.sh --kernel-only'
