#!/usr/bin/env bash
# Shared helpers (source, or run to emit target lists).
set -euo pipefail

TARGET_PLATFORMS=()
TARGET_RUST=()
TARGET_NAMES=()
TARGET_EXTS=()
TARGET_ASSETS=()
TARGET_ARCHIVES=()
REFS_LIST=()

declare -gA ASSET_OS=([linux]=linux [windows]=windows [darwin]=macos)
declare -gA ASSET_ARCH=([amd64]=x64 [arm64]=arm64)
declare -gA ASSET_EXT=([windows]=.exe)
declare -gA ASSET_ARCHIVE=([linux]=.tar.xz [windows]=.zip [darwin]=.tar.gz)

trim() {
  local s="${1-}"
  s="${s#"${s%%[![:space:]]*}"}"
  s="${s%"${s##*[![:space:]]}"}"
  printf '%s' "${s}"
}

read_lines() {
  local -n _lines_out="$1"
  local raw="${2-}" line
  _lines_out=()
  while IFS= read -r line || [ -n "${line}" ]; do
    line="$(trim "${line}")"
    [ -n "${line}" ] || continue
    _lines_out+=("${line}")
  done <<<"${raw}"
}

json_lines() {
  if [ "$#" -eq 0 ]; then
    printf '[]'
    return
  fi
  printf '%s\n' "$@" | jq -R . | jq -s -c .
}

csv() {
  local -n _csv_arr="$1"
  local IFS=,
  printf '%s' "${_csv_arr[*]}"
}

targets_parse() {
  local raw="${1-}" only_os="${2-}" line platform rust os arch ext i
  TARGET_PLATFORMS=()
  TARGET_RUST=()
  TARGET_NAMES=()
  TARGET_EXTS=()
  TARGET_ASSETS=()
  TARGET_ARCHIVES=()

  while IFS= read -r line || [ -n "${line}" ]; do
    line="$(trim "${line%%#*}")"
    [ -n "${line}" ] || continue

    if [[ ${line} != *=* ]]; then
      echo "Invalid target (expected platform=target): ${line}" >&2
      return 1
    fi

    platform="$(trim "${line%%=*}")"
    rust="$(trim "${line#*=}")"

    if [ -z "${platform}" ] || [ -z "${rust}" ]; then
      echo "Invalid target (empty platform or Rust target): ${line}" >&2
      return 1
    fi
    if [[ ${platform} != ?*/?* ]]; then
      echo "Invalid Docker platform (expected os/arch): ${platform}" >&2
      return 1
    fi
    if [[ ! ${rust} =~ ^[A-Za-z0-9._-]+$ ]]; then
      echo "Invalid Rust target: ${rust}" >&2
      return 1
    fi

    os="${platform%%/*}"
    arch="${platform#*/}"

    if [ -z "${ASSET_OS[${os}]-}" ] || [ -z "${ASSET_ARCH[${arch}]-}" ]; then
      echo "Unsupported platform: ${platform}" >&2
      return 1
    fi

    if [ -n "${only_os}" ] && [ "${os}" != "${only_os}" ]; then
      continue
    fi

    for i in "${!TARGET_PLATFORMS[@]}"; do
      if [ "${TARGET_PLATFORMS[$i]}" = "${platform}" ]; then
        echo "Duplicate Docker platform: ${platform}" >&2
        return 1
      fi
    done

    ext="${ASSET_EXT[${os}]-}"

    TARGET_PLATFORMS+=("${platform}")
    TARGET_RUST+=("${rust}")
    TARGET_NAMES+=("${platform//\//-}")
    TARGET_EXTS+=("${ext}")
    TARGET_ASSETS+=("${ASSET_OS[${os}]}-${ASSET_ARCH[${arch}]}")
    TARGET_ARCHIVES+=("${ASSET_ARCHIVE[${os}]}")
  done < <(LC_ALL=C sort -b -t= -k1,1 <<<"${raw}")

  if [ "${#TARGET_PLATFORMS[@]}" -eq 0 ]; then
    echo "No targets provided (expected platform=target lines)." >&2
    return 1
  fi
}

stage_name() { printf '%s' "${PACKAGE}-${TARGET_ASSETS[$1]}"; }

archive_name() { printf '%s' "${PACKAGE}${2:+-${2}}-${TARGET_ASSETS[$1]}${TARGET_ARCHIVES[$1]}"; }

# The binary takes its name from [[bin]], not the package.
binary_name() {
  cargo metadata --format-version 1 --no-deps |
    jq -er --arg pkg "${PACKAGE}" \
      'first(.packages[] | select(.name == $pkg) | .targets[] | select(.kind[] == "bin") | .name)'
}

targets_emit_outputs() {
  local out="${1-}"
  {
    echo "platforms=$(csv TARGET_PLATFORMS)"
    echo "rust_targets=$(csv TARGET_RUST)"
    echo "target_names=$(csv TARGET_NAMES)"
  } | if [ -n "${out}" ]; then cat >>"${out}"; else cat; fi
}

targets_cargo_args() {
  local -n _args_out="$1"
  local target
  _args_out=()
  for target in "${TARGET_RUST[@]}"; do
    _args_out+=(--target "${target}")
  done
}

require() {
  if [ -z "${2}" ]; then
    echo "${1} is required." >&2
    exit 1
  fi
}

uri() { printf '%s' "${1-}" | jq -sRr @uri; }

unshallow() {
  [ -f "$(git rev-parse --git-dir)/shallow" ] || return 0
  git fetch --quiet --unshallow origin
}

request() {
  local method="$1" url="$2"
  shift 2
  http_code="$(
    curl -sS -o "${body}" -w "%{http_code}" \
      --connect-timeout 10 \
      --speed-limit 1024 --speed-time 60 \
      --retry 3 --retry-connrefused \
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

normalize_host() {
  local host
  host="$(trim "${1-}")"
  if [[ ${host} == *://* ]]; then
    host="${host#*://}"
  fi
  host="${host%%/*}"
  trim "${host}"
}

ref_is_base() {
  local ref="${1-}" last
  [ -n "${ref}" ] || return 1
  [[ ${ref} != *@* ]] || return 1
  last="${ref##*/}"
  [[ ${last} != *:* ]]
}

refs_parse() {
  local raw="${1-}" ref
  read_lines REFS_LIST "${raw}"
  if [ "${#REFS_LIST[@]}" -eq 0 ]; then
    echo "REFS is empty; need at least one image name base." >&2
    return 1
  fi
  for ref in "${REFS_LIST[@]}"; do
    if ! ref_is_base "${ref}"; then
      echo "Invalid image ref (base name, no tag/digest): ${ref}" >&2
      return 1
    fi
  done
}

if [[ ${BASH_SOURCE[0]} == "${0}" ]]; then
  : "${TARGETS:?TARGETS is required}"
  targets_parse "${TARGETS}"
  targets_emit_outputs "${GITEA_OUTPUT-}"
fi
