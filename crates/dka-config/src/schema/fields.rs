use std::collections::BTreeMap;

use clap::Args;
use clap::builder::PossibleValuesParser;
use dka_presence::{AccountKind, ActivityPlatform, ActivityType, Device, Status};
use serde::Deserialize;

use crate::token::SecretString;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FieldSpec {
  pub toml: &'static str,
  pub env_suffix: Option<&'static str>,
  pub cli_long: Option<&'static str>,
}

type FieldType<T = String> = T;

macro_rules! fields {
  (
    $vis:vis enum $Name:ident => $Target:ident $(, derive($Derive:path))? {
      $(
        $Variant:ident($field:ident $(: $ty:ty)?) {
          toml: $toml:literal,
          env: $env:expr
          $(, cli: $cli:literal, help: $help:literal $(, choices: $choices:path)?)?
          $(,)?
        }
      ),* $(,)?
    }
  ) => {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    $vis enum $Name {
      $($Variant,)*
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize $(, $Derive)?)]
    $vis struct $Target {
      $(
        $(
          #[arg(
            id = $cli, long = $cli, help = $help
            $(, value_parser = PossibleValuesParser::new($choices()), ignore_case = true)?
          )]
        )?
        #[serde(default, rename = $toml)]
        pub $field: Option<FieldType<$($ty)?>>,
      )*
    }

    impl $Name {
      pub const ALL: &[Self] = &[$(Self::$Variant,)*];

      pub fn keys() -> impl Iterator<Item = &'static str> {
        Self::ALL.iter().map(|f| f.spec().toml)
      }

      pub fn from_toml(raw: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|f| f.spec().toml == raw)
      }

      pub fn spec(self) -> FieldSpec {
        match self {
          $(Self::$Variant => FieldSpec {
            toml: $toml,
            env_suffix: $env,
            cli_long: None $(.or(Some($cli)))?,
          },)*
        }
      }

      pub fn set(self, target: &mut $Target, value: String) {
        match self {
          $(Self::$Variant => target.$field = Some(value.into()),)*
        }
      }

      pub fn take(self, target: &mut $Target) -> Option<String> {
        match self {
          $(Self::$Variant => target.$field.take().map(|v| (*v).to_owned()),)*
        }
      }
    }
  };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefaultsProfile {
  Bot,
  Web,
  Desktop,
  Mobile,
}

impl DefaultsProfile {
  pub const ALL: &[Self] = &[Self::Bot, Self::Web, Self::Desktop, Self::Mobile];

  pub fn env_prefix(self) -> &'static str {
    match self {
      Self::Bot => "DEFAULTS_BOT_",
      Self::Web => "DEFAULTS_WEB_",
      Self::Desktop => "DEFAULTS_DESKTOP_",
      Self::Mobile => "DEFAULTS_MOBILE_",
    }
  }

  pub fn toml(self) -> &'static str {
    match self {
      Self::Bot => "bot",
      Self::Web => "web",
      Self::Desktop => "desktop",
      Self::Mobile => "mobile",
    }
  }

  pub fn props_mut(self, defaults: &mut PartialDefaults) -> &mut PartialClientProperties {
    match self {
      Self::Bot => &mut defaults.bot,
      Self::Web => &mut defaults.web,
      Self::Desktop => &mut defaults.desktop,
      Self::Mobile => &mut defaults.mobile,
    }
  }

  pub fn resolved_mut(
    self,
    defaults: &mut dka_presence::Defaults,
  ) -> &mut dka_presence::ClientProperties {
    match self {
      Self::Bot => &mut defaults.bot,
      Self::Web => &mut defaults.web,
      Self::Desktop => &mut defaults.desktop,
      Self::Mobile => &mut defaults.mobile,
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientPropEmptyPolicy {
  SkipEmpty,
  EmptyToNone,
  AssignEvenEmpty,
}

fields! {
  pub enum ClientPropField => PartialClientProperties {
    Os(os) { toml: "os", env: Some("OS") },
    Browser(browser) { toml: "browser", env: Some("BROWSER") },
    Device(device) { toml: "device", env: Some("DEVICE") },
    UserAgent(user_agent) { toml: "user_agent", env: Some("USER_AGENT") },
    ClientVersion(client_version) { toml: "client_version", env: Some("CLIENT_VERSION") },
    OsVersion(os_version) { toml: "os_version", env: Some("OS_VERSION") },
    OsArch(os_arch) { toml: "os_arch", env: Some("OS_ARCH") },
    AppArch(app_arch) { toml: "app_arch", env: Some("APP_ARCH") },
    SystemLocale(system_locale) { toml: "system_locale", env: Some("SYSTEM_LOCALE") },
    ReleaseChannel(release_channel) { toml: "release_channel", env: Some("RELEASE_CHANNEL") },
    BrowserVersion(browser_version) { toml: "browser_version", env: Some("BROWSER_VERSION") },
    OsSdkVersion(os_sdk_version) { toml: "os_sdk_version", env: Some("OS_SDK_VERSION") },
    ClientBuildNumber(client_build_number) {
      toml: "client_build_number",
      env: Some("CLIENT_BUILD_NUMBER"),
    },
    NativeBuildNumber(native_build_number) {
      toml: "native_build_number",
      env: Some("NATIVE_BUILD_NUMBER"),
    },
  }
}

pub(crate) fn empty_to_none(value: String) -> Option<String> {
  if value.is_empty() { None } else { Some(value) }
}

impl ClientPropField {
  pub const fn empty_policy(self) -> ClientPropEmptyPolicy {
    match self {
      Self::Os => ClientPropEmptyPolicy::SkipEmpty,
      Self::Device => ClientPropEmptyPolicy::AssignEvenEmpty,
      _ => ClientPropEmptyPolicy::EmptyToNone,
    }
  }

  pub fn apply_override(self, dst: &mut dka_presence::ClientProperties, value: String) {
    match self.empty_policy() {
      ClientPropEmptyPolicy::SkipEmpty => {
        if !value.is_empty() {
          dst.os = value;
        }
      }
      ClientPropEmptyPolicy::AssignEvenEmpty => dst.device = value,
      ClientPropEmptyPolicy::EmptyToNone => {
        let slot = match self {
          Self::Os | Self::Device => unreachable!("empty_policy must match field"),
          Self::Browser => &mut dst.browser,
          Self::UserAgent => &mut dst.user_agent,
          Self::ClientVersion => &mut dst.client_version,
          Self::OsVersion => &mut dst.os_version,
          Self::OsArch => &mut dst.os_arch,
          Self::AppArch => &mut dst.app_arch,
          Self::SystemLocale => &mut dst.system_locale,
          Self::ReleaseChannel => &mut dst.release_channel,
          Self::BrowserVersion => &mut dst.browser_version,
          Self::OsSdkVersion => &mut dst.os_sdk_version,
          Self::ClientBuildNumber => &mut dst.client_build_number,
          Self::NativeBuildNumber => &mut dst.native_build_number,
        };
        *slot = empty_to_none(value);
      }
    }
  }
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
  #[serde(flatten)]
  pub global: PartialGlobalDefaults,
}

fields! {
  pub enum GlobalDefaultField => PartialGlobalDefaults {
    HeartbeatVersion(heartbeat_version) {
      toml: "heartbeat_version",
      env: Some("HEARTBEAT_VERSION"),
    },
    Capabilities(capabilities) { toml: "capabilities", env: Some("CAPABILITIES") },
  }
}

impl GlobalDefaultField {
  pub fn resolved_mut(self, defaults: &mut dka_presence::Defaults) -> &mut Option<String> {
    match self {
      Self::HeartbeatVersion => &mut defaults.heartbeat_version,
      Self::Capabilities => &mut defaults.capabilities,
    }
  }
}

pub const ROOT_TABLES: [&str; 2] = ["defaults", "accounts"];
pub const ACCOUNT_KEYS: [&str; 3] = ["custom_status", "activity", "activities"];

pub fn activity_type_names() -> impl Iterator<Item = &'static str> {
  ActivityType::ALL
    .iter()
    .filter(|&&t| t != ActivityType::Custom)
    .map(|t| t.as_str())
}

fields! {
  pub enum RootField => RootScalars {
    LogLevel(log_level) { toml: "log_level", env: Some("LOG_LEVEL") },
    HealthSocket(health_socket) { toml: "health_socket", env: Some("HEALTH_SOCKET") },
  }
}

fields! {
  pub enum AccountScalarField => AccountScalars, derive(Args) {
    Token(token: SecretString) {
      toml: "token",
      env: Some("TOKEN"),
      cli: "token",
      help: "Account token",
    },
    Name(name) {
      toml: "name",
      env: None,
      cli: "account",
      help: "Account name, used in logs",
    },
    Kind(kind) {
      toml: "kind",
      env: Some("KIND"),
      cli: "kind",
      help: "Account kind",
      choices: AccountKind::names,
    },
    Device(device) {
      toml: "device",
      env: Some("DEVICE"),
      cli: "device",
      help: "Device to appear as (users only)",
      choices: Device::names,
    },
    Status(status) {
      toml: "status",
      env: Some("STATUS"),
      cli: "status",
      help: "Online status",
      choices: Status::names,
    },
  }
}

fields! {
  pub enum CustomStatusField => PartialCustomStatus, derive(Args) {
    Text(text) {
      toml: "text",
      env: Some("CUSTOM_STATUS"),
      cli: "custom-status",
      help: "Custom status text (users only)",
    },
    Emoji(emoji) {
      toml: "emoji",
      env: Some("CUSTOM_STATUS_EMOJI"),
      cli: "custom-status-emoji",
      help: "Custom status emoji (users only)",
    },
  }
}

fields! {
  pub enum ActivityField => PartialActivity, derive(Args) {
    Name(name) { toml: "name", env: None, cli: "activity", help: "Activity name" },
    Type(activity_type) {
      toml: "type",
      env: Some("TYPE"),
      cli: "activity-type",
      help: "Activity type",
      choices: activity_type_names,
    },
    Platform(platform) {
      toml: "platform",
      env: Some("PLATFORM"),
      cli: "activity-platform",
      help: "Platform shown on the activity",
      choices: ActivityPlatform::names,
    },
    Timestamp(timestamp) {
      toml: "timestamp",
      env: Some("TIMESTAMP"),
      cli: "activity-timestamp",
      help: "Activity start time (Unix seconds)",
    },
    ApplicationId(application_id) {
      toml: "application_id",
      env: Some("APPLICATION_ID"),
      cli: "activity-application-id",
      help: "Application ID",
    },
    Details(details) {
      toml: "details",
      env: Some("DETAILS"),
      cli: "activity-details",
      help: "Details line",
    },
    Url(url) {
      toml: "url",
      env: Some("URL"),
      cli: "activity-url",
      help: "Stream URL (required for `streaming`)",
    },
    LargeImage(large_image) {
      toml: "large_image",
      env: Some("LARGE_IMAGE"),
      cli: "activity-large-image",
      help: "Large image asset key",
    },
    LargeImageText(large_image_text) {
      toml: "large_image_text",
      env: Some("LARGE_IMAGE_TEXT"),
      cli: "activity-large-image-text",
      help: "Large image hover text",
    },
    SmallImage(small_image) {
      toml: "small_image",
      env: Some("SMALL_IMAGE"),
      cli: "activity-small-image",
      help: "Small image asset key",
    },
    SmallImageText(small_image_text) {
      toml: "small_image_text",
      env: Some("SMALL_IMAGE_TEXT"),
      cli: "activity-small-image-text",
      help: "Small image hover text",
    },
    Button(button) {
      toml: "button",
      env: Some("BUTTON"),
      cli: "activity-button",
      help: "Button 1 label",
    },
    ButtonUrl(button_url) {
      toml: "button_url",
      env: Some("BUTTON_URL"),
      cli: "activity-button-url",
      help: "Button 1 URL",
    },
    Button2(button_2) {
      toml: "button_2",
      env: Some("BUTTON_2"),
      cli: "activity-button-2",
      help: "Button 2 label",
    },
    Button2Url(button_2_url) {
      toml: "button_2_url",
      env: Some("BUTTON_2_URL"),
      cli: "activity-button-2-url",
      help: "Button 2 URL",
    },
    PartyId(party_id) {
      toml: "party_id",
      env: Some("PARTY_ID"),
      cli: "activity-party-id",
      help: "Party ID",
    },
    PartyCurrent(party_current) {
      toml: "party_current",
      env: Some("PARTY_CURRENT"),
      cli: "activity-party-current",
      help: "Current party size",
    },
    PartyMax(party_max) {
      toml: "party_max",
      env: Some("PARTY_MAX"),
      cli: "activity-party-max",
      help: "Maximum party size",
    },
  }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct PartialAccount {
  #[serde(flatten)]
  pub scalars: AccountScalars,
  #[serde(default)]
  pub custom_status: Option<PartialCustomStatus>,
  #[serde(default)]
  pub activities: BTreeMap<String, PartialActivity>,
  #[serde(default)]
  pub activity_order: Vec<String>,
}

impl std::ops::Deref for PartialAccount {
  type Target = AccountScalars;
  fn deref(&self) -> &Self::Target {
    &self.scalars
  }
}

impl std::ops::DerefMut for PartialAccount {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.scalars
  }
}

impl From<AccountScalars> for PartialAccount {
  fn from(scalars: AccountScalars) -> Self {
    Self {
      scalars,
      ..Default::default()
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn partial_activity_type_renames_for_serde() {
    use figment::providers::{Format, Toml};
    let act: PartialActivity = figment::Figment::new()
      .merge(Toml::string(r#"type = "playing""#))
      .extract()
      .unwrap();
    assert_eq!(act.activity_type.as_deref(), Some("playing"));
  }
}
