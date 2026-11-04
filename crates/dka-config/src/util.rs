const SUGGESTION_THRESHOLD: f64 = 0.6;
const NEAR_MISS_MAX_EDITS: usize = 2;

pub fn trim_nonempty(s: &str) -> Option<&str> {
  let t = s.trim();
  (!t.is_empty()).then_some(t)
}

pub fn trim_opt(s: Option<&str>) -> Option<&str> {
  s.and_then(trim_nonempty)
}

pub fn trim_owned(s: Option<String>) -> Option<String> {
  s.and_then(|v| trim_nonempty(&v).map(str::to_string))
}

pub fn trim_to_string(s: Option<&str>) -> Option<String> {
  trim_opt(s).map(str::to_string)
}

pub fn nearest<'a, S: AsRef<str>>(input: &str, candidates: &'a [S]) -> Option<&'a str> {
  let input = input.to_ascii_lowercase();
  candidates
    .iter()
    .map(|c| {
      (
        strsim::normalized_damerau_levenshtein(&input, c.as_ref()),
        c.as_ref(),
      )
    })
    .filter(|&(score, _)| score > SUGGESTION_THRESHOLD)
    .max_by(|a, b| a.0.total_cmp(&b.0))
    .map(|(_, name)| name)
}

pub fn near_miss(input: &str, candidate: &str) -> Option<usize> {
  if input == candidate {
    return None;
  }
  let distance =
    strsim::damerau_levenshtein(&input.to_ascii_lowercase(), &candidate.to_ascii_lowercase());
  (distance <= NEAR_MISS_MAX_EDITS && distance < input.chars().count()).then_some(distance)
}

pub fn hint<S: AsRef<str>>(input: &str, candidates: impl IntoIterator<Item = S>) -> String {
  let candidates: Vec<S> = candidates.into_iter().collect();
  match nearest(input, &candidates) {
    Some(name) => format!("did you mean '{name}'?"),
    None => {
      let names: Vec<&str> = candidates.iter().map(AsRef::as_ref).collect();
      format!("expected {}", names.join(", "))
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn hint_suggests_a_close_match() {
    assert_eq!(hint("onlin", ["online", "idle"]), "did you mean 'online'?");
    assert_eq!(hint("IDEL", ["online", "idle"]), "did you mean 'idle'?");
  }

  #[test]
  fn near_miss_counts_edits_up_to_two_ignoring_case() {
    for (input, candidate, edits) in [
      ("STAUTS", "STATUS", 1),
      ("stauts", "STATUS", 1),
      ("status", "STATUS", 0),
      ("TOKN", "TOKEN", 1),
      ("DETIALS", "DETAILS", 1),
      ("DEVCE", "DEVICE", 1),
      ("BUTON_2", "BUTTON_2", 1),
      ("TXT", "TEXT", 1),
    ] {
      assert_eq!(near_miss(input, candidate), Some(edits), "{input}");
    }
  }

  #[test]
  fn near_miss_rejects_equal_distant_and_tiny_inputs() {
    for (input, candidate) in [
      ("STATUS", "STATUS"),
      ("main", "KIND"),
      ("0", "URL"),
      ("l", "URL"),
      ("ABCDEF", "STATUS"),
    ] {
      assert_eq!(near_miss(input, candidate), None, "{input} vs {candidate}");
    }
  }

  #[test]
  fn hint_lists_candidates_when_none_is_close() {
    assert_eq!(
      hint("busy", ["online", "idle", "dnd"]),
      "expected online, idle, dnd"
    );
  }
}
