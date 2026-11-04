#!/usr/bin/env bash
set -euo pipefail

[ "${1-}" = version ] || exit 1
echo 0.15.1
