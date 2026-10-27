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
  PartialDefaults, any_account_field_set, any_activity_field_set,
};
use crate::schema::fields::{
  ACCOUNT_KEYS, AccountScalarField, ActivityField, ClientPropField, CustomStatusField,
  DefaultsProfile, GlobalDefaultField, ROOT_KEYS,
};
use crate::schema::id::{ACCOUNT_FLAT, ACTIVITY_SINGULAR};

#[derive(Debug, Clone, Default, Deserialize)]
struct FileConfig {
  #[serde(default)]
  log_level: Option<String>,
  #[serde(default)]
  health_socket: Option<String>,
  #[serde(default)]
  defaults: PartialDefaults,
  #[serde(default)]
  accounts: Vec<FileAccount>,
  #[serde(flatten)]
  account: FileAccount,
}

// Scalars shared with PartialAccount. The two activity containers are file-only.
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
  let dto: FileConfig = Value::Dict(Tag::default(), raw)
    .deserialize()
    .map_err(to_error)?;
  Ok(file_config_to_partial(dto))
}

fn check_keys(root: &Dict, file: &Path) -> Result<(), ConfigError> {
  check_account(root, "", &ROOT_KEYS, file)?;

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

fn file_config_to_partial(file: FileConfig) -> PartialConfig {
  let mut accounts = BTreeMap::new();
  let mut account_order = Vec::new();

  let flat = file_account_to_partial(file.account);
  if any_account_field_set(&flat) {
    accounts.insert(ACCOUNT_FLAT.into(), flat);
    account_order.push(ACCOUNT_FLAT.into());
  }

  // Non-empty [[accounts]] entries get ids "0", "1", … in file order.
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
    log_level: file.log_level,
    health_socket: file.health_socket,
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

  #[test]
  fn every_catalogue_key_is_accepted() {
    fn assign(keys: impl Iterator<Item = &'static str>) -> String {
      keys.map(|key| format!("{key} = \"x\"\n")).collect()
    }
    let sections = |prefix: &str| {
      format!(
        "[{prefix}custom_status]\n{}[{prefix}activity]\n{}[[{prefix}activities]]\n{}",
        assign(CustomStatusField::keys()),
        assign(ActivityField::keys()),
        assign(ActivityField::keys()),
      )
    };
    let toml = format!(
      "{}{}[defaults]\n{}[defaults.web]\n{}[[accounts]]\n{}{}",
      assign(AccountScalarField::keys()),
      sections(""),
      assign(GlobalDefaultField::keys()),
      assign(ClientPropField::keys()),
      assign(AccountScalarField::keys()),
      sections("accounts."),
    );
    load_toml_str(&toml).unwrap_or_else(|err| panic!("{err}\n{toml}"));
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
