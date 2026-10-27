#!/usr/bin/env bash
# Delete build units in PROFILES not referenced by the cargo JSON on stdin.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/common.sh"

: "${PROFILES:?PROFILES is required}"

declare -A live=()
while IFS= read -r hash; do
  live[${hash}]=1
done < <(grep -oE -- '-[0-9a-f]{16}\b' | cut -c2-)

if [ "${#live[@]}" -eq 0 ]; then
  echo "No build units on stdin; refusing to prune." >&2
  exit 1
fi

profiles=()
read_lines profiles "${PROFILES}"

shopt -s nullglob
pruned=0
for profile in "${profiles[@]}"; do
  for unit in "${profile}"/{deps,build,.fingerprint}/*; do
    [[ ${unit##*/} =~ -([0-9a-f]{16})(\.|$) ]] || continue
    [ -z "${live[${BASH_REMATCH[1]}]-}" ] || continue
    rm -rf "${unit}"
    pruned=$((pruned + 1))
  done
done
echo "Pruned ${pruned} stale build unit file(s)."
