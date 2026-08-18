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

request() {
  local method="$1" url="$2"
  shift 2
  http_code="$(
    curl -sS -o "${body}" -w "%{http_code}" \
      -X "${method}" \
      "${auth_hdr[@]}" \
      "$@" \
      "${url}"
  )" || return 1
}

fail_http() {
  echo "${1} (HTTP ${http_code}): $(cat "${body}")" >&2
  return 1
}

ensure_tag() {
  local tag="$1"
  local message="${2:-${tag}}"
  local tag_conflict="${3:-${CONFLICT}}"
  local tag_url
  tag_url="${api}/tags/$(uri "${tag}")"
  local created=false

  tag_commit() {
    request GET "${tag_url}" || return 1
    case "${http_code}" in
    200) jq -er '.commit.sha' "${body}" ;;
    404) return 2 ;;
    *) fail_http "Failed to fetch tag ${tag}" ;;
    esac
  }

  delete_release() {
    local release_id
    request GET "${api}/releases/tags/$(uri "${tag}")" || return 1
    case "${http_code}" in
    404) return 0 ;;
    200) release_id="$(jq -er '.id' "${body}")" || return 1 ;;
    *) fail_http "Failed to fetch release ${tag}" ;;
    esac

    request DELETE "${api}/releases/${release_id}" || return 1
    case "${http_code}" in
    200 | 204 | 404) echo "Removed release ${tag} so its tag can move." ;;
    *) fail_http "Failed to delete release ${tag}" ;;
    esac
  }

  # 409 means a release still points at the tag; drop it and retry once.
  delete_tag() {
    local attempt
    for attempt in 1 2; do
      request DELETE "${tag_url}" || return 1
      case "${http_code}" in
      200 | 204 | 404) return 0 ;;
      409)
        if [ "${attempt}" -eq 2 ]; then
          fail_http "Failed to delete tag ${tag}"
        fi
        delete_release || return 1
        ;;
      *) fail_http "Failed to delete tag ${tag}" ;;
      esac
    done
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

  place_tag() {
    local from="${1:-}"
    if [ -n "${from}" ]; then
      delete_tag || return 1
      create_tag || {
        echo "Failed to recreate tag ${tag} after delete." >&2
        return 1
      }
      created=true
      echo "Moved ${tag}: ${from} -> ${target}."
      return 0
    fi

    create_tag || return $?
    created=true
    echo "Created ${tag} -> ${target}."
  }

  resolve_existing() {
    local existing="$1"
    if [ "${existing}" = "${target}" ]; then
      echo "Tag ${tag} already points at ${target}; leaving unchanged."
      return 0
    fi

    case "${tag_conflict}" in
    fail)
      echo "Tag ${tag} already points at ${existing}, not ${target}." >&2
      return 1
      ;;
    skip)
      echo "Tag ${tag} already points at ${existing}, not ${target}. Skipping."
      ;;
    override)
      place_tag "${existing}"
      ;;
    esac
  }

  local rc=0
  local existing
  existing="$(tag_commit)" || rc=$?
  if [ "${rc}" -eq 0 ]; then
    resolve_existing "${existing}" || return 1
  elif [ "${rc}" -eq 2 ]; then
    local crc=0
    place_tag || crc=$?
    if [ "${crc}" -eq 2 ]; then
      rc=0
      existing="$(tag_commit)" || rc=$?
      if [ "${rc}" -ne 0 ]; then
        echo "Tag ${tag} reported as existing but could not be read." >&2
        return 1
      fi
      resolve_existing "${existing}" || return 1
    elif [ "${crc}" -ne 0 ]; then
      return 1
    fi
  else
    return 1
  fi

  ensure_created="${created}"
}

version_tag="${PREFIX}${VERSION}"
version_message="${MESSAGE:-${version_tag}}"
ensure_tag "${version_tag}" "${version_message}" || exit 1
created="${ensure_created}"

if [ "${LATEST}" = "true" ]; then
  if ! ensure_tag "latest" "latest" "override"; then
    echo "Retrying floating tag latest -> ${target} ..." >&2
    if ! ensure_tag "latest" "latest" "override"; then
      echo "Version tag ${version_tag} was handled, but floating tag latest failed to point at ${target}." >&2
      echo "Check ${version_tag} and latest against target ${target}; latest may be stale or missing." >&2
      exit 1
    fi
  fi
fi

{
  echo "tag=${version_tag}"
  echo "created=${created}"
} >>"${GITEA_OUTPUT}"
