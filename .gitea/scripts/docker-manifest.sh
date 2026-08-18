#!/usr/bin/env bash
# Create multi-arch tags from bake digests.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/common.sh"

: "${METADATA:?METADATA is required}"
: "${REFS:?REFS is required}"
: "${VERSION:?VERSION is required}"
: "${ALSO_LATEST:=true}"

refs_parse "${REFS}"

mapfile -t digests < <(
  printf '%s' "${METADATA}" | jq -r '
    if type != "object" then
      error("bake metadata must be a JSON object")
    else
      to_entries[]
      | .value
      | objects
      | ."containerimage.digest" // empty
    end
  ' | sed '/^$/d' | sort -u
)

if [ "${#digests[@]}" -eq 0 ]; then
  echo "No containerimage.digest values in bake metadata." >&2
  printf '%s\n' "${METADATA}" | jq . >&2 || printf '%s\n' "${METADATA}" >&2
  exit 1
fi

for digest in "${digests[@]}"; do
  if [[ ! ${digest} =~ ^sha256:[0-9a-f]{64}$ ]]; then
    echo "Unexpected containerimage.digest: ${digest}" >&2
    exit 1
  fi
done

echo "Creating manifests from ${#digests[@]} platform digest(s) for ${#REFS_LIST[@]} ref(s)."

for ref in "${REFS_LIST[@]}"; do
  sources=()
  for digest in "${digests[@]}"; do
    sources+=("${ref}@${digest}")
  done

  tag_args=(-t "${ref}:${VERSION}")
  if [ "${ALSO_LATEST}" = "true" ]; then
    tag_args+=(-t "${ref}:latest")
  fi

  echo "imagetools create ${tag_args[*]} (${#sources[@]} sources)"
  docker buildx imagetools create "${tag_args[@]}" "${sources[@]}"
done
