#!/usr/bin/env bash
set -euo pipefail
if ((EUID!=0)); then exec sudo bash "$0" "$@"; fi
kernel_only=0
[[ ${1:-} == --kernel-only ]] && kernel_only=1
state_dir=/var/lib/predator-sense
for file in /etc/modprobe.d/predator-sense-exclusive.conf /etc/modules-load.d/predator-sense.conf; do
    if [[ -f "$file" ]] && grep -q '^# Managed by Predator Sense' "$file"; then unlink "$file"; fi
done
for name in linuwu_sense.conf linuwu-sense.conf; do
    backup=$state_dir/activation-backup/$name
    target=/etc/modules-load.d/$name
    if [[ -f "$backup" ]]; then
        [[ ! -e "$target" ]] || { echo "Refusing to overwrite $target during rollback." >&2; exit 1; }
        mv "$backup" "$target"
    fi
done
if [[ -f "$state_dir/activation-backup/enabled-service" ]]; then systemctl enable linuwu_sense.service; unlink "$state_dir/activation-backup/enabled-service"; fi
if command -v dkms >/dev/null && dkms status -m predatrix_sense -v 0.6.1 | grep -q .; then
    dkms remove -m predatrix_sense -v 0.6.1 --all
    previous=$(dkms status -m predatrix_sense | awk -F '[/,]' '$2!="0.6.1" {print $2}' | sort -V | tail -n 1)
    if [[ -n "$previous" ]]; then dkms install -m predatrix_sense -v "$previous" -k "$(uname -r)"; fi
fi
if ((kernel_only)); then echo 'Kernel installation reverted. Regenerate initramfs if previously updated, then reboot.'; exit 0; fi
systemctl disable --now predator-sense.service || true
restore() {
    local target=$1
    local backup=$state_dir/install-backup${target}
    if [[ -f "$target" || -L "$target" ]]; then unlink "$target"; fi
    if [[ -e "$backup" || -L "$backup" ]]; then cp -a -- "$backup" "$target"; fi
}
for file in /usr/local/bin/predator-daemon /usr/local/bin/predatorctl /usr/local/bin/predator-sense /usr/local/bin/predator-tray /etc/systemd/system/predator-sense.service /usr/local/share/applications/predator-sense.desktop /usr/local/share/icons/hicolor/scalable/apps/predator-sense.svg; do restore "$file"; done
account=${SUDO_USER:-}
if [[ -n "$account" && "$account" != root ]]; then
    account_home=$(getent passwd "$account" | cut -d: -f6)
    restore "$account_home/.config/autostart/predator-sense-tray.desktop"
fi
systemctl daemon-reload
if [[ -f "$state_dir/enabled-services" ]]; then
    while IFS= read -r service; do case "$service" in predatrix-daemon.service|Predatrix-Daemon.service) systemctl enable "$service" ;; esac; done < "$state_dir/enabled-services"
    unlink "$state_dir/enabled-services"
fi
if [[ -f "$state_dir/stopped-services" ]]; then
    while IFS= read -r service; do case "$service" in predatrix-daemon.service|Predatrix-Daemon.service|predator-sense.service) systemctl start "$service" || true ;; esac; done < "$state_dir/stopped-services"
    unlink "$state_dir/stopped-services"
fi
printf '%s\n' 'Application files removed and backed-up originals restored.' 'Preferences, original binary state files, native DKMS keys and group membership are preserved.' 'The current kernel module remains loaded until reboot; a running tray exits at logout.'
