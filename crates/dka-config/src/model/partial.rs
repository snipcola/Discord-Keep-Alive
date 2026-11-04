use std::collections::BTreeMap;

use serde::Deserialize;

pub use crate::schema::fields::{
  AccountScalars, PartialAccount, PartialActivity, PartialClientProperties, PartialCustomStatus,
  PartialDefaults, RootScalars,
};

#[derive(Debug, Clone, Default, Deserialize)]
pub struct PartialConfig {
  #[serde(flatten)]
  pub root: RootScalars,

  #[serde(default)]
  pub defaults: PartialDefaults,

  #[serde(default)]
  pub accounts: BTreeMap<String, PartialAccount>,

  #[serde(default)]
  pub account_order: Vec<String>,
}

pub(crate) fn any_activity_field_set(act: &PartialActivity) -> bool {
  *act != PartialActivity::default()
}

pub(crate) fn any_client_prop_set(props: &PartialClientProperties) -> bool {
  *props != PartialClientProperties::default()
}

pub(crate) fn any_account_field_set(a: &PartialAccount) -> bool {
  a.scalars != AccountScalars::default()
    || a.custom_status.is_some()
    || a.activities.values().any(any_activity_field_set)
}
