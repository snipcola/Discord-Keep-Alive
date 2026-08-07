#!/usr/bin/env bash
# Shared docker target/ref helpers (sourceable; CLI emits cache_map).
set -euo pipefail

DOCKER_TARGET_PLATFORMS=()
DOCKER_TARGET_RUST=()
DOCKER_TARGET_NAMES=()
DOCKER_REFS=()

docker_trim() {
  local s="${1-}"
  s="${s#"${s%%[![:space:]]*}"}"
  s="${s%"${s##*[![:space:]]}"}"
  printf '%s' "${s}"
}

docker_targets_reset() {
  DOCKER_TARGET_PLATFORMS=()
  DOCKER_TARGET_RUST=()
  DOCKER_TARGET_NAMES=()
}

docker_targets_parse() {
  local raw="${1-}" line platform rust i
  docker_targets_reset

  while IFS= read -r line || [ -n "${line}" ]; do
    line="$(docker_trim "${line%%#*}")"
    [ -n "${line}" ] || continue

    if [[ ${line} != *=* ]]; then
      echo "Invalid target (expected platform=target): ${line}" >&2
      return 1
    fi

    platform="$(docker_trim "${line%%=*}")"
    rust="$(docker_trim "${line#*=}")"

    if [ -z "${platform}" ] || [ -z "${rust}" ]; then
      echo "Invalid target (empty platform or Rust target): ${line}" >&2
      return 1
    fi
    if [[ ${platform} != */* ]]; then
      echo "Invalid Docker platform (expected os/arch): ${platform}" >&2
      return 1
    fi
    if [[ ! ${rust} =~ ^[A-Za-z0-9._-]+$ ]]; then
      echo "Invalid Rust target: ${rust}" >&2
      return 1
    fi

    for i in "${!DOCKER_TARGET_PLATFORMS[@]}"; do
      if [ "${DOCKER_TARGET_PLATFORMS[$i]}" = "${platform}" ]; then
        echo "Duplicate Docker platform: ${platform}" >&2
        return 1
      fi
    done

    DOCKER_TARGET_PLATFORMS+=("${platform}")
    DOCKER_TARGET_RUST+=("${rust}")
    DOCKER_TARGET_NAMES+=("${platform//\//-}")
  done <<<"${raw}"

  if [ "${#DOCKER_TARGET_PLATFORMS[@]}" -eq 0 ]; then
    echo "No docker targets provided (expected platform=target lines)." >&2
    return 1
  fi
}

docker_targets_cache_map_json() {
  local base="${1:?cache-map base JSON is required}"
  local rust_json

  rust_json="$(printf '%s\n' "${DOCKER_TARGET_RUST[@]}" | jq -R . | jq -s -c .)"
  jq -nc --argjson base "${base}" --argjson rust "${rust_json}" '
    if ($base | type) != "object" then
      error("cache-map base must be a JSON object")
    else
      reduce $rust[] as $t (
        $base;
        . + {("cargo-target-" + $t): {target: "/app/target", id: ("cargo-target-" + $t)}}
      )
    end
  '
}

docker_read_lines() {
  local -n _docker_lines_out="$1"
  local raw="${2-}" line
  _docker_lines_out=()
  while IFS= read -r line || [ -n "${line}" ]; do
    line="$(docker_trim "${line}")"
    [ -n "${line}" ] || continue
    _docker_lines_out+=("${line}")
  done <<<"${raw}"
}

# Strip scheme/path from a registry host (or full URL).
docker_map_normalize_host() {
  local host
  host="$(docker_trim "${1-}")"
  if [[ ${host} == *://* ]]; then
    host="${host#*://}"
  fi
  host="${host%%/*}"
  docker_trim "${host}"
}


docker_ref_is_base() {
  local ref="${1-}" last
  [ -n "${ref}" ] || return 1
  [[ ${ref} != *@* ]] || return 1
  last="${ref##*/}"
  [[ ${last} != *:* ]]
}

docker_refs_parse() {
  local raw="${1-}" ref
  DOCKER_REFS=()
  docker_read_lines DOCKER_REFS "${raw}"
  if [ "${#DOCKER_REFS[@]}" -eq 0 ]; then
    echo "REFS is empty; need at least one image name base." >&2
    return 1
  fi
  for ref in "${DOCKER_REFS[@]}"; do
    if ! docker_ref_is_base "${ref}"; then
      echo "Invalid image ref (base name, no tag/digest): ${ref}" >&2
      return 1
    fi
  done
}

docker_json_lines() {
  if [ "$#" -eq 0 ]; then
    printf '[]'
    return
  fi
  printf '%s\n' "$@" | jq -R . | jq -s -c .
}

docker_targets_main() {
  local map
  : "${TARGETS:?TARGETS is required}"
  : "${CACHE_MAP:?CACHE_MAP is required}"

  docker_targets_parse "${TARGETS}"
  map="$(docker_targets_cache_map_json "${CACHE_MAP}")"

  if [ -n "${GITEA_OUTPUT-}" ]; then
    {
      echo "cache_map<<EOF"
      printf '%s\n' "${map}"
      echo "EOF"
    } >>"${GITEA_OUTPUT}"
  else
    printf '%s\n' "${map}"
  fi
}

if [[ ${BASH_SOURCE[0]} == "${0}" ]]; then
  docker_targets_main
fi
