use crate::model::partial::{
  PartialAccount, PartialActivity, PartialClientProperties, PartialCustomStatus, PartialDefaults,
};
use crate::token::SecretString;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FieldSpec {
  pub toml: &'static str,
  pub env_suffix: Option<&'static str>,
  pub cli_long: Option<&'static str>,
  pub set_suffix: &'static str,
}

/// Builds a string field enum with `ALL`, `spec`, and `get_mut`.
macro_rules! string_fields {
  (
    $(#[$meta:meta])*
    $vis:vis enum $Name:ident => $Target:ty {
      $(
        $Variant:ident($field:ident) {
          toml: $toml:expr,
          env_suffix: $env:expr,
          cli_long: $cli:expr,
          set_suffix: $set:expr $(,)?
        }
      ),* $(,)?
    }
    $($flags:ident)*
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

    $(string_fields!(@flag $Name $flags);)*
  };

  (@flag $Name:ident env_opt) => {
    impl $Name {
      pub fn env_suffix(self) -> Option<&'static str> {
        self.spec().env_suffix
      }
    }
  };
  (@flag $Name:ident env_req) => {
    impl $Name {
      pub fn env_suffix(self) -> &'static str {
        self.spec().env_suffix.unwrap()
      }
    }
  };
  (@flag $Name:ident cli_req) => {
    impl $Name {
      pub const fn cli_long(self) -> &'static str {
        match self.spec().cli_long {
          Some(v) => v,
          None => unreachable!(),
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
  env_req
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

string_fields! {
  pub enum CustomStatusField => PartialCustomStatus {
    Text(text) {
      toml: "text",
      env_suffix: Some("CUSTOM_STATUS_TEXT"),
      cli_long: Some("custom-status-text"),
      set_suffix: "text",
    },
    Emoji(emoji) {
      toml: "emoji",
      env_suffix: Some("CUSTOM_STATUS_EMOJI"),
      cli_long: Some("custom-status-emoji"),
      set_suffix: "emoji",
    },
  }
  env_req cli_req
}

string_fields! {
  pub enum ActivityField => PartialActivity {
    Name(name) {
      toml: "name",
      env_suffix: None,
      cli_long: Some("activity"),
      set_suffix: "name",
    },
    Type(activity_type) {
      toml: "type",
      env_suffix: Some("TYPE"),
      cli_long: Some("activity-type"),
      set_suffix: "type",
    },
    Platform(platform) {
      toml: "platform",
      env_suffix: Some("PLATFORM"),
      cli_long: Some("activity-platform"),
      set_suffix: "platform",
    },
    Timestamp(timestamp) {
      toml: "timestamp",
      env_suffix: Some("TIMESTAMP"),
      cli_long: Some("activity-timestamp"),
      set_suffix: "timestamp",
    },
    ApplicationId(application_id) {
      toml: "application_id",
      env_suffix: Some("APPLICATION_ID"),
      cli_long: Some("activity-application-id"),
      set_suffix: "application_id",
    },
    Details(details) {
      toml: "details",
      env_suffix: Some("DETAILS"),
      cli_long: Some("activity-details"),
      set_suffix: "details",
    },
    Url(url) {
      toml: "url",
      env_suffix: Some("URL"),
      cli_long: Some("activity-url"),
      set_suffix: "url",
    },
    LargeImage(large_image) {
      toml: "large_image",
      env_suffix: Some("LARGE_IMAGE"),
      cli_long: Some("activity-large-image"),
      set_suffix: "large_image",
    },
    LargeImageText(large_image_text) {
      toml: "large_image_text",
      env_suffix: Some("LARGE_IMAGE_TEXT"),
      cli_long: Some("activity-large-image-text"),
      set_suffix: "large_image_text",
    },
    SmallImage(small_image) {
      toml: "small_image",
      env_suffix: Some("SMALL_IMAGE"),
      cli_long: Some("activity-small-image"),
      set_suffix: "small_image",
    },
    SmallImageText(small_image_text) {
      toml: "small_image_text",
      env_suffix: Some("SMALL_IMAGE_TEXT"),
      cli_long: Some("activity-small-image-text"),
      set_suffix: "small_image_text",
    },
    Button(button) {
      toml: "button",
      env_suffix: Some("BUTTON"),
      cli_long: Some("activity-button"),
      set_suffix: "button",
    },
    ButtonUrl(button_url) {
      toml: "button_url",
      env_suffix: Some("BUTTON_URL"),
      cli_long: Some("activity-button-url"),
      set_suffix: "button_url",
    },
    Button2(button2) {
      toml: "button2",
      env_suffix: Some("BUTTON_2"),
      cli_long: Some("activity-button-2"),
      set_suffix: "button2",
    },
    Button2Url(button2_url) {
      toml: "button2_url",
      env_suffix: Some("BUTTON_2_URL"),
      cli_long: Some("activity-button-2-url"),
      set_suffix: "button2_url",
    },
    PartyId(party_id) {
      toml: "party_id",
      env_suffix: Some("PARTY_ID"),
      cli_long: Some("activity-party-id"),
      set_suffix: "party_id",
    },
    PartyCurrent(party_current) {
      toml: "party_current",
      env_suffix: Some("PARTY_CURRENT"),
      cli_long: Some("activity-party-current"),
      set_suffix: "party_current",
    },
    PartyMax(party_max) {
      toml: "party_max",
      env_suffix: Some("PARTY_MAX"),
      cli_long: Some("activity-party-max"),
      set_suffix: "party_max",
    },
  }
  env_opt cli_req
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
    assert_eq!(name.set_suffix, "name");

    let token = AccountScalarField::Token.spec();
    assert_eq!(token.env_suffix, Some("TOKEN"));
    assert_eq!(token.cli_long, Some("token"));

    let act_name = ActivityField::Name.spec();
    assert_eq!(act_name.env_suffix, None);
    assert_eq!(act_name.cli_long, Some("activity"));
    assert_eq!(act_name.toml, "name");

    let act_type = ActivityField::Type.spec();
    assert_eq!(act_type.env_suffix, Some("TYPE"));
    assert_eq!(act_type.cli_long, Some("activity-type"));
    assert_eq!(act_type.toml, "type");
    assert_eq!(act_type.set_suffix, "type");
  }

  #[test]
  fn cli_long_names_match_existing() {
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
