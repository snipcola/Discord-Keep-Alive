#!/usr/bin/env bash
# shellcheck disable=SC2016
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "${HERE}/../.." && pwd)"
GITEA_DIR="${GITEA_DIR:-${ROOT}/.gitea}"
SCRIPTS="${GITEA_DIR}/scripts"
RELEASE_RUN="${GITEA_DIR}/actions/gitea-release/run.sh"

for tool in git jq gawk; do
  if ! command -v "${tool}" >/dev/null; then
    echo "${tool} is required to run the script tests." >&2
    exit 1
  fi
done

root_tmp="$(mktemp -d)"
trap 'rm -rf "${root_tmp}"' EXIT

TEST_JOBS="${TEST_JOBS:-$(nproc 2>/dev/null || echo 4)}"

export GIT_AUTHOR_NAME=test GIT_COMMITTER_NAME=test
export GIT_AUTHOR_EMAIL=test@example.test GIT_COMMITTER_EMAIL=test@example.test
export GIT_CONFIG_COUNT=2
export GIT_CONFIG_KEY_0=commit.gpgsign GIT_CONFIG_VALUE_0=false
export GIT_CONFIG_KEY_1=tag.gpgsign GIT_CONFIG_VALUE_1=false

passes=0
failures=0

pass() { passes=$((passes + 1)); }

fail() {
  failures=$((failures + 1))
  echo "FAIL: $1" >&2
}

expect_eq() {
  local name="$1" expected="$2" actual="$3"
  if [ "${expected}" = "${actual}" ]; then
    pass
    return 0
  fi
  fail "${name}"
  printf '%s\n' "${expected}" >"${tmp}/expected"
  printf '%s\n' "${actual}" >"${tmp}/actual"
  diff "${tmp}/expected" "${tmp}/actual" | sed 's/^/  /' >&2 || true
}

expect_contains() {
  local name="$1" needle="$2" haystack="$3"
  if [[ ${haystack} == *"${needle}"* ]]; then
    pass
    return 0
  fi
  fail "${name}: expected to contain: ${needle}"
  printf '%s\n' "${haystack}" | sed 's/^/  | /' >&2
}

expect_lacks() {
  local name="$1" needle="$2" haystack="$3"
  if [[ ${haystack} != *"${needle}"* ]]; then
    pass
    return 0
  fi
  fail "${name}: expected not to contain: ${needle}"
  printf '%s\n' "${haystack}" | sed 's/^/  | /' >&2
}

lines() { printf '%s\n' "$@"; }

make_stub() {
  local dir="$1" name="$2"
  mkdir -p "${dir}"
  {
    printf '#!%s\nexec' "${BASH}"
    printf ' %q' "${@:3}"
    printf ' "$@"\n'
  } >"${dir}/${name}"
  chmod +x "${dir}/${name}"
}

init_repo() {
  local dir="$1"
  git init -q -b main "${dir}"
}

SHIPPED_LIST="crates Cargo.toml Cargo.lock rust-toolchain.toml Dockerfile config.example.toml"
SHIPPED_LINES="${SHIPPED_LIST// /$'\n'}"
DOCS_ONLY_LINES="$(lines .editorconfig .gitignore .config/about.hbs .config/about.toml docker-compose.example.yaml LICENSE README.md)"
SCRIPT_LINES="$(lines .gitea .config/deny.toml .config/about.toml)"

awk_run() {
  gawk -v repo=https://r.test -v "BREAKING_SHAS= $1" -v "BUMP_SHAS= $2" -v "SHIPPED=${SHIPPED_LIST}" \
    -v RS_MARK=$'\x01' -v FS_MARK=$'\x02' -f "${SCRIPTS}/changelog.awk"
}

log_entry() {
  local hash="$1" subject="$2" note="$3"
  shift 3
  printf '\x01%s\x02%s\x02%s\n' "${hash}" "${subject}" "${note}"
  printf '%s\n' "$@"
  printf '\n'
}

link() { printf '([`%s`](https://r.test/commit/%s))' "$1" "$1"; }

test_changelog_awk() {
  local actual

  actual="$(log_entry a1 'feat(api)!: drop v1' '' crates/x.rs | awk_run '' '')"
  expect_eq "awk breaking bang" "$(lines '### Breaking changes' '' "- api: drop v1 $(link a1)")" "${actual}"

  actual="$(log_entry a2 'feat: footer thing' '' crates/x.rs | awk_run 'a2 ' '')"
  expect_eq "awk breaking footer" "$(lines '### Breaking changes' '' "- footer thing $(link a2)")" "${actual}"

  actual="$(log_entry a3 'fix(cli): crash' '' crates/x.rs | awk_run '' '')"
  expect_eq "awk scope" "$(lines '### Fixes' '' "- cli: crash $(link a3)")" "${actual}"

  actual="$(log_entry a4 'chore(deps): bump serde (1.2.3)' '' Cargo.lock | awk_run '' '')"
  expect_eq "awk deps scope" "$(lines '### Dependencies' '' "- bump serde $(link a4)")" "${actual}"

  actual="$(log_entry a5 'feat: internal detail' 'Nicer wording' crates/x.rs | awk_run '' '')"
  expect_eq "awk Release-Note replaces subject" "$(lines '### Features' '' "- Nicer wording $(link a5)")" "${actual}"

  actual="$(log_entry a6 'fix: hidden' 'None' crates/x.rs | awk_run '' '')"
  expect_eq "awk Release-Note none hides commit" "No notable changes." "${actual}"

  actual="$(log_entry a7 'fix: handle `Vec<u8>` & <b>' '' crates/x.rs | awk_run '' '')"
  expect_eq "awk backticks keep code spans unescaped" \
    "$(lines '### Fixes' '' "- handle \`Vec<u8>\` &amp; &lt;b&gt; $(link a7)")" "${actual}"

  actual="$(log_entry a8 'Revert "feat: x"' '' crates/x.rs | awk_run '' '')"
  expect_eq "awk revert lands in Other" "$(lines '### Other' '' "- Revert \"feat: x\" $(link a8)")" "${actual}"

  actual="$(log_entry a9 'fix: base image' '' Dockerfile | awk_run '' '')"
  expect_eq "awk Dockerfile-only change is user-facing" \
    "$(lines '### Fixes' '' "- base image $(link a9)")" "${actual}"

  actual="$(log_entry c1 'fix: example default' '' config.example.toml | awk_run '' '')"
  expect_eq "awk config.example.toml-only change is user-facing" \
    "$(lines '### Fixes' '' "- example default $(link c1)")" "${actual}"

  actual="$(log_entry c2 'fix: dev image' '' Dockerfile.dev | awk_run '' '')"
  expect_eq "awk shipped path must match whole name" \
    "$(lines '<details><summary>Internal (1)</summary>' '' '#### Fixes' '' "- dev image $(link c2)" '' '</details>')" "${actual}"

  actual="$(log_entry b1 'docs: readme' '' README.md | awk_run '' '')"
  expect_eq "awk docs-only change lands in Internal" \
    "$(lines '<details><summary>Internal (1)</summary>' '' '#### Chores' '' "- readme $(link b1)" '' '</details>')" "${actual}"

  actual="$(printf '' | awk_run '' '')"
  expect_eq "awk empty input" "No notable changes." "${actual}"

  actual="$(log_entry b2 'chore: bump version to 1.2.3' '' Cargo.toml | awk_run '' 'b2 ')"
  expect_eq "awk bump exclusion" "No notable changes." "${actual}"
}

run_changelog() {
  local dir="$1" path="$2" tag="$3"
  (
    cd "${dir}"
    PATH="${path}" TAG="${tag}" REPO_URL=https://r.test/ SHIPPED_PATHS="${SHIPPED_LINES}" "${BASH}" "${SCRIPTS}/changelog.sh"
  ) 2>"${tmp}/changelog.err"
}

make_changelog_repo() {
  local dir="$1"
  init_repo "${dir}"
  mkdir -p "${dir}/crates"
  printf '1\n' >"${dir}/crates/a.rs"
  git -C "${dir}" add -A
  git -C "${dir}" commit -q -m 'feat: first'
  git -C "${dir}" tag v1.0.0
  printf '2\n' >>"${dir}/crates/a.rs"
  git -C "${dir}" commit -q -am 'feat(api): add thing'
  printf '3\n' >>"${dir}/crates/a.rs"
  git -C "${dir}" commit -q -am 'fix: crash'
  git -C "${dir}" tag v1.1.0
}

test_changelog_script() {
  local bin="${tmp}/changelog-bin" repo="${tmp}/changelog-repo" out rc err
  local fallback='**Full changelog**: [v1.0.0](https://r.test/commits/tag/v1.0.0)'

  make_stub "${bin}" dirname "$(command -v dirname)"

  rc=0
  out="$(run_changelog "${tmp}" "${bin}" v1.0.0)" || rc=$?
  err="$(<"${tmp}/changelog.err")"
  expect_eq "changelog without yq or gawk exits 0" 0 "${rc}"
  expect_eq "changelog without yq or gawk prints fallback" "${fallback}" "${out}"
  expect_contains "changelog without yq warns" "::warning::Release notes unavailable: yq is not installed." "${err}"

  make_stub "${bin}" yq "${BASH}" -c 'exit 0'
  rc=0
  out="$(run_changelog "${tmp}" "${bin}" v1.0.0)" || rc=$?
  err="$(<"${tmp}/changelog.err")"
  expect_eq "changelog without gawk exits 0" 0 "${rc}"
  expect_eq "changelog without gawk prints fallback" "${fallback}" "${out}"
  expect_contains "changelog without gawk warns" "::warning::Release notes unavailable: gawk is not installed." "${err}"

  make_stub "${bin}" gawk "${BASH}" -c 'exit 1'
  rc=0
  out="$(run_changelog "${tmp}" "${bin}" latest)" || rc=$?
  err="$(<"${tmp}/changelog.err")"
  expect_eq "changelog tag without version exits 0" 0 "${rc}"
  expect_contains "changelog tag without version warns" "has no version number" "${err}"

  rm "${bin}/gawk"
  make_changelog_repo "${repo}"

  rc=0
  out="$(run_changelog "${repo}" "${bin}:${PATH}" v1.1.0 | sed -E 's/[0-9a-f]{7,}/H/g')" || rc=$?
  expect_eq "changelog end to end exits 0" 0 "${rc}"
  expect_eq "changelog end to end output" "$(lines \
    '### Features' '' '- api: add thing ([`H`](https://r.test/commit/H))' '' \
    '### Fixes' '' '- crash ([`H`](https://r.test/commit/H))' '' \
    '**Full changelog**: [v1.0.0...v1.1.0](https://r.test/compare/v1.0.0...v1.1.0)')" "${out}"

  make_stub "${bin}" gawk "${BASH}" -c 'exit 1'
  rc=0
  out="$(run_changelog "${repo}" "${bin}:${PATH}" v1.1.0)" || rc=$?
  err="$(<"${tmp}/changelog.err")"
  expect_eq "changelog with failing gawk exits 0" 0 "${rc}"
  expect_eq "changelog with failing gawk prints only the link" \
    '**Full changelog**: [v1.0.0...v1.1.0](https://r.test/compare/v1.0.0...v1.1.0)' "${out}"
  expect_contains "changelog with failing gawk warns" "::warning::Release notes unavailable: generating them failed" "${err}"
}

test_targets_parse() {
  local input out rc
  input="$(lines \
    'linux/arm64=aarch64-unknown-linux-musl' \
    'windows/amd64=x86_64-pc-windows-gnu # trailing comment' \
    '' \
    '# full-line comment' \
    '  darwin/arm64 = aarch64-apple-darwin  ')"

  out="$(
    source "${SCRIPTS}/common.sh"
    targets_parse "${input}"
    for field in TARGET_PLATFORMS TARGET_RUST TARGET_NAMES TARGET_EXTS TARGET_ASSETS TARGET_ARCHIVES; do
      declare -n values="${field}"
      printf '%s=%s\n' "${field}" "$(printf '%s|' "${values[@]}")"
    done
  )"
  expect_eq "targets_parse sorted rows" "$(lines \
    'TARGET_PLATFORMS=linux/arm64|darwin/arm64|windows/amd64|' \
    'TARGET_RUST=aarch64-unknown-linux-musl|aarch64-apple-darwin|x86_64-pc-windows-gnu|' \
    'TARGET_NAMES=linux-arm64|darwin-arm64|windows-amd64|' \
    'TARGET_EXTS=||.exe|' \
    'TARGET_ASSETS=linux-arm64|macos-arm64|windows-x64|' \
    'TARGET_ARCHIVES=.tar.xz|.tar.gz|.zip|')" "${out}"

  out="$(
    source "${SCRIPTS}/common.sh"
    targets_parse "${input}" linux
    printf '%s|' "${TARGET_PLATFORMS[@]}"
  )"
  expect_eq "targets_parse os filter" 'linux/arm64|' "${out}"

  local name raw needle
  while IFS='~' read -r name raw needle; do
    raw="${raw//;/$'\n'}"
    rc=0
    out="$(
      source "${SCRIPTS}/common.sh"
      targets_parse "${raw}" 2>&1
    )" || rc=$?
    expect_eq "targets_parse rejects ${name}: exit status" 1 "${rc}"
    expect_contains "targets_parse rejects ${name}: message" "${needle}" "${out}"
  done <<'EOF'
missing equals~linux/amd64~Invalid target (expected platform=target)
empty rust target~linux/amd64=~Invalid target (empty platform or Rust target)
bad platform~linux=x86_64-unknown-linux-musl~Invalid Docker platform
bad rust target~linux/amd64=not valid~Invalid Rust target
unsupported os~freebsd/amd64=x86_64-unknown-freebsd~Unsupported platform
unsupported arch~linux/riscv64=riscv64gc-unknown-linux-gnu~Unsupported platform
duplicate~linux/amd64=a-b;linux/amd64=c-d~Duplicate Docker platform
no targets~# nothing~No targets provided
EOF
}

test_ref_is_base() {
  local ref
  for ref in name ghcr.io/o/p docker.io/o/p localhost:5000/o/p; do
    if (source "${SCRIPTS}/common.sh" && ref_is_base "${ref}"); then
      pass
    else
      fail "ref_is_base accepts ${ref}"
    fi
  done
  for ref in "" o/p:latest ghcr.io/o/p:1.2.3 o/p@sha256:abc ghcr.io/o/p:tag@sha256:abc; do
    if (source "${SCRIPTS}/common.sh" && ref_is_base "${ref}"); then
      fail "ref_is_base rejects '${ref}'"
    else
      pass
    fi
  done
}

bg_job() {
  sleep 0.05
  echo "job ${1} ran"
  return "${1}"
}

bg_batch_guarded() {
  local max="$1" n
  shift
  (
    source "${SCRIPTS}/common.sh"
    export BG_MAX="${max}"
    for n in "$@"; do
      bg_start "job-${n}" bg_job "${n}"
    done
    bg_wait || exit 7
    echo "batch succeeded"
  ) 2>&1
}

test_bg_wait() {
  local out rc

  rc=0
  out="$(bg_batch_guarded 0 0 3)" || rc=$?
  expect_eq "bg_wait under || reports a failed job: exit status" 7 "${rc}"
  expect_contains "bg_wait under || names the failed job" "Failed: job-3" "${out}"
  expect_contains "bg_wait under || prints job output" "job 3 ran" "${out}"
  expect_lacks "bg_wait under || does not report success" "batch succeeded" "${out}"

  rc=0
  out="$(bg_batch_guarded 0 0 0)" || rc=$?
  expect_eq "bg_wait under || with passing jobs: exit status" 0 "${rc}"
  expect_contains "bg_wait under || with passing jobs reports success" "batch succeeded" "${out}"
  expect_lacks "bg_wait with passing jobs reports no failure" "Failed:" "${out}"

  rc=0
  out="$(bg_batch_guarded 2 0 5 0 0 6 0)" || rc=$?
  expect_eq "bg_wait with BG_MAX=2: exit status" 7 "${rc}"
  expect_contains "bg_wait with BG_MAX=2 names the failures" "Failed: job-5 job-6" "${out}"

  rc=0
  out="$(
    bash -c 'source "$1/common.sh"; bg_start broken bash -c "exit 4"; bg_wait; echo "reached after failure"' _ "${SCRIPTS}" 2>&1
  )" || rc=$?
  expect_eq "bg_wait without || stops the caller: exit status" 1 "${rc}"
  expect_lacks "bg_wait without || stops the caller" "reached after failure" "${out}"
}

bake_json() {
  jq -S --arg dockerfile "$(realpath "${tmp}/Dockerfile")" \
    'walk(if type == "string" then sub("@DOCKERFILE@"; $dockerfile) else . end)' "$1"
}

run_bake_plan() {
  local push="$1" source="$2" licence="$3" out="$4"
  env -u GITEA_OUTPUT \
    PATH="${tmp}/bake-bin:${PATH}" \
    CARGO_METADATA_FILE="${HERE}/fixtures/cargo-metadata.json" \
    TARGETS="$(lines linux/arm64=aarch64-unknown-linux-musl linux/amd64=x86_64-unknown-linux-musl darwin/arm64=aarch64-apple-darwin)" \
    REFS="$(lines ghcr.io/o/p docker.io/o/p)" \
    BAKE_FILE="${out}" BUILD_DIR=build DOCKERFILE="${tmp}/Dockerfile" \
    VERSION=1.2.3 REVISION=deadbeef PACKAGE=pkg CREATED=2026-01-02T03:04:05Z \
    SOURCE="${source}" LICENCE="${licence}" PUSH="${push}" \
    bash "${SCRIPTS}/docker-bake-plan.sh" >/dev/null
}

test_docker_bake_plan() {
  : >"${tmp}/Dockerfile"
  make_stub "${tmp}/bake-bin" cargo "${BASH}" "${HERE}/stubs/cargo.sh"

  run_bake_plan true https://example.test/o/p MIT "${tmp}/bake-push.json"
  expect_eq "bake plan with push" "$(bake_json "${HERE}/fixtures/bake-push.json")" \
    "$(jq -S . "${tmp}/bake-push.json")"

  run_bake_plan false "" "" "${tmp}/bake-nopush.json"
  expect_eq "bake plan without push or optional labels" "$(bake_json "${HERE}/fixtures/bake-nopush.json")" \
    "$(jq -S . "${tmp}/bake-nopush.json")"
}

test_prune_target() {
  local profile="${tmp}/prune/debug" live=0123456789abcdef dead=fedcba9876543210 out rc

  mkdir -p "${profile}"/{deps,build,.fingerprint}
  : >"${profile}/deps/libfoo-${live}.rlib"
  : >"${profile}/deps/libfoo-${dead}.rlib"
  : >"${profile}/deps/foo-${dead}.d"
  : >"${profile}/deps/notes.txt"
  mkdir -p "${profile}/build/bar-${live}" "${profile}/build/bar-${dead}" "${profile}/.fingerprint/baz-${dead}"
  : >"${profile}/build/bar-${dead}/out"

  rc=0
  out="$(printf '{"filenames":["%s/deps/libfoo-%s.rlib"]}\n' "${profile}" "${live}" |
    env PROFILES="${profile}" bash "${SCRIPTS}/prune-target.sh" 2>&1)" || rc=$?
  expect_eq "prune-target exit status" 0 "${rc}"
  expect_eq "prune-target summary" "Pruned 4 stale build unit file(s)." "${out}"
  expect_eq "prune-target keeps live and unrelated units" "$(lines \
    "${profile}/.fingerprint" \
    "${profile}/build" \
    "${profile}/build/bar-${live}" \
    "${profile}/deps" \
    "${profile}/deps/libfoo-${live}.rlib" \
    "${profile}/deps/notes.txt")" "$(find "${profile}" -mindepth 1 | LC_ALL=C sort)"

  rc=0
  out="$(printf '' | env PROFILES="${profile}" bash "${SCRIPTS}/prune-target.sh" 2>&1)" || rc=$?
  expect_eq "prune-target refuses empty input: exit status" 1 "${rc}"
  expect_contains "prune-target refuses empty input: message" "refusing to prune" "${out}"
  expect_eq "prune-target refuses empty input: nothing deleted" 6 "$(find "${profile}" -mindepth 1 | wc -l | tr -d ' ')"
}

test_package_xz_filters() {
  local filter
  filter="$(
    eval "$(grep -E '^(XZ_LZMA2=|declare -A XZ_FILTERS=)' "${SCRIPTS}/package.sh")"
    echo "${XZ_FILTERS["linux/amd64"]-}"
  )"
  expect_eq "package xz filter for linux/amd64" "x86 lzma2:preset=9e" "${filter}"
}

run_release() {
  local dir="$1" fail_release="$2"
  shift 2
  mkdir -p "${dir}/build/archives"
  : >"${dir}/config.example.toml"
  : >"${dir}/build/archives/pkg-1.0.0-linux-x64.tar.xz"
  : >"${dir}/curl.log"
  : >"${dir}/output"
  make_stub "${dir}/bin" curl "${BASH}" "${HERE}/stubs/curl.sh"
  (
    cd "${dir}"
    env PATH="${dir}/bin:${PATH}" \
      CURL_LOG="${dir}/curl.log" CURL_FAIL_RELEASE="${fail_release}" GITEA_OUTPUT="${dir}/output" \
      VERSION=1.0.0 TAG=v1.0.0 PACKAGE=pkg TOKEN=secret REPOSITORY=owner/repo \
      API_URL=https://api.test REPO_URL=https://r.test TARGETS=linux/amd64=x86_64-unknown-linux-musl \
      BUILD_DIR=build DRAFT=false PRERELEASE=false LATEST=true UPLOAD_JOBS=2 BODY=notes \
      UPLOAD_RETRY_DELAY=0 "$@" \
      bash "${RELEASE_RUN}"
  ) 2>&1
}

count_lines() { grep -cxF -- "$1" <<<"$2" || true; }

expect_failure() {
  if [ "$2" -ne 0 ]; then
    pass
  else
    fail "$1: exit status was 0"
  fi
}

test_release_working_uploads() {
  local dir="${tmp}/release-ok" out rc=0 log

  out="$(run_release "${dir}" none)" || rc=$?
  log="$(<"${dir}/curl.log")"
  expect_eq "publish_latest with working uploads: exit status" 0 "${rc}"
  expect_contains "publish_latest with working uploads deletes the live release" "DELETE /releases/7" "${log}"
  expect_contains "publish_latest with working uploads deletes the old tag" "DELETE /tags/latest" "${log}"
  expect_contains "publish_latest with working uploads promotes the staged release" "PATCH /releases/9" "${log}"

  expect_contains "reused release is announced" "Reusing existing release v1.0.0 (1)" "${out}"
  expect_contains "reused release gets the regenerated notes" \
    'PATCH /releases/1 {"tag_name":"v1.0.0","body":"notes","draft":null}' "${log}"
  expect_lacks "reused release is not created again" 'POST /releases {"tag_name":"v1.0.0"' "${log}"

  expect_eq "retry flags are sent only for GET and DELETE" \
    "$(lines DELETE DELETE GET | LC_ALL=C sort -u)" "$(LC_ALL=C sort -u "${dir}/curl.log.retry")"
  expect_eq "no POST or PATCH is retried" 0 "$(grep -cE '^(POST|PATCH)$' "${dir}/curl.log.retry" || true)"
}

test_publish_latest() {
  local dir out rc log

  dir="${tmp}/release-stage-fails"
  rc=0
  out="$(run_release "${dir}" 9)" || rc=$?
  log="$(<"${dir}/curl.log")"
  expect_failure "publish_latest with a failed staged upload" "${rc}"
  expect_contains "publish_latest with a failed staged upload reports it" "Failed to upload" "${out}"
  expect_lacks "publish_latest with a failed staged upload keeps the live release" "DELETE /releases/7" "${log}"
  expect_lacks "publish_latest with a failed staged upload keeps the live tag" "DELETE /tags/latest" "${log}"
  expect_lacks "publish_latest with a failed staged upload does not promote" "PATCH /releases/9" "${log}"

  dir="${tmp}/release-main-fails"
  rc=0
  out="$(run_release "${dir}" 1)" || rc=$?
  log="$(<"${dir}/curl.log")"
  expect_failure "release with a failed upload" "${rc}"
  expect_lacks "release with a failed upload never touches latest" "latest" "${log}"
  expect_lacks "release with a failed upload never deletes" "DELETE" "${log}"
  expect_lacks "release output never shows the token" "secret" "${out}"
}

test_release_integrity() {
  local dir out rc log post='POST /releases/1/assets?name=pkg-1.0.0-linux-x64.tar.xz'

  dir="${tmp}/release-502-stored"
  rc=0
  out="$(run_release "${dir}" none CURL_BAD_GATEWAY_RELEASE=1)" || rc=$?
  log="$(<"${dir}/curl.log")"
  expect_eq "502 after the asset was stored: exit status" 0 "${rc}"
  expect_eq "502 after the asset was stored: one upload attempt" 1 "$(count_lines "${post}" "${log}")"
  expect_eq "502 after the asset was stored: no duplicate asset" 1 \
    "$(grep -c ' pkg-1.0.0-linux-x64.tar.xz$' "${dir}/curl.log.assets.1")"
  expect_contains "502 after the asset was stored is reported" "despite a failed response" "${out}"

  dir="${tmp}/release-502-lost"
  rc=0
  out="$(run_release "${dir}" none CURL_BAD_GATEWAY_RELEASE=1 CURL_BAD_GATEWAY_STORE=false)" || rc=$?
  log="$(<"${dir}/curl.log")"
  expect_eq "502 before the asset was stored: exit status" 0 "${rc}"
  expect_eq "502 before the asset was stored: uploaded again" 2 "$(count_lines "${post}" "${log}")"
  expect_eq "502 before the asset was stored: exactly one asset" 1 \
    "$(grep -c ' pkg-1.0.0-linux-x64.tar.xz$' "${dir}/curl.log.assets.1")"

  dir="${tmp}/release-500-always"
  rc=0
  out="$(run_release "${dir}" 1)" || rc=$?
  log="$(<"${dir}/curl.log")"
  expect_eq "persistent upload failure stops after three attempts" 3 "$(count_lines "${post}" "${log}")"
}

run_publish_meta() {
  local dir="$1" base="$2" out="$3"
  shift 3
  : >"${out}"
  (
    cd "${dir}"
    env PATH="${tmp}/meta-bin:${PATH}" GITEA_OUTPUT="${out}" BASE="${base}" \
      FORCE_RELEASE=false RELEASE_MARKER='#release' RELEASE_TRAILER=Force-Release \
      GH_USER=gh DH_USER=dh PUBLISH_LOCAL=false PUBLISH_GHCR=true PUBLISH_DOCKERHUB=true "$@" \
      bash "${GITEA_DIR}/actions/cargo-publish-meta/run.sh"
  ) >"${out}.log" 2>&1
}

make_meta_repo() {
  local repo="$1"

  make_stub "${tmp}/meta-bin" cargo "${BASH}" "${HERE}/stubs/cargo-manifest.sh"
  git init -q --bare "${repo}-origin.git"
  init_repo "${repo}"
  git -C "${repo}" remote add origin "${repo}-origin.git"
  printf 'version = "1.0.0"\n' >"${repo}/Cargo.toml"
  git -C "${repo}" add -A
  git -C "${repo}" commit -q -m 'feat: first'
  printf 'version = "1.0.1"\n' >"${repo}/Cargo.toml"
  git -C "${repo}" commit -q -am 'chore: bump version to 1.0.1'
  printf 'notes\n' >"${repo}/NOTES.md"
  git -C "${repo}" add -A
  git -C "${repo}" commit -q -m 'docs: notes'
}

test_publish_meta_tag_check() {
  local repo="${tmp}/meta-repo" out="${tmp}/meta-output" base rc

  make_meta_repo "${repo}"
  base="$(git -C "${repo}" rev-parse HEAD~1)"

  run_publish_meta "${repo}" "${base}" "${out}"
  expect_contains "untagged version: changed" "changed=true" "$(<"${out}")"
  expect_contains "untagged version: reason" "reason=untagged:1.0.1" "$(<"${out}")"

  run_publish_meta "${repo}" "${base}" "${out}" PUBLISH_GHCR=false PUBLISH_DOCKERHUB=false
  expect_contains "untagged version without destinations: unchanged" "changed=false" "$(<"${out}")"
  expect_lacks "untagged version without destinations: no untagged reason" "untagged" "$(<"${out}")"

  run_publish_meta "${repo}" "${base}" "${out}" PUBLISH_LOCAL=true PUBLISH_GHCR=false PUBLISH_DOCKERHUB=false \
    LOCAL_REGISTRY=registry.test LOCAL_USER=owner
  expect_contains "untagged version with one destination: reason" "reason=untagged:1.0.1" "$(<"${out}")"

  git -C "${repo}" tag 1.0.1 "${base}"
  git -C "${repo}" push -q origin 1.0.1
  run_publish_meta "${repo}" "${base}" "${out}"
  expect_contains "tagged version: unchanged" "changed=false" "$(<"${out}")"
  expect_contains "tagged version: no reason" "reason=" "$(<"${out}")"
  expect_lacks "tagged version: no untagged reason" "untagged" "$(<"${out}")"

  git -C "${repo}" remote set-url origin "${tmp}/missing.git"
  rc=0
  run_publish_meta "${repo}" "${base}" "${out}" || rc=$?
  expect_failure "unreachable origin" "${rc}"
  expect_contains "unreachable origin refuses to publish" "refusing to publish" "$(<"${out}.log")"
}

run_changes() {
  local dir="$1" base="$2" out="$3"
  shift 3
  : >"${out}"
  (
    cd "${dir}"
    env BASE="${base}" GITEA_OUTPUT="${out}" DOCS_ONLY="$(lines README.md .config/about.toml Dockerfile config.example.toml)" \
      SHIPPED_PATHS="${SHIPPED_LINES}" SCRIPT_PATHS="${SCRIPT_LINES}" "$@" \
      bash "${SCRIPTS}/changes.sh"
  ) >/dev/null 2>&1
}

test_changes() {
  local repo="${tmp}/changes-repo" out="${tmp}/changes-output" first second third fourth

  init_repo "${repo}"
  : >"${repo}/Dockerfile"
  : >"${repo}/README.md"
  git -C "${repo}" add -A
  git -C "${repo}" commit -q -m 'feat: first'
  first="$(git -C "${repo}" rev-parse HEAD)"
  printf 'x\n' >"${repo}/Dockerfile"
  git -C "${repo}" commit -q -am 'fix: image'
  second="$(git -C "${repo}" rev-parse HEAD)"
  printf 'x\n' >"${repo}/README.md"
  git -C "${repo}" commit -q -am 'docs: readme'
  third="$(git -C "${repo}" rev-parse HEAD)"

  run_changes "${repo}" "${first}" "${out}"
  expect_eq "Dockerfile change runs the checks and skips the script tests" \
    "$(lines checks=true scripts=false)" "$(<"${out}")"

  run_changes "${repo}" "${second}" "${out}"
  expect_eq "README-only change skips the checks and the script tests" \
    "$(lines checks=false scripts=false)" "$(<"${out}")"

  mkdir -p "${repo}/.config"
  printf 'x\n' >"${repo}/.config/about.toml"
  git -C "${repo}" add -A
  git -C "${repo}" commit -q -m 'docs: about'
  fourth="$(git -C "${repo}" rev-parse HEAD)"
  run_changes "${repo}" "${third}" "${out}"
  expect_eq ".config/about.toml-only change skips the checks but runs the script tests" \
    "$(lines checks=false scripts=true)" "$(<"${out}")"

  mkdir -p "${repo}/crates"
  printf 'x\n' >"${repo}/crates/a.rs"
  git -C "${repo}" add -A
  git -C "${repo}" commit -q -m 'feat: crate'
  run_changes "${repo}" "${fourth}" "${out}"
  expect_eq "crates-only change runs the checks and skips the script tests" \
    "$(lines checks=true scripts=false)" "$(<"${out}")"

  fourth="$(git -C "${repo}" rev-parse HEAD)"
  mkdir -p "${repo}/.gitea/scripts"
  printf 'x\n' >"${repo}/.gitea/scripts/a.sh"
  git -C "${repo}" add -A
  git -C "${repo}" commit -q -m 'ci: script'
  run_changes "${repo}" "${fourth}" "${out}"
  expect_eq ".gitea change runs the checks and the script tests" \
    "$(lines checks=true scripts=true)" "$(<"${out}")"

  run_changes "${repo}" 0 "${out}"
  expect_eq "no base runs the checks and the script tests" \
    "$(lines checks=true scripts=true)" "$(<"${out}")"

  run_changes "${repo}" "${fourth}" "${out}" RELEASE=true
  expect_eq "release run runs the checks and the script tests" \
    "$(lines checks=true scripts=true)" "$(<"${out}")"

  run_changes "${repo}" refs/heads/missing "${out}"
  expect_eq "unknown fork point runs the checks and the script tests" \
    "$(lines checks=true scripts=true)" "$(<"${out}")"
}

yaml_block() {
  gawk -v key="$1" '
    f { match($0, /^ */); if (RLENGTH <= ind && $0 !~ /^ *$/) exit; sub(/^ +/, ""); print; next }
    match($0, "^ *" key ": \\|$") { match($0, /^ */); ind = RLENGTH; f = 1 }
  ' "$2"
}

test_ci_paths() {
  local ci="${GITEA_DIR}/workflows/ci.yaml" out rc name
  expect_eq "ci.yaml SHIPPED_PATHS" "${SHIPPED_LINES}" "$(yaml_block SHIPPED_PATHS "${ci}")"
  expect_eq "ci.yaml DOCS_ONLY" "${DOCS_ONLY_LINES}" "$(yaml_block DOCS_ONLY "${ci}")"
  expect_eq "ci.yaml SCRIPT_PATHS" "${SCRIPT_LINES}" "$(yaml_block SCRIPT_PATHS "${ci}")"
  expect_eq "ci.yaml defines SHIPPED_PATHS once" 1 "$(grep -c '^ *SHIPPED_PATHS: |$' "${ci}")"
  expect_eq "ci.yaml passes shipped_paths to the release action" 1 \
    "$(grep -cF 'shipped_paths: ${{ env.SHIPPED_PATHS }}' "${ci}")"

  for name in SHIPPED_PATHS SCRIPT_PATHS; do
    : >"${tmp}/changes-missing"
    rc=0
    out="$(
      cd "${tmp}"
      env -u SHIPPED_PATHS -u SCRIPT_PATHS BASE=0 GITEA_OUTPUT="${tmp}/changes-missing" \
        SHIPPED_PATHS="${SHIPPED_LINES}" SCRIPT_PATHS="${SCRIPT_LINES}" "${name}=" \
        bash "${SCRIPTS}/changes.sh" 2>&1
    )" || rc=$?
    expect_failure "changes.sh without ${name}" "${rc}"
    expect_contains "changes.sh without ${name} names the variable" "${name} is required" "${out}"
  done
}

test_cache_key() {
  local out="${tmp}/cache-output" action="${GITEA_DIR}/actions/cargo-target-cache/action.yaml"

  make_stub "${tmp}/zig-bin" zig "${BASH}" "${HERE}/stubs/zig.sh"
  : >"${out}"
  env PATH="${tmp}/zig-bin:${PATH}" GITEA_OUTPUT="${out}" TARGETS="" \
    bash "${GITEA_DIR}/actions/cargo-target-cache/resolve.sh"
  expect_contains "cache resolves the installed zig version" "zig=0.15.1" "$(<"${out}")"
  expect_eq "cache key and restore key both carry the zig version" 2 \
    "$(grep -c 'zig${{ steps.paths.outputs.zig }}' "${action}")"
}

test_output_lines() {
  local out="${tmp}/output-lines" first second content body
  content="$(lines alpha EOF 'two words' '')"

  : >"${out}"
  printf '%s\n' "${content}" | (source "${SCRIPTS}/common.sh" && GITEA_OUTPUT="${out}" output_lines body)
  first="$(head -n 1 "${out}")"
  expect_contains "output_lines opens with name<<delimiter" "body<<EOF_" "${first}"
  expect_eq "output_lines closes with the same delimiter" "${first#body<<}" "$(tail -n 1 "${out}")"
  body="$(sed '1d;$d' "${out}")"
  expect_eq "output_lines keeps content, including a literal EOF line" "${content}" "${body}"

  : >"${out}"
  printf '%s\n' "${content}" | (source "${SCRIPTS}/common.sh" && GITEA_OUTPUT="${out}" output_lines body)
  second="$(head -n 1 "${out}")"
  if [ "${first}" != "${second}" ]; then pass; else fail "output_lines reused the delimiter ${first#body<<}"; fi

  : >"${out}"
  printf '' | (source "${SCRIPTS}/common.sh" && GITEA_OUTPUT="${out}" output_lines empty)
  expect_eq "output_lines with no content writes an empty value" 2 "$(wc -l <"${out}" | tr -d ' ')"
}

test_fingerprint() {
  local out
  out="$(source "${SCRIPTS}/common.sh" && printf 'abc' | fingerprint)"
  expect_eq "fingerprint is the first 16 hex digits of sha256" ba7816bf8f01cfea "${out}"
  out="$(source "${SCRIPTS}/common.sh" && printf 'abd' | fingerprint)"
  if [ "${out}" != ba7816bf8f01cfea ]; then pass; else fail "fingerprint differs for different input"; fi
}

test_gitea_api_init() {
  local out rc

  out="$(
    source "${SCRIPTS}/common.sh"
    TOKEN=secret REPOSITORY=owner/repo API_URL=https://api.test/ gitea_api_init
    # shellcheck disable=SC2154
    printf '%s\n' "${api}" "${auth_hdr[*]}" "${http_code}" "${body#"${tmp}"/}"
  )"
  expect_eq "gitea_api_init builds the repo API url and auth header" "$(lines \
    https://api.test/repos/owner/repo '-H Authorization: token secret' '' body)" "${out}"

  rc=0
  out="$(source "${SCRIPTS}/common.sh" && TOKEN='' REPOSITORY=owner/repo API_URL=https://api.test gitea_api_init 2>&1)" || rc=$?
  expect_eq "gitea_api_init without a token: exit status" 1 "${rc}"
  expect_contains "gitea_api_init without a token: message" "Token is required." "${out}"

  rc=0
  out="$(source "${SCRIPTS}/common.sh" && TOKEN=t REPOSITORY=repo API_URL=https://api.test gitea_api_init 2>&1)" || rc=$?
  expect_eq "gitea_api_init with a bare repository: exit status" 1 "${rc}"
  expect_contains "gitea_api_init with a bare repository: message" "Repository must be owner/repo." "${out}"
}

run_publish_plan() {
  local out="$1"
  shift
  : >"${out}"
  env GITEA_OUTPUT="${out}" LOCAL_REF=host/o/p GHCR_REF=ghcr.io/o/p DOCKER_REF=docker.io/o/p "$@" \
    bash "${SCRIPTS}/publish-plan.sh"
}

test_publish_plan() {
  local out="${tmp}/plan-output" rc

  run_publish_plan "${out}" PUBLISH_LOCAL=true PUBLISH_GHCR=false PUBLISH_DOCKERHUB=true DOCKER_NETWORK=host
  expect_contains "publish plan with a destination off: any" "any=true" "$(<"${out}")"
  expect_contains "publish plan with a destination off: network" "driver_opts=network=host" "$(<"${out}")"
  expect_eq "publish plan with a destination off: refs" "$(lines host/o/p docker.io/o/p)" \
    "$(sed -n '/^refs<</,$p' "${out}" | sed '1d;$d')"

  run_publish_plan "${out}" PUBLISH_LOCAL=false PUBLISH_GHCR=false PUBLISH_DOCKERHUB=false
  expect_contains "publish plan with every destination off: any" "any=false" "$(<"${out}")"
  expect_lacks "publish plan with every destination off: no refs" "ghcr.io" "$(<"${out}")"

  rc=0
  run_publish_plan "${out}" PUBLISH_LOCAL=false PUBLISH_GHCR=true PUBLISH_DOCKERHUB=false GHCR_REF= >/dev/null 2>&1 || rc=$?
  expect_failure "publish plan with an empty enabled ref" "${rc}"
}

test_publish_meta_flags() {
  local repo="${tmp}/meta-repo" out="${tmp}/meta-flags-output" base rc

  make_meta_repo "${repo}"
  base="$(git -C "${repo}" rev-parse HEAD~1)"

  run_publish_meta "${repo}" "${base}" "${out}" FORCE_RELEASE=true GH_USER= DH_USER= PUBLISH_GHCR=false PUBLISH_DOCKERHUB=false
  expect_contains "publish meta with registries off needs no user: ghcr ref empty" "ghcr=" "$(<"${out}")"
  expect_lacks "publish meta with registries off needs no user: no ghcr host" "ghcr.io" "$(<"${out}")"
  expect_lacks "publish meta with registries off needs no user: no docker host" "docker.io" "$(<"${out}")"

  rc=0
  run_publish_meta "${repo}" "${base}" "${out}" FORCE_RELEASE=true GH_USER= || rc=$?
  expect_failure "publish meta with GHCR on and no user" "${rc}"
  expect_contains "publish meta with GHCR on and no user refuses" "GH_USER is required" "$(<"${out}.log")"
}

licence_list() {
  sed -n "/^$2 = \\[/,/\\]/p" "$1" | grep -o '"[^"]*"' | LC_ALL=C sort
}

test_licence_sync() {
  local deny about
  deny="$(licence_list "${ROOT}/.config/deny.toml" allow)"
  about="$(licence_list "${ROOT}/.config/about.toml" accepted)"
  if [ -n "${deny}" ]; then pass; else fail ".config/deny.toml licences.allow was not found"; fi
  expect_eq ".config/about.toml accepted matches .config/deny.toml allow" "${deny}" "${about}"
}

test_shellcheck() {
  local files out rc=0
  if ! command -v shellcheck >/dev/null; then
    fail "shellcheck is required but not installed; install it on the runner image"
    return 0
  fi
  mapfile -t files < <(find "${GITEA_DIR}" "${HERE}" -name '*.sh' | LC_ALL=C sort)
  out="$(shellcheck -x --rcfile="${HERE}/shellcheckrc" "${files[@]}" 2>&1)" || rc=$?
  if [ "${rc}" -eq 0 ]; then
    pass
  else
    fail "shellcheck reported findings"
    printf '%s\n' "${out}" | sed 's/^/  | /' >&2
  fi
}

run_job() {
  tmp="${root_tmp}/$1"
  mkdir -p "${tmp}"
  trap 'printf "%s %s %s\n" "${passes}" "${failures}" "$?" >"${tmp}.counts"' EXIT
  "$1"
}

tests=(
  test_changelog_awk
  test_changelog_script
  test_targets_parse
  test_ref_is_base
  test_bg_wait
  test_docker_bake_plan
  test_prune_target
  test_package_xz_filters
  test_release_working_uploads
  test_publish_latest
  test_release_integrity
  test_publish_meta_flags
  test_publish_meta_tag_check
  test_changes
  test_ci_paths
  test_cache_key
  test_output_lines
  test_fingerprint
  test_gitea_api_init
  test_publish_plan
  test_licence_sync
  test_shellcheck
)

jobs_slowest_first=(
  test_publish_meta_tag_check
  test_release_integrity
  test_shellcheck
  test_changes
  test_publish_latest
  test_release_working_uploads
  test_changelog_script
)

start_order=("${jobs_slowest_first[@]}")
for job in "${tests[@]}"; do
  case " ${jobs_slowest_first[*]} " in
  *" ${job} "*) ;;
  *) start_order+=("${job}") ;;
  esac
done

running=0
for job in "${start_order[@]}"; do
  if [ "${running}" -ge "${TEST_JOBS}" ]; then
    wait -n || true
    running=$((running - 1))
  fi
  run_job "${job}" >"${root_tmp}/${job}.log" 2>&1 &
  running=$((running + 1))
done
wait

for job in "${tests[@]}"; do
  cat "${root_tmp}/${job}.log" >&2
  job_passes=0 job_failures=0 job_rc=1
  if [ -s "${root_tmp}/${job}.counts" ]; then
    read -r job_passes job_failures job_rc <"${root_tmp}/${job}.counts"
  fi
  passes=$((passes + job_passes))
  failures=$((failures + job_failures))
  if [ "${job_rc}" -ne 0 ]; then
    fail "${job}: job aborted with exit status ${job_rc}"
  fi
done

echo "${passes} passed, ${failures} failed."
[ "${failures}" -eq 0 ]
