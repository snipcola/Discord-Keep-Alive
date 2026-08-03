use super::merge::insert_indexed_account;
use super::merge::insert_indexed_activity;
use super::partial::{
  PartialAccount, PartialActivity, PartialClientProperties, PartialConfig, PartialCustomStatus,
  PartialDefaults, any_activity_field_set, any_client_prop_set, any_custom_status_field_set,
};
#[cfg(test)]
use super::schema::collect_indices_from;
use super::schema::{
  ACCOUNT_INDEX_PREFIX, ACCOUNT_INDEX_TOKEN_SUFFIX, ACTIVITY_INDEX_PREFIX, AccountScalarField,
  ActivityField, ClientPropField, CustomStatusField, DefaultsProfile, ENV_HEALTH_SOCKET,
  ENV_LOG_LEVEL, collect_indices, indexed_activity_env_key, singular_activity_env_key,
};

pub fn from_env() -> PartialConfig {
  from_env_lookup(|key| std::env::var(key).ok())
}

/// Build from `lookup`. Index discovery still scans process env; use [`from_env_map`] to inject both.
pub fn from_env_lookup(lookup: impl Fn(&str) -> Option<String>) -> PartialConfig {
  from_env_lookup_discover(lookup, collect_indices)
}

/// From an explicit map for lookup and discovery (never reads process env).
#[cfg(test)]
pub fn from_env_map(map: &std::collections::HashMap<String, String>) -> PartialConfig {
  let pairs: Vec<(&str, &str)> = map.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
  from_env_lookup_discover(
    |key| map.get(key).cloned(),
    move |prefix, suffix| collect_indices_from(pairs.iter().copied(), prefix, suffix),
  )
}

fn from_env_lookup_discover(
  lookup: impl Fn(&str) -> Option<String>,
  discover: impl Fn(&str, &str) -> Vec<usize>,
) -> PartialConfig {
  let mut partial = PartialConfig::default();

  if let Some(v) = env_opt(&lookup, ENV_LOG_LEVEL) {
    partial.log_level = Some(v);
  }
  if let Some(v) = env_opt(&lookup, ENV_HEALTH_SOCKET) {
    partial.health_socket = Some(v);
  }

  partial.defaults = defaults_from_env(&lookup);
  partial.account = flat_account_from_env(&lookup, "", &discover);
  partial.accounts = indexed_accounts_from_env(&lookup, &discover);

  partial
}

fn defaults_from_env(lookup: &impl Fn(&str) -> Option<String>) -> PartialDefaults {
  let mut defaults = PartialDefaults::default();
  for profile in DefaultsProfile::ALL {
    let props = client_props_from_env(lookup, profile.env_prefix());
    if any_client_prop_set(&props) {
      *profile.props_mut(&mut defaults) = props;
    }
  }
  defaults
}

fn client_props_from_env(
  lookup: &impl Fn(&str) -> Option<String>,
  prefix: &str,
) -> PartialClientProperties {
  let mut set = PartialClientProperties::default();
  for field in ClientPropField::ALL {
    let key = format!("{prefix}{}", field.env_suffix());
    if let Some(v) = env_opt(lookup, &key) {
      *field.get_mut(&mut set) = Some(v);
    }
  }
  set
}

fn flat_account_from_env(
  lookup: &impl Fn(&str) -> Option<String>,
  prefix: &str,
  discover: &impl Fn(&str, &str) -> Vec<usize>,
) -> PartialAccount {
  let mut acc = PartialAccount::default();
  for field in AccountScalarField::ALL {
    let key = format!("{prefix}{}", field.env_suffix());
    if let Some(v) = env_opt(lookup, &key) {
      field.set(&mut acc, v);
    }
  }
  acc.custom_status = custom_status_from_env(lookup, prefix);
  acc.activity = singular_activity_from_env(lookup, prefix);
  acc.activities = indexed_activities_from_env(lookup, prefix, discover);
  acc
}

fn indexed_accounts_from_env(
  lookup: &impl Fn(&str) -> Option<String>,
  discover: &impl Fn(&str, &str) -> Vec<usize>,
) -> Vec<PartialAccount> {
  let mut accounts = Vec::new();
  for index in discover(ACCOUNT_INDEX_PREFIX, ACCOUNT_INDEX_TOKEN_SUFFIX) {
    let prefix = format!("{ACCOUNT_INDEX_PREFIX}{index}_");
    let token_key = format!("{prefix}{}", AccountScalarField::Token.env_suffix());
    let Some(token) = env_opt(lookup, &token_key).filter(|t| !t.is_empty()) else {
      continue;
    };

    let mut acc = PartialAccount::default();
    AccountScalarField::Token.set(&mut acc, token);

    for field in AccountScalarField::ALL {
      if matches!(field, AccountScalarField::Token) {
        continue;
      }
      let key = format!("{prefix}{}", field.env_suffix());
      if let Some(v) = env_opt(lookup, &key) {
        field.set(&mut acc, v);
      }
    }

    acc.custom_status = custom_status_from_env(lookup, &prefix);
    acc.activity = singular_activity_from_env(lookup, &prefix);
    acc.activities = indexed_activities_from_env(lookup, &prefix, discover);

    insert_indexed_account(&mut accounts, index, acc);
  }
  accounts
}

fn custom_status_from_env(
  lookup: &impl Fn(&str) -> Option<String>,
  prefix: &str,
) -> Option<PartialCustomStatus> {
  let mut custom = PartialCustomStatus::default();
  for field in CustomStatusField::ALL {
    let key = format!("{prefix}{}", field.env_suffix());
    if let Some(v) = env_opt(lookup, &key) {
      *field.get_mut(&mut custom) = Some(v);
    }
  }
  any_custom_status_field_set(&custom).then_some(custom)
}

fn singular_activity_from_env(
  lookup: &impl Fn(&str) -> Option<String>,
  prefix: &str,
) -> Option<PartialActivity> {
  let mut act = PartialActivity::default();
  for &field in ActivityField::ALL {
    let key = singular_activity_env_key(prefix, field);
    if let Some(v) = env_opt(lookup, &key) {
      *field.get_mut(&mut act) = Some(v);
    }
  }
  any_activity_field_set(&act).then_some(act)
}

fn indexed_activities_from_env(
  lookup: &impl Fn(&str) -> Option<String>,
  account_prefix: &str,
  discover: &impl Fn(&str, &str) -> Vec<usize>,
) -> Vec<PartialActivity> {
  let mut activities = Vec::new();
  let base = format!("{account_prefix}{ACTIVITY_INDEX_PREFIX}");
  for index in discover(&base, "") {
    let mut act = PartialActivity::default();
    for &field in ActivityField::ALL {
      let key = indexed_activity_env_key(account_prefix, index, field);
      if let Some(v) = env_opt(lookup, &key) {
        *field.get_mut(&mut act) = Some(v);
      }
    }
    if any_activity_field_set(&act) {
      insert_indexed_activity(&mut activities, index, act);
    }
  }
  activities
}

fn env_opt(lookup: &impl Fn(&str) -> Option<String>, key: &str) -> Option<String> {
  lookup(key)
    .map(|v| v.trim().to_string())
    .filter(|v| !v.is_empty())
}
