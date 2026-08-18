# Group conventional commits into release note sections.
# Input: git log --format=$'\x01%h\x02%s' --name-only

function emit(  type, scope, desc, bucket, head, is_breaking, breaking_footer) {
  if (subject == "") return

  is_breaking = 0
  scope = ""
  type = ""
  desc = subject

  if (index(BREAKING_SHAS, " " hash " ")) breaking_footer = 1

  if (match(subject, /^[a-zA-Z]+(\([^)]+\))?!?: /)) {
    head = substr(subject, 1, RLENGTH - 2)
    desc = substr(subject, RLENGTH + 1)
    if (head ~ /!$/) {
      is_breaking = 1
      sub(/!$/, "", head)
    }
    if (match(head, /\(.*\)$/)) {
      scope = substr(head, RSTART + 1, RLENGTH - 2)
      type = substr(head, 1, RSTART - 1)
    } else {
      type = head
    }
  }

  if (type == "chore" && scope == "release") return
  if (desc ~ /^(bump version to|release) v?[0-9]+\.[0-9]+/) return

  sub(/ \(v?[0-9]+\.[0-9]+(\.[0-9]+)?\)$/, "", desc)

  if (is_breaking || breaking_footer) bucket = "breaking"
  else if (scope == "deps") bucket = "deps"
  else if (type == "") bucket = "other"
  else if (type == "feat") bucket = "feat"
  else if (type == "fix" || type == "perf") bucket = "fix"
  else if (type == "refactor") bucket = "refactor"
  else bucket = "chore"

  # Breaking changes stay visible even when they ship no code.
  if (!code && bucket != "breaking") bucket = "internal-" bucket

  lines[bucket, ++count[bucket]] = sprintf("- %s%s ([`%s`](%s/commit/%s))", \
    (bucket == "deps" || scope == "" ? "" : scope ": "), desc, hash, repo, hash)
}

function flush(bucket, title,   i, tmp, swapped, sorted) {
  if (!(bucket in count)) return

  for (i = 1; i <= count[bucket]; i++) sorted[i] = lines[bucket, i]
  # Sorted in-place; asort() is a gawk extension.
  do {
    swapped = 0
    for (i = 1; i < count[bucket]; i++) {
      if (sorted[i] > sorted[i + 1]) {
        tmp = sorted[i]; sorted[i] = sorted[i + 1]; sorted[i + 1] = tmp
        swapped = 1
      }
    }
  } while (swapped)

  printf "%s %s\n\n", heading, title
  for (i = 1; i <= count[bucket]; i++) print sorted[i]
  printf "\n"
}

function sections(prefix) {
  flush(prefix "breaking", "Breaking changes")
  flush(prefix "feat", "Features")
  flush(prefix "fix", "Fixes")
  flush(prefix "refactor", "Refactors")
  flush(prefix "chore", "Chores")
  flush(prefix "deps", "Dependencies")
  flush(prefix "other", "Other")
}

function total(prefix,   b, n) {
  for (b in count) if (index(b, prefix) == 1) n += count[b]
  return n
}

/^$/ { next }
substr($0, 1, 1) == RS_MARK {
  emit()
  split(substr($0, 2), parts, FS_MARK)
  hash = parts[1]
  subject = parts[2]
  code = 0
  next
}
/^(crates\/|Cargo\.(toml|lock)|rust-toolchain\.toml)/ { code = 1 }

END {
  emit()

  heading = "###"
  sections("")

  if (total("internal-") > 0) {
    printf "<details><summary>Internal (%d)</summary>\n\n", total("internal-")
    heading = "####"
    sections("internal-")
    printf "</details>\n\n"
  }
}
