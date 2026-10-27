#!/usr/bin/env bash
# Cross-compile release binaries for every target into its BUILD_DIR stage dir.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/common.sh"

: "${TARGETS:?TARGETS is required}"
: "${PACKAGE:?PACKAGE is required}"
: "${BUILD_DIR:=build}"
: "${CARGO_TARGET_DIR:=target}"

targets_parse "${TARGETS}"

binary="$(binary_name)"

# Single invocation; parallel ones would just wait on the build directory lock.
args=()
targets_cargo_args args
units="$(mktemp)"
cargo zbr -p "${PACKAGE}" "${args[@]}" --message-format=json-render-diagnostics >"${units}"

rm -rf "${BUILD_DIR}"
for i in "${!TARGET_RUST[@]}"; do
  target="${TARGET_RUST[$i]}"
  src="${CARGO_TARGET_DIR}/${target}/release/${binary}${TARGET_EXTS[$i]}"
  if [ ! -f "${src}" ]; then
    echo "Expected binary not found: ${src}" >&2
    exit 1
  fi
  dest="${BUILD_DIR}/$(stage_name "$i")/${binary}${TARGET_EXTS[$i]}"
  install -Dm755 "${src}" "${dest}"
  echo "${target} -> ${dest} ($(du -h "${src}" | cut -f1))"
done

if [ -n "${GITEA_OUTPUT-}" ]; then
  echo "units=${units}" >>"${GITEA_OUTPUT}"
fi
