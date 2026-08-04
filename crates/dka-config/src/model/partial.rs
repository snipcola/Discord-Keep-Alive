use std::collections::BTreeMap;

use serde::Deserialize;

use crate::token::SecretString;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct PartialConfig {
  #[serde(default)]
  pub log_level: Option<String>,

  #[serde(default)]
  pub health_socket: Option<String>,

  #[serde(default)]
  pub defaults: PartialDefaults,

  #[serde(default)]
  pub accounts: BTreeMap<String, PartialAccount>,

  #[serde(default)]
  pub account_order: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct PartialDefaults {
  #[serde(default)]
  pub bot: PartialClientProperties,
  #[serde(default)]
  pub web: PartialClientProperties,
  #[serde(default)]
  pub desktop: PartialClientProperties,
  #[serde(default)]
  pub mobile: PartialClientProperties,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct PartialClientProperties {
  #[serde(default)]
  pub os: Option<String>,
  #[serde(default)]
  pub browser: Option<String>,
  #[serde(default)]
  pub device: Option<String>,
  #[serde(default)]
  pub user_agent: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct PartialAccount {
  #[serde(default)]
  pub name: Option<String>,
  #[serde(default)]
  pub token: Option<SecretString>,
  #[serde(default)]
  pub kind: Option<String>,
  #[serde(default)]
  pub device: Option<String>,
  #[serde(default)]
  pub status: Option<String>,
  #[serde(default)]
  pub custom_status: Option<PartialCustomStatus>,
  #[serde(default)]
  pub activities: BTreeMap<String, PartialActivity>,
  #[serde(default)]
  pub activity_order: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct PartialCustomStatus {
  #[serde(default)]
  pub text: Option<String>,
  #[serde(default)]
  pub emoji: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct PartialActivity {
  #[serde(default)]
  pub name: Option<String>,
  #[serde(default, rename = "type")]
  pub activity_type: Option<String>,
  #[serde(default)]
  pub platform: Option<String>,
  #[serde(default)]
  pub timestamp: Option<String>,
  #[serde(default)]
  pub application_id: Option<String>,
  #[serde(default)]
  pub details: Option<String>,
  #[serde(default)]
  pub url: Option<String>,
  #[serde(default)]
  pub large_image: Option<String>,
  #[serde(default)]
  pub large_image_text: Option<String>,
  #[serde(default)]
  pub small_image: Option<String>,
  #[serde(default)]
  pub small_image_text: Option<String>,
  #[serde(default)]
  pub button: Option<String>,
  #[serde(default)]
  pub button_url: Option<String>,
  #[serde(default)]
  pub button2: Option<String>,
  #[serde(default)]
  pub button2_url: Option<String>,
  #[serde(default)]
  pub party_id: Option<String>,
  #[serde(default)]
  pub party_current: Option<String>,
  #[serde(default)]
  pub party_max: Option<String>,
}

pub(crate) fn any_activity_field_set(act: &PartialActivity) -> bool {
  *act != PartialActivity::default()
}

pub(crate) fn any_client_prop_set(props: &PartialClientProperties) -> bool {
  *props != PartialClientProperties::default()
}

pub(crate) fn any_account_field_set(a: &PartialAccount) -> bool {
  a.name.is_some()
    || a.token.is_some()
    || a.kind.is_some()
    || a.device.is_some()
    || a.status.is_some()
    || a.custom_status.is_some()
    || a.activities.values().any(any_activity_field_set)
}
