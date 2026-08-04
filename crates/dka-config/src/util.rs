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
