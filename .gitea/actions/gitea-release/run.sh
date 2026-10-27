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

release_id_for_tag() {
  local tag="$1"
  request GET "${api}/releases/tags/$(uri "${tag}")" || return 1
  case "${http_code}" in
  200) jq -er '.id' "${body}" ;;
  404) return 2 ;;
  *) fail_http "Failed to fetch release ${tag}" ;;
  esac
}

create_release() {
  local tag="$1" name="$2" notes="$3" is_draft="$4" is_prerelease="$5" payload
  payload="$(
    jq -nc \
      --arg tag "${tag}" \
      --arg name "${name}" \
      --arg body "${notes}" \
      --argjson draft "${is_draft}" \
      --argjson prerelease "${is_prerelease}" \
      '{tag_name: $tag, name: $name, body: $body, draft: $draft, prerelease: $prerelease}'
  )"
  request POST "${api}/releases" -H "Content-Type: application/json" -d "${payload}" || return 1
  case "${http_code}" in
  201) jq -er '.id' "${body}" ;;
  *) fail_http "Failed to create release ${tag}" ;;
  esac
}

delete_release() {
  local tag="$1" release_id rc=0
  release_id="$(release_id_for_tag "${tag}")" || rc=$?
  case "${rc}" in
  0) ;;
  2) return 0 ;;
  *) return 1 ;;
  esac

  request DELETE "${api}/releases/${release_id}" || return 1
  case "${http_code}" in
  200 | 204 | 404) echo "Removed release ${tag}." ;;
  *) fail_http "Failed to delete release ${tag}" ;;
  esac
}

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
  local release_id="$1" file="$2" name="$3" replace="$4"
  if [ "${replace}" = "true" ]; then
    delete_existing_asset "${release_id}" "${name}" || return 1
  fi
  request POST "${api}/releases/${release_id}/assets?name=$(uri "${name}")" \
    -F "attachment=@${file}" || return 1
  case "${http_code}" in
  201) echo "Uploaded ${name}" ;;
  *) fail_http "Failed to upload ${name}" ;;
  esac
}

upload_assets() {
  local release_id="$1" asset_version="$2" replace="$3" i work failed=0
  local files=("${archives[@]}" "${config}") names=()

  for i in "${!archives[@]}"; do
    names+=("$(archive_name "$i" "${asset_version}")")
  done
  names+=("${config}")

  work="$(mktemp -d -p "${tmp}")"
  for i in "${!files[@]}"; do
    while [ "$(jobs -rp | wc -l)" -ge "${UPLOAD_JOBS}" ]; do
      wait -n || true
    done
    # request() writes to a shared `body`, so each job needs its own.
    (
      body="${work}/${i}.body"
      upload_asset "${release_id}" "${files[$i]}" "${names[$i]}" "${replace}" &&
        touch "${work}/${i}.ok"
    ) >"${work}/${i}.log" 2>&1 &
  done
  wait

  for i in "${!files[@]}"; do
    cat "${work}/${i}.log"
    [ -f "${work}/${i}.ok" ] || failed=1
  done
  [ "${failed}" -eq 0 ]
}

rc=0
release_id="$(release_id_for_tag "${TAG}")" || rc=$?
if [ "${rc}" -eq 2 ]; then
  release_id="$(create_release "${TAG}" "${VERSION}" "${BODY-}" "${DRAFT}" "${PRERELEASE}")" || exit 1
  echo "Created release ${TAG}."
elif [ "${rc}" -ne 0 ]; then
  exit 1
else
  echo "Reusing existing release ${TAG} (${release_id})."
fi

upload_assets "${release_id}" "${VERSION}" true || exit 1
echo "release_id=${release_id}" >>"${GITEA_OUTPUT}"

if [ "${LATEST}" = "true" ]; then
  delete_release latest || exit 1
  latest_id="$(create_release latest latest "[${VERSION}](${REPO_URL%/}/releases/tag/$(uri "${TAG}"))" false true)" || exit 1
  echo "Created release latest."
  upload_assets "${latest_id}" "" false || exit 1
fi
