#!/usr/bin/env bash
# Render release notes for the commits between the previous tag and TAG.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/common.sh"

require Tag "${TAG-}"
require "Repository URL" "${REPO_URL-}"

repo_url="${REPO_URL%/}"

if ! command -v yq >/dev/null; then
  echo "yq is required." >&2
  exit 1
fi

if [[ ! ${TAG} =~ [0-9] ]]; then
  echo "Tag ${TAG} has no version number." >&2
  exit 1
fi

prefix="${TAG%%[0-9]*}"
version="${TAG#"${prefix}"}"
tag_ref="refs/tags/${TAG}"
link_text="${TAG}"
link_url="${repo_url}/commits/tag/${TAG}"

full_changelog() {
  printf '**Full changelog**: [%s](%s)\n' "${link_text}" "${link_url}"
}

# Notes are a nicety; never fail a release that already published.
fetch=(git fetch --quiet --tags --force)
if [ -f "$(git rev-parse --git-dir)/shallow" ]; then
  "${fetch[@]}" --unshallow 2>/dev/null || "${fetch[@]}" 2>/dev/null || true
else
  "${fetch[@]}" 2>/dev/null || true
fi

if ! git rev-parse -q --verify "${tag_ref}^{commit}" >/dev/null; then
  echo "::warning::Tag ${TAG} is not available locally; release notes only link to it." >&2
  full_changelog
  exit 0
fi

# Sort 1.0.0-rc.1 below 1.0.0; stable releases skip rcs.
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

# A BREAKING CHANGE footer is equivalent to `!` on the type.
breaking="$("${log[@]}" -E --grep='^BREAKING[- ]CHANGE:' --format='%h' | tr '\n' ' ')"

manifest_expr='([.package.version, .workspace.package.version] | map(select(tag == "!!str")) | .[0] // ""),
  (del(.package.version) | del(.workspace.package.version))'
third_party_expr='[.package[] | select(has("source"))]'

toml() { git show "$1" | yq -p toml -o json -I0 "$2"; }

# Sets the caller's before/after.
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

"${log[@]}" --name-only \
  --format=$'\x01%h\x02%s\x02%(trailers:key=Release-Note,valueonly,unfold,separator=%x20)' |
  gawk -v repo="${repo_url}" -v BREAKING_SHAS=" ${breaking}" -v BUMP_SHAS=" ${bumps}" \
    -v RS_MARK=$'\x01' -v FS_MARK=$'\x02' \
    -f "${SCRIPT_DIR}/changelog.awk"

full_changelog
