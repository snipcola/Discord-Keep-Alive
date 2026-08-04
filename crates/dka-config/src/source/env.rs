use std::collections::HashMap;
use std::sync::LazyLock;

use crate::model::partial::PartialConfig;
use crate::schema::fields::{
  AccountScalarField, ActivityField, ClientPropField, CustomStatusField, DefaultsProfile,
};
use crate::schema::id::{ACCOUNT_FLAT, ACTIVITY_SINGULAR, parse_user_id};
use crate::schema::path::{
  ConfigPath, account_scalar_path, activity_field_path, apply_path, custom_status_path,
};
use crate::util::trim_nonempty;

const ENV_LOG_LEVEL: &str = "LOG_LEVEL";
const ENV_HEALTH_SOCKET: &str = "HEALTH_SOCKET";

#[derive(Clone, Copy)]
enum AccountEnvField {
  Scalar(AccountScalarField),
  Custom(CustomStatusField),
}

// Longest suffix first so CUSTOM_STATUS_TEXT wins over shorter tails.
static ACCOUNT_SUFFIXES_LONGEST_FIRST: LazyLock<Vec<(&'static str, AccountEnvField)>> =
  LazyLock::new(|| {
    let mut v: Vec<_> = AccountScalarField::ALL
      .iter()
      .filter_map(|&f| f.env_suffix().map(|s| (s, AccountEnvField::Scalar(f))))
      .chain(
        CustomStatusField::ALL
          .iter()
          .map(|&f| (f.env_suffix(), AccountEnvField::Custom(f))),
      )
      .collect();
    v.sort_by_key(|(s, _)| std::cmp::Reverse(s.len()));
    v
  });

// Longest ACTIVITY_* suffix wins (same rule as account suffixes).
static ACTIVITY_SUFFIXES_LONGEST_FIRST: LazyLock<Vec<(ActivityField, &'static str)>> =
  LazyLock::new(|| {
    let mut v: Vec<_> = ActivityField::ALL
      .iter()
      .filter_map(|&f| f.env_suffix().map(|s| (f, s)))
      .collect();
    v.sort_by_key(|(_, s)| std::cmp::Reverse(s.len()));
    v
  });

fn account_suffixes_longest_first() -> &'static [(&'static str, AccountEnvField)] {
  &ACCOUNT_SUFFIXES_LONGEST_FIRST
}

fn activity_suffixes_longest_first() -> &'static [(ActivityField, &'static str)] {
  &ACTIVITY_SUFFIXES_LONGEST_FIRST
}

pub fn from_env() -> PartialConfig {
  let map: HashMap<String, String> = std::env::vars().collect();
  from_env_map(&map)
}

pub fn from_env_map(map: &HashMap<String, String>) -> PartialConfig {
  let mut pairs: Vec<(&str, &str)> = map.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
  pairs.sort_by(|a, b| a.0.cmp(b.0));
  from_env_pairs(pairs)
}

#[cfg(test)]
pub fn from_env_lookup(lookup: impl Fn(&str) -> Option<String>) -> PartialConfig {
  let mut map = HashMap::new();
  for (k, _) in std::env::vars() {
    if let Some(v) = lookup(&k) {
      map.insert(k, v);
    }
  }
  for key in probe_flat_keys() {
    if map.contains_key(&key) {
      continue;
    }
    if let Some(v) = lookup(&key) {
      map.insert(key, v);
    }
  }
  from_env_map(&map)
}

#[cfg(test)]
fn probe_flat_keys() -> Vec<String> {
  let mut keys = vec![
    ENV_LOG_LEVEL.into(),
    ENV_HEALTH_SOCKET.into(),
    "ACCOUNT".into(),
    "ACTIVITY".into(),
  ];
  for &field in AccountScalarField::ALL {
    if let Some(suffix) = field.env_suffix() {
      keys.push(suffix.into());
    }
  }
  for &field in CustomStatusField::ALL {
    keys.push(field.env_suffix().into());
  }
  for &field in ActivityField::ALL {
    if let Some(suffix) = field.env_suffix() {
      keys.push(format!("ACTIVITY_{suffix}"));
    }
  }
  for &profile in DefaultsProfile::ALL {
    for &field in ClientPropField::ALL {
      keys.push(format!("{}{}", profile.env_prefix(), field.env_suffix()));
    }
  }
  keys
}

fn from_env_pairs<'a, I>(pairs: I) -> PartialConfig
where
  I: IntoIterator<Item = (&'a str, &'a str)>,
{
  let mut partial = PartialConfig::default();
  for (key, value) in pairs {
    if !is_ascii_env_key(key) {
      continue;
    }
    let Some(value) = trim_nonempty(value).map(str::to_string) else {
      continue;
    };
    let Some(path) = parse_env_key(key) else {
      continue;
    };
    apply_path(&mut partial, &path, value);
  }
  partial
}

// Env keys must be ASCII letters, digits, `_`, or `-` (no case folding).
fn is_ascii_env_key(key: &str) -> bool {
  !key.is_empty()
    && key
      .bytes()
      .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

fn parse_env_key(key: &str) -> Option<ConfigPath> {
  match key {
    ENV_LOG_LEVEL => Some(ConfigPath::LogLevel),
    ENV_HEALTH_SOCKET => Some(ConfigPath::HealthSocket),
    "ACCOUNT" => Some(account_scalar_path(ACCOUNT_FLAT, AccountScalarField::Name)),
    k if k.starts_with("DEFAULTS_") => parse_defaults_key(k),
    k if k.starts_with("ACTIVITY") => {
      let rest = &k["ACTIVITY".len()..];
      let (act_id, field) = parse_activity_token(rest)?;
      Some(activity_field_path(ACCOUNT_FLAT, act_id, field))
    }
    k if k.starts_with("ACCOUNT_") => parse_account_rest(&k["ACCOUNT_".len()..]),
    _ => parse_flat_account_env_key(key),
  }
}

fn parse_flat_account_env_key(key: &str) -> Option<ConfigPath> {
  for &(suffix, field) in account_suffixes_longest_first() {
    if key == suffix {
      return Some(match field {
        AccountEnvField::Scalar(f) => account_scalar_path(ACCOUNT_FLAT, f),
        AccountEnvField::Custom(f) => custom_status_path(ACCOUNT_FLAT, f),
      });
    }
  }
  None
}

fn parse_defaults_key(key: &str) -> Option<ConfigPath> {
  for &profile in DefaultsProfile::ALL {
    let Some(field_part) = key.strip_prefix(profile.env_prefix()) else {
      continue;
    };
    for &field in ClientPropField::ALL {
      if field.env_suffix() == field_part {
        return Some(ConfigPath::Defaults(profile, field));
      }
    }
  }
  None
}

fn parse_account_rest(rest: &str) -> Option<ConfigPath> {
  if let Some(idx) = rest.find("_ACTIVITY") {
    let left = &rest[..idx];
    let right = &rest[idx + "_ACTIVITY".len()..];
    let account_id = parse_user_id(left).ok()?;
    let (act_id, field) = parse_activity_token(right)?;
    return Some(activity_field_path(account_id, act_id, field));
  }

  for &(suffix, field) in account_suffixes_longest_first() {
    if rest == suffix {
      // Bare ACCOUNT_TOKEN (no id) is not a keyed account; handled as flat TOKEN.
      return None;
    }
    let patterned = format!("_{suffix}");
    if let Some(prefix) = rest.strip_suffix(patterned.as_str())
      && !prefix.is_empty()
    {
      let id = parse_user_id(prefix).ok()?;
      return Some(match field {
        AccountEnvField::Scalar(f) => account_scalar_path(id, f),
        AccountEnvField::Custom(f) => custom_status_path(id, f),
      });
    }
  }

  if let Ok(id) = parse_user_id(rest) {
    return Some(account_scalar_path(id, AccountScalarField::Name));
  }

  None
}

fn parse_activity_token(rest: &str) -> Option<(String, ActivityField)> {
  if rest.is_empty() {
    return Some((ACTIVITY_SINGULAR.into(), ActivityField::Name));
  }
  if !rest.starts_with('_') {
    return None;
  }
  let rest = &rest[1..];

  // ACTIVITY_TYPE → singular activity, field TYPE.
  if let Some(field) = match_activity_suffix(rest) {
    return Some((ACTIVITY_SINGULAR.into(), field));
  }

  // ACTIVITY_foo_TYPE → activity id foo, field TYPE.
  for &(field, suffix) in activity_suffixes_longest_first() {
    let patterned = format!("_{suffix}");
    if let Some(prefix) = rest.strip_suffix(patterned.as_str())
      && !prefix.is_empty()
      && let Ok(id) = parse_user_id(prefix)
    {
      return Some((id, field));
    }
  }

  // ACTIVITY_foo → activity id foo, name field.
  if let Ok(id) = parse_user_id(rest) {
    return Some((id, ActivityField::Name));
  }

  None
}

fn match_activity_suffix(rest: &str) -> Option<ActivityField> {
  for &(field, suffix) in activity_suffixes_longest_first() {
    if rest == suffix {
      return Some(field);
    }
  }
  None
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::schema::id::{ACCOUNT_FLAT, ACTIVITY_SINGULAR};
  use crate::test_support::*;

  #[test]
  fn flat_token_and_indexed_accounts() {
    let p = from_env_map(&env_map(&[
      ("TOKEN", "flat"),
      ("ACCOUNT_0_TOKEN", "tok-0"),
      ("ACCOUNT_0", "zero"),
      ("ACCOUNT_test_TOKEN", "tok-test"),
      ("ACTIVITY_foo_TYPE", "playing"),
      ("ACTIVITY_foo", "Game"),
    ]));

    assert_eq!(token_of(&p, ACCOUNT_FLAT), Some("flat"));
    assert_eq!(token_of(&p, "0"), Some("tok-0"));
    assert_eq!(name_of(&p, "0"), Some("zero"));
    assert_eq!(token_of(&p, "test"), Some("tok-test"));
    let flat = p.accounts.get(ACCOUNT_FLAT).unwrap();
    assert_eq!(
      flat.activities.get("foo").and_then(|a| a.name.as_deref()),
      Some("Game")
    );
    assert_eq!(
      flat
        .activities
        .get("foo")
        .and_then(|a| a.activity_type.as_deref()),
      Some("playing")
    );
  }

  #[test]
  fn ambiguity_custom_status_and_large_image_text() {
    let p = from_env_map(&env_map(&[
      ("TOKEN", "t"),
      ("ACCOUNT_CUSTOM_STATUS_TEXT", "ignored"),
      ("CUSTOM_STATUS_TEXT", "works"),
      ("ACTIVITY_LARGE_IMAGE_TEXT", "hover"),
    ]));

    let flat = p.accounts.get(ACCOUNT_FLAT).unwrap();
    assert_eq!(
      flat.custom_status.as_ref().and_then(|c| c.text.as_deref()),
      Some("works")
    );
    assert!(!p.accounts.contains_key("CUSTOM"));
    assert_eq!(
      flat
        .activities
        .get(ACTIVITY_SINGULAR)
        .and_then(|a| a.large_image_text.as_deref()),
      Some("hover")
    );
    assert!(!flat.activities.contains_key("LARGE"));
  }

  #[test]
  fn no_pack_account_one_stays_one() {
    let p = from_env_map(&env_map(&[("ACCOUNT_1_TOKEN", "tok-1")]));
    assert_eq!(p.accounts.len(), 1);
    assert!(p.accounts.contains_key("1"));
    assert!(!p.accounts.contains_key("0"));
    assert!(!p.accounts.contains_key(ACCOUNT_FLAT));
    assert_eq!(token_of(&p, "1"), Some("tok-1"));
  }

  #[test]
  fn empty_values_unset_and_case_sensitive() {
    let p = from_env_map(&env_map(&[
      ("TOKEN", "  "),
      ("token", "lower"),
      ("STATUS", "  dnd  "),
      ("ACCOUNT_0_TOKEN", ""),
      ("ACCOUNT_0_STATUS", "idle"),
    ]));
    assert_eq!(status_of(&p, ACCOUNT_FLAT), Some("dnd"));
    assert!(
      p.accounts
        .get(ACCOUNT_FLAT)
        .map(|a| a.token.is_none())
        .unwrap_or(true)
    );
    assert_eq!(status_of(&p, "0"), Some("idle"));
    assert!(
      p.accounts
        .get("0")
        .map(|a| a.token.is_none())
        .unwrap_or(true)
    );
  }

  #[test]
  fn account_activity_nested_keys() {
    let p = from_env_map(&env_map(&[
      ("ACCOUNT_main_TOKEN", "t"),
      ("ACCOUNT_main_ACTIVITY", "Game"),
      ("ACCOUNT_main_ACTIVITY_TYPE", "playing"),
      ("ACCOUNT_main_ACTIVITY_1", "Second"),
      ("ACCOUNT_main_ACTIVITY_1_DETAILS", "d"),
    ]));
    let acc = p.accounts.get("main").unwrap();
    assert_eq!(
      acc
        .activities
        .get(ACTIVITY_SINGULAR)
        .and_then(|a| a.name.as_deref()),
      Some("Game")
    );
    assert_eq!(
      acc
        .activities
        .get(ACTIVITY_SINGULAR)
        .and_then(|a| a.activity_type.as_deref()),
      Some("playing")
    );
    assert_eq!(
      acc.activities.get("1").and_then(|a| a.name.as_deref()),
      Some("Second")
    );
    assert_eq!(
      acc.activities.get("1").and_then(|a| a.details.as_deref()),
      Some("d")
    );
  }

  #[test]
  fn defaults_from_env() {
    let p = from_env_map(&env_map(&[
      ("TOKEN", "t"),
      ("DEFAULTS_WEB_OS", "Linux"),
      ("DEFAULTS_BOT_BROWSER", "bot-browser"),
    ]));
    assert_eq!(p.defaults.web.os.as_deref(), Some("Linux"));
    assert_eq!(p.defaults.bot.browser.as_deref(), Some("bot-browser"));
  }

  #[test]
  fn from_env_lookup_well_known() {
    let p = from_env_lookup(|key| match key {
      "LOG_LEVEL" => Some("debug".into()),
      "HEALTH_SOCKET" => Some("/tmp/h".into()),
      "TOKEN" => Some("tok".into()),
      _ => None,
    });
    assert_eq!(p.log_level.as_deref(), Some("debug"));
    assert_eq!(p.health_socket.as_deref(), Some("/tmp/h"));
    assert_eq!(token_of(&p, ACCOUNT_FLAT), Some("tok"));
  }

  #[test]
  fn discover_account_without_token() {
    // Non-token keys still create the account slot; resolve enforces token later.
    let p = from_env_map(&env_map(&[("ACCOUNT_x_STATUS", "idle")]));
    assert!(p.accounts.contains_key("x"));
    assert_eq!(status_of(&p, "x"), Some("idle"));
  }

  #[test]
  fn catalog_env_suffixes_are_reachable() {
    let account_suffixes = account_suffixes_longest_first();
    let act_suffixes = activity_suffixes_longest_first();
    for_each_catalog_field(
      |field| {
        let Some(suffix) = field.env_suffix() else {
          return;
        };
        assert!(
          account_suffixes
            .iter()
            .any(|(s, f)| *s == suffix && matches!(f, AccountEnvField::Scalar(x) if *x == field)),
          "AccountScalarField env_suffix {suffix} missing from account_suffixes"
        );
        let path = parse_env_key(suffix).expect(suffix);
        assert_eq!(
          path,
          account_scalar_path(ACCOUNT_FLAT, field),
          "flat env key {suffix}"
        );
        let key = format!("ACCOUNT_0_{suffix}");
        let path = parse_env_key(&key).expect(&key);
        assert_eq!(path, account_scalar_path("0", field), "indexed {key}");
      },
      |field| {
        let suffix = field.env_suffix();
        assert!(
          account_suffixes
            .iter()
            .any(|(s, f)| *s == suffix && matches!(f, AccountEnvField::Custom(x) if *x == field)),
          "CustomStatusField env_suffix {suffix} missing"
        );
        let path = parse_env_key(suffix).expect(suffix);
        assert_eq!(path, custom_status_path(ACCOUNT_FLAT, field));
        let key = format!("ACCOUNT_0_{suffix}");
        let path = parse_env_key(&key).expect(&key);
        assert_eq!(path, custom_status_path("0", field));
      },
      |field| {
        let Some(suffix) = field.env_suffix() else {
          return;
        };
        assert!(
          act_suffixes
            .iter()
            .any(|(f, s)| *f == field && *s == suffix),
          "ActivityField env_suffix {suffix} missing from activity_suffixes"
        );
        let key = format!("ACTIVITY_{suffix}");
        let path = parse_env_key(&key).expect(&key);
        assert_eq!(
          path,
          activity_field_path(ACCOUNT_FLAT, ACTIVITY_SINGULAR, field)
        );
      },
    );
  }
}
