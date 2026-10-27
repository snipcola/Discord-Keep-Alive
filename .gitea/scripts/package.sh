#!/usr/bin/env bash
# Complete each BUILD_DIR stage dir and pack it into a reproducible archive.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/common.sh"

: "${TARGETS:?TARGETS is required}"
: "${PACKAGE:?PACKAGE is required}"
: "${VERSION:?VERSION is required}"
: "${BUILD_DIR:=build}"
: "${SOURCE_DATE_EPOCH:=$(git log -1 --format=%ct)}"

targets_parse "${TARGETS}"

XZ_LZMA2=lzma2:preset=9e
declare -A XZ_FILTERS=([linux/amd64]="x86 ${XZ_LZMA2}")

tarball() {
  tar --sort=name --owner=0 --group=0 --numeric-owner --mtime="@${SOURCE_DATE_EPOCH}" \
    -C "${BUILD_DIR}" -cf - "$1"
}

pack() {
  local i="$1" stage archive
  stage="$(stage_name "$i")"
  archive="archives/$(archive_name "$i" "${VERSION}")"

  chmod 755 "${BUILD_DIR}/${stage}"
  install -m644 config.example.toml LICENSE NOTICE "${BUILD_DIR}/${stage}"

  case "${TARGET_ARCHIVES[$i]}" in
  .tar.xz) tarball "${stage}" | xz -T1 --filters="${XZ_FILTERS[${TARGET_PLATFORMS[$i]}]:-${XZ_LZMA2}}" >"${BUILD_DIR}/${archive}" ;;
  .tar.gz) tarball "${stage}" | gzip -9n >"${BUILD_DIR}/${archive}" ;;

  .zip)
    find "${BUILD_DIR}/${stage}" -exec touch -d "@${SOURCE_DATE_EPOCH}" {} +
    (cd "${BUILD_DIR}" && find "${stage}" | LC_ALL=C sort | TZ=UTC zip -9 -X -q -@ "${archive}")
    ;;

  *)
    echo "Unsupported archive format: ${TARGET_ARCHIVES[$i]}" >&2
    return 1
    ;;
  esac

  echo "${stage} -> ${BUILD_DIR}/${archive} ($(du -h "${BUILD_DIR}/${archive}" | cut -f1))"
}

rm -rf "${BUILD_DIR}/archives"
mkdir -p "${BUILD_DIR}/archives"

for i in "${!TARGET_ASSETS[@]}"; do
  bg_start "$(stage_name "$i")" pack "$i"
done
bg_wait
