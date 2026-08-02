use dka_presence::{
  AccountKind, ActivityButton, ActivityConfig, ActivityParty, ActivityPlatform, ActivityType,
  CustomStatusConfig, Device, ImageAsset, Status,
};

use super::file::{
  FileAccount, FileActivity, FileClientProperties, FileConfig, FileCustomStatus, FileDefaults,
};
use super::{AccountConfig, ConfigError};
use crate::gateway::properties::{ClientProperties, Defaults};

fn nonempty(s: Option<&str>) -> Option<&str> {
  s.map(str::trim).filter(|s| !s.is_empty())
}

// Token required so bare DEVICE/STATUS env alone does not invent an account.
fn flat_account_configured(account: &FileAccount) -> bool {
  nonempty(account.token.as_deref()).is_some()
}

fn account_slot_configured(a: &FileAccount) -> bool {
  nonempty(a.token.as_deref()).is_some()
    || nonempty(a.name.as_deref()).is_some()
    || nonempty(a.kind.as_deref()).is_some()
    || nonempty(a.device.as_deref()).is_some()
    || nonempty(a.status.as_deref()).is_some()
    || a.custom_status.is_some()
    || a.activity.is_some()
    || a.activities.iter().any(activity_configured)
}

// Name required to emit an activity (same role token plays for accounts).
fn activity_configured(a: &FileActivity) -> bool {
  nonempty(a.name.as_deref()).is_some()
}

fn custom_status_configured(cs: &FileCustomStatus) -> bool {
  nonempty(cs.text.as_deref()).is_some()
}

pub fn resolve_config(
  file: FileConfig,
) -> Result<(String, Defaults, Vec<AccountConfig>), ConfigError> {
  let log_level = file.log_level.clone();
  let defaults = resolve_defaults(file.defaults);
  let accounts = resolve_accounts(file.account, file.accounts)?;
  Ok((log_level, defaults, accounts))
}

fn resolve_defaults(raw: FileDefaults) -> Defaults {
  let mut defaults = Defaults::builtin();
  apply_client_property_overrides(&mut defaults.bot, raw.bot);
  apply_client_property_overrides(&mut defaults.web, raw.web);
  apply_client_property_overrides(&mut defaults.desktop, raw.desktop);
  apply_client_property_overrides(&mut defaults.mobile, raw.mobile);
  defaults
}

fn apply_client_property_overrides(dst: &mut ClientProperties, src: FileClientProperties) {
  if let Some(os) = src.os.filter(|s| !s.is_empty()) {
    dst.os = os;
  }
  if let Some(browser) = src.browser {
    dst.browser = if browser.is_empty() {
      None
    } else {
      Some(browser)
    };
  }
  if let Some(device) = src.device {
    dst.device = device;
  }
  if let Some(user_agent) = src.user_agent {
    dst.user_agent = if user_agent.is_empty() {
      None
    } else {
      Some(user_agent)
    };
  }
}

fn resolve_accounts(
  flat: FileAccount,
  array: Vec<FileAccount>,
) -> Result<Vec<AccountConfig>, ConfigError> {
  let mut raw_accounts = Vec::new();

  if flat_account_configured(&flat) {
    raw_accounts.push(flat);
  }

  raw_accounts.extend(array.into_iter().filter(account_slot_configured));

  if raw_accounts.is_empty() {
    return Err(ConfigError::NoAccounts);
  }

  let mut accounts = Vec::with_capacity(raw_accounts.len());
  for (i, raw) in raw_accounts.into_iter().enumerate() {
    let name = nonempty(raw.name.as_deref())
      .map(str::to_string)
      .unwrap_or_else(|| format!("account-{i}"));
    let token = nonempty(raw.token.as_deref())
      .map(str::to_string)
      .ok_or_else(|| ConfigError::MissingToken(name.clone()))?;

    let kind = match nonempty(raw.kind.as_deref()) {
      None => AccountKind::User,
      Some(v) => AccountKind::parse(v)
        .ok_or_else(|| ConfigError::Invalid(name.clone(), format!("invalid kind '{v}'")))?,
    };

    let device = match nonempty(raw.device.as_deref()) {
      None => None,
      Some(v) => {
        if kind == AccountKind::Bot {
          None
        } else {
          Some(
            Device::parse(v)
              .ok_or_else(|| ConfigError::Invalid(name.clone(), format!("invalid device '{v}'")))?,
          )
        }
      }
    };

    let status = match nonempty(raw.status.as_deref()) {
      None => None,
      Some(v) => Some(
        Status::parse(v)
          .ok_or_else(|| ConfigError::Invalid(name.clone(), format!("invalid status '{v}'")))?,
      ),
    };

    let custom_status = match raw.custom_status {
      Some(cs) if custom_status_configured(&cs) && kind == AccountKind::User => {
        Some(parse_custom_status(cs))
      }
      _ => None,
    };

    let activities = resolve_activities(&name, raw.activity, raw.activities)?;

    accounts.push(AccountConfig {
      name,
      token,
      kind,
      device,
      status,
      custom_status,
      activities,
    });
  }

  Ok(accounts)
}

fn resolve_activities(
  account: &str,
  singular: Option<FileActivity>,
  array: Vec<FileActivity>,
) -> Result<Vec<ActivityConfig>, ConfigError> {
  let mut raw = Vec::new();

  if let Some(act) = singular
    && activity_configured(&act)
  {
    raw.push(act);
  }

  raw.extend(array.into_iter().filter(activity_configured));

  let mut out = Vec::with_capacity(raw.len());
  for (i, act) in raw.into_iter().enumerate() {
    // Unset application_id / party_id default to 1, 2, 3... by activity index.
    let default_id = (i as u64 + 1).to_string();
    out.push(parse_activity(account, act, &default_id)?);
  }
  Ok(out)
}

fn parse_custom_status(raw: FileCustomStatus) -> CustomStatusConfig {
  CustomStatusConfig {
    text: nonempty(raw.text.as_deref()).map(str::to_string),
    emoji: nonempty(raw.emoji.as_deref()).map(str::to_string),
  }
}

fn parse_i64_field(
  account: &str,
  field: &str,
  raw: Option<String>,
) -> Result<Option<String>, ConfigError> {
  match nonempty(raw.as_deref()) {
    None => Ok(None),
    Some(v) => {
      v.parse::<i64>()
        .map_err(|_| ConfigError::Invalid(account.into(), format!("invalid {field} '{v}'")))?;
      Ok(Some(v.to_string()))
    }
  }
}

fn parse_activity(
  account: &str,
  raw: FileActivity,
  default_id: &str,
) -> Result<ActivityConfig, ConfigError> {
  let mut activity = ActivityConfig::new();
  activity.name = nonempty(raw.name.as_deref()).map(str::to_string);

  if let Some(ty) = nonempty(raw.activity_type.as_deref()) {
    let parsed = ActivityType::parse(ty).ok_or_else(|| {
      ConfigError::Invalid(account.into(), format!("invalid activity type '{ty}'"))
    })?;
    if parsed == ActivityType::Custom {
      return Err(ConfigError::Invalid(
        account.into(),
        "activity type 'custom' is not valid here; use [custom_status]".into(),
      ));
    }
    activity.activity_type = Some(parsed);
  }

  if let Some(platform) = nonempty(raw.platform.as_deref()) {
    activity.platform = Some(ActivityPlatform::parse(platform).ok_or_else(|| {
      ConfigError::Invalid(
        account.into(),
        format!("invalid activity platform '{platform}'"),
      )
    })?);
  }

  activity.timestamp = parse_i64_field(account, "activity timestamp", raw.timestamp)?;
  activity.application_id = nonempty(raw.application_id.as_deref())
    .map(str::to_string)
    .unwrap_or_else(|| default_id.to_string());
  activity.details = nonempty(raw.details.as_deref()).map(str::to_string);
  activity.url = nonempty(raw.url.as_deref()).map(str::to_string);
  activity.large_image = ImageAsset {
    image: nonempty(raw.large_image.as_deref()).map(str::to_string),
    text: nonempty(raw.large_image_text.as_deref()).map(str::to_string),
  };
  activity.small_image = ImageAsset {
    image: nonempty(raw.small_image.as_deref()).map(str::to_string),
    text: nonempty(raw.small_image_text.as_deref()).map(str::to_string),
  };
  activity.button = ActivityButton {
    name: nonempty(raw.button.as_deref()).map(str::to_string),
    url: nonempty(raw.button_url.as_deref()).map(str::to_string),
  };
  activity.button2 = ActivityButton {
    name: nonempty(raw.button2.as_deref()).map(str::to_string),
    url: nonempty(raw.button2_url.as_deref()).map(str::to_string),
  };
  activity.party = ActivityParty {
    id: nonempty(raw.party_id.as_deref())
      .map(str::to_string)
      .unwrap_or_else(|| default_id.to_string()),
    current: parse_i64_field(account, "party_current", raw.party_current)?,
    max: parse_i64_field(account, "party_max", raw.party_max)?,
  };

  if activity.activity_type == Some(ActivityType::Streaming)
    && activity.url.as_ref().is_none_or(|u| u.is_empty())
  {
    return Err(ConfigError::Invalid(
      account.into(),
      "streaming activity requires a url".into(),
    ));
  }

  Ok(activity)
}

#[cfg(test)]
mod tests {
  use super::*;
  use dka_presence::{AccountKind, Device};

  #[test]
  fn flat_account_prepended_before_toml_accounts() {
    let file = FileConfig {
      log_level: "info".into(),
      account: FileAccount {
        token: Some("flat-token".into()),
        name: Some("from-env".into()),
        device: Some("mobile".into()),
        status: Some("dnd".into()),
        activity: None,
        ..Default::default()
      },
      accounts: vec![FileAccount {
        name: Some("from-toml".into()),
        token: Some("toml-token".into()),
        device: Some("desktop".into()),
        status: Some("online".into()),
        activity: None,
        ..Default::default()
      }],
      ..Default::default()
    };

    let (_, _, accounts) = resolve_config(file).unwrap();
    assert_eq!(accounts.len(), 2);
    assert_eq!(accounts[0].name, "from-env");
    assert_eq!(accounts[0].token, "flat-token");
    assert_eq!(accounts[0].kind, AccountKind::User);
    assert_eq!(accounts[0].device, Some(Device::Mobile));
    assert_eq!(accounts[1].name, "from-toml");
    assert_eq!(accounts[1].token, "toml-token");
  }

  #[test]
  fn bot_kind_ignores_device() {
    let file = FileConfig {
      account: FileAccount {
        token: Some("bot-token".into()),
        kind: Some("bot".into()),
        device: Some("desktop".into()),
        ..Default::default()
      },
      ..Default::default()
    };
    let (_, _, accounts) = resolve_config(file).unwrap();
    assert_eq!(accounts[0].kind, AccountKind::Bot);
    assert_eq!(accounts[0].device, None);
  }

  #[test]
  fn invalid_kind_errors() {
    let file = FileConfig {
      account: FileAccount {
        token: Some("t".into()),
        kind: Some("alien".into()),
        ..Default::default()
      },
      ..Default::default()
    };
    let err = resolve_config(file).unwrap_err();
    assert!(matches!(err, ConfigError::Invalid(_, _)));
  }

  #[test]
  fn toml_accounts_alone_work() {
    let file = FileConfig {
      accounts: vec![FileAccount {
        name: Some("only".into()),
        token: Some("t".into()),
        ..Default::default()
      }],
      ..Default::default()
    };
    let (_, _, accounts) = resolve_config(file).unwrap();
    assert_eq!(accounts.len(), 1);
    assert_eq!(accounts[0].name, "only");
  }

  #[test]
  fn flat_only_works() {
    let file = FileConfig {
      account: FileAccount {
        token: Some("solo".into()),
        ..Default::default()
      },
      ..Default::default()
    };
    let (_, _, accounts) = resolve_config(file).unwrap();
    assert_eq!(accounts.len(), 1);
    assert_eq!(accounts[0].token, "solo");
    assert_eq!(accounts[0].name, "account-0");
  }

  #[test]
  fn empty_errors() {
    let err = resolve_config(FileConfig::default()).unwrap_err();
    assert!(matches!(err, ConfigError::NoAccounts));
  }

  #[test]
  fn flat_without_token_ignored_when_toml_accounts_exist() {
    let file = FileConfig {
      account: FileAccount {
        device: Some("desktop".into()),
        status: Some("online".into()),
        ..Default::default()
      },
      accounts: vec![FileAccount {
        name: Some("only".into()),
        token: Some("t".into()),
        ..Default::default()
      }],
      ..Default::default()
    };
    let (_, _, accounts) = resolve_config(file).unwrap();
    assert_eq!(accounts.len(), 1);
    assert_eq!(accounts[0].name, "only");
  }

  #[test]
  fn invalid_device_errors() {
    let file = FileConfig {
      account: FileAccount {
        token: Some("t".into()),
        device: Some("toaster".into()),
        ..Default::default()
      },
      ..Default::default()
    };
    let err = resolve_config(file).unwrap_err();
    assert!(matches!(err, ConfigError::Invalid(_, _)));
  }

  #[test]
  fn missing_token_on_named_account() {
    let file = FileConfig {
      accounts: vec![FileAccount {
        name: Some("broken".into()),
        token: None,
        device: Some("web".into()),
        ..Default::default()
      }],
      ..Default::default()
    };
    let err = resolve_config(file).unwrap_err();
    assert!(matches!(err, ConfigError::MissingToken(ref n) if n == "broken"));
  }

  #[test]
  fn activity_defaults_ids_sequential() {
    let file = FileConfig {
      account: FileAccount {
        token: Some("t".into()),
        activity: Some(FileActivity {
          name: Some("first".into()),
          ..Default::default()
        }),
        activities: vec![
          FileActivity {
            name: Some("second".into()),
            ..Default::default()
          },
          FileActivity {
            name: Some("third".into()),
            application_id: Some("99".into()),
            party_id: Some("party-x".into()),
            ..Default::default()
          },
        ],
        ..Default::default()
      },
      ..Default::default()
    };
    let (_, _, accounts) = resolve_config(file).unwrap();
    assert_eq!(accounts[0].activities.len(), 3);
    assert_eq!(accounts[0].activities[0].application_id, "1");
    assert_eq!(accounts[0].activities[0].party.id, "1");
    assert_eq!(accounts[0].activities[1].application_id, "2");
    assert_eq!(accounts[0].activities[1].party.id, "2");
    assert_eq!(accounts[0].activities[2].application_id, "99");
    assert_eq!(accounts[0].activities[2].party.id, "party-x");
  }

  #[test]
  fn singular_activity_prepended_before_array() {
    let file = FileConfig {
      account: FileAccount {
        token: Some("t".into()),
        activity: Some(FileActivity {
          name: Some("first".into()),
          activity_type: Some("playing".into()),
          ..Default::default()
        }),
        activities: vec![
          FileActivity {
            name: Some("second".into()),
            activity_type: Some("listening".into()),
            ..Default::default()
          },
          FileActivity {
            name: Some("third".into()),
            activity_type: Some("watching".into()),
            ..Default::default()
          },
        ],
        ..Default::default()
      },
      ..Default::default()
    };
    let (_, _, accounts) = resolve_config(file).unwrap();
    let names: Vec<_> = accounts[0]
      .activities
      .iter()
      .map(|a| a.name.as_deref().unwrap())
      .collect();
    assert_eq!(names, ["first", "second", "third"]);
  }

  #[test]
  fn activity_without_name_skipped() {
    let file = FileConfig {
      account: FileAccount {
        token: Some("t".into()),
        activity: Some(FileActivity {
          details: Some("no name".into()),
          ..Default::default()
        }),
        activities: vec![
          FileActivity {
            details: Some("also no name".into()),
            ..Default::default()
          },
          FileActivity {
            name: Some("kept".into()),
            ..Default::default()
          },
        ],
        ..Default::default()
      },
      ..Default::default()
    };
    let (_, _, accounts) = resolve_config(file).unwrap();
    assert_eq!(accounts[0].activities.len(), 1);
    assert_eq!(accounts[0].activities[0].name.as_deref(), Some("kept"));
  }

  #[test]
  fn custom_status_user_only() {
    let user = FileConfig {
      account: FileAccount {
        token: Some("t".into()),
        custom_status: Some(FileCustomStatus {
          text: Some("brb".into()),
          emoji: Some("💤".into()),
        }),
        ..Default::default()
      },
      ..Default::default()
    };
    let (_, _, accounts) = resolve_config(user).unwrap();
    let cs = accounts[0].custom_status.as_ref().unwrap();
    assert_eq!(cs.text.as_deref(), Some("brb"));
    assert_eq!(cs.emoji.as_deref(), Some("💤"));

    let bot = FileConfig {
      account: FileAccount {
        token: Some("t".into()),
        kind: Some("bot".into()),
        custom_status: Some(FileCustomStatus {
          text: Some("ignored".into()),
          emoji: Some("x".into()),
        }),
        ..Default::default()
      },
      ..Default::default()
    };
    let (_, _, accounts) = resolve_config(bot).unwrap();
    assert!(accounts[0].custom_status.is_none());
  }

  #[test]
  fn custom_status_without_text_ignored() {
    let file = FileConfig {
      account: FileAccount {
        token: Some("t".into()),
        custom_status: Some(FileCustomStatus {
          text: None,
          emoji: Some("🔥".into()),
        }),
        ..Default::default()
      },
      ..Default::default()
    };
    let (_, _, accounts) = resolve_config(file).unwrap();
    assert!(accounts[0].custom_status.is_none());
  }

  #[test]
  fn activity_type_custom_errors() {
    let file = FileConfig {
      account: FileAccount {
        token: Some("t".into()),
        activity: Some(FileActivity {
          name: Some("x".into()),
          activity_type: Some("custom".into()),
          ..Default::default()
        }),
        ..Default::default()
      },
      ..Default::default()
    };
    let err = resolve_config(file).unwrap_err();
    assert!(matches!(err, ConfigError::Invalid(_, ref m) if m.contains("custom_status")));
  }

  #[test]
  fn invalid_timestamp_errors() {
    let file = FileConfig {
      account: FileAccount {
        token: Some("t".into()),
        activity: Some(FileActivity {
          name: Some("x".into()),
          timestamp: Some("not-a-number".into()),
          ..Default::default()
        }),
        ..Default::default()
      },
      ..Default::default()
    };
    let err = resolve_config(file).unwrap_err();
    assert!(matches!(err, ConfigError::Invalid(_, ref m) if m.contains("timestamp")));
  }

  #[test]
  fn streaming_requires_url() {
    let file = FileConfig {
      account: FileAccount {
        token: Some("t".into()),
        activity: Some(FileActivity {
          name: Some("live".into()),
          activity_type: Some("streaming".into()),
          ..Default::default()
        }),
        ..Default::default()
      },
      ..Default::default()
    };
    let err = resolve_config(file).unwrap_err();
    assert!(matches!(err, ConfigError::Invalid(_, ref m) if m.contains("url")));
  }

  #[test]
  fn whitespace_token_is_missing() {
    let file = FileConfig {
      account: FileAccount {
        token: Some("   ".into()),
        ..Default::default()
      },
      ..Default::default()
    };
    let err = resolve_config(file).unwrap_err();
    assert!(matches!(err, ConfigError::NoAccounts));
  }

  #[test]
  fn account_config_debug_redacts_token() {
    let cfg = AccountConfig {
      name: "n".into(),
      token: "super-secret".into(),
      kind: AccountKind::User,
      device: None,
      status: None,
      custom_status: None,
      activities: vec![],
    };
    let rendered = format!("{cfg:?}");
    assert!(rendered.contains("[redacted]"));
    assert!(!rendered.contains("super-secret"));
  }

  #[test]
  fn defaults_builtin_when_unset() {
    let file = FileConfig {
      account: FileAccount {
        token: Some("t".into()),
        ..Default::default()
      },
      ..Default::default()
    };
    let (_, defaults, _) = resolve_config(file).unwrap();
    assert_eq!(defaults, Defaults::builtin());
  }

  #[test]
  fn defaults_partial_override() {
    let file = FileConfig {
      defaults: FileDefaults {
        bot: FileClientProperties {
          os: Some("FreeBSD".into()),
          browser: None,
          device: None,
          user_agent: Some("bot-ua".into()),
        },
        web: FileClientProperties {
          os: None,
          browser: Some("Chrome".into()),
          device: Some("".into()),
          user_agent: Some("".into()),
        },
        mobile: FileClientProperties {
          os: None,
          browser: Some("".into()),
          device: Some("Pixel".into()),
          user_agent: None,
        },
        ..Default::default()
      },
      account: FileAccount {
        token: Some("t".into()),
        ..Default::default()
      },
      ..Default::default()
    };
    let (_, defaults, _) = resolve_config(file).unwrap();
    let builtin = Defaults::builtin();

    assert_eq!(defaults.bot.os, "FreeBSD");
    assert_eq!(defaults.bot.browser, builtin.bot.browser);
    assert_eq!(defaults.bot.device, builtin.bot.device);
    assert_eq!(defaults.bot.user_agent.as_deref(), Some("bot-ua"));

    assert_eq!(defaults.web.os, builtin.web.os);
    assert_eq!(defaults.web.browser.as_deref(), Some("Chrome"));
    assert_eq!(defaults.web.device, "");
    assert!(defaults.web.user_agent.is_none());

    assert_eq!(defaults.desktop, builtin.desktop);
    assert_eq!(
      defaults.desktop.user_agent.as_deref(),
      Some(crate::gateway::properties::DEFAULT_DESKTOP_UA)
    );

    assert_eq!(defaults.mobile.os, builtin.mobile.os);
    assert!(defaults.mobile.browser.is_none());
    assert_eq!(defaults.mobile.device, "Pixel");
    assert!(defaults.mobile.user_agent.is_none());
  }
}
