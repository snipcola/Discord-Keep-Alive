#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/../../scripts/common.sh"

: "${MANIFEST_PATH:=Cargo.toml}"

cargo_metadata() {
  local root="$1"
  if [ ! -f "${root}/${MANIFEST_PATH}" ]; then
    echo "Manifest not found: ${root}/${MANIFEST_PATH}" >&2
    return 1
  fi

  cargo metadata --format-version 1 --no-deps --manifest-path "${root}/${MANIFEST_PATH}" || {
    echo "cargo metadata failed for ${root}/${MANIFEST_PATH}" >&2
    return 1
  }
}

resolve_package_name() {
  local meta="$1" name

  if [ -n "${PACKAGE_NAME-}" ]; then
    printf '%s\n' "${PACKAGE_NAME}"
    return 0
  fi

  name="$(
    printf '%s\n' "${meta}" | jq -r '
      . as $m
      | ($m.workspace_default_members // [])
      | if length == 0 then
          error("workspace has no default members")
        elif length > 1 then
          error("workspace has multiple default members; pass package_name explicitly")
        else .[0] end
      | . as $id
      | ($m.packages[] | select(.id == $id) | .name)
    '
  )" || {
    echo "Could not resolve default package name from ${MANIFEST_PATH}." >&2
    return 1
  }

  if [ -z "${name}" ] || [ "${name}" = "null" ]; then
    echo "Could not resolve default package name from ${MANIFEST_PATH}." >&2
    return 1
  fi

  printf '%s\n' "${name}"
}

package_field() {
  local meta="$1" name="$2" field="$3" value
  value="$(
    printf '%s\n' "${meta}" | jq -r --arg name "${name}" --arg field "${field}" '
      .packages[] | select(.name == $name) | .[$field] // empty
    ' | head -n 1
  )"
  if [ -z "${value}" ] || [ "${value}" = "null" ]; then
    value=""
  fi
  printf '%s\n' "${value}"
}

package_version() {
  local meta="$1" name="$2" version
  version="$(package_field "${meta}" "${name}" version)"
  if [ -z "${version}" ]; then
    echo "Package ${name} not found in ${MANIFEST_PATH}." >&2
    return 1
  fi
  printf '%s\n' "${version}"
}

package_version_at() {
  local commit="$1" name="$2" tree meta version rc=0

  tree="$(mktemp -d)"
  rmdir "${tree}"

  if ! git worktree add --detach "${tree}" "${commit}" >/dev/null 2>&1; then
    echo "Could not create worktree for ${commit}; refusing to publish." >&2
    return 1
  fi

  meta="$(cargo_metadata "${tree}")" || rc=$?
  if [ "${rc}" -eq 0 ]; then
    version="$(package_version "${meta}" "${name}")" || rc=$?
  fi
  git worktree remove --force "${tree}" 2>/dev/null || true
  rm -rf "${tree}"

  if [ "${rc}" -ne 0 ]; then
    return "${rc}"
  fi
  printf '%s\n' "${version}"
}

meta="$(cargo_metadata .)" || {
  echo "Could not load cargo metadata from HEAD; refusing to publish." >&2
  exit 1
}

package_name="$(resolve_package_name "${meta}")" || exit 1
version="$(package_version "${meta}" "${package_name}")" || {
  echo "Could not resolve ${package_name} version from HEAD; refusing to publish." >&2
  exit 1
}
license="$(package_field "${meta}" "${package_name}" license)"
repository="$(package_field "${meta}" "${package_name}" repository)"

reason=""
prev=""
parent=""
if [ "${FORCE_RELEASE}" = "true" ]; then
  reason="force-release"
else
  parents="$(git show -s --format=%P HEAD)"
  if [ -z "${parents}" ]; then
    reason="no-previous-version"
  else
    parent="${parents%% *}"
    if ! git cat-file -e "${parent}^{commit}" 2>/dev/null; then
      echo "First parent ${parent} is not available locally (need fetch-depth >= 2); refusing to publish." >&2
      exit 1
    fi

    prev="$(package_version_at "${parent}" "${package_name}")" || {
      echo "Could not resolve ${package_name} version from ${parent}; refusing to publish." >&2
      exit 1
    }

    if [ "${prev}" != "${version}" ]; then
      reason="version:${prev}->${version}"
    elif git log -1 --format=%s HEAD | grep -qF "${RELEASE_MARKER}"; then
      reason="commit-marker"
    fi
  fi
fi

if [ -n "${reason}" ]; then
  changed=true
else
  changed=false
fi

image_name="${IMAGE_NAME:-${package_name}}"
local_user="${LOCAL_USER:-}"
local_registry="${LOCAL_REGISTRY:-}"
if [ -z "${local_registry}" ] && [ -n "${SERVER_URL-}" ]; then
  local_registry="${SERVER_URL}"
fi
local_registry="$(normalize_host "${local_registry}")"

local_ref=""
if [ -n "${local_registry}" ] && [ -n "${local_user}" ]; then
  local_ref="${local_registry}/${local_user}/${image_name}"
elif [ "${PUBLISH_LOCAL:-true}" = "true" ]; then
  if [ -z "${local_registry}" ]; then
    echo "Local registry host is required (local_registry or SERVER_URL)." >&2
    exit 1
  fi
  echo "Local registry user is required." >&2
  exit 1
else
  local_registry=""
fi

: "${GH_USER:?GH_USER is required}"
: "${DH_USER:?DH_USER is required}"
ghcr_ref="ghcr.io/${GH_USER}/${image_name}"
docker_ref="docker.io/${DH_USER}/${image_name}"

{
  echo "package_name=${package_name}"
  echo "version=${version}"
  echo "changed=${changed}"
  echo "reason=${reason}"
  echo "image_name=${image_name}"
  echo "license=${license}"
  echo "repository=${repository}"
  echo "local_registry=${local_registry}"
  echo "local_user=${local_user}"
  echo "local=${local_ref}"
  echo "ghcr=${ghcr_ref}"
  echo "docker=${docker_ref}"
} >>"${GITEA_OUTPUT}"

if [ "${changed}" = true ]; then
  if [ -n "${parent}" ]; then
    echo "Publishing ${package_name}@${version} (${reason}; parent=${parent})."
  else
    echo "Publishing ${package_name}@${version} (${reason})."
  fi
else
  echo "No version change and no ${RELEASE_MARKER} marker (${prev}@${parent} == ${version}); skipping image publish."
fi
