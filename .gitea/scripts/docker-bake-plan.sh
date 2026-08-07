#!/usr/bin/env bash
# TARGETS + REFS → bake JSON (and optional GITEA_OUTPUT).
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/docker-targets.sh"

: "${TARGETS:?TARGETS is required}"
: "${BAKE_FILE:?BAKE_FILE is required}"
: "${CONTEXT:=.}"
: "${DOCKERFILE:=./Dockerfile}"
: "${VERSION:?VERSION is required}"
: "${REVISION:?REVISION is required}"
: "${PACKAGE:?PACKAGE is required}"
: "${REFS:?REFS is required}"
: "${CREATED:=$(date -u +%Y-%m-%dT%H:%M:%SZ)}"
: "${SOURCE:=}"
: "${LICENSE:=}"
: "${PUSH:=true}"

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
refs_csv="$(
  IFS=,
  echo "${DOCKER_REFS[*]}"
)"
push_json=false
if [ "${PUSH}" = "true" ]; then
  push_json=true
fi

# Digests only when pushing; tags in docker-manifest.sh.
jq -nc \
  --arg context "${CONTEXT}" \
  --arg dockerfile "${DOCKERFILE}" \
  --arg version "${VERSION}" \
  --arg revision "${REVISION}" \
  --arg package "${PACKAGE}" \
  --arg source "${SOURCE}" \
  --arg created "${CREATED}" \
  --arg license "${LICENSE}" \
  --arg refs_csv "${refs_csv}" \
  --argjson push "${push_json}" \
  --argjson platforms "${platforms_json}" \
  --argjson rust "${rust_json}" \
  --argjson names "${names_json}" \
  --argjson cache_from "${cache_from_json}" \
  --argjson cache_to "${cache_to_json}" \
  '
    def pin_gha_scope($platform):
      ("-platform-" + $platform) as $tail
      | map(
          if (split(",") | any(. == "type=gha" or startswith("type=gha"))) then
            (split(",")
              | map(
                  if startswith("scope=") then
                    "scope=" + (.[6:] | if endswith($tail) then . else . + $tail end)
                  else . end
                )
              | join(","))
          else . end
        );

    def opt_label($key; $val):
      if $val == "" then {} else {($key): $val} end;

    def target_output:
      if $push then
        [{
          type: "image",
          name: $refs_csv,
          "push-by-digest": "true",
          "name-canonical": "true",
          push: "true"
        }]
      else
        [{ type: "cacheonly" }]
      end;

    [range(0; $platforms | length)] as $idx
    | (
        $idx
        | map(
            . as $i
            | {
                key: ("image-" + $names[$i]),
                value: {
                  context: $context,
                  dockerfile: $dockerfile,
                  platforms: [$platforms[$i]],
                  args: {
                    RUST_TARGET: $rust[$i],
                    PACKAGE: $package
                  },
                  labels: (
                    {
                      "org.opencontainers.image.title": $package,
                      "org.opencontainers.image.version": $version,
                      "org.opencontainers.image.revision": $revision,
                      "org.opencontainers.image.created": $created
                    }
                    + opt_label("org.opencontainers.image.source"; $source)
                    + opt_label("org.opencontainers.image.licenses"; $license)
                  ),
                  "cache-from": ($cache_from | pin_gha_scope($names[$i])),
                  "cache-to": ($cache_to | pin_gha_scope($names[$i])),
                  output: target_output
                }
              }
          )
        | from_entries
      ) as $targets
    | {
        group: { default: { targets: ($idx | map("image-" + $names[.])) } },
        target: $targets
      }
  ' >"${BAKE_FILE}"

if [ -n "${GITEA_OUTPUT-}" ]; then
  {
    echo "bake_file=${BAKE_FILE}"
    echo "platforms=$(
      IFS=,
      echo "${DOCKER_TARGET_PLATFORMS[*]}"
    )"
    echo "rust_targets=$(
      IFS=,
      echo "${DOCKER_TARGET_RUST[*]}"
    )"
    echo "target_names=$(
      IFS=,
      echo "${DOCKER_TARGET_NAMES[*]}"
    )"
  } >>"${GITEA_OUTPUT}"
fi

echo "Bake plan: ${#DOCKER_TARGET_PLATFORMS[@]} target(s) -> ${BAKE_FILE}"
for i in "${!DOCKER_TARGET_PLATFORMS[@]}"; do
  echo "  ${DOCKER_TARGET_PLATFORMS[$i]} => ${DOCKER_TARGET_RUST[$i]}"
done
