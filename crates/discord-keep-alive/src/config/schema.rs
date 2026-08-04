use super::partial::{
  PartialAccount, PartialActivity, PartialClientProperties, PartialCustomStatus, PartialDefaults,
};

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
    defaults: &mut dka_gateway::properties::Defaults,
  ) -> &mut dka_gateway::properties::ClientProperties {
    match self {
      Self::Bot => &mut defaults.bot,
      Self::Web => &mut defaults.web,
      Self::Desktop => &mut defaults.desktop,
      Self::Mobile => &mut defaults.mobile,
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientPropField {
  Os,
  Browser,
  Device,
  UserAgent,
}

impl ClientPropField {
  pub const ALL: &[Self] = &[Self::Os, Self::Browser, Self::Device, Self::UserAgent];

  pub fn env_suffix(self) -> &'static str {
    match self {
      Self::Os => "OS",
      Self::Browser => "BROWSER",
      Self::Device => "DEVICE",
      Self::UserAgent => "USER_AGENT",
    }
  }

  pub fn get_mut(self, props: &mut PartialClientProperties) -> &mut Option<String> {
    match self {
      Self::Os => &mut props.os,
      Self::Browser => &mut props.browser,
      Self::Device => &mut props.device,
      Self::UserAgent => &mut props.user_agent,
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountScalarField {
  Token,
  Name,
  Kind,
  Device,
  Status,
}

impl AccountScalarField {
  pub const ALL: &[Self] = &[
    Self::Token,
    Self::Name,
    Self::Kind,
    Self::Device,
    Self::Status,
  ];

  // Name has no env suffix (ACCOUNT / ACCOUNT_0, not ACCOUNT_NAME).
  pub fn env_suffix(self) -> Option<&'static str> {
    match self {
      Self::Name => None,
      Self::Token => Some("TOKEN"),
      Self::Kind => Some("KIND"),
      Self::Device => Some("DEVICE"),
      Self::Status => Some("STATUS"),
    }
  }

  // Name's CLI flag is --account, not --name.
  pub const fn cli_long(self) -> &'static str {
    match self {
      Self::Token => "token",
      Self::Name => "account",
      Self::Kind => "kind",
      Self::Device => "device",
      Self::Status => "status",
    }
  }

  // Token is SecretString; call set/take instead of this.
  pub fn get_mut(self, account: &mut PartialAccount) -> &mut Option<String> {
    match self {
      Self::Token => unreachable!("AccountScalarField::Token: use set/take"),
      Self::Name => &mut account.name,
      Self::Kind => &mut account.kind,
      Self::Device => &mut account.device,
      Self::Status => &mut account.status,
    }
  }

  pub fn set(self, account: &mut PartialAccount, value: String) {
    match self {
      Self::Token => account.token = Some(super::token::SecretString::new(value)),
      Self::Name => account.name = Some(value),
      Self::Kind => account.kind = Some(value),
      Self::Device => account.device = Some(value),
      Self::Status => account.status = Some(value),
    }
  }

  pub fn take(self, account: &mut PartialAccount) -> Option<String> {
    match self {
      Self::Token => account
        .token
        .take()
        .map(super::token::SecretString::into_inner),
      Self::Name => account.name.take(),
      Self::Kind => account.kind.take(),
      Self::Device => account.device.take(),
      Self::Status => account.status.take(),
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CustomStatusField {
  Text,
  Emoji,
}

impl CustomStatusField {
  pub const ALL: &[Self] = &[Self::Text, Self::Emoji];

  pub fn env_suffix(self) -> &'static str {
    match self {
      Self::Text => "CUSTOM_STATUS_TEXT",
      Self::Emoji => "CUSTOM_STATUS_EMOJI",
    }
  }

  pub const fn cli_long(self) -> &'static str {
    match self {
      Self::Text => "custom-status-text",
      Self::Emoji => "custom-status-emoji",
    }
  }

  pub fn get_mut(self, cs: &mut PartialCustomStatus) -> &mut Option<String> {
    match self {
      Self::Text => &mut cs.text,
      Self::Emoji => &mut cs.emoji,
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivityField {
  Name,
  Type,
  Platform,
  Timestamp,
  ApplicationId,
  Details,
  Url,
  LargeImage,
  LargeImageText,
  SmallImage,
  SmallImageText,
  Button,
  ButtonUrl,
  Button2,
  Button2Url,
  PartyId,
  PartyCurrent,
  PartyMax,
}

impl ActivityField {
  pub const ALL: &[Self] = &[
    Self::Name,
    Self::Type,
    Self::Platform,
    Self::Timestamp,
    Self::ApplicationId,
    Self::Details,
    Self::Url,
    Self::LargeImage,
    Self::LargeImageText,
    Self::SmallImage,
    Self::SmallImageText,
    Self::Button,
    Self::ButtonUrl,
    Self::Button2,
    Self::Button2Url,
    Self::PartyId,
    Self::PartyCurrent,
    Self::PartyMax,
  ];

  // Env suffix (None for bare name) and clap long without leading dashes.
  const fn meta(self) -> (Option<&'static str>, &'static str) {
    match self {
      Self::Name => (None, "activity"),
      Self::Type => (Some("TYPE"), "activity-type"),
      Self::Platform => (Some("PLATFORM"), "activity-platform"),
      Self::Timestamp => (Some("TIMESTAMP"), "activity-timestamp"),
      Self::ApplicationId => (Some("APPLICATION_ID"), "activity-application-id"),
      Self::Details => (Some("DETAILS"), "activity-details"),
      Self::Url => (Some("URL"), "activity-url"),
      Self::LargeImage => (Some("LARGE_IMAGE"), "activity-large-image"),
      Self::LargeImageText => (Some("LARGE_IMAGE_TEXT"), "activity-large-image-text"),
      Self::SmallImage => (Some("SMALL_IMAGE"), "activity-small-image"),
      Self::SmallImageText => (Some("SMALL_IMAGE_TEXT"), "activity-small-image-text"),
      Self::Button => (Some("BUTTON"), "activity-button"),
      Self::ButtonUrl => (Some("BUTTON_URL"), "activity-button-url"),
      Self::Button2 => (Some("BUTTON_2"), "activity-button-2"),
      Self::Button2Url => (Some("BUTTON_2_URL"), "activity-button-2-url"),
      Self::PartyId => (Some("PARTY_ID"), "activity-party-id"),
      Self::PartyCurrent => (Some("PARTY_CURRENT"), "activity-party-current"),
      Self::PartyMax => (Some("PARTY_MAX"), "activity-party-max"),
    }
  }

  pub fn env_suffix(self) -> Option<&'static str> {
    self.meta().0
  }

  pub const fn cli_long(self) -> &'static str {
    self.meta().1
  }

  pub fn get_mut(self, act: &mut PartialActivity) -> &mut Option<String> {
    match self {
      Self::Name => &mut act.name,
      Self::Type => &mut act.activity_type,
      Self::Platform => &mut act.platform,
      Self::Timestamp => &mut act.timestamp,
      Self::ApplicationId => &mut act.application_id,
      Self::Details => &mut act.details,
      Self::Url => &mut act.url,
      Self::LargeImage => &mut act.large_image,
      Self::LargeImageText => &mut act.large_image_text,
      Self::SmallImage => &mut act.small_image,
      Self::SmallImageText => &mut act.small_image_text,
      Self::Button => &mut act.button,
      Self::ButtonUrl => &mut act.button_url,
      Self::Button2 => &mut act.button2,
      Self::Button2Url => &mut act.button2_url,
      Self::PartyId => &mut act.party_id,
      Self::PartyCurrent => &mut act.party_current,
      Self::PartyMax => &mut act.party_max,
    }
  }
}

pub const ENV_LOG_LEVEL: &str = "LOG_LEVEL";
pub const ENV_HEALTH_SOCKET: &str = "HEALTH_SOCKET";

// Indexed accounts are found via non-empty ACCOUNT_N_TOKEN.
pub const ACCOUNT_INDEX_PREFIX: &str = "ACCOUNT_";
pub const ACCOUNT_INDEX_TOKEN_SUFFIX: &str = "_TOKEN";
pub const ACCOUNT_SINGULAR: &str = "ACCOUNT";

// Indexed activities are found via bare ACTIVITY_N (not ACTIVITY_N_TYPE alone).
pub const ACTIVITY_INDEX_PREFIX: &str = "ACTIVITY_";

// Index must be plain digits with no leading zeros (except 0).
pub fn parse_indexed_key(key: &str, prefix: &str, suffix: &str) -> Option<usize> {
  let rest = key.strip_prefix(prefix)?;
  let index_str = if suffix.is_empty() {
    rest
  } else {
    rest.strip_suffix(suffix)?
  };
  if index_str.is_empty() || !index_str.bytes().all(|b| b.is_ascii_digit()) {
    return None;
  }
  if index_str.len() > 1 && index_str.starts_with('0') {
    return None;
  }
  index_str.parse().ok()
}

pub fn collect_indices_from<'a, I>(pairs: I, prefix: &str, suffix: &str) -> Vec<usize>
where
  I: IntoIterator<Item = (&'a str, &'a str)>,
{
  let mut found: Vec<usize> = pairs
    .into_iter()
    .filter_map(|(key, value)| {
      if value.is_empty() {
        return None;
      }
      parse_indexed_key(key, prefix, suffix)
    })
    .collect();
  found.sort_unstable();
  found.dedup();
  found
}

pub fn collect_indices(prefix: &str, suffix: &str) -> Vec<usize> {
  let pairs: Vec<(String, String)> = std::env::vars().collect();
  collect_indices_from(
    pairs.iter().map(|(k, v)| (k.as_str(), v.as_str())),
    prefix,
    suffix,
  )
}

pub fn singular_account_env_key(field: AccountScalarField) -> String {
  match field.env_suffix() {
    None => ACCOUNT_SINGULAR.into(),
    Some(suffix) => suffix.into(),
  }
}

pub fn indexed_account_env_key(account_index: usize, field: AccountScalarField) -> String {
  match field.env_suffix() {
    None => format!("{ACCOUNT_INDEX_PREFIX}{account_index}"),
    Some(suffix) => format!("{ACCOUNT_INDEX_PREFIX}{account_index}_{suffix}"),
  }
}

// account_prefix is "" or ACCOUNT_3_.
pub fn singular_activity_env_key(account_prefix: &str, field: ActivityField) -> String {
  match field.env_suffix() {
    None => format!("{account_prefix}ACTIVITY"),
    Some(suffix) => format!("{account_prefix}ACTIVITY_{suffix}"),
  }
}

pub fn indexed_activity_env_key(
  account_prefix: &str,
  activity_index: usize,
  field: ActivityField,
) -> String {
  match field.env_suffix() {
    None => format!("{account_prefix}{ACTIVITY_INDEX_PREFIX}{activity_index}"),
    Some(suffix) => {
      format!("{account_prefix}{ACTIVITY_INDEX_PREFIX}{activity_index}_{suffix}")
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn parse_indexed_key_accepts() {
    for (label, key, prefix, suffix, expected) in [
      (
        "account_0",
        "ACCOUNT_0_TOKEN",
        "ACCOUNT_",
        "_TOKEN",
        Some(0usize),
      ),
      (
        "account_31",
        "ACCOUNT_31_TOKEN",
        "ACCOUNT_",
        "_TOKEN",
        Some(31),
      ),
      (
        "account_100",
        "ACCOUNT_100_TOKEN",
        "ACCOUNT_",
        "_TOKEN",
        Some(100),
      ),
      ("activity_0", "ACTIVITY_0", "ACTIVITY_", "", Some(0)),
      ("activity_12", "ACTIVITY_12", "ACTIVITY_", "", Some(12)),
      (
        "acct_act_1",
        "ACCOUNT_0_ACTIVITY_1",
        "ACCOUNT_0_ACTIVITY_",
        "",
        Some(1),
      ),
    ] {
      assert_eq!(parse_indexed_key(key, prefix, suffix), expected, "{label}");
    }
  }

  #[test]
  fn parse_indexed_key_rejects() {
    for (label, key, prefix, suffix) in [
      ("empty_index", "ACCOUNT__TOKEN", "ACCOUNT_", "_TOKEN"),
      ("leading_zero", "ACCOUNT_01_TOKEN", "ACCOUNT_", "_TOKEN"),
      ("non_digit", "ACCOUNT_x_TOKEN", "ACCOUNT_", "_TOKEN"),
      ("missing_suffix", "ACCOUNT_0", "ACCOUNT_", "_TOKEN"),
      ("no_prefix", "TOKEN", "ACCOUNT_", "_TOKEN"),
      ("activity_empty", "ACTIVITY_", "ACTIVITY_", ""),
      ("activity_leading_zero", "ACTIVITY_01", "ACTIVITY_", ""),
      ("activity_type_suffix", "ACTIVITY_0_TYPE", "ACTIVITY_", ""),
      (
        "activity_details_suffix",
        "ACTIVITY_0_DETAILS",
        "ACTIVITY_",
        "",
      ),
      (
        "account_activity_type",
        "ACCOUNT_0_ACTIVITY_1_TYPE",
        "ACCOUNT_0_ACTIVITY_",
        "",
      ),
    ] {
      assert_eq!(parse_indexed_key(key, prefix, suffix), None, "{label}");
    }
  }

  #[test]
  fn account_env_keys_follow_catalog() {
    for (label, got, want) in [
      (
        "singular_name",
        singular_account_env_key(AccountScalarField::Name),
        "ACCOUNT",
      ),
      (
        "singular_token",
        singular_account_env_key(AccountScalarField::Token),
        "TOKEN",
      ),
      (
        "singular_status",
        singular_account_env_key(AccountScalarField::Status),
        "STATUS",
      ),
      (
        "idx_name",
        indexed_account_env_key(0, AccountScalarField::Name),
        "ACCOUNT_0",
      ),
      (
        "idx_token",
        indexed_account_env_key(2, AccountScalarField::Token),
        "ACCOUNT_2_TOKEN",
      ),
      (
        "idx_kind",
        indexed_account_env_key(1, AccountScalarField::Kind),
        "ACCOUNT_1_KIND",
      ),
    ] {
      assert_eq!(got, want, "{label}");
    }
  }

  #[test]
  fn activity_env_keys_follow_catalog() {
    for (label, got, want) in [
      (
        "sing_name",
        singular_activity_env_key("", ActivityField::Name),
        "ACTIVITY",
      ),
      (
        "sing_type",
        singular_activity_env_key("", ActivityField::Type),
        "ACTIVITY_TYPE",
      ),
      (
        "sing_details_pref",
        singular_activity_env_key("ACCOUNT_2_", ActivityField::Details),
        "ACCOUNT_2_ACTIVITY_DETAILS",
      ),
      (
        "idx_name",
        indexed_activity_env_key("", 5, ActivityField::Name),
        "ACTIVITY_5",
      ),
      (
        "idx_type_pref",
        indexed_activity_env_key("ACCOUNT_1_", 0, ActivityField::Type),
        "ACCOUNT_1_ACTIVITY_0_TYPE",
      ),
    ] {
      assert_eq!(got, want, "{label}");
    }
  }

  #[test]
  fn cli_long_names_match_env_identity() {
    for (label, got, want) in [
      ("token", AccountScalarField::Token.cli_long(), "token"),
      ("account", AccountScalarField::Name.cli_long(), "account"),
      (
        "cs_text",
        CustomStatusField::Text.cli_long(),
        "custom-status-text",
      ),
      ("act_name", ActivityField::Name.cli_long(), "activity"),
      ("act_type", ActivityField::Type.cli_long(), "activity-type"),
      (
        "act_btn2",
        ActivityField::Button2.cli_long(),
        "activity-button-2",
      ),
      (
        "act_app_id",
        ActivityField::ApplicationId.cli_long(),
        "activity-application-id",
      ),
    ] {
      assert_eq!(got, want, "{label}");
    }
  }
}
