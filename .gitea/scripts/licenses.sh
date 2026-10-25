#!/usr/bin/env bash
# Generate third-party licence notices per target.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/common.sh"

: "${TARGETS:?TARGETS is required}"

targets_parse "${TARGETS}"

# Must match the path licenses.rs embeds from.
out=licenses

rm -rf "${out}"
mkdir "${out}"

for target in "${TARGET_RUST[@]}"; do
  cargo ab --target "${target}" >"${out}/${target}.md"
  echo "${target} -> ${out}/${target}.md"
done
