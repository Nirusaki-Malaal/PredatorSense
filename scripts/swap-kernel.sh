#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' 'Live module swapping is disabled during active operation.' 'Install 0.6.1, stop old firmware daemons, then use activate-kernel.sh --next-boot.' >&2
exit 2
