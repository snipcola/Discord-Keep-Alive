#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/../../scripts/common.sh"

: "${GITEA_OUTPUT:?GITEA_OUTPUT is required}"

join_segments() {
  local out="" seg
  for seg in "$@"; do
    seg="$(trim "${seg}")"
    [ -n "${seg}" ] || continue
    if [ -n "${out}" ]; then
      out="${out}-${seg}"
    else
      out="${seg}"
    fi
  done
  printf '%s' "${out}"
}

prefix="$(trim "${INPUT_PREFIX-}")"
suffix="$(trim "${INPUT_SUFFIX-}")"
key_core="$(trim "${INPUT_KEY-}")"
if [ -z "${key_core}" ]; then
  echo "cache key is empty" >&2
  exit 1
fi

platform_seg=
if [ "${INPUT_PLATFORM-}" = "true" ]; then
  platform_seg="runner-${RUNNER_OS:-unknown}-${RUNNER_ARCH:-unknown}"
fi

full_key="$(join_segments "${prefix}" "${platform_seg}" "${key_core}" "${suffix}")"
if [ -z "${full_key}" ]; then
  echo "resolved cache key is empty" >&2
  exit 1
fi

{
  echo "key=${full_key}"
  echo "restore-keys<<EOF"
  while IFS= read -r line || [ -n "${line}" ]; do
    line="$(trim "${line}")"
    [ -n "${line}" ] || continue
    printf '%s\n' "$(join_segments "${prefix}" "${platform_seg}" "${line}" "${suffix}")"
  done <<<"${INPUT_RESTORE_KEYS-}"
  echo "EOF"
} >>"${GITEA_OUTPUT}"
