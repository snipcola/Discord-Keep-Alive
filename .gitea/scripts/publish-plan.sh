#!/usr/bin/env bash
set -euo pipefail

any=false
tags=()

add_tags() {
  any=true
  tags+=("${1}:${VERSION}" "${1}:latest")
}

if [ "${PUBLISH_LOCAL}" = "true" ]; then
  add_tags "${LOCAL_REF}"
fi
if [ "${PUBLISH_GHCR}" = "true" ]; then
  add_tags "${GHCR_REF}"
fi
if [ "${PUBLISH_DOCKERHUB}" = "true" ]; then
  add_tags "${DOCKER_REF}"
fi

driver_opts="${DOCKER_NETWORK:+network=${DOCKER_NETWORK}}"

cache_from=("type=gha,scope=${IMAGE_NAME}")
cache_to=("type=gha,mode=max,scope=${IMAGE_NAME}")
if [ "${REGISTRY_CACHE}" = "true" ] && [ "${PUBLISH_LOCAL}" = "true" ] && [ -n "${LOCAL_REF}" ]; then
  cache_tag="${REGISTRY_CACHE_TAG:-cache}"
  cache_from+=("type=registry,ref=${LOCAL_REF}:${cache_tag},ignore-error=true")
  cache_to+=("type=registry,ref=${LOCAL_REF}:${cache_tag},mode=max,ignore-error=true")
fi

{
  echo "any=${any}"
  echo "driver_opts=${driver_opts}"
  echo "tags<<EOF"
  ((${#tags[@]})) && printf '%s\n' "${tags[@]}"
  echo "EOF"
  echo "cache_from<<EOF"
  printf '%s\n' "${cache_from[@]}"
  echo "EOF"
  echo "cache_to<<EOF"
  printf '%s\n' "${cache_to[@]}"
  echo "EOF"
} >> "${GITHUB_OUTPUT}"
