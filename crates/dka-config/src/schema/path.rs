use crate::error::ConfigError;
use crate::merge::ensure_id_in_order;
use crate::model::partial::{PartialAccount, PartialConfig, PartialCustomStatus};
use crate::schema::fields::{
  AccountScalarField, ActivityField, ClientPropField, CustomStatusField, DefaultsProfile,
};
use crate::schema::id::{self, AccountId, ActivityId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigPath {
  LogLevel,
  HealthSocket,
  Defaults(DefaultsProfile, ClientPropField),
  Account(AccountId, AccountPath),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccountPath {
  Scalar(AccountScalarField),
  CustomStatus(CustomStatusField),
  Activity(ActivityId, ActivityField),
}

pub fn apply_path(partial: &mut PartialConfig, path: &ConfigPath, value: String) {
  match path {
    ConfigPath::LogLevel => {
      partial.log_level = Some(value);
    }
    ConfigPath::HealthSocket => {
      partial.health_socket = Some(value);
    }
    ConfigPath::Defaults(profile, field) => {
      let props = profile.props_mut(&mut partial.defaults);
      *field.get_mut(props) = Some(value);
    }
    ConfigPath::Account(account_id, account_path) => {
      ensure_id_in_order(&mut partial.account_order, account_id);
      let account = partial.accounts.entry(account_id.clone()).or_default();
      apply_account_path(account, account_path, value);
    }
  }
}

fn apply_account_path(account: &mut PartialAccount, path: &AccountPath, value: String) {
  match path {
    AccountPath::Scalar(field) => field.set(account, value),
    AccountPath::CustomStatus(field) => {
      let cs = account
        .custom_status
        .get_or_insert_with(PartialCustomStatus::default);
      *field.get_mut(cs) = Some(value);
    }
    AccountPath::Activity(activity_id, field) => {
      ensure_id_in_order(&mut account.activity_order, activity_id);
      let act = account.activities.entry(activity_id.clone()).or_default();
      *field.get_mut(act) = Some(value);
    }
  }
}

pub fn account_scalar_path(account_id: impl Into<String>, field: AccountScalarField) -> ConfigPath {
  ConfigPath::Account(account_id.into(), AccountPath::Scalar(field))
}

pub fn activity_field_path(
  account_id: impl Into<String>,
  activity_id: impl Into<String>,
  field: ActivityField,
) -> ConfigPath {
  ConfigPath::Account(
    account_id.into(),
    AccountPath::Activity(activity_id.into(), field),
  )
}

pub fn custom_status_path(account_id: impl Into<String>, field: CustomStatusField) -> ConfigPath {
  ConfigPath::Account(account_id.into(), AccountPath::CustomStatus(field))
}

pub fn parse_account_id(raw: &str) -> Result<AccountId, ConfigError> {
  if raw == id::ACCOUNT_FLAT {
    return Ok(id::ACCOUNT_FLAT.into());
  }
  id::parse_user_id(raw)
}

pub fn parse_activity_id(raw: &str) -> Result<ActivityId, ConfigError> {
  if raw == id::ACTIVITY_SINGULAR {
    return Ok(id::ACTIVITY_SINGULAR.into());
  }
  id::parse_user_id(raw)
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::schema::id::{ACCOUNT_FLAT, ACTIVITY_SINGULAR};

  #[test]
  fn apply_path_account_and_activity_fields() {
    let mut partial = PartialConfig::default();
    apply_path(
      &mut partial,
      &account_scalar_path("primary", AccountScalarField::Token),
      "tok".into(),
    );
    apply_path(
      &mut partial,
      &account_scalar_path("primary", AccountScalarField::Name),
      "Primary".into(),
    );
    apply_path(
      &mut partial,
      &custom_status_path("primary", CustomStatusField::Text),
      "brb".into(),
    );
    apply_path(
      &mut partial,
      &activity_field_path("primary", "main", ActivityField::Name),
      "Game".into(),
    );
    apply_path(
      &mut partial,
      &activity_field_path("primary", "main", ActivityField::Type),
      "playing".into(),
    );
    apply_path(
      &mut partial,
      &activity_field_path("primary", ACTIVITY_SINGULAR, ActivityField::Details),
      "details".into(),
    );

    assert_eq!(partial.account_order, vec!["primary".to_string()]);
    let acc = partial.accounts.get("primary").unwrap();
    assert_eq!(acc.token.as_deref(), Some("tok"));
    assert_eq!(acc.name.as_deref(), Some("Primary"));
    assert_eq!(
      acc.custom_status.as_ref().and_then(|c| c.text.as_deref()),
      Some("brb")
    );
    assert_eq!(
      acc.activity_order,
      vec!["main".to_string(), ACTIVITY_SINGULAR.to_string()]
    );
    assert_eq!(
      acc.activities.get("main").and_then(|a| a.name.as_deref()),
      Some("Game")
    );
    assert_eq!(
      acc
        .activities
        .get("main")
        .and_then(|a| a.activity_type.as_deref()),
      Some("playing")
    );
    assert_eq!(
      acc
        .activities
        .get(ACTIVITY_SINGULAR)
        .and_then(|a| a.details.as_deref()),
      Some("details")
    );
  }

  #[test]
  fn apply_path_defaults_and_scalars() {
    let mut partial = PartialConfig::default();
    apply_path(&mut partial, &ConfigPath::LogLevel, "debug".into());
    apply_path(&mut partial, &ConfigPath::HealthSocket, "/tmp/h".into());
    apply_path(
      &mut partial,
      &ConfigPath::Defaults(DefaultsProfile::Web, ClientPropField::Os),
      "Linux".into(),
    );
    apply_path(
      &mut partial,
      &account_scalar_path(ACCOUNT_FLAT, AccountScalarField::Token),
      "flat-tok".into(),
    );

    assert_eq!(partial.log_level.as_deref(), Some("debug"));
    assert_eq!(partial.health_socket.as_deref(), Some("/tmp/h"));
    assert_eq!(partial.defaults.web.os.as_deref(), Some("Linux"));
    assert_eq!(
      partial
        .accounts
        .get(ACCOUNT_FLAT)
        .and_then(|a| a.token.as_deref()),
      Some("flat-tok")
    );
    assert_eq!(partial.account_order, vec![ACCOUNT_FLAT.to_string()]);
  }
}
