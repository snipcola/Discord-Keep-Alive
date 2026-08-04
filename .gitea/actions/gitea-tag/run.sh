#!/usr/bin/env bash
set -euo pipefail

require() {
  if [ -z "${2}" ]; then
    echo "${1} is required." >&2
    exit 1
  fi
}

require Version "${VERSION}"
require Token "${TOKEN}"

case "${CONFLICT}" in
  skip|fail|override) ;;
  *)
    echo "Conflict must be skip, fail, or override (got ${CONFLICT})." >&2
    exit 1
    ;;
esac

server_url="${SERVER_URL:-${GITHUB_SERVER_URL:-}}"
server_url="${server_url%/}"
require "Server URL" "${server_url}"

repository="${REPOSITORY:-${GITHUB_REPOSITORY:-}}"
if [ -z "${repository}" ] || [[ "${repository}" != */* ]]; then
  echo "Repository must be owner/repo." >&2
  exit 1
fi

target="${TARGET:-${GITHUB_SHA:-}}"
require "Target commit" "${target}"

owner="${repository%%/*}"
repo="${repository#*/}"
api="${server_url}/api/v1/repos/${owner}/${repo}"
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
  local tag_url="${api}/tags/$(printf '%s' "${tag}" | jq -sRr @uri)"
  local created=false

  tag_commit() {
    request GET "${tag_url}" || return 1
    case "${http_code}" in
      200) jq -er '.commit.sha' "${body}" ;;
      404) return 2 ;;
      *) fail_http "Failed to fetch tag ${tag}" ;;
    esac
  }

  delete_tag() {
    request DELETE "${tag_url}" || return 1
    case "${http_code}" in
      200|204|404) ;;
      *) fail_http "Failed to delete tag ${tag}" ;;
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
} >> "${GITHUB_OUTPUT}"
