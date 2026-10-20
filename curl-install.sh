#!/usr/bin/env bash
# Universal curl entrypoint for PredatorSense (Acer Predator PHN16-71).
# Forwards to bootstrap.sh.
set -euo pipefail
script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
if [[ -f "$script_dir/bootstrap.sh" ]]; then
    exec bash "$script_dir/bootstrap.sh" "$@"
fi
exec curl -fsSL "https://raw.githubusercontent.com/Nirusaki-Malaal/PredatorSense/main/bootstrap.sh" | bash -s -- "$@"
