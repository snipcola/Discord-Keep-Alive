#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/common.sh"

bg_start Deny cargo dn
bg_start Shear cargo sh

rc=0
cargo c || rc=$?
bg_wait || rc=1
exit "${rc}"
