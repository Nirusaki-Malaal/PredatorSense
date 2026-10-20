#!/usr/bin/env bash
# Universal All-In-One Menu & Installer for PredatorSense (Acer Predator PHN16-71).
set -euo pipefail

REPO_TARBALL="https://github.com/Nirusaki-Malaal/PredatorSense/archive/refs/heads/main.tar.gz"

action=""
install_deps=0
userspace_only=0
check_only=0
kernel_target=$(uname -r)

# Parse CLI flags
for arg in "$@"; do
    case "$arg" in
        --install) action="install" ;;
        --update) action="update" ;;
        --uninstall) action="uninstall" ;;
        --check) check_only=1; action="check" ;;
        --install-deps) install_deps=1 ;;
        --userspace-only) userspace_only=1 ;;
        --kernel-release) shift; kernel_target="${1:-$(uname -r)}" ;;
        --help|-h)
            cat <<'EOF'
PredatorSense Universal All-In-One Control Script
Target Hardware: Acer Predator Helios Neo 16 (PHN16-71)

Usage:
  Interactive Menu:
    curl -fsSL https://raw.githubusercontent.com/Nirusaki-Malaal/PredatorSense/main/bootstrap.sh | bash

  Direct Commands:
    curl -fsSL https://raw.githubusercontent.com/Nirusaki-Malaal/PredatorSense/main/bootstrap.sh | bash -s -- --install
    curl -fsSL https://raw.githubusercontent.com/Nirusaki-Malaal/PredatorSense/main/bootstrap.sh | bash -s -- --update
    curl -fsSL https://raw.githubusercontent.com/Nirusaki-Malaal/PredatorSense/main/bootstrap.sh | bash -s -- --uninstall

Options:
  --install-deps        Automatically install toolchain & system build dependencies
  --userspace-only      Build/install daemon, tray and GTK studio (skips DKMS kernel module)
  --check               Perform read-only preflight hardware and dependency check
  --kernel-release VER  Build module for a specific installed kernel version
  --help, -h            Show this help dialog
EOF
            exit 0
            ;;
    esac
done

# Hardware check: strictly Acer Predator PHN16-71
vendor=$(cat /sys/class/dmi/id/sys_vendor 2>/dev/null || true)
product=$(cat /sys/class/dmi/id/product_name 2>/dev/null || true)

if [[ "$vendor" != "Acer" || "$product" != "Predator PHN16-71" ]]; then
    echo "=================================================================" >&2
    echo "ERROR: Unsupported hardware: $vendor $product." >&2
    echo "PredatorSense is specialized exclusively for Acer Predator PHN16-71." >&2
    echo "Generic models are excluded to maximize performance, stability, and reduce memory footprint." >&2
    echo "=================================================================" >&2
    exit 1
fi

# Interactive menu if no action specified
if [[ -z "$action" ]]; then
    # When piped via curl | bash, reconnect stdin to /dev/tty if available
    if [[ ! -t 0 ]] && [[ -c /dev/tty ]]; then
        exec </dev/tty
    fi

    if [[ -t 0 ]]; then
        cat <<'MENU'
========================================================================
     PredatorSense Control Suite — Acer Predator Helios Neo 16
========================================================================
  1) Install PredatorSense (Kernel module, daemon, GUI & tray)
  2) Update existing installation (Rebuild & refresh binaries)
  3) Uninstall PredatorSense (Restore backups & remove services)
  4) Run preflight system diagnostic check
  5) Exit
========================================================================
MENU
        read -r -p "Select an option [1-5]: " choice
        case "$choice" in
            1) action="install" ;;
            2) action="update" ;;
            3) action="uninstall" ;;
            4) action="check"; check_only=1 ;;
            5) echo "Cancelled."; exit 0 ;;
            *) echo "Invalid choice. Exiting."; exit 1 ;;
        esac
    else
        action="install"
    fi
fi

# Quick uninstall path if local repository exists
script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" 2>/dev/null && pwd || true)
if [[ "$action" == "uninstall" ]]; then
    if [[ -f "$script_dir/scripts/uninstall.sh" ]]; then
        echo "Running uninstaller from local repository..."
        exec sudo bash "$script_dir/scripts/uninstall.sh"
    elif [[ -x /usr/local/lib/predator-sense/uninstall.sh ]]; then
        echo "Running uninstaller from system path..."
        exec sudo bash /usr/local/lib/predator-sense/uninstall.sh
    fi
fi

# For install, update, check, or online uninstall, prepare source tree
workdir=$(mktemp -d /tmp/predator-sense-bootstrap.XXXXXXXX)
cleanup() { rm -rf "$workdir"; }
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

for tool in curl tar; do
    if ! command -v "$tool" >/dev/null 2>&1; then
        echo "Missing required tool: $tool. Please install it first." >&2
        exit 1
    fi
done

if [[ -f "$script_dir/install.sh" && -d "$script_dir/crates" && -d "$script_dir/kernel" ]]; then
    echo "Using local source tree at: $script_dir"
    src_dir="$script_dir"
else
    echo "Downloading latest PredatorSense release archive..."
    curl -fL --retry 3 --connect-timeout 15 --max-time 180 "$REPO_TARBALL" -o "$workdir/source.tar.gz"
    mkdir -p "$workdir/src"
    tar -xzf "$workdir/source.tar.gz" --strip-components=1 -C "$workdir/src"
    src_dir="$workdir/src"
fi

if [[ "$action" == "uninstall" ]]; then
    echo "Executing uninstaller..."
    exec sudo bash "$src_dir/scripts/uninstall.sh"
fi

install_flags=()
((check_only)) && install_flags+=(--check)
((install_deps)) && install_flags+=(--install-deps)
((userspace_only)) && install_flags+=(--userspace-only)
[[ -n "$kernel_target" ]] && install_flags+=(--kernel-release "$kernel_target")

if [[ "$action" == "update" ]]; then
    echo "========================================================================"
    echo " Updating PredatorSense (preserving user configurations and preferences)"
    echo "========================================================================"
    bash "$src_dir/install.sh" "${install_flags[@]}"
    echo "Restarting predator-sense service..."
    sudo systemctl restart predator-sense.service || true
    echo "PredatorSense updated successfully."
    exit 0
fi

# Default install / check
echo "========================================================================"
echo " Launching PredatorSense installation..."
echo "========================================================================"
bash "$src_dir/install.sh" "${install_flags[@]}"
