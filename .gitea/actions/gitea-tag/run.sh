#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/../../scripts/common.sh"

require Version "${VERSION}"
require Token "${TOKEN}"
require Repository "${REPOSITORY}"
require "API URL" "${API_URL}"

target="${TARGET-}"
require "Target commit" "${target}"

case "${CONFLICT}" in
skip | fail | override) ;;
*)
  echo "Conflict must be skip, fail, or override (got ${CONFLICT})." >&2
  exit 1
  ;;
esac

if [[ ${REPOSITORY} != */* ]]; then
  echo "Repository must be owner/repo." >&2
  exit 1
fi

owner="${REPOSITORY%%/*}"
repo="${REPOSITORY#*/}"
api="${API_URL%/}/repos/${owner}/${repo}"

body="$(mktemp)"
trap 'rm -f "${body}"' EXIT
auth_hdr=(-H "Authorization: token ${TOKEN}" -H "Content-Type: application/json")

tag="${PREFIX}${VERSION}"
message="${MESSAGE:-${tag}}"
created=false

delete_release() {
  local release_id
  release_id="$(release_id_for_tag "${tag}")" || return 1
  [ -n "${release_id}" ] || return 0

  request DELETE "${api}/releases/${release_id}" || return 1
  case "${http_code}" in
  200 | 204 | 404) echo "Removed release ${tag} so its tag can move." ;;
  *) fail_http "Failed to delete release ${tag}" ;;
  esac
}

create_tag() {
  local payload
  payload="$(
    jq -nc \
      --arg tag_name "${tag}" \
      --arg target "${target}" \
      --arg message "${message}" \
      '{tag_name: $tag_name, target: $target, message: $message}'
  )"
  request POST "${api}/tags" -d "${payload}" || return 1
  case "${http_code}" in
  201) return 0 ;;
  409) return 2 ;;
  *) fail_http "Failed to create tag ${tag}" ;;
  esac
}

move_tag() {
  local from="$1"
  delete_release || return 1
  delete_tag "${tag}" || return 1
  create_tag || {
    echo "Failed to recreate tag ${tag} after delete." >&2
    return 1
  }
  created=true
  echo "Moved ${tag}: ${from} -> ${target}."
}

resolve_existing() {
  local existing="$1"
  if [ "${existing}" = "${target}" ]; then
    echo "Tag ${tag} already points at ${target}; leaving unchanged."
    return 0
  fi

  case "${CONFLICT}" in
  fail)
    echo "Tag ${tag} already points at ${existing}, not ${target}." >&2
    return 1
    ;;
  skip)
    echo "Tag ${tag} already points at ${existing}, not ${target}. Skipping."
    ;;
  override)
    move_tag "${existing}"
    ;;
  esac
}

existing="$(tag_commit "${tag}")" || exit 1
if [ -z "${existing}" ]; then
  rc=0
  create_tag || rc=$?
  case "${rc}" in
  0)
    created=true
    echo "Created ${tag} -> ${target}."
    ;;
  2)
    existing="$(tag_commit "${tag}")" || exit 1
    if [ -z "${existing}" ]; then
      echo "Tag ${tag} reported as existing but could not be read." >&2
      exit 1
    fi
    ;;
  *) exit 1 ;;
  esac
fi

if [ -n "${existing}" ]; then
  resolve_existing "${existing}" || exit 1
fi

{
  echo "tag=${tag}"
  echo "created=${created}"
} >>"${GITEA_OUTPUT}"
