#!/usr/bin/env bash
set -euo pipefail

package_version() {
  local root="$1" meta version
  if [ ! -f "${root}/${MANIFEST_PATH}" ]; then
    echo "Manifest not found: ${root}/${MANIFEST_PATH}" >&2
    return 1
  fi

  meta="$(cargo metadata --format-version 1 --no-deps --manifest-path "${root}/${MANIFEST_PATH}")" || {
    echo "cargo metadata failed for ${root}/${MANIFEST_PATH}" >&2
    return 1
  }

  version="$(
    printf '%s\n' "${meta}" | jq -r --arg name "${PACKAGE_NAME}" '
      .packages[] | select(.name == $name) | .version
    ' | head -n 1
  )"

  if [ -z "${version}" ] || [ "${version}" = "null" ]; then
    echo "Package ${PACKAGE_NAME} not found in ${root}/${MANIFEST_PATH}." >&2
    return 1
  fi
  printf '%s\n' "${version}"
}

package_version_at() {
  local commit="$1" tree version rc=0

  tree="$(mktemp -d)"
  rmdir "${tree}"

  if ! git worktree add --detach "${tree}" "${commit}" >/dev/null 2>&1; then
    echo "Could not create worktree for ${commit}; refusing to publish." >&2
    return 1
  fi

  version="$(package_version "${tree}")" || rc=$?
  git worktree remove --force "${tree}" 2>/dev/null || true
  rm -rf "${tree}"

  if [ "${rc}" -ne 0 ]; then
    return "${rc}"
  fi
  printf '%s\n' "${version}"
}

version="$(package_version .)" || {
  echo "Could not resolve ${PACKAGE_NAME} version from HEAD; refusing to publish." >&2
  exit 1
}

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

    prev="$(package_version_at "${parent}")" || {
      echo "Could not resolve ${PACKAGE_NAME} version from ${parent}; refusing to publish." >&2
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

{
  echo "version=${version}"
  echo "changed=${changed}"
  echo "reason=${reason}"
  echo "local=${LOCAL_REGISTRY}/${LOCAL_USER}/${IMAGE_NAME}"
  echo "ghcr=ghcr.io/${GH_USER}/${IMAGE_NAME}"
  echo "docker=docker.io/${DH_USER}/${IMAGE_NAME}"
} >> "${GITHUB_OUTPUT}"

if [ "${changed}" = true ]; then
  if [ -n "${parent}" ]; then
    echo "Publishing ${version} (${reason}; parent=${parent})."
  else
    echo "Publishing ${version} (${reason})."
  fi
else
  echo "No version change and no ${RELEASE_MARKER} marker (${prev}@${parent} == ${version}); skipping image publish."
fi
