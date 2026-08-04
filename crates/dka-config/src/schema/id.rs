use crate::error::ConfigError;

pub type AccountId = String;
pub type ActivityId = String;

pub const ACCOUNT_FLAT: &str = "__flat__";
pub const ACTIVITY_SINGULAR: &str = "__singular__";

// 1-64 chars: start letter/digit, then letter/digit/hyphen; no trailing hyphen, `_`, or reserved id.
pub fn parse_user_id(raw: &str) -> Result<String, ConfigError> {
  if raw.is_empty() {
    return Err(ConfigError::InvalidId("id must not be empty".into()));
  }
  if raw == ACCOUNT_FLAT || raw == ACTIVITY_SINGULAR {
    return Err(ConfigError::InvalidId(format!("reserved id '{raw}'")));
  }
  if raw.len() > 64 {
    return Err(ConfigError::InvalidId(format!(
      "id '{raw}' exceeds 64 characters"
    )));
  }
  let bytes = raw.as_bytes();
  let first = bytes[0];
  if !first.is_ascii_alphanumeric() {
    return Err(ConfigError::InvalidId(format!(
      "id '{raw}' must start with an alphanumeric character"
    )));
  }
  for &b in &bytes[1..] {
    if !(b.is_ascii_alphanumeric() || b == b'-') {
      return Err(ConfigError::InvalidId(format!(
        "id '{raw}' may only contain alphanumeric characters and '-'"
      )));
    }
  }
  if bytes.last() == Some(&b'-') {
    return Err(ConfigError::InvalidId(format!(
      "id '{raw}' must not end with '-'"
    )));
  }
  Ok(raw.to_string())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn parse_user_id_accepts() {
    for id in ["a", "0", "account-1", "A1b2", "x", "abc-def-ghi", "1"] {
      assert_eq!(parse_user_id(id).unwrap(), id, "{id}");
    }
    let max = format!("a{}", "b".repeat(63));
    assert_eq!(max.len(), 64);
    assert_eq!(parse_user_id(&max).unwrap(), max);
  }

  #[test]
  fn parse_user_id_rejects() {
    for id in [
      "",
      "_bad",
      "has_under",
      "trail-",
      "-lead",
      "a.b",
      "a b",
      ACCOUNT_FLAT,
      ACTIVITY_SINGULAR,
    ] {
      assert!(parse_user_id(id).is_err(), "{id}");
    }
    let too_long = format!("a{}", "b".repeat(64));
    assert_eq!(too_long.len(), 65);
    assert!(parse_user_id(&too_long).is_err());
  }
}
