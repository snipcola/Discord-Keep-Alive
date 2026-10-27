#!/usr/bin/env bash
# Build docker bake JSON from TARGETS + REFS.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/common.sh"

: "${TARGETS:?TARGETS is required}"
: "${BAKE_FILE:?BAKE_FILE is required}"
: "${BUILD_DIR:=build}"
: "${DOCKERFILE:=./Dockerfile}"
: "${VERSION:?VERSION is required}"
: "${REVISION:?REVISION is required}"
: "${PACKAGE:?PACKAGE is required}"
: "${REFS:?REFS is required}"
: "${CREATED:=$(date -u +%Y-%m-%dT%H:%M:%SZ)}"
: "${SOURCE:=}"
: "${LICENSE:=}"
: "${PUSH:=true}"

targets_parse "${TARGETS}" linux
refs_parse "${REFS}"

# Bake resolves relative Dockerfile paths against each target's context.
dockerfile="$(realpath "${DOCKERFILE}")"
binary="$(binary_name)"
contexts=()
for i in "${!TARGET_ASSETS[@]}"; do
  contexts+=("${BUILD_DIR}/$(stage_name "$i")")
done

platforms_json="$(json_lines "${TARGET_PLATFORMS[@]}")"
contexts_json="$(json_lines "${contexts[@]}")"
names_json="$(json_lines "${TARGET_NAMES[@]}")"
refs_csv="$(
  IFS=,
  echo "${REFS_LIST[*]}"
)"
push_json=false
if [ "${PUSH}" = "true" ]; then
  push_json=true
fi

# Pushes by digest; docker-manifest.sh adds the version tags.
jq -nc \
  --arg dockerfile "${dockerfile}" \
  --arg binary "${binary}" \
  --arg version "${VERSION}" \
  --arg revision "${REVISION}" \
  --arg package "${PACKAGE}" \
  --arg source "${SOURCE}" \
  --arg created "${CREATED}" \
  --arg license "${LICENSE}" \
  --arg refs_csv "${refs_csv}" \
  --argjson push "${push_json}" \
  --argjson platforms "${platforms_json}" \
  --argjson contexts "${contexts_json}" \
  --argjson names "${names_json}" \
  '
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
                  context: $contexts[$i],
                  dockerfile: $dockerfile,
                  platforms: [$platforms[$i]],
                  args: { BINARY: $binary },
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
  echo "bake_file=${BAKE_FILE}" >>"${GITEA_OUTPUT}"
  targets_emit_outputs "${GITEA_OUTPUT}"
fi

echo "Bake plan: ${#TARGET_PLATFORMS[@]} target(s) -> ${BAKE_FILE}"
for i in "${!TARGET_PLATFORMS[@]}"; do
  echo "  ${TARGET_PLATFORMS[$i]} => ${TARGET_RUST[$i]}"
done
