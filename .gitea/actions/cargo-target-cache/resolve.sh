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

{
  echo "profiles<<EOF"
  printf "%s\n" "${profiles[@]/#/${dir}/}"
  echo "EOF"
  echo "flags=$(printf '%s' "${RUSTFLAGS-}" | sha256sum | cut -c1-16)"
} >>"${GITEA_OUTPUT}"
