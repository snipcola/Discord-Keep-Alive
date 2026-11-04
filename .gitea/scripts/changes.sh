#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/common.sh"

: "${GITEA_OUTPUT:?GITEA_OUTPUT is required}"
: "${SHIPPED_PATHS:?SHIPPED_PATHS is required}"
: "${SCRIPT_PATHS:?SCRIPT_PATHS is required}"

shipped=()
script_paths=()
read_lines shipped "${SHIPPED_PATHS}"
read_lines script_paths "${SCRIPT_PATHS}"

base=""
if [ "${RELEASE-}" != "true" ] && [[ ! ${BASE-} =~ ^0*$ ]]; then
  base="$(fork_point "${BASE}")" || base=""
fi

docs_only() {
  local patterns=() pathspec=(.) pattern
  [ -n "${base}" ] || return 1
  read_lines patterns "${DOCS_ONLY-}"
  for pattern in "${patterns[@]+"${patterns[@]}"}"; do
    pathspec+=(":(exclude,glob)${pattern}")
  done
  git diff --quiet "${base}" HEAD -- "${pathspec[@]}" || return 1
  git diff --quiet "${base}" HEAD -- "${shipped[@]}" || return 1
  echo "Only docs changed since ${base}; skipping checks."
}

checks=true
if docs_only; then
  checks=false
fi

scripts=true
if [ -n "${base}" ] && git diff --quiet "${base}" HEAD -- "${script_paths[@]}"; then
  scripts=false
fi
{
  echo "checks=${checks}"
  echo "scripts=${scripts}"
} >>"${GITEA_OUTPUT}"
