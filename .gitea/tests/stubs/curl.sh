#!/usr/bin/env bash
set -euo pipefail

out=""
method=GET
data=""
retried=false
while [ "$#" -gt 1 ]; do
  case "$1" in
  -o)
    out="$2"
    shift 2
    ;;
  -X)
    method="$2"
    shift 2
    ;;
  -d)
    data="$2"
    shift 2
    ;;
  --retry)
    retried=true
    shift 2
    ;;
  -w | -H | -F | --connect-timeout | --speed-limit | --speed-time) shift 2 ;;
  *) shift ;;
  esac
done
url="$1"
path="${url#*/repos/owner/repo}"

respond() {
  printf '%s' "$2" >"${out}"
  printf '%s' "$1"
}

store_asset() {
  local release="$1" name="$2" sum
  sum="$(printf '%s' "${name}" | cksum | cut -d' ' -f1)"
  printf '%s %s\n' "$((1000 + sum % 1000))" "${name}" >>"${CURL_LOG}.assets.${release}"
}

list_assets() {
  local file="${CURL_LOG}.assets.$1"
  if [ -f "${file}" ]; then
    jq -Rnc '[inputs | split(" ") | {id: (.[0] | tonumber), name: .[1]}]' <"${file}"
  else
    printf '[]'
  fi
}

printf '%s %s%s\n' "${method}" "${path}" "${data:+ ${data}}" >>"${CURL_LOG}"
[ "${retried}" = false ] || printf '%s\n' "${method}" >>"${CURL_LOG}.retry"

case "${method} ${path}" in
"GET /tags/v1.0.0") respond 200 '{"commit":{"sha":"newsha"}}' ;;
"GET /releases/tags/v1.0.0") respond 200 '{"id":1}' ;;
"GET /releases/tags/latest") respond 200 '{"id":7}' ;;
"GET /releases/tags/latest-next") respond 404 '{}' ;;
"GET /tags/latest")
  if grep -qx 'DELETE /tags/latest' "${CURL_LOG}"; then
    respond 200 '{"commit":{"sha":"newsha"}}'
  else
    respond 200 '{"commit":{"sha":"oldsha"}}'
  fi
  ;;
"POST /releases") respond 201 '{"id":9}' ;;
"GET /releases/"*"/assets")
  release="${path#/releases/}"
  respond 200 "$(list_assets "${release%/assets}")"
  ;;
"POST /releases/${CURL_FAIL_RELEASE}/assets?name="*) respond 500 '{"message":"upload failed"}' ;;
"POST /releases/"*"/assets?name="*)
  release="${path#/releases/}"
  release="${release%%/*}"
  name="${path#*name=}"
  if [ "${release}" = "${CURL_BAD_GATEWAY_RELEASE-}" ] && [ ! -e "${CURL_LOG}.bad.${name}" ]; then
    : >"${CURL_LOG}.bad.${name}"
    [ "${CURL_BAD_GATEWAY_STORE-true}" != true ] || store_asset "${release}" "${name}"
    respond 502 '{"message":"bad gateway"}'
  else
    store_asset "${release}" "${name}"
    respond 201 '{}'
  fi
  ;;
*) respond 200 '{}' ;;
esac
