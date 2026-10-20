#!/usr/bin/env bash
set -euo pipefail
source_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
mode=kernel
check=0
install_deps=0
kernel=$(uname -r)
usage() {
    cat <<'HELP'
Usage: ./install.sh [--check] [--userspace-only] [--install-deps]
                    [--kernel-release RELEASE]

Default: build the Rust kernel port, GTK UI, tray, CLI and PHN16-71 daemon;
install with native DKMS signing. The live kernel driver is never unloaded.
--check           Read-only prerequisite report; do not build or install.
--userspace-only  Install UI/daemon over an already installed sysfs driver.
--install-deps    Install build dependencies with apt/dnf/pacman/zypper and
                  install a pinned bindgen in your own cache if it is missing.
--kernel-release  Build for these installed kernel headers (default: uname -r).

Rust kernel modules need CONFIG_RUST=y, matching compiler/headers and modern WMI
APIs. Unsupported kernels fail preflight; no kernel configuration is replaced.
Secure Boot enrollment may require a password and reboot-time confirmation.
HELP
}
while (($#)); do
    case "$1" in
        --check) check=1 ;;
        --userspace-only) mode=userspace ;;
        --kernel) mode=kernel ;;
        --install-deps) install_deps=1 ;;
        --kernel-release) shift; kernel=${1:?Missing release} ;;
        --help|-h) usage; exit 0 ;;
        *) usage >&2; exit 2 ;;
    esac
    shift
done
[[ $(cat /sys/class/dmi/id/sys_vendor 2>/dev/null) == Acer && $(cat /sys/class/dmi/id/product_name 2>/dev/null) == 'Predator PHN16-71' ]] || { echo 'PredatorSense supports only Acer Predator PHN16-71.' >&2; exit 1; }
[[ $(uname -s) == Linux ]] || { echo 'Linux is required.' >&2; exit 1; }
[[ "$kernel" =~ ^[[:alnum:]_.+-]+$ ]] || { echo 'Invalid kernel release' >&2; exit 2; }
if ((install_deps && !check)); then
    ((EUID!=0)) || { echo 'Run as a regular user; sudo is used only for system installation.' >&2; exit 1; }
    if command -v pacman >/dev/null; then sudo pacman -S --needed --noconfirm base-devel rust gtk4 pkgconf clang python dkms openssl mokutil;
    elif command -v apt-get >/dev/null; then sudo apt-get update; sudo apt-get install -y build-essential cargo rustc libgtk-4-dev pkg-config libclang-dev python3 dkms openssl mokutil;
    elif command -v dnf >/dev/null; then sudo dnf install -y gcc gcc-c++ cargo rust gtk4-devel pkgconf-pkg-config clang-devel python3 dkms openssl mokutil make;
    elif command -v zypper >/dev/null; then sudo zypper --non-interactive install gcc gcc-c++ cargo rust gtk4-devel pkgconf-pkg-config clang-devel python3 dkms openssl mokutil make;
    else echo 'No supported package manager found. Install prerequisites using your distribution.' >&2; exit 1; fi
fi
missing=0
for tool in cargo rustc cc pkg-config make systemctl install python3; do
    if ! command -v "$tool" >/dev/null; then echo "Missing: $tool" >&2; missing=1; fi
done
if ! pkg-config --atleast-version=4.10 gtk4 2>/dev/null; then echo 'GTK 4.10 or newer development files are required.' >&2; missing=1; fi
[[ -d /run/systemd/system ]] || { echo 'This installer requires systemd.' >&2; missing=1; }
bindgen=${BINDGEN:-}
if [[ "$mode" == kernel ]]; then
    for tool in dkms openssl modinfo; do
        if ! command -v "$tool" >/dev/null; then echo "Missing: $tool" >&2; missing=1; fi
    done
    if [[ -z "$bindgen" ]]; then
        if [[ -x /usr/local/lib/predator-sense/bindgen ]]; then
            bindgen=/usr/local/lib/predator-sense/bindgen
        else
            bindgen=$(command -v bindgen || true)
        fi
    fi
    if [[ -z "$bindgen" && "$install_deps" == 1 && "$check" == 0 && "$missing" == 0 ]]; then
        tools_dir=${XDG_CACHE_HOME:-$HOME/.cache}/predator-sense/tools
        cargo install bindgen-cli --version 0.73.2 --locked --root "$tools_dir"
        bindgen=$tools_dir/bin/bindgen
    fi
    if [[ -z "$bindgen" || ! -x "$bindgen" ]]; then echo 'bindgen is required; use --install-deps or set BINDGEN to its executable.' >&2; missing=1; fi
    headers=/lib/modules/$kernel/build
    if [[ ! -f "$headers/rust/libkernel.rmeta" ]]; then echo "Kernel $kernel lacks prepared Rust metadata; install matching CONFIG_RUST=y headers." >&2; missing=1; fi
    if [[ ! -f "$headers/include/linux/wmi.h" ]] || ! grep -q wmidev_invoke_method "$headers/include/linux/wmi.h"; then echo 'Modern WMI buffer APIs are required.' >&2; missing=1; fi
fi
if ((missing)); then echo 'Preflight failed. No build or installation was attempted. Use --install-deps for build tools; kernel headers/toolchain must match the target kernel.' >&2; exit 1; fi
printf 'Preflight passed: %s, kernel %s, GTK %s\n' "$mode" "$kernel" "$(pkg-config --modversion gtk4)"
((check)) && exit 0
((EUID!=0)) || { echo 'Run as a regular user so Cargo dependencies are not built as root.' >&2; exit 1; }
cargo build --manifest-path "$source_dir/Cargo.toml" --workspace --release --locked
if [[ "$mode" == kernel ]]; then BINDGEN="$bindgen" "$source_dir/scripts/build-kernel.sh" "$kernel"; fi
sudo bash "$source_dir/scripts/install-system.sh" "$source_dir" "$mode" "$(id -un)" "$kernel" "$bindgen"
