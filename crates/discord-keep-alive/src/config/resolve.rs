use dka_presence::{
  AccountKind, ActivityButton, ActivityConfig, ActivityParty, ActivityPlatform, ActivityType,
  CustomStatusConfig, Device, ImageAsset, Status,
};

use super::partial::{
  PartialAccount, PartialActivity, PartialClientProperties, PartialConfig, PartialCustomStatus,
  PartialDefaults,
};
use super::schema::DefaultsProfile;
use super::{AccountConfig, ConfigError, DEFAULT_LOG_LEVEL, trim_opt, trim_owned};
use dka_gateway::properties::{ClientProperties, Defaults};

use crate::defaults::product_defaults;

// Flat account is real only when a token is present.
fn flat_account_configured(account: &PartialAccount) -> bool {
  trim_opt(account.token.as_deref()).is_some()
}

fn account_slot_configured(a: &PartialAccount) -> bool {
  trim_opt(a.token.as_deref()).is_some()
    || trim_opt(a.name.as_deref()).is_some()
    || trim_opt(a.kind.as_deref()).is_some()
    || trim_opt(a.device.as_deref()).is_some()
    || trim_opt(a.status.as_deref()).is_some()
    || a.custom_status.is_some()
    || a.activity.is_some()
    || a.activities.iter().any(activity_configured)
}

// Activities without a name are ignored.
fn activity_configured(a: &PartialActivity) -> bool {
  trim_opt(a.name.as_deref()).is_some()
}

fn custom_status_configured(cs: &PartialCustomStatus) -> bool {
  trim_opt(cs.text.as_deref()).is_some()
}

pub fn resolve_config(
  partial: PartialConfig,
) -> Result<(String, Option<String>, Defaults, Vec<AccountConfig>), ConfigError> {
  let log_level = trim_owned(partial.log_level).unwrap_or_else(|| DEFAULT_LOG_LEVEL.into());
  let health_socket = super::normalize_health_socket(partial.health_socket);
  let defaults = resolve_defaults(partial.defaults);
  let accounts = resolve_accounts(partial.account, partial.accounts)?;
  Ok((log_level, health_socket, defaults, accounts))
}

fn resolve_defaults(raw: PartialDefaults) -> Defaults {
  let mut defaults = product_defaults();
  let PartialDefaults {
    bot,
    web,
    desktop,
    mobile,
  } = raw;
  for (profile, src) in [
    (DefaultsProfile::Bot, bot),
    (DefaultsProfile::Web, web),
    (DefaultsProfile::Desktop, desktop),
    (DefaultsProfile::Mobile, mobile),
  ] {
    apply_client_property_overrides(profile.resolved_mut(&mut defaults), src);
  }
  defaults
}

fn apply_client_property_overrides(dst: &mut ClientProperties, src: PartialClientProperties) {
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
  flat: PartialAccount,
  array: Vec<PartialAccount>,
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
    let name = trim_opt(raw.name.as_deref())
      .map(str::to_string)
      .unwrap_or_else(|| format!("account-{i}"));
    let token = trim_opt(raw.token.as_deref())
      .map(super::token::SecretString::new)
      .ok_or_else(|| ConfigError::MissingToken(name.clone()))?;

    let kind =
      parse_enum_field(&name, "kind", raw.kind, AccountKind::parse)?.unwrap_or(AccountKind::User);

    let device = if kind == AccountKind::Bot {
      None
    } else {
      parse_enum_field(&name, "device", raw.device, Device::parse)?
    };

    let status = parse_enum_field(&name, "status", raw.status, Status::parse)?;

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
  singular: Option<PartialActivity>,
  array: Vec<PartialActivity>,
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
    // Missing application_id / party_id become "1", "2", ... in resolve order.
    let default_id = (i as u64 + 1).to_string();
    out.push(parse_activity(account, act, &default_id)?);
  }
  Ok(out)
}

fn parse_custom_status(raw: PartialCustomStatus) -> CustomStatusConfig {
  CustomStatusConfig {
    text: trim_opt(raw.text.as_deref()).map(str::to_string),
    emoji: trim_opt(raw.emoji.as_deref()).map(str::to_string),
  }
}

fn parse_enum_field<T>(
  account: &str,
  field: &str,
  raw: Option<String>,
  parse: impl FnOnce(&str) -> Option<T>,
) -> Result<Option<T>, ConfigError> {
  match trim_opt(raw.as_deref()) {
    None => Ok(None),
    Some(v) => parse(v)
      .map(Some)
      .ok_or_else(|| ConfigError::Invalid(account.into(), format!("invalid {field} '{v}'"))),
  }
}

fn parse_i64_field(
  account: &str,
  field: &str,
  raw: Option<String>,
) -> Result<Option<String>, ConfigError> {
  match trim_opt(raw.as_deref()) {
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
  raw: PartialActivity,
  default_id: &str,
) -> Result<ActivityConfig, ConfigError> {
  let mut activity = ActivityConfig::new();
  activity.name = trim_opt(raw.name.as_deref()).map(str::to_string);

  if let Some(ty) = trim_opt(raw.activity_type.as_deref()) {
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

  activity.platform = parse_enum_field(
    account,
    "activity platform",
    raw.platform,
    ActivityPlatform::parse,
  )?;

  activity.timestamp = parse_i64_field(account, "activity timestamp", raw.timestamp)?;
  activity.application_id = trim_opt(raw.application_id.as_deref())
    .map(str::to_string)
    .unwrap_or_else(|| default_id.to_string());
  activity.details = trim_opt(raw.details.as_deref()).map(str::to_string);
  activity.url = trim_opt(raw.url.as_deref()).map(str::to_string);
  activity.large_image = ImageAsset {
    image: trim_opt(raw.large_image.as_deref()).map(str::to_string),
    text: trim_opt(raw.large_image_text.as_deref()).map(str::to_string),
  };
  activity.small_image = ImageAsset {
    image: trim_opt(raw.small_image.as_deref()).map(str::to_string),
    text: trim_opt(raw.small_image_text.as_deref()).map(str::to_string),
  };
  activity.button = ActivityButton {
    name: trim_opt(raw.button.as_deref()).map(str::to_string),
    url: trim_opt(raw.button_url.as_deref()).map(str::to_string),
  };
  activity.button2 = ActivityButton {
    name: trim_opt(raw.button2.as_deref()).map(str::to_string),
    url: trim_opt(raw.button2_url.as_deref()).map(str::to_string),
  };
  activity.party = ActivityParty {
    id: trim_opt(raw.party_id.as_deref())
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

  fn acc(token: &str) -> PartialAccount {
    PartialAccount {
      token: Some(token.into()),
      ..Default::default()
    }
  }

  fn partial_flat(account: PartialAccount) -> PartialConfig {
    PartialConfig {
      account,
      ..Default::default()
    }
  }

  fn resolve_ok(p: PartialConfig) -> Vec<AccountConfig> {
    let (_, _, _, accounts) = resolve_config(p).unwrap();
    accounts
  }

  fn assert_invalid(label: &str, p: PartialConfig) {
    let err = resolve_config(p).unwrap_err();
    assert!(
      matches!(err, ConfigError::Invalid(_, _)),
      "{label}: {err:?}"
    );
  }

  fn assert_invalid_msg(label: &str, p: PartialConfig, needle: &str) {
    let err = resolve_config(p).unwrap_err();
    match &err {
      ConfigError::Invalid(_, m) if m.contains(needle) => {}
      _ => panic!("{label}: expected Invalid containing {needle:?}, got {err:?}"),
    }
  }

  fn flat_with_activity(act: PartialActivity) -> PartialConfig {
    partial_flat(PartialAccount {
      token: Some("t".into()),
      activity: Some(act),
      ..Default::default()
    })
  }

  fn named_act(name: &str) -> PartialActivity {
    PartialActivity {
      name: Some(name.into()),
      ..Default::default()
    }
  }

  fn three_activities(
    first: PartialActivity,
    second: PartialActivity,
    third: PartialActivity,
  ) -> PartialConfig {
    partial_flat(PartialAccount {
      token: Some("t".into()),
      activity: Some(first),
      activities: vec![second, third],
      ..Default::default()
    })
  }

  #[test]
  fn flat_account_prepended_before_toml_accounts() {
    let a = resolve_ok(PartialConfig {
      account: PartialAccount {
        name: Some("from-env".into()),
        device: Some("mobile".into()),
        status: Some("dnd".into()),
        ..acc("flat-token")
      },
      accounts: vec![PartialAccount {
        name: Some("from-toml".into()),
        device: Some("desktop".into()),
        status: Some("online".into()),
        ..acc("toml-token")
      }],
      ..Default::default()
    });
    assert_eq!(a.len(), 2);
    assert_eq!((&*a[0].name, &*a[0].token), ("from-env", "flat-token"));
    assert_eq!(a[0].kind, AccountKind::User);
    assert_eq!(a[0].device, Some(Device::Mobile));
    assert_eq!((&*a[1].name, &*a[1].token), ("from-toml", "toml-token"));
  }

  #[test]
  fn bot_kind_ignores_device() {
    let accounts = resolve_ok(partial_flat(PartialAccount {
      kind: Some("bot".into()),
      device: Some("desktop".into()),
      ..acc("bot-token")
    }));
    assert_eq!(accounts[0].kind, AccountKind::Bot);
    assert_eq!(accounts[0].device, None);
  }

  #[test]
  fn invalid_field_errors() {
    let cases: &[(&str, PartialConfig, Option<&str>)] = &[
      (
        "kind",
        partial_flat(PartialAccount {
          kind: Some("alien".into()),
          ..acc("t")
        }),
        None,
      ),
      (
        "device",
        partial_flat(PartialAccount {
          device: Some("toaster".into()),
          ..acc("t")
        }),
        None,
      ),
      (
        "custom_activity_type",
        flat_with_activity(PartialActivity {
          activity_type: Some("custom".into()),
          ..named_act("x")
        }),
        Some("custom_status"),
      ),
      (
        "timestamp",
        flat_with_activity(PartialActivity {
          timestamp: Some("not-a-number".into()),
          ..named_act("x")
        }),
        Some("timestamp"),
      ),
      (
        "streaming_url",
        flat_with_activity(PartialActivity {
          activity_type: Some("streaming".into()),
          ..named_act("live")
        }),
        Some("url"),
      ),
    ];
    for &(label, ref partial, needle) in cases {
      match needle {
        None => assert_invalid(label, partial.clone()),
        Some(n) => assert_invalid_msg(label, partial.clone(), n),
      }
    }
  }

  #[test]
  fn resolve_minimal_account_sources() {
    let a = resolve_ok(partial_flat(acc("solo")));
    assert_eq!(a.len(), 1, "flat");
    assert_eq!(a[0].token, "solo", "flat");
    assert_eq!(a[0].name, "account-0", "flat");

    let a = resolve_ok(PartialConfig {
      accounts: vec![PartialAccount {
        name: Some("only".into()),
        ..acc("t")
      }],
      ..Default::default()
    });
    assert_eq!(a.len(), 1, "toml");
    assert_eq!(a[0].name, "only", "toml");
  }

  #[test]
  fn empty_errors() {
    let err = resolve_config(PartialConfig::default()).unwrap_err();
    assert!(matches!(err, ConfigError::NoAccounts));
  }

  #[test]
  fn flat_without_token_ignored_when_toml_accounts_exist() {
    let accounts = resolve_ok(PartialConfig {
      account: PartialAccount {
        device: Some("desktop".into()),
        status: Some("online".into()),
        ..Default::default()
      },
      accounts: vec![PartialAccount {
        name: Some("only".into()),
        ..acc("t")
      }],
      ..Default::default()
    });
    assert_eq!(accounts.len(), 1);
    assert_eq!(accounts[0].name, "only");
  }

  #[test]
  fn missing_token_on_named_account() {
    let partial = PartialConfig {
      accounts: vec![PartialAccount {
        name: Some("broken".into()),
        token: None,
        device: Some("web".into()),
        ..Default::default()
      }],
      ..Default::default()
    };
    let err = resolve_config(partial).unwrap_err();
    assert!(matches!(err, ConfigError::MissingToken(ref n) if n == "broken"));
  }

  #[test]
  fn activity_defaults_ids_sequential() {
    let acts = &resolve_ok(three_activities(
      named_act("first"),
      named_act("second"),
      PartialActivity {
        application_id: Some("99".into()),
        party_id: Some("party-x".into()),
        ..named_act("third")
      },
    ))[0]
      .activities;
    assert_eq!(acts.len(), 3);
    assert_eq!(acts[0].application_id, "1");
    assert_eq!(acts[0].party.id, "1");
    assert_eq!(acts[1].application_id, "2");
    assert_eq!(acts[1].party.id, "2");
    assert_eq!(acts[2].application_id, "99");
    assert_eq!(acts[2].party.id, "party-x");
  }

  #[test]
  fn singular_activity_prepended_before_array() {
    let accounts = resolve_ok(three_activities(
      PartialActivity {
        activity_type: Some("playing".into()),
        ..named_act("first")
      },
      PartialActivity {
        activity_type: Some("listening".into()),
        ..named_act("second")
      },
      PartialActivity {
        activity_type: Some("watching".into()),
        ..named_act("third")
      },
    ));
    let names: Vec<_> = accounts[0]
      .activities
      .iter()
      .map(|a| a.name.as_deref().unwrap())
      .collect();
    assert_eq!(names, ["first", "second", "third"]);
  }

  #[test]
  fn activity_without_name_skipped() {
    let accounts = resolve_ok(partial_flat(PartialAccount {
      activity: Some(PartialActivity {
        details: Some("no name".into()),
        ..Default::default()
      }),
      activities: vec![
        PartialActivity {
          details: Some("also no name".into()),
          ..Default::default()
        },
        named_act("kept"),
      ],
      ..acc("t")
    }));
    assert_eq!(accounts[0].activities.len(), 1);
    assert_eq!(accounts[0].activities[0].name.as_deref(), Some("kept"));
  }

  #[test]
  fn custom_status_user_only() {
    let accounts = resolve_ok(partial_flat(PartialAccount {
      custom_status: Some(PartialCustomStatus {
        text: Some("brb".into()),
        emoji: Some("💤".into()),
      }),
      ..acc("t")
    }));
    let cs = accounts[0].custom_status.as_ref().unwrap();
    assert_eq!(cs.text.as_deref(), Some("brb"));
    assert_eq!(cs.emoji.as_deref(), Some("💤"));

    let accounts = resolve_ok(partial_flat(PartialAccount {
      kind: Some("bot".into()),
      custom_status: Some(PartialCustomStatus {
        text: Some("ignored".into()),
        emoji: Some("x".into()),
      }),
      ..acc("t")
    }));
    assert!(accounts[0].custom_status.is_none());
  }

  #[test]
  fn custom_status_without_text_ignored() {
    let accounts = resolve_ok(partial_flat(PartialAccount {
      custom_status: Some(PartialCustomStatus {
        text: None,
        emoji: Some("🔥".into()),
      }),
      ..acc("t")
    }));
    assert!(accounts[0].custom_status.is_none());
  }

  #[test]
  fn whitespace_token_is_missing() {
    let err = resolve_config(partial_flat(PartialAccount {
      token: Some("   ".into()),
      ..Default::default()
    }))
    .unwrap_err();
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
    assert!(rendered.contains("<redacted>"));
    assert!(!rendered.contains("super-secret"));
  }

  #[test]
  fn defaults_builtin_when_unset() {
    let (_, _, defaults, _) = resolve_config(partial_flat(acc("t"))).unwrap();
    assert_eq!(defaults, product_defaults());
  }

  #[test]
  fn defaults_partial_override() {
    let props =
      |os: Option<&str>, browser: Option<&str>, device: Option<&str>, ua: Option<&str>| {
        PartialClientProperties {
          os: os.map(str::to_string),
          browser: browser.map(str::to_string),
          device: device.map(str::to_string),
          user_agent: ua.map(str::to_string),
        }
      };
    let (_, _, d, _) = resolve_config(PartialConfig {
      defaults: PartialDefaults {
        bot: props(Some("FreeBSD"), None, None, Some("bot-ua")),
        web: props(None, Some("Chrome"), Some(""), Some("")),
        mobile: props(None, Some(""), Some("Pixel"), None),
        ..Default::default()
      },
      account: acc("t"),
      ..Default::default()
    })
    .unwrap();
    let b = product_defaults();
    assert_eq!(d.bot.os, "FreeBSD");
    assert_eq!(d.bot.browser, b.bot.browser);
    assert_eq!(d.bot.device, b.bot.device);
    assert_eq!(d.bot.user_agent.as_deref(), Some("bot-ua"));
    assert_eq!(d.web.os, b.web.os);
    assert_eq!(d.web.browser.as_deref(), Some("Chrome"));
    assert_eq!(d.web.device, "");
    assert!(d.web.user_agent.is_none());
    assert_eq!(d.desktop, b.desktop);
    assert_eq!(
      d.desktop.user_agent.as_deref(),
      Some(crate::defaults::DEFAULT_DESKTOP_UA)
    );
    assert_eq!(d.mobile.os, b.mobile.os);
    assert!(d.mobile.browser.is_none());
    assert_eq!(d.mobile.device, "Pixel");
    assert!(d.mobile.user_agent.is_none());
  }
}
