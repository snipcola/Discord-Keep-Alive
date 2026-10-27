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

: "${BUILD_DIR:=build}"
: "${LATEST:=true}"
: "${UPLOAD_JOBS:=5}"

if [[ ! ${UPLOAD_JOBS} =~ ^[1-9][0-9]*$ ]]; then
  echo "UPLOAD_JOBS must be a positive integer." >&2
  exit 1
fi
BG_MAX="${UPLOAD_JOBS}"

if [ "${LATEST}" = "true" ]; then
  require "Repository URL" "${REPO_URL}"
fi

if [[ ${REPOSITORY} != */* ]]; then
  echo "Repository must be owner/repo." >&2
  exit 1
fi

targets_parse "${TARGETS}"

config="config.example.toml"
if [ ! -f "${config}" ]; then
  echo "Missing ${config}" >&2
  exit 1
fi

archives=()
for i in "${!TARGET_RUST[@]}"; do
  file="${BUILD_DIR}/archives/$(archive_name "$i" "${VERSION}")"
  if [ ! -f "${file}" ]; then
    echo "Missing archive for ${TARGET_RUST[$i]}: ${file}" >&2
    exit 1
  fi
  archives+=("${file}")
done

owner="${REPOSITORY%%/*}"
repo="${REPOSITORY#*/}"
api="${API_URL%/}/repos/${owner}/${repo}"

tmp="$(mktemp -d)"
trap 'rm -rf "${tmp}"' EXIT
body="${tmp}/body"
auth_hdr=(-H "Authorization: token ${TOKEN}")

create_release() {
  local tag="$1" name="$2" notes="$3" is_draft="$4" is_prerelease="$5" target="${6-}" payload
  payload="$(
    jq -nc \
      --arg tag "${tag}" \
      --arg name "${name}" \
      --arg body "${notes}" \
      --argjson draft "${is_draft}" \
      --argjson prerelease "${is_prerelease}" \
      --arg target "${target}" \
      '{tag_name: $tag, name: $name, body: $body, draft: $draft, prerelease: $prerelease, target_commitish: $target}'
  )"
  request POST "${api}/releases" -H "Content-Type: application/json" -d "${payload}" || return 1
  case "${http_code}" in
  201) jq -er '.id' "${body}" ;;
  *) fail_http "Failed to create release ${tag}" ;;
  esac
}

edit_release() {
  local release_id="$1" tag="$2" target="$3" notes="$4" draft="${5-null}" payload
  payload="$(
    jq -nc \
      --arg tag "${tag}" \
      --arg target "${target}" \
      --arg body "${notes}" \
      --argjson draft "${draft}" \
      '{tag_name: $tag, target_commitish: $target, body: $body, draft: $draft}'
  )"
  request PATCH "${api}/releases/${release_id}" -H "Content-Type: application/json" -d "${payload}" || return 1
  case "${http_code}" in
  200) ;;
  *) fail_http "Failed to edit release ${tag}" ;;
  esac
}

asset_ids() {
  local release_id="$1"
  request GET "${api}/releases/${release_id}/assets" || return 1
  case "${http_code}" in
  200) jq -r '.[].id' "${body}" ;;
  *) fail_http "Failed to list assets for release ${release_id}" ;;
  esac
}

# request() writes to a shared `body`, so each job needs its own.
own_body() {
  body="$(mktemp -p "${tmp}")"
  "$@"
}

upload_asset() {
  local release_id="$1" file="$2" name="$3"
  request POST "${api}/releases/${release_id}/assets?name=$(uri "${name}")" \
    -F "attachment=@${file}" || return 1
  case "${http_code}" in
  201) echo "Uploaded ${name}" ;;
  *) fail_http "Failed to upload ${name}" ;;
  esac
}

delete_asset() {
  local release_id="$1" asset_id="$2"
  request DELETE "${api}/releases/${release_id}/assets/${asset_id}" || return 1
  case "${http_code}" in
  200 | 204 | 404) ;;
  *) fail_http "Failed to delete asset ${asset_id}" ;;
  esac
}

# Gitea allows duplicate asset names, so upload before deleting.
sync_assets() {
  local release_id="$1" asset_version="$2" ids i asset_id
  local files=("${archives[@]}" "${config}") names=() stale=()

  for i in "${!archives[@]}"; do
    names+=("$(archive_name "$i" "${asset_version}")")
  done
  names+=("${config}")

  ids="$(asset_ids "${release_id}")" || return 1
  read_lines stale "${ids}"

  for i in "${!files[@]}"; do
    bg_start "${names[$i]}" own_body upload_asset "${release_id}" "${files[$i]}" "${names[$i]}"
  done
  bg_wait || return 1

  for asset_id in "${stale[@]+"${stale[@]}"}"; do
    bg_start "asset ${asset_id}" own_body delete_asset "${release_id}" "${asset_id}"
  done
  bg_wait
}

# Staged as a draft so latest is only missing during the swap.
publish_latest() {
  local stage="latest-next" sha notes live_id stage_id current=""
  sha="$(tag_commit "${TAG}")" || return 1
  notes="[${VERSION}](${REPO_URL%/}/releases/tag/$(uri "${TAG}"))"

  live_id="$(release_id_for_tag latest)" || return 1
  [ -z "${live_id}" ] || current="$(tag_commit latest)" || return 1
  if [ "${current}" = "${sha}" ]; then
    edit_release "${live_id}" latest "${sha}" "${notes}" || return 1
    echo "Updating release latest."
    sync_assets "${live_id}" ""
    return
  fi

  stage_id="$(release_id_for_tag "${stage}")" || return 1
  if [ -z "${stage_id}" ]; then
    stage_id="$(create_release "${stage}" latest "${notes}" true true "${sha}")" || return 1
  else
    edit_release "${stage_id}" "${stage}" "${sha}" "${notes}" || return 1
  fi
  echo "Staging release latest."
  sync_assets "${stage_id}" "" || return 1

  [ -z "${live_id}" ] || delete_release "${live_id}" || return 1
  delete_tag latest || return 1
  edit_release "${stage_id}" latest "${sha}" "${notes}" false || return 1

  [ "$(tag_commit latest)" = "${sha}" ] || {
    echo "Tag latest does not point to ${sha}." >&2
    return 1
  }
  echo "Moved latest: ${current:-none} -> ${sha}."
}

release_id="$(release_id_for_tag "${TAG}")" || exit 1
if [ -z "${release_id}" ]; then
  release_id="$(create_release "${TAG}" "${VERSION}" "${BODY-}" "${DRAFT}" "${PRERELEASE}")" || exit 1
  echo "Created release ${TAG}."
else
  echo "Reusing existing release ${TAG} (${release_id})."
fi

sync_assets "${release_id}" "${VERSION}"
echo "release_id=${release_id}" >>"${GITEA_OUTPUT}"

if [ "${LATEST}" = "true" ]; then
  publish_latest
fi
