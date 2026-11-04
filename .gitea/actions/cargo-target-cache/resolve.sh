#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/../../scripts/common.sh"

: "${GITEA_OUTPUT:?GITEA_OUTPUT is required}"

dir="${CARGO_TARGET_DIR:-target}"
profiles=(debug)
if [ -n "$(trim "${TARGETS-}")" ]; then
  targets_parse "${TARGETS}"
  # Host build scripts and proc macros build into release/.
  profiles=(release "${TARGET_RUST[@]/%//release}")
fi

zig=none
if command -v zig >/dev/null; then
  zig="$(zig version)"
fi

{
  echo "zig=${zig}"
  echo "flags=$(printf '%s' "${RUSTFLAGS-}" | fingerprint)"
} >>"${GITEA_OUTPUT}"
printf '%s\n' "${profiles[@]/#/${dir}/}" | output_lines profiles
