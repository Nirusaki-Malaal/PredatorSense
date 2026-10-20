#!/bin/sh
# Universal curl entrypoint for PredatorSense (Acer Predator PHN16-71).
# Forwards to bootstrap.sh.
set -eu
script_dir=""
if [ -n "${BASH_SOURCE[0]:-}" ] && [ -f "${BASH_SOURCE[0]}" ]; then
    script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" 2>/dev/null && pwd || true)
elif [ -n "${0:-}" ] && [ -f "$0" ]; then
    script_dir=$(cd -- "$(dirname -- "$0")" 2>/dev/null && pwd || true)
fi

if [ -n "$script_dir" ] && [ -f "$script_dir/bootstrap.sh" ]; then
    exec sh "$script_dir/bootstrap.sh" "$@"
fi

curl -fsSL "https://raw.githubusercontent.com/Nirusaki-Malaal/PredatorSense/main/bootstrap.sh" | sh -s -- "$@"
