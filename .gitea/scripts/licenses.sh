#!/usr/bin/env bash
# Generate third-party licence notices covering every target.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/common.sh"

: "${TARGETS:?TARGETS is required}"

targets_parse "${TARGETS}"

args=()
targets_cargo_args args

# Must match the path licenses.rs embeds from.
out=licenses.md

cargo ab "${args[@]}" >"${out}"
echo "${#TARGET_RUST[@]} target(s) -> ${out}"
