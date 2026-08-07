#!/usr/bin/env bash
# Publish flags → refs, driver opts, cache-from/to.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/docker-targets.sh"

: "${PUBLISH_LOCAL:?PUBLISH_LOCAL is required}"
: "${PUBLISH_GHCR:?PUBLISH_GHCR is required}"
: "${PUBLISH_DOCKERHUB:?PUBLISH_DOCKERHUB is required}"
: "${IMAGE_NAME:?IMAGE_NAME is required}"
: "${GITEA_OUTPUT:?GITEA_OUTPUT is required}"

any=false
refs=()

add_dest() {
  local ref="${1-}"
  if [ -z "${ref}" ]; then
    echo "Ref is empty for an enabled publish destination." >&2
    exit 1
  fi
  if ! docker_ref_is_base "${ref}"; then
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

DOCKER_NETWORK="${INPUT_DOCKER_NETWORK:-${DOCKER_NETWORK-}}"
driver_opts="${DOCKER_NETWORK:+network=${DOCKER_NETWORK}}"

gha_scope="${IMAGE_NAME}"
if [ -n "${RUNNER_OS-}" ] && [ -n "${RUNNER_ARCH-}" ]; then
  gha_scope="${IMAGE_NAME}-runner-${RUNNER_OS}-${RUNNER_ARCH}"
fi
cache_from=("type=gha,scope=${gha_scope}")
cache_to=("type=gha,mode=max,scope=${gha_scope}")

{
  echo "any=${any}"
  echo "driver_opts=${driver_opts}"
  echo "refs<<EOF"
  if [ "${#refs[@]}" -gt 0 ]; then
    printf '%s\n' "${refs[@]}"
  fi
  echo "EOF"
  echo "cache_from<<EOF"
  printf '%s\n' "${cache_from[@]}"
  echo "EOF"
  echo "cache_to<<EOF"
  printf '%s\n' "${cache_to[@]}"
  echo "EOF"
} >>"${GITEA_OUTPUT}"
