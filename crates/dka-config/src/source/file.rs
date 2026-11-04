use std::collections::BTreeMap;
use std::path::Path;

use figment::{
  Figment,
  providers::{Format, Toml},
  value::{Dict, Tag, Value},
};
use serde::Deserialize;

use crate::error::ConfigError;
use crate::model::partial::{
  AccountScalars, PartialAccount, PartialActivity, PartialConfig, PartialCustomStatus,
  PartialDefaults, RootScalars, any_account_field_set, any_activity_field_set,
};
use crate::schema::fields::{
  ACCOUNT_KEYS, AccountScalarField, ActivityField, ClientPropField, CustomStatusField,
  DefaultsProfile, GlobalDefaultField, ROOT_TABLES, RootField,
};
use crate::schema::id::{ACCOUNT_FLAT, ACTIVITY_SINGULAR};

#[derive(Debug, Clone, Default, Deserialize)]
struct FileConfig {
  #[serde(default)]
  defaults: PartialDefaults,
  #[serde(default)]
  accounts: Vec<FileAccount>,
  #[serde(flatten)]
  account: FileAccount,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct FileAccount {
  #[serde(flatten)]
  scalars: AccountScalars,
  #[serde(default)]
  custom_status: Option<PartialCustomStatus>,
  #[serde(default)]
  activity: Option<PartialActivity>,
  #[serde(default)]
  activities: Vec<PartialActivity>,
}

pub fn load_file(path: &Path) -> Result<PartialConfig, ConfigError> {
  if !path.exists() {
    return Err(ConfigError::ConfigNotFound(path.to_path_buf()));
  }
  parse(&Figment::new().merge(Toml::file(path)), path)
}

#[cfg(test)]
pub fn load_toml_str(contents: &str) -> Result<PartialConfig, ConfigError> {
  parse(
    &Figment::new().merge(Toml::string(contents)),
    Path::new("<inline>"),
  )
}

fn parse(figment: &Figment, file: &Path) -> Result<PartialConfig, ConfigError> {
  let to_error = |err: figment::Error| ConfigError::from_figment(file, &err);
  let raw: Dict = figment.extract().map_err(to_error)?;
  check_keys(&raw, file)?;
  let root: RootScalars = Value::Dict(Tag::default(), raw.clone())
    .deserialize()
    .map_err(to_error)?;
  let dto: FileConfig = Value::Dict(Tag::default(), raw)
    .deserialize()
    .map_err(to_error)?;
  Ok(file_config_to_partial(root, dto))
}

fn check_keys(root: &Dict, file: &Path) -> Result<(), ConfigError> {
  let extra: Vec<&str> = RootField::keys().chain(ROOT_TABLES).collect();
  check_account(root, "", &extra, file)?;

  if let Some(defaults) = section(root, "defaults") {
    let profiles = DefaultsProfile::ALL.iter().map(|p| p.toml());
    check_table(
      defaults,
      "defaults",
      GlobalDefaultField::keys().chain(profiles),
      file,
    )?;
    for profile in DefaultsProfile::ALL {
      if let Some(props) = section(defaults, profile.toml()) {
        let scope = format!("defaults.{}", profile.toml());
        check_table(props, &scope, ClientPropField::keys(), file)?;
      }
    }
  }

  for (i, account) in entries(root, "accounts").enumerate() {
    check_account(account, &format!("accounts[{i}]"), &[], file)?;
  }
  Ok(())
}

fn check_account(
  table: &Dict,
  scope: &str,
  extra: &[&'static str],
  file: &Path,
) -> Result<(), ConfigError> {
  let keys = AccountScalarField::keys()
    .chain(ACCOUNT_KEYS)
    .chain(extra.iter().copied());
  check_table(table, scope, keys, file)?;

  let nested = |name: &str| match scope {
    "" => name.to_string(),
    _ => format!("{scope}.{name}"),
  };
  if let Some(custom_status) = section(table, "custom_status") {
    check_table(
      custom_status,
      &nested("custom_status"),
      CustomStatusField::keys(),
      file,
    )?;
  }
  if let Some(activity) = section(table, "activity") {
    check_table(activity, &nested("activity"), ActivityField::keys(), file)?;
  }
  for (i, activity) in entries(table, "activities").enumerate() {
    let scope = format!("{}[{i}]", nested("activities"));
    check_table(activity, &scope, ActivityField::keys(), file)?;
  }
  Ok(())
}

fn check_table(
  table: &Dict,
  scope: &str,
  keys: impl Iterator<Item = &'static str>,
  file: &Path,
) -> Result<(), ConfigError> {
  let keys: Vec<&str> = keys.collect();
  match table.keys().find(|key| !keys.contains(&key.as_str())) {
    Some(key) => Err(ConfigError::unknown_key(file, scope, key, keys)),
    None => Ok(()),
  }
}

fn section<'a>(table: &'a Dict, key: &str) -> Option<&'a Dict> {
  table.get(key).and_then(Value::as_dict)
}

fn entries<'a>(table: &'a Dict, key: &str) -> impl Iterator<Item = &'a Dict> {
  table
    .get(key)
    .and_then(Value::as_array)
    .into_iter()
    .flatten()
    .filter_map(Value::as_dict)
}

fn file_config_to_partial(root: RootScalars, file: FileConfig) -> PartialConfig {
  let mut accounts = BTreeMap::new();
  let mut account_order = Vec::new();

  let flat = file_account_to_partial(file.account);
  if any_account_field_set(&flat) {
    accounts.insert(ACCOUNT_FLAT.into(), flat);
    account_order.push(ACCOUNT_FLAT.into());
  }

  let mut next_account_id = 0usize;
  for acc in file.accounts {
    let partial = file_account_to_partial(acc);
    if any_account_field_set(&partial) {
      let id = next_account_id.to_string();
      next_account_id += 1;
      accounts.insert(id.clone(), partial);
      account_order.push(id);
    }
  }

  PartialConfig {
    root,
    defaults: file.defaults,
    accounts,
    account_order,
  }
}

fn file_account_to_partial(acc: FileAccount) -> PartialAccount {
  let mut activities = BTreeMap::new();
  let mut activity_order = Vec::new();

  if let Some(act) = acc.activity
    && any_activity_field_set(&act)
  {
    activities.insert(ACTIVITY_SINGULAR.into(), act);
    activity_order.push(ACTIVITY_SINGULAR.into());
  }

  let mut next_activity_id = 0usize;
  for act in acc.activities {
    if any_activity_field_set(&act) {
      let id = next_activity_id.to_string();
      next_activity_id += 1;
      activities.insert(id.clone(), act);
      activity_order.push(id);
    }
  }

  PartialAccount {
    scalars: acc.scalars,
    custom_status: acc.custom_status,
    activities,
    activity_order,
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn load_toml_str_assigns_ids() {
    let partial = load_toml_str(
      r#"
token = "flat-tok"
name = "flat"

[activity]
name = "Singular"
type = "playing"

[[activities]]
name = "Zero"
type = "listening"

[[activities]]
name = "One"
type = "watching"

[[accounts]]
name = "first"
token = "tok-0"
status = "online"

[[accounts]]
name = "second"
token = "tok-1"
kind = "bot"
"#,
    )
    .unwrap();

    assert_eq!(
      partial.account_order,
      vec![ACCOUNT_FLAT.to_string(), "0".to_string(), "1".to_string()]
    );
    assert_eq!(
      partial
        .accounts
        .get(ACCOUNT_FLAT)
        .and_then(|a| a.token.as_deref()),
      Some("flat-tok")
    );
    assert_eq!(
      partial.accounts.get("0").and_then(|a| a.name.as_deref()),
      Some("first")
    );
    assert_eq!(
      partial.accounts.get("1").and_then(|a| a.token.as_deref()),
      Some("tok-1")
    );

    let flat = partial.accounts.get(ACCOUNT_FLAT).unwrap();
    assert_eq!(
      flat.activity_order,
      vec![
        ACTIVITY_SINGULAR.to_string(),
        "0".to_string(),
        "1".to_string()
      ]
    );
    assert_eq!(
      flat
        .activities
        .get(ACTIVITY_SINGULAR)
        .and_then(|a| a.name.as_deref()),
      Some("Singular")
    );
    assert_eq!(
      flat.activities.get("0").and_then(|a| a.name.as_deref()),
      Some("Zero")
    );
    assert_eq!(
      flat.activities.get("1").and_then(|a| a.name.as_deref()),
      Some("One")
    );
  }

  #[test]
  fn load_file_missing_errors() {
    let err = load_file(Path::new("definitely-missing-dka-config-xyz.toml")).unwrap_err();
    assert!(matches!(err, ConfigError::ConfigNotFound(_)));
  }

  #[test]
  fn empty_account_slots_get_dense_ids() {
    let partial = load_toml_str(
      r#"
[[accounts]]

[[accounts]]
token = "tok-real"
name = "real"
"#,
    )
    .unwrap();

    assert_eq!(partial.account_order, vec!["0".to_string()]);
    assert!(partial.accounts.contains_key("0"));
    assert!(!partial.accounts.contains_key("1"));
    assert_eq!(
      partial.accounts.get("0").and_then(|a| a.token.as_deref()),
      Some("tok-real")
    );
    assert_eq!(
      partial.accounts.get("0").and_then(|a| a.name.as_deref()),
      Some("real")
    );
  }

  #[test]
  fn empty_activity_slots_get_dense_ids() {
    let partial = load_toml_str(
      r#"
token = "t"

[[activities]]

[[activities]]
name = "Second"
type = "playing"
"#,
    )
    .unwrap();

    let flat = partial.accounts.get(ACCOUNT_FLAT).unwrap();
    assert_eq!(flat.activity_order, vec!["0".to_string()]);
    assert_eq!(
      flat.activities.get("0").and_then(|a| a.name.as_deref()),
      Some("Second")
    );
    assert!(!flat.activities.contains_key("1"));
  }

  fn load_error(toml: &str) -> String {
    load_toml_str(toml).unwrap_err().to_string()
  }

  #[test]
  fn unknown_keys_are_rejected_with_a_suggestion() {
    assert_eq!(
      load_error("tokn = \"x\""),
      "<inline>: unknown key 'tokn' (did you mean 'token'?)"
    );
    assert_eq!(
      load_error("[[accounts]]\nstauts = \"dnd\""),
      "<inline>: unknown key 'stauts' in accounts[0] (did you mean 'status'?)"
    );
    assert_eq!(
      load_error("[[accounts]]\ntoken = \"x\"\n[accounts.activity]\nnme = \"a\""),
      "<inline>: unknown key 'nme' in accounts[0].activity (did you mean 'name'?)"
    );
    assert!(
      load_error("[[activities]]\nzzz = \"a\"")
        .starts_with("<inline>: unknown key 'zzz' in activities[0] (expected name, ")
    );
    assert!(
      load_error("[defaults.web]\nzzz = \"a\"")
        .starts_with("<inline>: unknown key 'zzz' in defaults.web (expected ")
    );
  }

  #[test]
  fn type_errors_name_the_key() {
    assert_eq!(
      load_error("log_level = 3"),
      "<inline>: log_level: invalid type: found signed int `3`, expected a string"
    );
  }

  fn is_example_key_or_table(line: &str) -> bool {
    line.starts_with('[')
      || line.split_once(" = ").is_some_and(|(key, _)| {
        key
          .chars()
          .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
      })
  }

  #[test]
  fn example_file_loads_when_uncommented_and_lists_every_key() {
    let toml: String = include_str!("../../../../config.example.toml")
      .lines()
      .map(|line| match line.strip_prefix("# ") {
        Some(rest) if is_example_key_or_table(rest) => rest,
        _ => line,
      })
      .collect::<Vec<_>>()
      .join("\n");
    let mut partial = load_toml_str(&toml).unwrap_or_else(|err| panic!("{err}\n{toml}"));

    for &field in RootField::ALL {
      assert!(
        field.take(&mut partial.root).is_some(),
        "example sets no root key '{}'",
        field.spec().toml
      );
    }
    let mut flat = partial.accounts.remove(ACCOUNT_FLAT).expect("flat account");
    for &field in AccountScalarField::ALL {
      assert!(
        field.take(&mut flat).is_some(),
        "example sets no account key '{}'",
        field.spec().toml
      );
    }
    let mut custom_status = flat.custom_status.expect("example has no [custom_status]");
    for &field in CustomStatusField::ALL {
      assert!(
        field.take(&mut custom_status).is_some(),
        "example sets no custom_status key '{}'",
        field.spec().toml
      );
    }
    let mut activity = flat
      .activities
      .remove(ACTIVITY_SINGULAR)
      .expect("example has no [activity]");
    for &field in ActivityField::ALL {
      assert!(
        field.take(&mut activity).is_some(),
        "example sets no activity key '{}'",
        field.spec().toml
      );
    }
    for &field in GlobalDefaultField::ALL {
      assert!(
        field.take(&mut partial.defaults.global).is_some(),
        "example sets no defaults key '{}'",
        field.spec().toml
      );
    }
    for &field in ClientPropField::ALL {
      assert!(
        DefaultsProfile::ALL.iter().any(|profile| field
          .take(profile.props_mut(&mut partial.defaults))
          .is_some()),
        "example sets no [defaults.*] key '{}'",
        field.spec().toml
      );
    }
  }

  #[test]
  fn global_defaults_sit_beside_profiles() {
    let partial = load_toml_str(
      r#"
[defaults]
heartbeat_version = "32"
capabilities = ""

[defaults.web]
os = "Linux"
"#,
    )
    .unwrap();

    let defaults = &partial.defaults;
    assert_eq!(defaults.global.heartbeat_version.as_deref(), Some("32"));
    assert_eq!(defaults.global.capabilities.as_deref(), Some(""));
    assert_eq!(defaults.web.os.as_deref(), Some("Linux"));
  }
}
