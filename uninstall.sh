#!/usr/bin/env bash
# Uninstaller for PredatorSense (Acer Predator PHN16-71).
set -euo pipefail
script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
exec sudo bash "$script_dir/scripts/uninstall.sh" "$@"
