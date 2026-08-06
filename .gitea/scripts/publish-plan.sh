#!/usr/bin/env bash
# Publish flags → refs, driver opts, cache-from/to.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=docker-targets.sh
source "${SCRIPT_DIR}/docker-targets.sh"

: "${PUBLISH_LOCAL:?PUBLISH_LOCAL is required}"
: "${PUBLISH_GHCR:?PUBLISH_GHCR is required}"
: "${PUBLISH_DOCKERHUB:?PUBLISH_DOCKERHUB is required}"
: "${REGISTRY_CACHE:?REGISTRY_CACHE is required}"
: "${IMAGE_NAME:?IMAGE_NAME is required}"
: "${GITHUB_OUTPUT:?GITHUB_OUTPUT is required}"

any=false
refs=()
local_ref=""

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

cache_slug() {
  local s="${1-}"
  s="${s//\//-}"
  s="$(printf '%s' "${s}" | tr -c 'A-Za-z0-9._-' '-')"
  s="$(printf '%s' "${s}" | tr -s '-')"
  s="${s#-}"
  s="${s%-}"
  if [ -z "${s}" ]; then
    s="local"
  fi
  printf '%s' "${s}"
}

# feature/foo vs feature-foo slug to the same string without this.
cache_name_hash() {
  printf '%s' "${1-}" | sha256sum | cut -c1-8
}

cache_branch_id() {
  printf '%s-%s' "$(cache_slug "${1-}")" "$(cache_name_hash "${1-}")"
}

registry_cache_tag() {
  local prefix="$1" raw_name="$2" os="$3" arch="$4"
  local max=128
  local slug h fixed room
  slug="$(cache_slug "${raw_name}")"
  h="$(cache_name_hash "${raw_name}")"
  fixed=$((${#prefix} + ${#h} + ${#os} + ${#arch} + 4))
  if [ "${fixed}" -ge "${max}" ]; then
    echo "registry cache tag fixed segments exceed ${max} chars: ${prefix}-…-${h}-${os}-${arch}" >&2
    exit 1
  fi
  room=$((max - fixed))
  if [ "${#slug}" -gt "${room}" ]; then
    slug="${slug:0:room}"
    slug="${slug%-}"
    if [ -z "${slug}" ]; then
      slug="x"
    fi
  fi
  printf '%s-%s-%s-%s-%s' "${prefix}" "${slug}" "${h}" "${os}" "${arch}"
}

if [ "${PUBLISH_LOCAL}" = "true" ]; then
  local_ref="${LOCAL_REF-}"
  add_dest "${local_ref}"
fi
if [ "${PUBLISH_GHCR}" = "true" ]; then
  add_dest "${GHCR_REF-}"
fi
if [ "${PUBLISH_DOCKERHUB}" = "true" ]; then
  add_dest "${DOCKER_REF-}"
fi

driver_opts="${DOCKER_NETWORK:+network=${DOCKER_NETWORK}}"

gha_scope="${IMAGE_NAME}"
if [ -n "${RUNNER_OS-}" ] && [ -n "${RUNNER_ARCH-}" ]; then
  gha_scope="${IMAGE_NAME}-${RUNNER_OS}-${RUNNER_ARCH}"
fi
cache_from=("type=gha,scope=${gha_scope}")
cache_to=("type=gha,mode=max,scope=${gha_scope}")

if [ "${REGISTRY_CACHE}" = "true" ] && [ -n "${local_ref}" ]; then
  cache_prefix="$(cache_slug "${REGISTRY_CACHE_TAG:-buildcache}")"
  export_mode="${REGISTRY_CACHE_EXPORT:-always}"
  case "${export_mode}" in
    always | base | never) ;;
    *)
      echo "REGISTRY_CACHE_EXPORT must be always|base|never (got: ${export_mode})" >&2
      exit 1
      ;;
  esac

  branch_raw="${GITHUB_REF_NAME:-}"
  base_raw="${REGISTRY_CACHE_BASE_BRANCH:-main}"
  branch_id="$(cache_branch_id "${branch_raw}")"
  base_id="$(cache_branch_id "${base_raw}")"
  runner_os="${RUNNER_OS:-unknown}"
  runner_arch="${RUNNER_ARCH:-unknown}"

  self_tag="$(registry_cache_tag "${cache_prefix}" "${branch_raw}" "${runner_os}" "${runner_arch}")"
  self_ref="${local_ref}:${self_tag}"
  cache_from+=("type=registry,ref=${self_ref},ignore-error=true")

  if [ "${branch_id}" != "${base_id}" ]; then
    base_tag="$(registry_cache_tag "${cache_prefix}" "${base_raw}" "${runner_os}" "${runner_arch}")"
    cache_from+=("type=registry,ref=${local_ref}:${base_tag},ignore-error=true")
  fi

  do_export=false
  case "${export_mode}" in
    always) do_export=true ;;
    base)
      if [ "${branch_id}" = "${base_id}" ]; then
        do_export=true
      fi
      ;;
    never) do_export=false ;;
  esac
  if [ "${do_export}" = "true" ]; then
    cache_to+=("type=registry,ref=${self_ref},mode=max,ignore-error=true")
  fi
fi

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
} >> "${GITHUB_OUTPUT}"
