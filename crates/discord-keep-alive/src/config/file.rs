use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileConfig {
  #[serde(default = "default_log_level")]
  pub log_level: String,

  #[serde(default)]
  pub defaults: FileDefaults,

  #[serde(default)]
  pub accounts: Vec<FileAccount>,

  // Flattened top-level fields; when token is set, prepended before `[[accounts]]`.
  #[serde(flatten)]
  pub account: FileAccount,
}

fn default_log_level() -> String {
  "info".into()
}

impl Default for FileConfig {
  fn default() -> Self {
    Self {
      log_level: default_log_level(),
      defaults: FileDefaults::default(),
      accounts: Vec::new(),
      account: FileAccount::default(),
    }
  }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FileDefaults {
  #[serde(default)]
  pub bot: FileClientProperties,
  #[serde(default)]
  pub web: FileClientProperties,
  #[serde(default)]
  pub desktop: FileClientProperties,
  #[serde(default)]
  pub mobile: FileClientProperties,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileClientProperties {
  #[serde(default)]
  pub os: Option<String>,
  #[serde(default)]
  pub browser: Option<String>,
  #[serde(default)]
  pub device: Option<String>,
  #[serde(default)]
  pub user_agent: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FileAccount {
  #[serde(default)]
  pub name: Option<String>,
  #[serde(default)]
  pub token: Option<String>,
  #[serde(default)]
  pub kind: Option<String>,
  #[serde(default)]
  pub device: Option<String>,
  #[serde(default)]
  pub status: Option<String>,
  #[serde(default)]
  pub custom_status: Option<FileCustomStatus>,
  // When name is set, prepended before `activities` (same idea as the flat account).
  #[serde(default)]
  pub activity: Option<FileActivity>,
  #[serde(default)]
  pub activities: Vec<FileActivity>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileCustomStatus {
  #[serde(default)]
  pub text: Option<String>,
  #[serde(default)]
  pub emoji: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileActivity {
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

pub fn any_activity_field_set(act: &FileActivity) -> bool {
  *act != FileActivity::default()
}

pub fn any_custom_status_field_set(cs: &FileCustomStatus) -> bool {
  *cs != FileCustomStatus::default()
}
