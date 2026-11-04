#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/../../scripts/common.sh"

: "${GITEA_OUTPUT:?GITEA_OUTPUT is required}"
: "${TOOLS:=}"

rust_targets=""
if [ -n "$(trim "${TARGETS-}")" ]; then
  targets_parse "${TARGETS}"
  rust_targets="$(csv TARGET_RUST)"
fi

supported=()
read_lines supported "${SUPPORTED-}"

wanted=()
read_lines wanted "${TOOLS//,/$'\n'}"

specs=()
paths=()
zig=false
cargo_bin="${CARGO_HOME:-${HOME}/.cargo}/bin"

for name in "${wanted[@]+"${wanted[@]}"}"; do
  version=""
  for entry in "${supported[@]+"${supported[@]}"}"; do
    if [ "${entry%%=*}" = "${name}" ]; then
      version="${entry#*=}"
      break
    fi
  done

  if [ -z "${version}" ]; then
    echo "Unsupported tool: ${name}" >&2
    exit 1
  fi

  specs+=("${name}@${version}")
  paths+=("${cargo_bin}/${name}")
  [ "${name}" != "cargo-zigbuild" ] || zig=true
done

{
  echo "rust_targets=${rust_targets}"
  echo "zig=${zig}"
  echo "specs=$(csv specs)"
  echo "fingerprint=$(printf '%s\n' "${specs[@]+"${specs[@]}"}" | sort | fingerprint)"
} >>"${GITEA_OUTPUT}"
printf '%s\n' "${paths[@]+"${paths[@]}"}" | output_lines paths
