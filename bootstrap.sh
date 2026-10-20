#!/usr/bin/env bash
# Universal online installer for Acer Predator Helios Neo 16 (PHN16-71).
set -euo pipefail

REPO_TARBALL="https://github.com/Nirusaki-Malaal/PredatorSense/archive/refs/heads/main.tar.gz"

help=false
dry_run=false
for opt in "$@"; do
    case "$opt" in
        --help|-h) help=true ;;
        --dry-run) dry_run=true ;;
    esac
done

if [[ "$help" == true ]]; then
    cat <<'EOF'
PredatorSense Universal Installer
Usage:
  curl -fsSL https://raw.githubusercontent.com/Nirusaki-Malaal/PredatorSense/main/bootstrap.sh | sh

Options:
  --install-deps          Automatically install build dependencies via package manager
  --userspace-only        Install daemon, CLI, GTK studio and tray without DKMS module
  --check                 Perform preflight checks without installing
  --kernel-release VER    Target specific kernel version (default: current uname -r)
  --dry-run               Display target host information and exit
  --help, -h              Display this help message

Pass options with: curl -fsSL <URL> | sh -s -- --install-deps
EOF
    exit 0
fi

# Hardware specialization verification:
# PredatorSense is stripped down specifically to support Acer Predator Helios Neo 16 (PHN16-71)
vendor=$(cat /sys/class/dmi/id/sys_vendor 2>/dev/null || true)
product=$(cat /sys/class/dmi/id/product_name 2>/dev/null || true)

if [[ "$vendor" != "Acer" || "$product" != "Predator PHN16-71" ]]; then
    echo "Unsupported hardware: $vendor $product." >&2
    echo "PredatorSense is specialized exclusively for Acer Predator Helios Neo 16 (PHN16-71)." >&2
    echo "Generic models are stripped out to maximize performance, stability, and reduce memory footprint." >&2
    exit 1
fi

if [[ "$dry_run" == true ]]; then
    echo "Hardware: $vendor $product (Verified Acer Predator Helios Neo 16)"
    echo "Kernel: $(uname -r)"
    echo "Archive: $REPO_TARBALL"
    echo "Plan: Download source, verify Rust/GTK environment, compile release binaries, build & sign DKMS module."
    exit 0
fi

[[ "$(id -u)" != 0 ]] || { echo "Run as your desktop user without sudo. Sudo will be requested during system installation." >&2; exit 1; }

for tool in curl tar; do
    if ! command -v "$tool" >/dev/null 2>&1; then
        echo "Missing required tool: $tool. Please install it first." >&2
        exit 1
    fi
done

workdir=$(mktemp -d /tmp/predator-sense-bootstrap.XXXXXXXX)
cleanup() { rm -rf "$workdir"; }
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

echo "Downloading PredatorSense source for Acer Predator PHN16-71..."
curl -fL --retry 3 --connect-timeout 15 --max-time 180 "$REPO_TARBALL" -o "$workdir/source.tar.gz"

echo "Extracting archive..."
mkdir -p "$workdir/src"
tar -xzf "$workdir/source.tar.gz" --strip-components=1 -C "$workdir/src"

installer="$workdir/src/install.sh"
[[ -f "$installer" ]] || { echo "Invalid source archive: install.sh missing." >&2; exit 1; }
chmod +x "$installer"

echo "Executing PredatorSense installer..."
if ( : </dev/tty ) 2>/dev/null; then
    bash "$installer" "$@" </dev/tty
else
    bash "$installer" "$@" </dev/null
fi
