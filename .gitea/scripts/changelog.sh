#!/usr/bin/env bash
# Render release notes for the commits between the previous tag and TAG.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/common.sh"

require Tag "${TAG-}"
require "Repository URL" "${REPO_URL-}"

repo_url="${REPO_URL%/}"

# Notes are a nicety; never fail a release that already published.
fetch=(git fetch --quiet --tags --force)
if [ -f "$(git rev-parse --git-dir)/shallow" ]; then
  "${fetch[@]}" --unshallow 2>/dev/null || "${fetch[@]}" 2>/dev/null || true
else
  "${fetch[@]}" 2>/dev/null || true
fi

prev="$(git tag --list 'v*' --sort=-version:refname | grep -A1 -xF "${TAG}" | tail -n +2 | head -n 1 || true)"

if [ -n "${prev}" ]; then
  # A BREAKING CHANGE footer is equivalent to `!` on the type.
  breaking="$(git log "${prev}..${TAG}" -E --grep='^BREAKING[- ]CHANGE:' --format='%h' | tr '\n' ' ')"

  git log "${prev}..${TAG}" --format=$'\x01%h\x02%s' --name-only |
    awk -v repo="${repo_url}" -v BREAKING_SHAS=" ${breaking}" \
      -v RS_MARK=$'\x01' -v FS_MARK=$'\x02' \
      -f "${SCRIPT_DIR}/changelog.awk"
  printf '**Full changelog**: [%s...%s](%s/compare/%s...%s)\n' \
    "${prev}" "${TAG}" "${repo_url}" "${prev}" "${TAG}"
else
  printf '**Full changelog**: [%s](%s/commits/tag/%s)\n' \
    "${TAG}" "${repo_url}" "${TAG}"
fi
