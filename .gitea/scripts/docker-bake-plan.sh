#!/usr/bin/env bash
# TARGETS + REFS → bake JSON (and optional GITHUB_OUTPUT).
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=docker-targets.sh
source "${SCRIPT_DIR}/docker-targets.sh"

: "${TARGETS:?TARGETS is required}"
: "${BAKE_FILE:?BAKE_FILE is required}"
: "${CONTEXT:=.}"
: "${DOCKERFILE:=./Dockerfile}"
: "${VERSION:?VERSION is required}"
: "${REVISION:?REVISION is required}"
: "${PACKAGE:?PACKAGE is required}"
: "${REFS:?REFS is required}"

docker_targets_parse "${TARGETS}"
docker_refs_parse "${REFS}"

cache_from=()
cache_to=()
docker_read_lines cache_from "${CACHE_FROM-}"
docker_read_lines cache_to "${CACHE_TO-}"

platforms_json="$(docker_json_lines "${DOCKER_TARGET_PLATFORMS[@]}")"
rust_json="$(docker_json_lines "${DOCKER_TARGET_RUST[@]}")"
names_json="$(docker_json_lines "${DOCKER_TARGET_NAMES[@]}")"
cache_from_json="$(docker_json_lines "${cache_from[@]+"${cache_from[@]}"}")"
cache_to_json="$(docker_json_lines "${cache_to[@]+"${cache_to[@]}"}")"
refs_csv="$(IFS=,; echo "${DOCKER_REFS[*]}")"

# Digests only; tags in docker-manifest.sh. One name= list → all registries, no tag races.
output_entry="type=image,name=${refs_csv},push-by-digest=true,name-canonical=true,push=true"

jq -nc \
  --arg context "${CONTEXT}" \
  --arg dockerfile "${DOCKERFILE}" \
  --arg version "${VERSION}" \
  --arg revision "${REVISION}" \
  --arg package "${PACKAGE}" \
  --arg output "${output_entry}" \
  --argjson platforms "${platforms_json}" \
  --argjson rust "${rust_json}" \
  --argjson names "${names_json}" \
  --argjson cache_from "${cache_from_json}" \
  --argjson cache_to "${cache_to_json}" \
  '
    [range(0; $platforms | length)] as $idx
    | (
        $idx
        | map({
            key: ("image-" + $names[.]),
            value: {
              context: $context,
              dockerfile: $dockerfile,
              platforms: [$platforms[.]],
              args: {
                RUST_TARGET: $rust[.],
                VERSION: $version,
                REVISION: $revision,
                PACKAGE: $package
              },
              "cache-from": $cache_from,
              "cache-to": $cache_to,
              output: [$output]
            }
          })
        | from_entries
      ) as $targets
    | {
        group: { default: { targets: ($idx | map("image-" + $names[.])) } },
        target: $targets
      }
  ' > "${BAKE_FILE}"

if [ -n "${GITHUB_OUTPUT-}" ]; then
  {
    echo "bake_file=${BAKE_FILE}"
    echo "platforms=$(IFS=,; echo "${DOCKER_TARGET_PLATFORMS[*]}")"
    echo "rust_targets=$(IFS=,; echo "${DOCKER_TARGET_RUST[*]}")"
    echo "target_names=$(IFS=,; echo "${DOCKER_TARGET_NAMES[*]}")"
  } >> "${GITHUB_OUTPUT}"
fi

echo "Bake plan: ${#DOCKER_TARGET_PLATFORMS[@]} target(s) -> ${BAKE_FILE}"
for i in "${!DOCKER_TARGET_PLATFORMS[@]}"; do
  echo "  ${DOCKER_TARGET_PLATFORMS[$i]} => ${DOCKER_TARGET_RUST[$i]}"
done
