#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/../../scripts/common.sh"

require Version "${VERSION}"
require Tag "${TAG}"
require Package "${PACKAGE}"
require Token "${TOKEN}"
require Repository "${REPOSITORY}"
require "API URL" "${API_URL}"

if [[ ${REPOSITORY} != */* ]]; then
  echo "Repository must be owner/repo." >&2
  exit 1
fi

targets_parse "${TARGETS}"

owner="${REPOSITORY%%/*}"
repo="${REPOSITORY#*/}"
api="${API_URL%/}/repos/${owner}/${repo}"

body="$(mktemp)"
trap 'rm -f "${body}"' EXIT
auth_hdr=(-H "Authorization: token ${TOKEN}")

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

release_id_for_tag() {
  request GET "${api}/releases/tags/$(uri "${TAG}")" || return 1
  case "${http_code}" in
  200) jq -er '.id' "${body}" ;;
  404) return 2 ;;
  *) fail_http "Failed to fetch release ${TAG}" ;;
  esac
}

create_release() {
  local payload
  payload="$(
    jq -nc \
      --arg tag "${TAG}" \
      --arg name "${VERSION}" \
      --arg body "${BODY-}" \
      --argjson draft "${DRAFT}" \
      --argjson prerelease "${PRERELEASE}" \
      '{tag_name: $tag, name: $name, body: $body, draft: $draft, prerelease: $prerelease}'
  )"
  request POST "${api}/releases" -H "Content-Type: application/json" -d "${payload}" || return 1
  case "${http_code}" in
  201) jq -er '.id' "${body}" ;;
  *) fail_http "Failed to create release ${TAG}" ;;
  esac
}

# Replace assets so re-runs against the same tag stay idempotent.
delete_existing_asset() {
  local release_id="$1" name="$2" asset_id
  request GET "${api}/releases/${release_id}/assets" || return 1
  case "${http_code}" in
  200) ;;
  *) fail_http "Failed to list assets for release ${release_id}" ;;
  esac

  asset_id="$(jq -r --arg name "${name}" '.[] | select(.name == $name) | .id' "${body}" | head -n 1)"
  if [ -n "${asset_id}" ] && [ "${asset_id}" != "null" ]; then
    request DELETE "${api}/releases/${release_id}/assets/${asset_id}" || return 1
    case "${http_code}" in
    200 | 204 | 404) ;;
    *) fail_http "Failed to delete asset ${name}" ;;
    esac
  fi
}

upload_asset() {
  local release_id="$1" file="$2" name="$3"
  delete_existing_asset "${release_id}" "${name}" || return 1
  request POST "${api}/releases/${release_id}/assets?name=$(uri "${name}")" \
    -F "attachment=@${file}" || return 1
  case "${http_code}" in
  201) echo "Uploaded ${name}" ;;
  *) fail_http "Failed to upload ${name}" ;;
  esac
}

rc=0
release_id="$(release_id_for_tag)" || rc=$?
if [ "${rc}" -eq 2 ]; then
  release_id="$(create_release)" || exit 1
  echo "Created release ${TAG}."
elif [ "${rc}" -ne 0 ]; then
  exit 1
else
  echo "Reusing existing release ${TAG} (${release_id})."
fi

for target in "${TARGET_RUST[@]}"; do
  file="${DIST_DIR}/${target}/${PACKAGE}"
  if [ ! -f "${file}" ]; then
    echo "Missing binary for ${target}: ${file}" >&2
    exit 1
  fi
  upload_asset "${release_id}" "${file}" "${PACKAGE}-${target}" || exit 1
done

echo "release_id=${release_id}" >>"${GITEA_OUTPUT}"
