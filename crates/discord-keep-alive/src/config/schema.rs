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

  /// Env suffix after `ACCOUNT_{i}_` or bare flat key; `Name` has none.
  pub fn env_suffix(self) -> Option<&'static str> {
    match self {
      Self::Name => None,
      Self::Token => Some("TOKEN"),
      Self::Kind => Some("KIND"),
      Self::Device => Some("DEVICE"),
      Self::Status => Some("STATUS"),
    }
  }

  /// Clap long flag without `--`; `Name` is bare `--account`.
  pub const fn cli_long(self) -> &'static str {
    match self {
      Self::Token => "token",
      Self::Name => "account",
      Self::Kind => "kind",
      Self::Device => "device",
      Self::Status => "status",
    }
  }

  /// Non-token fields only (`Token` → `set` / `take`).
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

  /// Env suffix after `ACTIVITY_{j}_` or singular `ACTIVITY_`; `Name` has none.
  pub fn env_suffix(self) -> Option<&'static str> {
    match self {
      Self::Name => None,
      Self::Type => Some("TYPE"),
      Self::Platform => Some("PLATFORM"),
      Self::Timestamp => Some("TIMESTAMP"),
      Self::ApplicationId => Some("APPLICATION_ID"),
      Self::Details => Some("DETAILS"),
      Self::Url => Some("URL"),
      Self::LargeImage => Some("LARGE_IMAGE"),
      Self::LargeImageText => Some("LARGE_IMAGE_TEXT"),
      Self::SmallImage => Some("SMALL_IMAGE"),
      Self::SmallImageText => Some("SMALL_IMAGE_TEXT"),
      Self::Button => Some("BUTTON"),
      Self::ButtonUrl => Some("BUTTON_URL"),
      Self::Button2 => Some("BUTTON_2"),
      Self::Button2Url => Some("BUTTON_2_URL"),
      Self::PartyId => Some("PARTY_ID"),
      Self::PartyCurrent => Some("PARTY_CURRENT"),
      Self::PartyMax => Some("PARTY_MAX"),
    }
  }

  /// Clap long flag without `--`; `Name` is bare `--activity`.
  pub const fn cli_long(self) -> &'static str {
    match self {
      Self::Name => "activity",
      Self::Type => "activity-type",
      Self::Platform => "activity-platform",
      Self::Timestamp => "activity-timestamp",
      Self::ApplicationId => "activity-application-id",
      Self::Details => "activity-details",
      Self::Url => "activity-url",
      Self::LargeImage => "activity-large-image",
      Self::LargeImageText => "activity-large-image-text",
      Self::SmallImage => "activity-small-image",
      Self::SmallImageText => "activity-small-image-text",
      Self::Button => "activity-button",
      Self::ButtonUrl => "activity-button-url",
      Self::Button2 => "activity-button-2",
      Self::Button2Url => "activity-button-2-url",
      Self::PartyId => "activity-party-id",
      Self::PartyCurrent => "activity-party-current",
      Self::PartyMax => "activity-party-max",
    }
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

/// Account discovery anchors on non-empty `ACCOUNT_{i}_TOKEN`.
pub const ACCOUNT_INDEX_PREFIX: &str = "ACCOUNT_";
pub const ACCOUNT_INDEX_TOKEN_SUFFIX: &str = "_TOKEN";
pub const ACCOUNT_SINGULAR: &str = "ACCOUNT";

/// Activity discovery anchors on bare `ACTIVITY_{j}` (not `_TYPE` alone).
pub const ACTIVITY_INDEX_PREFIX: &str = "ACTIVITY_";

/// Parse `{prefix}{index}{suffix}`: ASCII digits only, no leading zeros (except `0`).
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

/// Sorted unique indices from non-empty `{prefix}{index}{suffix}` pairs.
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

/// Name: bare `ACCOUNT`; other fields: flat `TOKEN`, `KIND`, …
pub fn singular_account_env_key(field: AccountScalarField) -> String {
  match field.env_suffix() {
    None => ACCOUNT_SINGULAR.into(),
    Some(suffix) => suffix.into(),
  }
}

/// Name: bare `ACCOUNT_{i}`; other fields: `ACCOUNT_{i}_{SUFFIX}`.
pub fn indexed_account_env_key(account_index: usize, field: AccountScalarField) -> String {
  match field.env_suffix() {
    None => format!("{ACCOUNT_INDEX_PREFIX}{account_index}"),
    Some(suffix) => format!("{ACCOUNT_INDEX_PREFIX}{account_index}_{suffix}"),
  }
}

/// `account_prefix` is `""` or e.g. `ACCOUNT_3_`.
pub fn singular_activity_env_key(account_prefix: &str, field: ActivityField) -> String {
  match field.env_suffix() {
    None => format!("{account_prefix}ACTIVITY"),
    Some(suffix) => format!("{account_prefix}ACTIVITY_{suffix}"),
  }
}

/// Name: bare `…ACTIVITY_{j}`; other fields: `…ACTIVITY_{j}_{SUFFIX}`.
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
  fn parse_indexed_key_account_token() {
    assert_eq!(
      parse_indexed_key("ACCOUNT_0_TOKEN", "ACCOUNT_", "_TOKEN"),
      Some(0)
    );
    assert_eq!(
      parse_indexed_key("ACCOUNT_31_TOKEN", "ACCOUNT_", "_TOKEN"),
      Some(31)
    );
    assert_eq!(
      parse_indexed_key("ACCOUNT_100_TOKEN", "ACCOUNT_", "_TOKEN"),
      Some(100)
    );
  }

  #[test]
  fn parse_indexed_key_activity_name() {
    assert_eq!(parse_indexed_key("ACTIVITY_0", "ACTIVITY_", ""), Some(0));
    assert_eq!(parse_indexed_key("ACTIVITY_12", "ACTIVITY_", ""), Some(12));
    assert_eq!(parse_indexed_key("ACTIVITY_0_TYPE", "ACTIVITY_", ""), None);
    assert_eq!(
      parse_indexed_key("ACTIVITY_0_DETAILS", "ACTIVITY_", ""),
      None
    );
  }

  #[test]
  fn parse_indexed_key_account_activity() {
    assert_eq!(
      parse_indexed_key("ACCOUNT_0_ACTIVITY_1", "ACCOUNT_0_ACTIVITY_", ""),
      Some(1)
    );
    assert_eq!(
      parse_indexed_key("ACCOUNT_0_ACTIVITY_1_TYPE", "ACCOUNT_0_ACTIVITY_", ""),
      None
    );
  }

  #[test]
  fn parse_indexed_key_rejects_noise() {
    assert_eq!(
      parse_indexed_key("ACCOUNT__TOKEN", "ACCOUNT_", "_TOKEN"),
      None
    );
    assert_eq!(
      parse_indexed_key("ACCOUNT_01_TOKEN", "ACCOUNT_", "_TOKEN"),
      None
    );
    assert_eq!(
      parse_indexed_key("ACCOUNT_x_TOKEN", "ACCOUNT_", "_TOKEN"),
      None
    );
    assert_eq!(parse_indexed_key("ACCOUNT_0", "ACCOUNT_", "_TOKEN"), None);
    assert_eq!(parse_indexed_key("TOKEN", "ACCOUNT_", "_TOKEN"), None);
    assert_eq!(parse_indexed_key("ACTIVITY_", "ACTIVITY_", ""), None);
    assert_eq!(parse_indexed_key("ACTIVITY_01", "ACTIVITY_", ""), None);
  }

  #[test]
  fn account_env_keys_follow_catalog() {
    assert_eq!(
      singular_account_env_key(AccountScalarField::Name),
      "ACCOUNT"
    );
    assert_eq!(singular_account_env_key(AccountScalarField::Token), "TOKEN");
    assert_eq!(
      singular_account_env_key(AccountScalarField::Status),
      "STATUS"
    );
    assert_eq!(
      indexed_account_env_key(0, AccountScalarField::Name),
      "ACCOUNT_0"
    );
    assert_eq!(
      indexed_account_env_key(2, AccountScalarField::Token),
      "ACCOUNT_2_TOKEN"
    );
    assert_eq!(
      indexed_account_env_key(1, AccountScalarField::Kind),
      "ACCOUNT_1_KIND"
    );
  }

  #[test]
  fn activity_env_keys_follow_catalog() {
    assert_eq!(
      singular_activity_env_key("", ActivityField::Name),
      "ACTIVITY"
    );
    assert_eq!(
      singular_activity_env_key("", ActivityField::Type),
      "ACTIVITY_TYPE"
    );
    assert_eq!(
      singular_activity_env_key("ACCOUNT_2_", ActivityField::Details),
      "ACCOUNT_2_ACTIVITY_DETAILS"
    );
    assert_eq!(
      indexed_activity_env_key("", 5, ActivityField::Name),
      "ACTIVITY_5"
    );
    assert_eq!(
      indexed_activity_env_key("ACCOUNT_1_", 0, ActivityField::Type),
      "ACCOUNT_1_ACTIVITY_0_TYPE"
    );
  }

  #[test]
  fn cli_long_names_match_env_identity() {
    assert_eq!(AccountScalarField::Token.cli_long(), "token");
    assert_eq!(AccountScalarField::Name.cli_long(), "account");
    assert_eq!(CustomStatusField::Text.cli_long(), "custom-status-text");
    assert_eq!(ActivityField::Name.cli_long(), "activity");
    assert_eq!(ActivityField::Type.cli_long(), "activity-type");
    assert_eq!(ActivityField::Button2.cli_long(), "activity-button-2");
    assert_eq!(
      ActivityField::ApplicationId.cli_long(),
      "activity-application-id"
    );
  }
}
