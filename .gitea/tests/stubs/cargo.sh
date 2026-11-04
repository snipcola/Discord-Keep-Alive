#!/usr/bin/env bash
set -euo pipefail

if [ "${1-}" != metadata ]; then
  echo "cargo stub: unexpected arguments: $*" >&2
  exit 1
fi
cat "${CARGO_METADATA_FILE}"
