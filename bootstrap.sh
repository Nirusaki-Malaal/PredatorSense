#!/bin/sh
# Universal All-In-One Menu & Installer for PredatorSense (Acer Predator PHN16-71).
set -eu

REPO_TARBALL="https://github.com/Nirusaki-Malaal/PredatorSense/archive/refs/heads/main.tar.gz"

action=""
install_deps=0
userspace_only=0
check_only=0
kernel_target=$(uname -r)

# Parse CLI arguments
while [ $# -gt 0 ]; do
    case "$1" in
        --install) action="install" ;;
        --update) action="update" ;;
        --uninstall) action="uninstall" ;;
        --check) check_only=1; action="check" ;;
        --install-deps) install_deps=1 ;;
        --userspace-only) userspace_only=1 ;;
        --kernel-release)
            shift
            if [ $# -gt 0 ]; then kernel_target="$1"; fi
            ;;
        --help|-h)
            cat <<'EOF'
PredatorSense Universal All-In-One Control Script
Target Hardware: Acer Predator Helios Neo 16 (PHN16-71)

Usage:
  Interactive Menu:
    curl -fsSL https://raw.githubusercontent.com/Nirusaki-Malaal/PredatorSense/main/bootstrap.sh | sh

  Direct Commands:
    curl -fsSL https://raw.githubusercontent.com/Nirusaki-Malaal/PredatorSense/main/bootstrap.sh | sh -s -- --install
    curl -fsSL https://raw.githubusercontent.com/Nirusaki-Malaal/PredatorSense/main/bootstrap.sh | sh -s -- --update
    curl -fsSL https://raw.githubusercontent.com/Nirusaki-Malaal/PredatorSense/main/bootstrap.sh | sh -s -- --uninstall

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
    shift
done

# Hardware check: strictly Acer Predator PHN16-71
vendor=$(cat /sys/class/dmi/id/sys_vendor 2>/dev/null || true)
product=$(cat /sys/class/dmi/id/product_name 2>/dev/null || true)

if [ "$vendor" != "Acer" ] || [ "$product" != "Predator PHN16-71" ]; then
    echo "=================================================================" >&2
    echo "ERROR: Unsupported hardware: $vendor $product." >&2
    echo "PredatorSense is specialized exclusively for Acer Predator PHN16-71." >&2
    echo "Generic models are excluded to maximize performance, stability, and reduce memory footprint." >&2
    echo "=================================================================" >&2
    exit 1
fi

# Interactive menu if no action was passed via flags
if [ -z "$action" ]; then
    if [ -c /dev/tty ]; then
        cat <<'MENU'
========================================================================
     PredatorSense Control Suite - Acer Predator Helios Neo 16
========================================================================
  1) Install PredatorSense (Kernel module, daemon, GUI & tray)
  2) Update existing installation (Rebuild & refresh binaries)
  3) Uninstall PredatorSense (Restore backups & remove services)
  4) Run preflight system diagnostic check
  5) Exit
========================================================================
MENU
        printf "Select an option [1-5]: "
        read -r choice </dev/tty || choice="5"
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

# Locate existing local source tree if available
script_dir=""
if [ -n "${BASH_SOURCE[0]:-}" ] && [ -f "${BASH_SOURCE[0]}" ]; then
    script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" 2>/dev/null && pwd || true)
elif [ -n "${0:-}" ] && [ -f "$0" ]; then
    script_dir=$(cd -- "$(dirname -- "$0")" 2>/dev/null && pwd || true)
fi

# Quick uninstall path if local uninstaller exists
if [ "$action" = "uninstall" ]; then
    if [ -n "$script_dir" ] && [ -f "$script_dir/scripts/uninstall.sh" ]; then
        echo "Running uninstaller from local repository..."
        if [ -c /dev/tty ]; then
            exec sudo bash "$script_dir/scripts/uninstall.sh" </dev/tty
        else
            exec sudo bash "$script_dir/scripts/uninstall.sh"
        fi
    elif [ -x /usr/local/lib/predator-sense/uninstall.sh ]; then
        echo "Running uninstaller from system path..."
        if [ -c /dev/tty ]; then
            exec sudo bash /usr/local/lib/predator-sense/uninstall.sh </dev/tty
        else
            exec sudo bash /usr/local/lib/predator-sense/uninstall.sh
        fi
    fi
fi

# Prepare source directory
workdir=""
if [ -n "$script_dir" ] && [ -f "$script_dir/install.sh" ] && [ -d "$script_dir/crates" ] && [ -d "$script_dir/kernel" ]; then
    echo "Using local source tree at: $script_dir"
    src_dir="$script_dir"
else
    for tool in curl tar; do
        if ! command -v "$tool" >/dev/null 2>&1; then
            echo "Missing required tool: $tool. Please install it first." >&2
            exit 1
        fi
    done

    workdir=$(mktemp -d /tmp/predator-sense-bootstrap.XXXXXXXX)
    cleanup() { if [ -n "$workdir" ] && [ -d "$workdir" ]; then rm -rf "$workdir"; fi; }
    trap cleanup EXIT INT TERM

    echo "Downloading latest PredatorSense release archive..."
    curl -fL --retry 3 --connect-timeout 15 --max-time 180 "$REPO_TARBALL" -o "$workdir/source.tar.gz"
    mkdir -p "$workdir/src"
    tar -xzf "$workdir/source.tar.gz" --strip-components=1 -C "$workdir/src"
    src_dir="$workdir/src"
fi

if [ "$action" = "uninstall" ]; then
    echo "Executing uninstaller..."
    if [ -c /dev/tty ]; then
        exec sudo bash "$src_dir/scripts/uninstall.sh" </dev/tty
    else
        exec sudo bash "$src_dir/scripts/uninstall.sh"
    fi
fi

flags=""
if [ "$check_only" -eq 1 ]; then flags="$flags --check"; fi
if [ "$install_deps" -eq 1 ]; then flags="$flags --install-deps"; fi
if [ "$userspace_only" -eq 1 ]; then flags="$flags --userspace-only"; fi
if [ -n "$kernel_target" ]; then flags="$flags --kernel-release $kernel_target"; fi

if [ "$action" = "update" ]; then
    echo "========================================================================"
    echo " Updating PredatorSense (preserving user configurations and preferences)"
    echo "========================================================================"
    if [ -c /dev/tty ]; then
        # shellcheck disable=SC2086
        bash "$src_dir/install.sh" $flags </dev/tty
    else
        # shellcheck disable=SC2086
        bash "$src_dir/install.sh" $flags
    fi
    echo "Restarting predator-sense service..."
    sudo systemctl restart predator-sense.service 2>/dev/null || true
    echo "PredatorSense updated successfully."
    exit 0
fi

# Default install / check
echo "========================================================================"
echo " Launching PredatorSense installation..."
echo "========================================================================"
if [ -c /dev/tty ]; then
    # shellcheck disable=SC2086
    bash "$src_dir/install.sh" $flags </dev/tty
else
    # shellcheck disable=SC2086
    bash "$src_dir/install.sh" $flags
fi
