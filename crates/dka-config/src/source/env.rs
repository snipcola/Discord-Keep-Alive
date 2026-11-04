use std::collections::HashMap;
use std::ffi::OsString;
use std::sync::LazyLock;

use crate::model::partial::PartialConfig;
use crate::schema::fields::{
  AccountScalarField, ActivityField, ClientPropEmptyPolicy, ClientPropField, CustomStatusField,
  DefaultsProfile, GlobalDefaultField, RootField,
};
use crate::schema::id::{ACCOUNT_FLAT, ACTIVITY_SINGULAR, parse_user_id};
use crate::schema::path::{
  AccountPath, ConfigPath, account_scalar_path, activity_field_path, apply_path, custom_status_path,
};
use crate::util::{near_miss, trim_nonempty};
use crate::warning::ConfigWarning;

const ENV_DEFAULTS_PREFIX: &str = "DEFAULTS_";
const ENV_ACCOUNT_PREFIX: &str = "ACCOUNT_";
const ENV_ACTIVITY_WORD: &str = "ACTIVITY";
const ENV_NESTED_ACTIVITY: &str = "_ACTIVITY";

pub struct EnvLayer {
  pub partial: PartialConfig,
  pub warnings: Vec<ConfigWarning>,
}

#[derive(Clone, Copy)]
enum AccountEnvField {
  Scalar(AccountScalarField),
  Custom(CustomStatusField),
}

// Longest suffix first, so CUSTOM_STATUS_EMOJI isn't read as a shorter match.
static ACCOUNT_SUFFIXES_LONGEST_FIRST: LazyLock<Vec<(&'static str, AccountEnvField)>> =
  LazyLock::new(|| {
    let mut v: Vec<_> = AccountScalarField::ALL
      .iter()
      .filter_map(|&f| f.spec().env_suffix.map(|s| (s, AccountEnvField::Scalar(f))))
      .chain(
        CustomStatusField::ALL
          .iter()
          .filter_map(|&f| f.spec().env_suffix.map(|s| (s, AccountEnvField::Custom(f)))),
      )
      .collect();
    v.sort_by_key(|(s, _)| std::cmp::Reverse(s.len()));
    v
  });

static ACTIVITY_SUFFIXES_LONGEST_FIRST: LazyLock<Vec<(ActivityField, &'static str)>> =
  LazyLock::new(|| {
    let mut v: Vec<_> = ActivityField::ALL
      .iter()
      .filter_map(|&f| f.spec().env_suffix.map(|s| (f, s)))
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

pub fn from_env() -> EnvLayer {
  from_env_vars(std::env::vars_os())
}

pub(crate) fn from_env_vars(vars: impl IntoIterator<Item = (OsString, OsString)>) -> EnvLayer {
  let map: HashMap<String, String> = vars
    .into_iter()
    .filter_map(|(key, value)| Some((key.into_string().ok()?, value.into_string().ok()?)))
    .collect();
  EnvLayer {
    partial: from_env_map(&map),
    warnings: env_warnings(&map),
  }
}

pub fn from_env_map(map: &HashMap<String, String>) -> PartialConfig {
  let mut pairs: Vec<(&str, &str)> = map.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
  pairs.sort_by(|a, b| a.0.cmp(b.0));
  from_env_pairs(pairs)
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
    let Some(path) = parse_env_key(key) else {
      continue;
    };
    match trim_nonempty(value) {
      Some(value) => apply_path(&mut partial, &path, value.to_string()),
      None if empty_disables(&path) => apply_path(&mut partial, &path, String::new()),
      None => {}
    }
  }
  partial
}

fn empty_disables(path: &ConfigPath) -> bool {
  match path {
    ConfigPath::Root(field) => *field == RootField::HealthSocket,
    ConfigPath::GlobalDefault(_) => true,
    ConfigPath::Defaults(_, field) => field.empty_policy() != ClientPropEmptyPolicy::SkipEmpty,
    ConfigPath::Account(..) => false,
  }
}

fn is_ascii_env_key(key: &str) -> bool {
  !key.is_empty()
    && key
      .bytes()
      .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

fn parse_env_key(key: &str) -> Option<ConfigPath> {
  if let Some(&field) = RootField::ALL
    .iter()
    .find(|f| f.spec().env_suffix == Some(key))
  {
    return Some(ConfigPath::Root(field));
  }
  match key {
    "ACCOUNT" => Some(account_scalar_path(ACCOUNT_FLAT, AccountScalarField::Name)),
    k if k.starts_with(ENV_DEFAULTS_PREFIX) => parse_defaults_key(k),
    k if k.starts_with(ENV_ACTIVITY_WORD) => {
      let rest = &k[ENV_ACTIVITY_WORD.len()..];
      let (act_id, field) = parse_activity_token(rest)?;
      Some(activity_field_path(ACCOUNT_FLAT, act_id, field))
    }
    k if k.starts_with(ENV_ACCOUNT_PREFIX) => parse_account_rest(&k[ENV_ACCOUNT_PREFIX.len()..]),
    _ => parse_flat_account_env_key(key),
  }
}

fn in_diagnosed_family(key: &str) -> bool {
  key.starts_with(ENV_ACCOUNT_PREFIX)
    || key.starts_with(ENV_DEFAULTS_PREFIX)
    || key
      .strip_prefix(ENV_ACTIVITY_WORD)
      .is_some_and(|rest| rest.starts_with('_'))
}

fn account_suffix_names() -> impl Iterator<Item = &'static str> {
  account_suffixes_longest_first()
    .iter()
    .map(|&(suffix, _)| suffix)
}

fn activity_suffix_names() -> impl Iterator<Item = &'static str> {
  activity_suffixes_longest_first()
    .iter()
    .map(|&(_, suffix)| suffix)
}

fn closest<T: AsRef<str>>(input: &str, candidates: impl IntoIterator<Item = T>) -> Option<T> {
  candidates
    .into_iter()
    .filter_map(|candidate| near_miss(input, candidate.as_ref()).map(|edits| (edits, candidate)))
    .min_by_key(|&(edits, _)| edits)
    .map(|(_, candidate)| candidate)
}

pub(crate) fn env_warnings(map: &HashMap<String, String>) -> Vec<ConfigWarning> {
  let mut keys: Vec<&str> = map
    .keys()
    .map(String::as_str)
    .filter(|key| is_ascii_env_key(key))
    .collect();
  keys.sort_unstable();
  keys.into_iter().filter_map(diagnose_key).collect()
}

fn diagnose_key(key: &str) -> Option<ConfigWarning> {
  if !in_diagnosed_family(key) {
    return None;
  }
  match parse_env_key(key) {
    Some(ConfigPath::Account(id, AccountPath::Scalar(AccountScalarField::Name)))
      if id != ACCOUNT_FLAT =>
    {
      suspect_id_key(key, "account", id, account_suffix_names())
    }
    Some(ConfigPath::Account(_, AccountPath::Activity(id, ActivityField::Name)))
      if id != ACTIVITY_SINGULAR =>
    {
      suspect_id_key(key, "activity", id, activity_suffix_names())
    }
    Some(_) => None,
    None => Some(ConfigWarning::UnknownEnvKey {
      key: key.to_string(),
      suggestion: suggest_key(key),
    }),
  }
}

fn suspect_id_key(
  key: &str,
  kind: &'static str,
  id: String,
  suffixes: impl Iterator<Item = &'static str>,
) -> Option<ConfigWarning> {
  let suffix = closest(&id, suffixes)?;
  let prefix = &key[..key.len() - id.len()];
  Some(ConfigWarning::SuspectEnvKey {
    key: key.to_string(),
    kind,
    id,
    suggestion: format!("{prefix}{suffix}"),
  })
}

fn suggest_key(key: &str) -> Option<String> {
  if key.starts_with(ENV_DEFAULTS_PREFIX) {
    return closest(key, defaults_keys());
  }
  if let Some(rest) = key.strip_prefix(ENV_ACCOUNT_PREFIX) {
    return match rest.split_once(ENV_NESTED_ACTIVITY) {
      Some((account, activity)) => {
        let activity = activity.strip_prefix('_')?;
        let prefix = format!("{ENV_ACCOUNT_PREFIX}{account}{ENV_NESTED_ACTIVITY}_");
        suggest_after_id(&prefix, activity, activity_suffix_names())
      }
      None => suggest_after_id(ENV_ACCOUNT_PREFIX, rest, account_suffix_names()),
    };
  }
  let rest = key.strip_prefix(ENV_ACTIVITY_WORD)?.strip_prefix('_')?;
  suggest_after_id(
    &format!("{ENV_ACTIVITY_WORD}_"),
    rest,
    activity_suffix_names(),
  )
}

fn suggest_after_id(
  prefix: &str,
  rest: &str,
  suffixes: impl Iterator<Item = &'static str>,
) -> Option<String> {
  let (id, tail) = rest.split_once('_')?;
  parse_user_id(id).ok()?;
  closest(tail, suffixes).map(|suffix| format!("{prefix}{id}_{suffix}"))
}

fn defaults_keys() -> Vec<String> {
  let globals = GlobalDefaultField::ALL
    .iter()
    .filter_map(|field| field.spec().env_suffix)
    .map(|suffix| (ENV_DEFAULTS_PREFIX, suffix));
  let profiles = DefaultsProfile::ALL.iter().flat_map(|&profile| {
    ClientPropField::ALL
      .iter()
      .filter_map(|field| field.spec().env_suffix)
      .map(move |suffix| (profile.env_prefix(), suffix))
  });
  globals
    .chain(profiles)
    .map(|(prefix, suffix)| format!("{prefix}{suffix}"))
    .collect()
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
  let global = key.strip_prefix(ENV_DEFAULTS_PREFIX)?;
  if let Some(&field) = GlobalDefaultField::ALL
    .iter()
    .find(|f| f.spec().env_suffix == Some(global))
  {
    return Some(ConfigPath::GlobalDefault(field));
  }
  for &profile in DefaultsProfile::ALL {
    let Some(field_part) = key.strip_prefix(profile.env_prefix()) else {
      continue;
    };
    for &field in ClientPropField::ALL {
      if field.spec().env_suffix == Some(field_part) {
        return Some(ConfigPath::Defaults(profile, field));
      }
    }
  }
  None
}

fn parse_account_rest(rest: &str) -> Option<ConfigPath> {
  if let Some(idx) = rest.find(ENV_NESTED_ACTIVITY) {
    let left = &rest[..idx];
    let right = &rest[idx + ENV_NESTED_ACTIVITY.len()..];
    let account_id = parse_user_id(left).ok()?;
    let (act_id, field) = parse_activity_token(right)?;
    return Some(activity_field_path(account_id, act_id, field));
  }

  for &(suffix, field) in account_suffixes_longest_first() {
    if rest == suffix {
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

  if let Some(field) = match_activity_suffix(rest) {
    return Some((ACTIVITY_SINGULAR.into(), field));
  }

  for &(field, suffix) in activity_suffixes_longest_first() {
    let patterned = format!("_{suffix}");
    if let Some(prefix) = rest.strip_suffix(patterned.as_str())
      && !prefix.is_empty()
      && let Ok(id) = parse_user_id(prefix)
    {
      return Some((id, field));
    }
  }

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
      ("ACCOUNT_CUSTOM_STATUS", "ignored"),
      ("CUSTOM_STATUS", "works"),
      ("CUSTOM_STATUS_EMOJI", "💤"),
      ("ACTIVITY_LARGE_IMAGE_TEXT", "hover"),
    ]));

    let flat = p.accounts.get(ACCOUNT_FLAT).unwrap();
    assert_eq!(
      flat.custom_status.as_ref().and_then(|c| c.text.as_deref()),
      Some("works")
    );
    assert_eq!(
      flat.custom_status.as_ref().and_then(|c| c.emoji.as_deref()),
      Some("💤")
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
  fn empty_value_disables_only_keys_that_support_disabling() {
    let p = from_env_map(&env_map(&[
      ("DEFAULTS_CAPABILITIES", ""),
      ("DEFAULTS_HEARTBEAT_VERSION", "  "),
      ("DEFAULTS_WEB_BROWSER", ""),
      ("DEFAULTS_WEB_DEVICE", ""),
      ("DEFAULTS_WEB_OS", ""),
      ("HEALTH_SOCKET", ""),
      ("LOG_LEVEL", ""),
      ("TOKEN", ""),
      ("STATUS", ""),
      ("ACTIVITY_DETAILS", ""),
    ]));
    assert_eq!(p.defaults.global.capabilities.as_deref(), Some(""));
    assert_eq!(p.defaults.global.heartbeat_version.as_deref(), Some(""));
    assert_eq!(p.defaults.web.browser.as_deref(), Some(""));
    assert_eq!(p.defaults.web.device.as_deref(), Some(""));
    assert_eq!(p.root.health_socket.as_deref(), Some(""));
    assert_eq!(
      p.defaults.web.os, None,
      "DEFAULTS_WEB_OS cannot be disabled"
    );
    assert_eq!(p.root.log_level, None, "LOG_LEVEL cannot be disabled");
    assert!(
      p.accounts.is_empty(),
      "empty TOKEN, STATUS and ACTIVITY_DETAILS must stay unset: {:?}",
      p.accounts
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

  #[cfg(windows)]
  fn non_unicode() -> OsString {
    std::os::windows::ffi::OsStringExt::from_wide(&[0x0066, 0xD800, 0x0067])
  }

  #[cfg(unix)]
  fn non_unicode() -> OsString {
    std::os::unix::ffi::OsStringExt::from_vec(vec![0x66, 0xFF, 0x67])
  }

  #[test]
  fn non_unicode_entries_are_skipped() {
    assert!(non_unicode().to_str().is_none());
    let p = from_env_vars([
      (OsString::from("TOKEN"), OsString::from("tok")),
      (non_unicode(), OsString::from("ignored-key")),
      (OsString::from("STATUS"), non_unicode()),
      (OsString::from("LOG_LEVEL"), OsString::from("debug")),
    ])
    .partial;
    assert_eq!(
      token_of(&p, ACCOUNT_FLAT),
      Some("tok"),
      "TOKEN survives a non-Unicode neighbour"
    );
    assert_eq!(
      status_of(&p, ACCOUNT_FLAT),
      None,
      "STATUS with a non-Unicode value is skipped"
    );
    assert_eq!(
      p.root.log_level.as_deref(),
      Some("debug"),
      "LOG_LEVEL survives a non-Unicode neighbour"
    );
  }

  #[test]
  fn discover_account_without_token() {
    let p = from_env_map(&env_map(&[("ACCOUNT_x_STATUS", "idle")]));
    assert!(p.accounts.contains_key("x"));
    assert_eq!(status_of(&p, "x"), Some("idle"));
  }

  fn warnings_for(pairs: &[(&str, &str)]) -> Vec<String> {
    env_warnings(&env_map(pairs))
      .iter()
      .map(ToString::to_string)
      .collect()
  }

  #[test]
  fn existing_id_forms_and_settings_do_not_warn() {
    let keys = [
      "TOKEN",
      "STATUS",
      "CUSTOM_STATUS_EMOJI",
      "LOG_LEVEL",
      "CONFIG_PATH",
      "RUST_LOG",
      "PATH",
      "ACCOUNT",
      "ACCOUNT_0",
      "ACCOUNT_1",
      "ACCOUNT_main",
      "ACCOUNT_primary",
      "ACCOUNT_alt-2",
      "ACCOUNT_0_TOKEN",
      "ACCOUNT_test_TOKEN",
      "ACCOUNT_acc-1_STATUS",
      "ACCOUNT_main_CUSTOM_STATUS_EMOJI",
      "ACCOUNT_main_ACTIVITY",
      "ACCOUNT_main_ACTIVITY_TYPE",
      "ACCOUNT_main_ACTIVITY_1",
      "ACCOUNT_main_ACTIVITY_1_DETAILS",
      "ACCOUNT_main_ACTIVITY_LARGE_IMAGE_TEXT",
      "ACTIVITY",
      "ACTIVITY_TYPE",
      "ACTIVITY_0",
      "ACTIVITY_foo",
      "ACTIVITY_foo_TYPE",
      "ACTIVITY_LARGE_IMAGE_TEXT",
      "DEFAULTS_CAPABILITIES",
      "DEFAULTS_WEB_OS",
      "DEFAULTS_BOT_BROWSER",
    ];
    let pairs: Vec<(&str, &str)> = keys.iter().map(|&key| (key, "x")).collect();
    assert_eq!(warnings_for(&pairs), Vec::<String>::new());

    let p = from_env_map(&env_map(&pairs));
    for id in ["0", "1", "main", "primary", "alt-2", "test", "acc-1"] {
      assert!(p.accounts.contains_key(id), "account id {id} still parsed");
    }
    for id in ["0", "foo"] {
      assert!(
        p.accounts[ACCOUNT_FLAT].activities.contains_key(id),
        "activity id {id} still parsed"
      );
    }
    assert!(p.accounts["main"].activities.contains_key("1"));
  }

  #[test]
  fn warnings_follow_key_order_and_skip_non_ascii_keys() {
    assert_eq!(
      warnings_for(&[
        ("ACCOUNT_STAUTS", "x"),
        ("ACCOUNT_0_STATUZ", "x"),
        ("ACCOUNT_é_STATUS", "x"),
      ]),
      [
        "environment variable ACCOUNT_0_STATUZ matches no setting and is ignored (did you mean 'ACCOUNT_0_STATUS'?)",
        "environment variable ACCOUNT_STAUTS defines account 'STAUTS', not a setting (did you mean 'ACCOUNT_STATUS'?)",
      ]
    );
  }

  fn compose_env_names(yaml: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut env_indent = None;
    for line in yaml.lines() {
      let trimmed = line.trim_start();
      let indent = line.len() - trimmed.len();
      if trimmed == "environment:" {
        env_indent = Some(indent);
        continue;
      }
      let body = match trimmed.strip_prefix('#') {
        Some(commented) => commented.trim_start(),
        None if trimmed.is_empty() => continue,
        None if env_indent.is_some_and(|start| indent <= start) => {
          env_indent = None;
          continue;
        }
        None => trimmed,
      };
      let Some((name, _)) = body.split_once(':') else {
        continue;
      };
      let is_env_name = name.starts_with(|c: char| c.is_ascii_uppercase())
        && name
          .bytes()
          .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_');
      if env_indent.is_some() && is_env_name {
        names.push(name.to_string());
      }
    }
    names
  }

  #[test]
  fn compose_example_has_one_environment_block() {
    let yaml = include_str!("../../../../docker-compose.example.yaml");
    let blocks = yaml
      .lines()
      .filter(|line| line.trim_start() == "environment:")
      .count();
    assert_eq!(
      blocks, 1,
      "docker-compose.example.yaml must have a single uncommented environment: key"
    );
  }

  #[test]
  fn compose_example_env_names_map_to_known_settings() {
    let yaml = include_str!("../../../../docker-compose.example.yaml");
    let names = compose_env_names(yaml);
    for expected in [
      "TOKEN",
      "LOG_LEVEL",
      "ACCOUNT_0_ACTIVITY_0_TYPE",
      "ACCOUNT_1_STATUS",
    ] {
      assert!(
        names.iter().any(|name| name == expected),
        "docker-compose.example.yaml: expected env name {expected} was not found; parsed {names:?}"
      );
    }
    for name in &names {
      assert!(
        name == crate::load::CONFIG_PATH_ENV || parse_env_key(name).is_some(),
        "docker-compose.example.yaml: env name {name} maps to no known setting"
      );
    }
    let warnings: Vec<String> =
      env_warnings(&names.iter().map(|n| (n.clone(), "x".into())).collect())
        .iter()
        .map(ToString::to_string)
        .collect();
    assert!(
      warnings.is_empty(),
      "docker-compose.example.yaml: names that would warn at startup: {warnings:?}"
    );
  }

  #[test]
  fn catalogue_env_suffixes_are_reachable() {
    let account_suffixes = account_suffixes_longest_first();
    let act_suffixes = activity_suffixes_longest_first();
    for &field in AccountScalarField::ALL {
      let Some(suffix) = field.spec().env_suffix else {
        continue;
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
    }
    for &field in CustomStatusField::ALL {
      let suffix = field.spec().env_suffix.expect("custom_status env suffix");
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
    }
    for &field in ActivityField::ALL {
      let Some(suffix) = field.spec().env_suffix else {
        continue;
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
    }
  }

  #[test]
  fn catalogue_env_suffixes_apply_into_partial() {
    for &field in AccountScalarField::ALL {
      let Some(suffix) = field.spec().env_suffix else {
        continue;
      };
      let sample = account_sample(field);
      let p = from_env_map(&env_map(&[(suffix, sample.as_str())]));
      let mut acc = p
        .accounts
        .get(ACCOUNT_FLAT)
        .cloned()
        .unwrap_or_else(|| panic!("{suffix}: no flat account"));
      assert_eq!(
        field.take(&mut acc).as_deref(),
        Some(sample.as_str()),
        "flat {suffix}"
      );

      let key = format!("ACCOUNT_main_{suffix}");
      let p = from_env_map(&env_map(&[(key.as_str(), sample.as_str())]));
      let mut acc = p
        .accounts
        .get("main")
        .cloned()
        .unwrap_or_else(|| panic!("{key}: no account"));
      assert_eq!(
        field.take(&mut acc).as_deref(),
        Some(sample.as_str()),
        "{key}"
      );
    }
    for &field in CustomStatusField::ALL {
      let suffix = field.spec().env_suffix.expect("custom_status env suffix");
      let sample = plain_sample(suffix);
      let p = from_env_map(&env_map(&[(suffix, sample.as_str())]));
      let mut cs = p
        .accounts
        .get(ACCOUNT_FLAT)
        .and_then(|a| a.custom_status.clone())
        .unwrap_or_else(|| panic!("{suffix}: no custom_status"));
      assert_eq!(
        field.take(&mut cs).as_deref(),
        Some(sample.as_str()),
        "{suffix}"
      );
    }
    for &field in ActivityField::ALL {
      let Some(suffix) = field.spec().env_suffix else {
        continue;
      };
      let sample = activity_sample(field);
      let key = format!("ACTIVITY_{suffix}");
      let p = from_env_map(&env_map(&[(key.as_str(), sample.as_str())]));
      let mut act = p
        .accounts
        .get(ACCOUNT_FLAT)
        .and_then(|a| a.activities.get(ACTIVITY_SINGULAR).cloned())
        .unwrap_or_else(|| panic!("{key}: no activity"));
      assert_eq!(
        field.take(&mut act).as_deref(),
        Some(sample.as_str()),
        "{key}"
      );
    }
  }

  #[test]
  fn catalogue_defaults_env_keys_are_reachable() {
    for &profile in DefaultsProfile::ALL {
      for &field in ClientPropField::ALL {
        let suffix = field.spec().env_suffix.expect("client property env suffix");
        let key = format!("{}{}", profile.env_prefix(), suffix);
        let path = parse_env_key(&key).unwrap_or_else(|| panic!("parse {key}"));
        assert_eq!(path, ConfigPath::Defaults(profile, field), "{key}");
        let sample = plain_sample(suffix);
        let mut p = from_env_map(&env_map(&[(key.as_str(), sample.as_str())]));
        let props = profile.props_mut(&mut p.defaults);
        assert_eq!(field.take(props).as_deref(), Some(sample.as_str()), "{key}");
      }
    }
  }

  #[test]
  fn catalogue_global_defaults_env_keys_are_reachable() {
    for &field in GlobalDefaultField::ALL {
      let suffix = field.spec().env_suffix.expect("defaults env suffix");
      let key = format!("{}{}", ENV_DEFAULTS_PREFIX, suffix);
      assert_eq!(
        parse_env_key(&key),
        Some(ConfigPath::GlobalDefault(field)),
        "{key}"
      );
      let sample = plain_sample(suffix);
      let mut p = from_env_map(&env_map(&[(key.as_str(), sample.as_str())]));
      assert_eq!(
        field.take(&mut p.defaults.global).as_deref(),
        Some(sample.as_str()),
        "{key}"
      );
    }
  }
}
