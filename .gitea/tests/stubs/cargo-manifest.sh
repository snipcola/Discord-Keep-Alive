#!/usr/bin/env bash
set -euo pipefail

manifest=""
while [ "$#" -gt 0 ]; do
  case "$1" in
  --manifest-path)
    manifest="$2"
    shift 2
    ;;
  *) shift ;;
  esac
done

version="$(sed -n 's/^version = "\([^"]*\)".*/\1/p' "${manifest}")"
jq -nc --arg version "${version}" '
  {
    packages: [{id: "pkg-id", name: "pkg", version: $version, license: "MIT", repository: "https://r.test/o/p"}],
    workspace_default_members: ["pkg-id"]
  }'
