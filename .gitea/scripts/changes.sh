#!/usr/bin/env bash
# Skip checks when only DOCS_ONLY paths changed since BASE, unless releasing.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/common.sh"

: "${GITEA_OUTPUT:?GITEA_OUTPUT is required}"

docs_only() {
  local patterns=() pathspec=(.) pattern
  [ "${RELEASE-}" != "true" ] || return 1
  [[ ! ${BASE-} =~ ^0*$ ]] || return 1
  fetch_commit "${BASE}" || return 1
  read_lines patterns "${DOCS_ONLY-}"
  for pattern in "${patterns[@]+"${patterns[@]}"}"; do
    pathspec+=(":(exclude,glob)${pattern}")
  done
  git diff --quiet "${BASE}" HEAD -- "${pathspec[@]}"
}

checks=true
if docs_only; then
  checks=false
  echo "Only docs changed since ${BASE}; skipping checks."
fi
echo "checks=${checks}" >>"${GITEA_OUTPUT}"
