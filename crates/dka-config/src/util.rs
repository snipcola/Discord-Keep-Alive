const SUGGESTION_THRESHOLD: f64 = 0.6;

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

pub fn hint<S: AsRef<str>>(input: &str, candidates: impl IntoIterator<Item = S>) -> String {
  let candidates: Vec<S> = candidates.into_iter().collect();
  let input = input.to_ascii_lowercase();
  let closest = candidates
    .iter()
    .map(|c| {
      (
        strsim::normalized_damerau_levenshtein(&input, c.as_ref()),
        c.as_ref(),
      )
    })
    .filter(|&(score, _)| score > SUGGESTION_THRESHOLD)
    .max_by(|a, b| a.0.total_cmp(&b.0));
  match closest {
    Some((_, name)) => format!("did you mean '{name}'?"),
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
  fn hint_lists_candidates_when_none_is_close() {
    assert_eq!(
      hint("busy", ["online", "idle", "dnd"]),
      "expected online, idle, dnd"
    );
  }
}
