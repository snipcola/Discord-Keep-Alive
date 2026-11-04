#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/common.sh"

require Tag "${TAG-}"
require "Repository URL" "${REPO_URL-}"
: "${SHIPPED_PATHS:?SHIPPED_PATHS is required}"

shipped=()
read_lines shipped "${SHIPPED_PATHS}"

repo_url="${REPO_URL%/}"

tag_ref="refs/tags/${TAG}"
link_text="${TAG}"
link_url="${repo_url}/commits/tag/${TAG}"

full_changelog() {
  printf '**Full changelog**: [%s](%s)\n' "${link_text}" "${link_url}"
}

fallback() {
  echo "::warning::Release notes unavailable: $1" >&2
  full_changelog
  exit 0
}

# Release notes are optional; never fail a published release over them.
trap 'fallback "generating them failed (exit $?)."' ERR

for tool in yq gawk; do
  command -v "${tool}" >/dev/null || fallback "${tool} is not installed."
done

[[ ${TAG} =~ [0-9] ]] || fallback "tag ${TAG} has no version number."

prefix="${TAG%%[0-9]*}"
version="${TAG#"${prefix}"}"

unshallow 2>/dev/null || true
git fetch --quiet --tags --force 2>/dev/null || true

if ! git rev-parse -q --verify "${tag_ref}^{commit}" >/dev/null; then
  echo "::warning::Tag ${TAG} is not available locally; release notes only link to it." >&2
  full_changelog
  exit 0
fi

# Orders 1.0.0-rc.1 before 1.0.0; stable releases skip release candidates.
prev=""
seen=0
while IFS= read -r t; do
  if [ "${seen}" -eq 0 ]; then
    [ "${t}" = "${TAG}" ] && seen=1
    continue
  fi
  if [[ ${version} != *-* && ${t#"${prefix}"} == *-* ]]; then
    continue
  fi
  prev="${t}"
  break
done < <(git -c versionsort.suffix=- tag --list "${prefix}[0-9]*" --sort=-version:refname)

range="${tag_ref}"
if [ -n "${prev}" ]; then
  range="refs/tags/${prev}..${tag_ref}"
  link_text="${prev}...${TAG}"
  link_url="${repo_url}/compare/${prev}...${TAG}"
fi

log=(git -c core.quotePath=false log "${range}" --no-merges)

breaking="$("${log[@]}" -E --grep='^BREAKING[- ]CHANGE:' --format='%h' | tr '\n' ' ')"

manifest_expr='([.package.version, .workspace.package.version] | map(select(tag == "!!str")) | .[0] // ""),
  (del(.package.version) | del(.workspace.package.version))'
third_party_expr='[.package[] | select(has("source"))]'

toml() { git show "$1" | yq -p toml -o json -I0 "$2"; }

# Sets `before` and `after` in the caller.
toml_pair() {
  before="$(toml "$1^:$2" "$3")" && after="$(toml "$1:$2" "$3")"
}

is_bump() {
  local sha="$1" files before after
  files="$(git show --format= --name-only "${sha}")"
  case "${files}" in
  Cargo.toml | $'Cargo.lock\nCargo.toml') ;;
  *) return 1 ;;
  esac

  toml_pair "${sha}" Cargo.toml "${manifest_expr}" || return 1
  [ "${before%%$'\n'*}" != "${after%%$'\n'*}" ] || return 1
  [ "${before#*$'\n'}" = "${after#*$'\n'}" ] || return 1

  [[ ${files} == Cargo.lock* ]] || return 0
  toml_pair "${sha}" Cargo.lock "${third_party_expr}" && [ "${before}" = "${after}" ]
}

bumps=""
while IFS= read -r sha; do
  is_bump "${sha}" 2>/dev/null && bumps+="${sha} "
done < <("${log[@]}" --format='%h' -- Cargo.toml)

notes="$(
  "${log[@]}" --name-only \
    --format=$'\x01%h\x02%s\x02%(trailers:key=Release-Note,valueonly,unfold,separator=%x20)' |
    gawk -v repo="${repo_url}" -v BREAKING_SHAS=" ${breaking}" -v BUMP_SHAS=" ${bumps}" -v SHIPPED="${shipped[*]}" \
      -v RS_MARK=$'\x01' -v FS_MARK=$'\x02' \
      -f "${SCRIPT_DIR}/changelog.awk"
)"

printf '%s\n\n' "${notes}"
full_changelog
