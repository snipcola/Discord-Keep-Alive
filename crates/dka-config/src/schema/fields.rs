use std::collections::BTreeMap;

use clap::Args;
use serde::Deserialize;

use crate::token::SecretString;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FieldSpec {
  pub toml: &'static str,
  pub env_suffix: Option<&'static str>,
  pub cli_long: Option<&'static str>,
  pub set_suffix: &'static str,
}

/// String-field catalog: enum + `ALL`/`spec`/`get_mut`, optional partial/cli_args/cli_get.
///
/// Per-field: `serde_rename`, paired `cli_field` + `help`.
/// Flags: `env_opt`/`env_req`, `cli_req`, `partial`, `cli_args(ArgsStruct)`.
macro_rules! string_fields {
  (
    $(#[$meta:meta])*
    $vis:vis enum $Name:ident => $Target:ident {
      $(
        $Variant:ident($field:ident) {
          toml: $toml:expr,
          env_suffix: $env:expr,
          cli_long: $cli:expr,
          set_suffix: $set:expr
          $(, serde_rename: $rename:expr)?
          $(, cli_field: $cli_field:ident, help: $help:expr)?
          $(,)?
        }
      ),* $(,)?
    }
    $($flags:tt)*
  ) => {
    $(#[$meta])*
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    $vis enum $Name {
      $($Variant,)*
    }

    impl $Name {
      pub const ALL: &[Self] = &[$(Self::$Variant,)*];

      pub const fn spec(self) -> FieldSpec {
        match self {
          $(
            Self::$Variant => FieldSpec {
              toml: $toml,
              env_suffix: $env,
              cli_long: $cli,
              set_suffix: $set,
            },
          )*
        }
      }

      pub fn get_mut(self, target: &mut $Target) -> &mut Option<String> {
        match self {
          $(Self::$Variant => &mut target.$field,)*
        }
      }
    }

    string_fields!(@flags $vis $Name $Target {
      $(
        $Variant($field) {
          toml: $toml,
          env_suffix: $env,
          cli_long: $cli,
          set_suffix: $set
          $(, serde_rename: $rename)?
          $(, cli_field: $cli_field, help: $help)?
        }
      ),*
    } $($flags)*);
  };

  (@flags $vis:vis $Name:ident $Target:ident $body:tt) => {};

  (@flags $vis:vis $Name:ident $Target:ident $body:tt env_opt $($rest:tt)*) => {
    impl $Name {
      pub fn env_suffix(self) -> Option<&'static str> {
        self.spec().env_suffix
      }
    }
    string_fields!(@flags $vis $Name $Target $body $($rest)*);
  };

  (@flags $vis:vis $Name:ident $Target:ident $body:tt env_req $($rest:tt)*) => {
    impl $Name {
      pub fn env_suffix(self) -> &'static str {
        self.spec().env_suffix.unwrap()
      }
    }
    string_fields!(@flags $vis $Name $Target $body $($rest)*);
  };

  (@flags $vis:vis $Name:ident $Target:ident $body:tt cli_req $($rest:tt)*) => {
    impl $Name {
      pub const fn cli_long(self) -> &'static str {
        match self.spec().cli_long {
          Some(v) => v,
          None => unreachable!(),
        }
      }
    }
    string_fields!(@flags $vis $Name $Target $body $($rest)*);
  };

  (@flags $vis:vis $Name:ident $Target:ident {
    $(
      $Variant:ident($field:ident) {
        toml: $toml:expr,
        env_suffix: $env:expr,
        cli_long: $cli:expr,
        set_suffix: $set:expr
        $(, serde_rename: $rename:expr)?
        $(, cli_field: $cli_field:ident, help: $help:expr)?
        $(,)?
      }
    ),* $(,)?
  } partial $($rest:tt)*) => {
    #[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
    $vis struct $Target {
      $(
        $(#[serde(rename = $rename)])?
        #[serde(default)]
        pub $field: Option<String>,
      )*
    }
    string_fields!(@flags $vis $Name $Target {
      $(
        $Variant($field) {
          toml: $toml,
          env_suffix: $env,
          cli_long: $cli,
          set_suffix: $set
          $(, serde_rename: $rename)?
          $(, cli_field: $cli_field, help: $help)?
        }
      ),*
    } $($rest)*);
  };

  // clap::Args group for `#[command(flatten)]` (macros cannot expand into struct fields).
  // Also emits `cli_get` so apply_*_cli is a pure catalog loop.
  (@flags $vis:vis $Name:ident $Target:ident {
    $(
      $Variant:ident($field:ident) {
        toml: $toml:expr,
        env_suffix: $env:expr,
        cli_long: $cli:expr,
        set_suffix: $set:expr
        $(, serde_rename: $rename:expr)?
        $(, cli_field: $cli_field:ident, help: $help:expr)?
        $(,)?
      }
    ),* $(,)?
  } cli_args($ArgsName:ident) $($rest:tt)*) => {
    #[derive(Debug, Default, Args)]
    $vis struct $ArgsName {
      $(
        $(
          #[arg(long = $Name::$Variant.cli_long(), help = $help)]
          pub $cli_field: Option<String>,
        )?
      )*
    }

    impl $Name {
      pub fn cli_get(self, args: &$ArgsName) -> Option<&str> {
        match self {
          $(
            $(Self::$Variant => args.$cli_field.as_deref(),)?
          )*
        }
      }
    }

    string_fields!(@flags $vis $Name $Target {
      $(
        $Variant($field) {
          toml: $toml,
          env_suffix: $env,
          cli_long: $cli,
          set_suffix: $set
          $(, serde_rename: $rename)?
          $(, cli_field: $cli_field, help: $help)?
        }
      ),*
    } $($rest)*);
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

/// How empty override strings map onto resolved `ClientProperties`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientPropEmptyPolicy {
  /// Leave the built-in value when the override is empty.
  SkipEmpty,
  /// Map empty to `None` (optional resolved field).
  EmptyToNone,
  /// Assign even when empty (resolved field is `String`).
  AssignEvenEmpty,
}

string_fields! {
  pub enum ClientPropField => PartialClientProperties {
    Os(os) {
      toml: "os",
      env_suffix: Some("OS"),
      cli_long: None,
      set_suffix: "os",
    },
    Browser(browser) {
      toml: "browser",
      env_suffix: Some("BROWSER"),
      cli_long: None,
      set_suffix: "browser",
    },
    Device(device) {
      toml: "device",
      env_suffix: Some("DEVICE"),
      cli_long: None,
      set_suffix: "device",
    },
    UserAgent(user_agent) {
      toml: "user_agent",
      env_suffix: Some("USER_AGENT"),
      cli_long: None,
      set_suffix: "user_agent",
    },
  }
  env_req partial
}

impl ClientPropField {
  pub const fn empty_policy(self) -> ClientPropEmptyPolicy {
    match self {
      Self::Os => ClientPropEmptyPolicy::SkipEmpty,
      Self::Browser | Self::UserAgent => ClientPropEmptyPolicy::EmptyToNone,
      Self::Device => ClientPropEmptyPolicy::AssignEvenEmpty,
    }
  }

  pub fn apply_override(self, dst: &mut dka_gateway::properties::ClientProperties, value: String) {
    match (self, self.empty_policy()) {
      (Self::Os, ClientPropEmptyPolicy::SkipEmpty) => {
        if !value.is_empty() {
          dst.os = value;
        }
      }
      (Self::Browser, ClientPropEmptyPolicy::EmptyToNone) => {
        dst.browser = if value.is_empty() { None } else { Some(value) };
      }
      (Self::UserAgent, ClientPropEmptyPolicy::EmptyToNone) => {
        dst.user_agent = if value.is_empty() { None } else { Some(value) };
      }
      (Self::Device, ClientPropEmptyPolicy::AssignEvenEmpty) => {
        dst.device = value;
      }
      _ => unreachable!("empty_policy must match field"),
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
}

// Manual: Token uses SecretString; Name uses bare ACCOUNT env and --account.
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

  pub const fn spec(self) -> FieldSpec {
    match self {
      Self::Token => FieldSpec {
        toml: "token",
        env_suffix: Some("TOKEN"),
        cli_long: Some("token"),
        set_suffix: "token",
      },
      // No env_suffix: name is ACCOUNT / ACCOUNT_<id>, not ACCOUNT_NAME.
      Self::Name => FieldSpec {
        toml: "name",
        env_suffix: None,
        cli_long: Some("account"),
        set_suffix: "name",
      },
      Self::Kind => FieldSpec {
        toml: "kind",
        env_suffix: Some("KIND"),
        cli_long: Some("kind"),
        set_suffix: "kind",
      },
      Self::Device => FieldSpec {
        toml: "device",
        env_suffix: Some("DEVICE"),
        cli_long: Some("device"),
        set_suffix: "device",
      },
      Self::Status => FieldSpec {
        toml: "status",
        env_suffix: Some("STATUS"),
        cli_long: Some("status"),
        set_suffix: "status",
      },
    }
  }

  pub fn env_suffix(self) -> Option<&'static str> {
    self.spec().env_suffix
  }

  pub const fn cli_long(self) -> &'static str {
    match self.spec().cli_long {
      Some(v) => v,
      None => unreachable!(),
    }
  }

  pub fn set(self, account: &mut PartialAccount, value: String) {
    match self {
      Self::Token => account.token = Some(SecretString::new(value)),
      Self::Name => account.name = Some(value),
      Self::Kind => account.kind = Some(value),
      Self::Device => account.device = Some(value),
      Self::Status => account.status = Some(value),
    }
  }

  pub fn take(self, account: &mut PartialAccount) -> Option<String> {
    match self {
      Self::Token => account.token.take().map(SecretString::into_inner),
      Self::Name => account.name.take(),
      Self::Kind => account.kind.take(),
      Self::Device => account.device.take(),
      Self::Status => account.status.take(),
    }
  }
}

/// Account scalar clap group. Catalog owns long names; Token stays `SecretString`.
#[derive(Debug, Default, Args)]
pub struct AccountScalarCliArgs {
  /// Token for the flat (default) account.
  #[arg(long = AccountScalarField::Token.cli_long())]
  pub token: Option<SecretString>,

  /// Display name for the flat account.
  #[arg(long = AccountScalarField::Name.cli_long())]
  pub name: Option<String>,

  /// Account kind (`user` or `bot`).
  #[arg(long = AccountScalarField::Kind.cli_long())]
  pub kind: Option<String>,

  /// User device (`desktop`, `web`, or `mobile`).
  #[arg(long = AccountScalarField::Device.cli_long())]
  pub device: Option<String>,

  /// Presence (`online`, `idle`, `dnd`, or `invisible`).
  #[arg(long = AccountScalarField::Status.cli_long())]
  pub status: Option<String>,
}

string_fields! {
  pub enum CustomStatusField => PartialCustomStatus {
    Text(text) {
      toml: "text",
      env_suffix: Some("CUSTOM_STATUS"),
      cli_long: Some("custom-status"),
      set_suffix: "text",
      cli_field: custom_status,
      help: "Custom status text (users only).",
    },
    Emoji(emoji) {
      toml: "emoji",
      env_suffix: Some("CUSTOM_STATUS_EMOJI"),
      cli_long: Some("custom-status-emoji"),
      set_suffix: "emoji",
      cli_field: custom_status_emoji,
      help: "Custom status emoji (users only).",
    },
  }
  env_req cli_req partial cli_args(CustomStatusCliArgs)
}

string_fields! {
  pub enum ActivityField => PartialActivity {
    Name(name) {
      toml: "name",
      env_suffix: None,
      cli_long: Some("activity"),
      set_suffix: "name",
      cli_field: activity,
      help: "Flat activity name.",
    },
    Type(activity_type) {
      toml: "type",
      env_suffix: Some("TYPE"),
      cli_long: Some("activity-type"),
      set_suffix: "type",
      serde_rename: "type",
      cli_field: activity_type,
      help: "Activity type (`playing`, `streaming`, `listening`, `watching`, `competing`).",
    },
    Platform(platform) {
      toml: "platform",
      env_suffix: Some("PLATFORM"),
      cli_long: Some("activity-platform"),
      set_suffix: "platform",
      cli_field: activity_platform,
      help: "Activity platform string.",
    },
    Timestamp(timestamp) {
      toml: "timestamp",
      env_suffix: Some("TIMESTAMP"),
      cli_long: Some("activity-timestamp"),
      set_suffix: "timestamp",
      cli_field: activity_timestamp,
      help: "Activity start time (Unix seconds).",
    },
    ApplicationId(application_id) {
      toml: "application_id",
      env_suffix: Some("APPLICATION_ID"),
      cli_long: Some("activity-application-id"),
      set_suffix: "application_id",
      cli_field: activity_application_id,
      help: "Discord application id.",
    },
    Details(details) {
      toml: "details",
      env_suffix: Some("DETAILS"),
      cli_long: Some("activity-details"),
      set_suffix: "details",
      cli_field: activity_details,
      help: "Activity details line.",
    },
    Url(url) {
      toml: "url",
      env_suffix: Some("URL"),
      cli_long: Some("activity-url"),
      set_suffix: "url",
      cli_field: activity_url,
      help: "Stream URL (required when type is `streaming`).",
    },
    LargeImage(large_image) {
      toml: "large_image",
      env_suffix: Some("LARGE_IMAGE"),
      cli_long: Some("activity-large-image"),
      set_suffix: "large_image",
      cli_field: activity_large_image,
      help: "Large image asset key.",
    },
    LargeImageText(large_image_text) {
      toml: "large_image_text",
      env_suffix: Some("LARGE_IMAGE_TEXT"),
      cli_long: Some("activity-large-image-text"),
      set_suffix: "large_image_text",
      cli_field: activity_large_image_text,
      help: "Large image hover text.",
    },
    SmallImage(small_image) {
      toml: "small_image",
      env_suffix: Some("SMALL_IMAGE"),
      cli_long: Some("activity-small-image"),
      set_suffix: "small_image",
      cli_field: activity_small_image,
      help: "Small image asset key.",
    },
    SmallImageText(small_image_text) {
      toml: "small_image_text",
      env_suffix: Some("SMALL_IMAGE_TEXT"),
      cli_long: Some("activity-small-image-text"),
      set_suffix: "small_image_text",
      cli_field: activity_small_image_text,
      help: "Small image hover text.",
    },
    Button(button) {
      toml: "button",
      env_suffix: Some("BUTTON"),
      cli_long: Some("activity-button"),
      set_suffix: "button",
      cli_field: activity_button,
      help: "Button 1 label.",
    },
    ButtonUrl(button_url) {
      toml: "button_url",
      env_suffix: Some("BUTTON_URL"),
      cli_long: Some("activity-button-url"),
      set_suffix: "button_url",
      cli_field: activity_button_url,
      help: "Button 1 URL.",
    },
    Button2(button2) {
      toml: "button2",
      env_suffix: Some("BUTTON_2"),
      cli_long: Some("activity-button-2"),
      set_suffix: "button2",
      cli_field: activity_button_2,
      help: "Button 2 label.",
    },
    Button2Url(button2_url) {
      toml: "button2_url",
      env_suffix: Some("BUTTON_2_URL"),
      cli_long: Some("activity-button-2-url"),
      set_suffix: "button2_url",
      cli_field: activity_button_2_url,
      help: "Button 2 URL.",
    },
    PartyId(party_id) {
      toml: "party_id",
      env_suffix: Some("PARTY_ID"),
      cli_long: Some("activity-party-id"),
      set_suffix: "party_id",
      cli_field: activity_party_id,
      help: "Party id.",
    },
    PartyCurrent(party_current) {
      toml: "party_current",
      env_suffix: Some("PARTY_CURRENT"),
      cli_long: Some("activity-party-current"),
      set_suffix: "party_current",
      cli_field: activity_party_current,
      help: "Party current size.",
    },
    PartyMax(party_max) {
      toml: "party_max",
      env_suffix: Some("PARTY_MAX"),
      cli_long: Some("activity-party-max"),
      set_suffix: "party_max",
      cli_field: activity_party_max,
      help: "Party max size.",
    },
  }
  env_opt cli_req partial cli_args(ActivityCliArgs)
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct AccountScalars {
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

pub fn apply_custom_status_cli(
  partial: &mut crate::model::partial::PartialConfig,
  args: &CustomStatusCliArgs,
) {
  for &field in CustomStatusField::ALL {
    if let Some(v) = crate::util::trim_to_string(field.cli_get(args)) {
      crate::schema::path::apply_path(
        partial,
        &crate::schema::path::custom_status_path(crate::schema::id::ACCOUNT_FLAT, field),
        v,
      );
    }
  }
}

pub fn apply_activity_cli(
  partial: &mut crate::model::partial::PartialConfig,
  args: &ActivityCliArgs,
) {
  for &field in ActivityField::ALL {
    if let Some(v) = crate::util::trim_to_string(field.cli_get(args)) {
      crate::schema::path::apply_path(
        partial,
        &crate::schema::path::activity_field_path(
          crate::schema::id::ACCOUNT_FLAT,
          crate::schema::id::ACTIVITY_SINGULAR,
          field,
        ),
        v,
      );
    }
  }
}

pub fn apply_account_scalar_cli(
  partial: &mut crate::model::partial::PartialConfig,
  args: &AccountScalarCliArgs,
) {
  for (field, raw) in [
    (AccountScalarField::Token, args.token.as_deref()),
    (AccountScalarField::Name, args.name.as_deref()),
    (AccountScalarField::Kind, args.kind.as_deref()),
    (AccountScalarField::Device, args.device.as_deref()),
    (AccountScalarField::Status, args.status.as_deref()),
  ] {
    if let Some(v) = crate::util::trim_to_string(raw) {
      crate::schema::path::apply_path(
        partial,
        &crate::schema::path::account_scalar_path(crate::schema::id::ACCOUNT_FLAT, field),
        v,
      );
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn field_spec_name_special_cases() {
    let name = AccountScalarField::Name.spec();
    assert_eq!(name.env_suffix, None);
    assert_eq!(name.cli_long, Some("account"));
    assert_eq!(name.toml, "name");

    let act_name = ActivityField::Name.spec();
    assert_eq!(act_name.env_suffix, None);
    assert_eq!(act_name.cli_long, Some("activity"));
    assert_eq!(ActivityField::Type.spec().toml, "type");

    let cs_text = CustomStatusField::Text.spec();
    assert_eq!(cs_text.env_suffix, Some("CUSTOM_STATUS"));
    assert_eq!(cs_text.cli_long, Some("custom-status"));
    assert_eq!(cs_text.toml, "text");
  }

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
