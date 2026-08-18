#!/usr/bin/env bash
# Cross-compile release binaries for every target into DIST_DIR.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/common.sh"

: "${TARGETS:?TARGETS is required}"
: "${PACKAGE:?PACKAGE is required}"
: "${DIST_DIR:=dist}"
: "${CARGO_TARGET_DIR:=target}"

targets_parse "${TARGETS}"

# One invocation: concurrent ones would serialise on the build directory lock.
args=()
for target in "${TARGET_RUST[@]}"; do
  args+=(--target "${target}")
done

cargo zigbuild --release --locked -p "${PACKAGE}" "${args[@]}"

rm -rf "${DIST_DIR}"
for target in "${TARGET_RUST[@]}"; do
  src="${CARGO_TARGET_DIR}/${target}/release/${PACKAGE}"
  if [ ! -f "${src}" ]; then
    echo "Expected binary not found: ${src}" >&2
    exit 1
  fi
  install -Dm755 "${src}" "${DIST_DIR}/${target}/${PACKAGE}"
  echo "${target} -> ${DIST_DIR}/${target}/${PACKAGE} ($(du -h "${src}" | cut -f1))"
done
