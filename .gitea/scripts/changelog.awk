# Group conventional commits into release note sections.
# Input: git log --format=$'\x01%h\x02%s\x02<Release-Note trailer>' --name-only

function emit(  type, scope, desc, bucket, head, is_breaking, breaking_footer) {
  if (subject == "" || tolower(note) == "none" || listed(BUMP_SHAS)) return

  is_breaking = 0
  scope = ""
  type = ""
  desc = subject

  if (listed(BREAKING_SHAS)) breaking_footer = 1

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
    type = tolower(type)
  }

  sub(/ \(v?[0-9]+\.[0-9]+(\.[0-9]+)?\)$/, "", desc)
  if (note != "") desc = note

  if (is_breaking || breaking_footer) bucket = "breaking"
  else if (scope == "deps") bucket = "deps"
  else if (type == "") bucket = "other"
  else if (type == "feat") bucket = "feat"
  else if (type == "fix" || type == "perf") bucket = "fix"
  else if (type == "refactor") bucket = "refactor"
  else bucket = "chore"

  # Breaking changes stay visible even when they ship no code.
  if (!code && bucket != "breaking") bucket = "internal-" bucket

  lines[bucket, ++count[bucket]] = sprintf("- %s ([`%s`](%s/commit/%s))", \
    escape((bucket == "deps" || scope == "" ? "" : scope ": ") desc), hash, repo, hash)
}

function listed(shas) {
  return index(shas, " " hash " ")
}

function escape(s,   n, parts, seps, i, out) {
  n = split(s, parts, "`", seps)
  for (i = 1; i <= n; i++) {
    if (i % 2 || (i == n && n % 2 == 0)) {
      gsub(/&/, "\\&amp;", parts[i])
      gsub(/</, "\\&lt;", parts[i])
      gsub(/>/, "\\&gt;", parts[i])
    }
    out = out parts[i] seps[i]
  }
  return out
}

function by_lower(i1, v1, i2, v2) {
  v1 = tolower(v1)
  v2 = tolower(v2)
  return v1 < v2 ? -1 : v1 > v2
}

function flush(bucket, title,   i, n, unsorted, sorted) {
  if (!(bucket in count)) return

  for (i = 1; i <= count[bucket]; i++) unsorted[i] = lines[bucket, i]
  n = asort(unsorted, sorted, "by_lower")

  printf "%s %s\n\n", heading, title
  for (i = 1; i <= n; i++) print sorted[i]
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
  split(substr($0, 2), fields, FS_MARK)
  hash = fields[1]
  subject = fields[2]
  note = fields[3]
  code = 0
  next
}
/^(crates\/|Cargo\.(toml|lock)|rust-toolchain\.toml)/ { code = 1 }

END {
  emit()

  if (total("") == 0) {
    printf "No notable changes.\n\n"
    exit
  }

  heading = "###"
  sections("")

  if (total("internal-") > 0) {
    printf "<details><summary>Internal (%d)</summary>\n\n", total("internal-")
    heading = "####"
    sections("internal-")
    printf "</details>\n\n"
  }
}
