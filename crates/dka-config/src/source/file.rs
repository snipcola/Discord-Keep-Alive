use std::collections::BTreeMap;
use std::path::Path;

use figment::{
  Figment,
  providers::{Format, Toml},
};
use serde::Deserialize;

use crate::error::ConfigError;
use crate::model::partial::{
  PartialAccount, PartialActivity, PartialConfig, PartialCustomStatus, PartialDefaults,
  any_account_field_set, any_activity_field_set,
};
use crate::schema::id::{ACCOUNT_FLAT, ACTIVITY_SINGULAR};
use crate::token::SecretString;

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

#[derive(Debug, Clone, Default, Deserialize)]
struct FileAccount {
  #[serde(default)]
  name: Option<String>,
  #[serde(default)]
  token: Option<SecretString>,
  #[serde(default)]
  kind: Option<String>,
  #[serde(default)]
  device: Option<String>,
  #[serde(default)]
  status: Option<String>,
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
  let dto: FileConfig = Figment::new().merge(Toml::file(path)).extract()?;
  Ok(file_config_to_partial(dto))
}

#[cfg(test)]
pub fn load_toml_str(contents: &str) -> Result<PartialConfig, ConfigError> {
  let dto: FileConfig = Figment::new().merge(Toml::string(contents)).extract()?;
  Ok(file_config_to_partial(dto))
}

fn file_config_to_partial(file: FileConfig) -> PartialConfig {
  let mut accounts = BTreeMap::new();
  let mut account_order = Vec::new();

  let flat = file_account_to_partial(file.account);
  if any_account_field_set(&flat) {
    accounts.insert(ACCOUNT_FLAT.into(), flat);
    account_order.push(ACCOUNT_FLAT.into());
  }

  // Non-empty [[accounts]] entries get dense ids "0","1",… in file order.
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

  // Non-empty [[activities]] entries get dense ids the same way.
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
    name: acc.name,
    token: acc.token,
    kind: acc.kind,
    device: acc.device,
    status: acc.status,
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
}
