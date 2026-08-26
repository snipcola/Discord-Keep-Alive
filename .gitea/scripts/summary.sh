#!/usr/bin/env bash
# Write a run summary describing the publish decision and its outputs.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/common.sh"

: "${GITEA_STEP_SUMMARY:?GITEA_STEP_SUMMARY is required}"
: "${PACKAGE:?PACKAGE is required}"
: "${ARTIFACT:=result}"
: "${CHANGED:=false}"
: "${DRY_RUN:=false}"

published=no
suffix=
if [ "${CHANGED}" = "true" ]; then
  published=yes
  if [ "${DRY_RUN}" = "true" ]; then
    published="no (dry run)"
    suffix=" (not pushed)"
  fi
fi

# A failure can land between destinations, so nothing below is verified.
if [ "${FAILED:-false}" = "true" ]; then
  published="incomplete (run failed)"
  suffix=" (unverified)"
fi

row() { printf -- '- **%s:** %s\n' "$1" "$2"; }
item() { printf -- '- `%s`%s\n' "$1" "${2:+ ($2)}"; }
heading() { printf '\n#### %s\n\n' "$1"; }

summary() {
  echo "### ${PACKAGE}${VERSION:+ v${VERSION}}"
  echo
  row Published "${published}"
  [ -z "${REASON-}" ] || row Reason "\`${REASON}\`"
  [ -z "${TAG-}" ] || row Tag "\`${TAG}\`"
  [ -z "${REVISION-}" ] || row Commit "\`${REVISION:0:12}\`"

  [ "${CHANGED}" = "true" ] || return 0

  if [ -n "$(trim "${TARGETS-}")" ]; then
    targets_parse "${TARGETS}"
    heading Binaries
    for target in "${TARGET_RUST[@]}"; do
      bin="${DIST_DIR:-dist}/${target}/${ARTIFACT}"
      if [ -f "${bin}" ]; then
        item "${target}" "$(du -h "${bin}" | cut -f1)"
      else
        item "${target}" missing
      fi
    done
  fi

  if [ -n "$(trim "${REFS-}")" ]; then
    refs_parse "${REFS}"
    heading "Images${suffix}"
    for ref in "${REFS_LIST[@]}"; do
      item "${ref}:${VERSION}"
    done
  fi
}

summary >>"${GITEA_STEP_SUMMARY}"
