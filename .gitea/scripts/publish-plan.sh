#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/common.sh"

: "${PUBLISH_LOCAL:?PUBLISH_LOCAL is required}"
: "${PUBLISH_GHCR:?PUBLISH_GHCR is required}"
: "${PUBLISH_DOCKERHUB:?PUBLISH_DOCKERHUB is required}"
: "${GITEA_OUTPUT:?GITEA_OUTPUT is required}"

any=false
refs=()

add_dest() {
  local ref="${1-}"
  if [ -z "${ref}" ]; then
    echo "Ref is empty for an enabled publish destination." >&2
    exit 1
  fi
  if ! ref_is_base "${ref}"; then
    echo "Ref must be an image name base (no tag/digest): ${ref}" >&2
    exit 1
  fi
  any=true
  refs+=("${ref}")
}

if [ "${PUBLISH_LOCAL}" = "true" ]; then
  add_dest "${LOCAL_REF-}"
fi
if [ "${PUBLISH_GHCR}" = "true" ]; then
  add_dest "${GHCR_REF-}"
fi
if [ "${PUBLISH_DOCKERHUB}" = "true" ]; then
  add_dest "${DOCKER_REF-}"
fi

driver_opts="${DOCKER_NETWORK:+network=${DOCKER_NETWORK}}"

{
  echo "any=${any}"
  echo "driver_opts=${driver_opts}"
} >>"${GITEA_OUTPUT}"
printf '%s\n' "${refs[@]+"${refs[@]}"}" | output_lines refs
