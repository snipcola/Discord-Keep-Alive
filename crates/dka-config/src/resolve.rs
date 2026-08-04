use std::collections::BTreeMap;

use dka_gateway::properties::{ClientProperties, Defaults};
use dka_presence::{
  AccountKind, ActivityButton, ActivityConfig, ActivityParty, ActivityPlatform, ActivityType,
  CustomStatusConfig, Device, ImageAsset, Status,
};

use crate::error::ConfigError;
use crate::model::partial::{
  PartialAccount, PartialActivity, PartialClientProperties, PartialConfig, PartialCustomStatus,
  PartialDefaults, any_account_field_set,
};
use crate::model::resolved::{AccountConfig, AppConfig};
use crate::product_defaults::product_defaults;
use crate::schema::fields::DefaultsProfile;
use crate::schema::id::{ACCOUNT_FLAT, ACTIVITY_SINGULAR};
use crate::source::defaults::DEFAULT_LOG_LEVEL;
use crate::token::SecretString;
use crate::util::{trim_opt, trim_owned};

pub(crate) fn resolve_config(partial: PartialConfig) -> Result<AppConfig, ConfigError> {
  let log_level = trim_owned(partial.log_level).unwrap_or_else(|| DEFAULT_LOG_LEVEL.into());
  let health_socket = trim_owned(partial.health_socket);
  let defaults = resolve_defaults(partial.defaults);
  let accounts = resolve_accounts(partial.accounts, &partial.account_order)?;
  Ok(AppConfig {
    log_level,
    health_socket,
    defaults,
    accounts,
  })
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

fn ordered_ids<V>(
  items: &BTreeMap<String, V>,
  order: &[String],
  special_id: &str,
  special_ok: impl Fn(&V) -> bool,
  configured: impl Fn(&V) -> bool,
) -> Vec<String> {
  let mut ordered = Vec::new();
  let mut seen = std::collections::HashSet::new();

  if let Some(special) = items.get(special_id)
    && special_ok(special)
  {
    ordered.push(special_id.into());
    seen.insert(special_id.to_string());
  }

  for id in order {
    if id == special_id || seen.contains(id) {
      continue;
    }
    if items.get(id).is_some_and(&configured) {
      ordered.push(id.clone());
      seen.insert(id.clone());
    }
  }

  for id in items.keys() {
    if id == special_id || seen.contains(id) {
      continue;
    }
    if items.get(id).is_some_and(&configured) {
      ordered.push(id.clone());
      seen.insert(id.clone());
    }
  }

  ordered
}

fn has_token(account: &PartialAccount) -> bool {
  trim_opt(account.token.as_deref()).is_some()
}

fn activity_configured(a: &PartialActivity) -> bool {
  trim_opt(a.name.as_deref()).is_some()
}

fn custom_status_configured(cs: &PartialCustomStatus) -> bool {
  trim_opt(cs.text.as_deref()).is_some()
}

fn resolve_accounts(
  mut accounts: BTreeMap<String, PartialAccount>,
  account_order: &[String],
) -> Result<Vec<AccountConfig>, ConfigError> {
  let ids = ordered_ids(
    &accounts,
    account_order,
    ACCOUNT_FLAT,
    has_token,
    any_account_field_set,
  );
  if ids.is_empty() {
    return Err(ConfigError::NoAccounts);
  }

  let mut out = Vec::with_capacity(ids.len());
  for (i, id) in ids.into_iter().enumerate() {
    let raw = accounts.remove(&id).unwrap_or_default();
    out.push(resolve_one_account(i, raw)?);
  }
  Ok(out)
}

fn resolve_one_account(index: usize, raw: PartialAccount) -> Result<AccountConfig, ConfigError> {
  let name = trim_opt(raw.name.as_deref())
    .map(str::to_string)
    .unwrap_or_else(|| format!("account-{index}"));
  let token = trim_opt(raw.token.as_deref())
    .map(SecretString::new)
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

  let activities = resolve_activities(&name, raw.activities, &raw.activity_order)?;

  Ok(AccountConfig {
    name,
    token,
    kind,
    device,
    status,
    custom_status,
    activities,
  })
}

fn resolve_activities(
  account: &str,
  mut activities: BTreeMap<String, PartialActivity>,
  activity_order: &[String],
) -> Result<Vec<ActivityConfig>, ConfigError> {
  let ids = ordered_ids(
    &activities,
    activity_order,
    ACTIVITY_SINGULAR,
    activity_configured,
    activity_configured,
  );
  let mut out = Vec::with_capacity(ids.len());
  for (i, id) in ids.into_iter().enumerate() {
    let Some(act) = activities.remove(&id) else {
      continue;
    };
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
      .ok_or_else(|| ConfigError::invalid_field(account, field, v)),
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
        .map_err(|_| ConfigError::invalid_field(account, field, v))?;
      Ok(Some(v.to_string()))
    }
  }
}

fn image_asset(image: Option<String>, text: Option<String>) -> ImageAsset {
  ImageAsset {
    image: trim_owned(image),
    text: trim_owned(text),
  }
}

fn activity_button(name: Option<String>, url: Option<String>) -> ActivityButton {
  ActivityButton {
    name: trim_owned(name),
    url: trim_owned(url),
  }
}

fn parse_activity(
  account: &str,
  raw: PartialActivity,
  default_id: &str,
) -> Result<ActivityConfig, ConfigError> {
  let mut activity = ActivityConfig::new();
  activity.name = trim_owned(raw.name);

  if let Some(ty) = trim_opt(raw.activity_type.as_deref()) {
    let parsed = ActivityType::parse(ty)
      .ok_or_else(|| ConfigError::invalid_field(account, "activity type", ty))?;
    if parsed == ActivityType::Custom {
      return Err(ConfigError::invalid(
        account,
        "activity type 'custom' is not valid here; use [custom_status]",
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
  activity.application_id = trim_owned(raw.application_id).unwrap_or_else(|| default_id.into());
  activity.details = trim_owned(raw.details);
  activity.url = trim_owned(raw.url);
  activity.large_image = image_asset(raw.large_image, raw.large_image_text);
  activity.small_image = image_asset(raw.small_image, raw.small_image_text);
  activity.button = activity_button(raw.button, raw.button_url);
  activity.button2 = activity_button(raw.button2, raw.button2_url);
  activity.party = ActivityParty {
    id: trim_owned(raw.party_id).unwrap_or_else(|| default_id.into()),
    current: parse_i64_field(account, "party_current", raw.party_current)?,
    max: parse_i64_field(account, "party_max", raw.party_max)?,
  };

  if activity.activity_type == Some(ActivityType::Streaming)
    && activity.url.as_ref().is_none_or(|u| u.is_empty())
  {
    return Err(ConfigError::invalid(
      account,
      "streaming activity requires a url",
    ));
  }

  Ok(activity)
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::model::partial::{PartialAccount, PartialActivity, PartialCustomStatus};
  use crate::schema::id::{ACCOUNT_FLAT, ACTIVITY_SINGULAR};
  use crate::test_support::*;
  use dka_presence::{AccountKind, ActivityType, Device};

  fn resolve_ok(p: PartialConfig) -> Vec<AccountConfig> {
    resolve_config(p).unwrap().accounts
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

  #[test]
  fn multi_account_order_flat_first() {
    let a = resolve_ok(PartialConfig {
      account_order: vec!["0".into(), ACCOUNT_FLAT.into()],
      accounts: BTreeMap::from([
        (
          ACCOUNT_FLAT.into(),
          PartialAccount {
            name: Some("from-env".into()),
            device: Some("mobile".into()),
            status: Some("dnd".into()),
            ..account_with_token("flat-token")
          },
        ),
        (
          "0".into(),
          PartialAccount {
            name: Some("from-toml".into()),
            device: Some("desktop".into()),
            status: Some("online".into()),
            ..account_with_token("toml-token")
          },
        ),
      ]),
      ..Default::default()
    });
    assert_eq!(a.len(), 2);
    assert_eq!((&*a[0].name, &*a[0].token), ("from-env", "flat-token"));
    assert_eq!(a[0].kind, AccountKind::User);
    assert_eq!(a[0].device, Some(Device::Mobile));
    assert_eq!((&*a[1].name, &*a[1].token), ("from-toml", "toml-token"));
  }

  #[test]
  fn bot_kind_clears_device() {
    let accounts = resolve_ok(partial_flat(PartialAccount {
      kind: Some("bot".into()),
      device: Some("desktop".into()),
      ..account_with_token("bot-token")
    }));
    assert_eq!(accounts[0].kind, AccountKind::Bot);
    assert_eq!(accounts[0].device, None);
  }

  #[test]
  fn streaming_activity_url_rules() {
    assert_invalid_msg(
      "streaming_url",
      flat_with_activities(
        BTreeMap::from([(
          ACTIVITY_SINGULAR.into(),
          PartialActivity {
            activity_type: Some("streaming".into()),
            ..named_activity("live")
          },
        )]),
        vec![ACTIVITY_SINGULAR.into()],
      ),
      "url",
    );

    let acts = &resolve_ok(flat_with_activities(
      BTreeMap::from([(
        ACTIVITY_SINGULAR.into(),
        PartialActivity {
          activity_type: Some("streaming".into()),
          url: Some("https://twitch.tv/example".into()),
          ..named_activity("live")
        },
      )]),
      vec![ACTIVITY_SINGULAR.into()],
    ))[0]
      .activities;
    assert_eq!(acts.len(), 1);
    assert_eq!(acts[0].activity_type, Some(ActivityType::Streaming));
    assert_eq!(acts[0].url.as_deref(), Some("https://twitch.tv/example"));
  }

  #[test]
  fn invalid_field_errors() {
    let cases: &[(&str, PartialConfig, Option<&str>)] = &[
      (
        "kind",
        partial_flat(PartialAccount {
          kind: Some("alien".into()),
          ..account_with_token("t")
        }),
        None,
      ),
      (
        "device",
        partial_flat(PartialAccount {
          device: Some("toaster".into()),
          ..account_with_token("t")
        }),
        None,
      ),
      (
        "status",
        partial_flat(PartialAccount {
          status: Some("busy".into()),
          ..account_with_token("t")
        }),
        None,
      ),
      (
        "custom_activity_type",
        flat_with_activities(
          BTreeMap::from([(
            ACTIVITY_SINGULAR.into(),
            PartialActivity {
              activity_type: Some("custom".into()),
              ..named_activity("x")
            },
          )]),
          vec![ACTIVITY_SINGULAR.into()],
        ),
        Some("custom_status"),
      ),
      (
        "timestamp",
        flat_with_activities(
          BTreeMap::from([(
            ACTIVITY_SINGULAR.into(),
            PartialActivity {
              timestamp: Some("not-a-number".into()),
              ..named_activity("x")
            },
          )]),
          vec![ACTIVITY_SINGULAR.into()],
        ),
        Some("timestamp"),
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
  fn resolve_minimal_and_default_name() {
    let a = resolve_ok(partial_flat(account_with_token("solo")));
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].token, "solo");
    assert_eq!(a[0].name, "account-0");
  }

  #[test]
  fn empty_errors() {
    let err = resolve_config(PartialConfig::default()).unwrap_err();
    assert!(matches!(err, ConfigError::NoAccounts));
  }

  #[test]
  fn flat_without_token_ignored_when_others_exist() {
    let accounts = resolve_ok(PartialConfig {
      account_order: vec![ACCOUNT_FLAT.into(), "0".into()],
      accounts: BTreeMap::from([
        (
          ACCOUNT_FLAT.into(),
          PartialAccount {
            device: Some("desktop".into()),
            status: Some("online".into()),
            ..Default::default()
          },
        ),
        (
          "0".into(),
          PartialAccount {
            name: Some("only".into()),
            ..account_with_token("t")
          },
        ),
      ]),
      ..Default::default()
    });
    assert_eq!(accounts.len(), 1);
    assert_eq!(accounts[0].name, "only");
  }

  #[test]
  fn missing_token_on_named_account() {
    let partial = PartialConfig {
      account_order: vec!["0".into()],
      accounts: BTreeMap::from([(
        "0".into(),
        PartialAccount {
          name: Some("broken".into()),
          token: None,
          device: Some("web".into()),
          ..Default::default()
        },
      )]),
      ..Default::default()
    };
    let err = resolve_config(partial).unwrap_err();
    assert!(matches!(err, ConfigError::MissingToken(ref n) if n == "broken"));
  }

  #[test]
  fn activity_order_and_default_ids() {
    let acts = &resolve_ok(flat_with_activities(
      BTreeMap::from([
        (
          ACTIVITY_SINGULAR.into(),
          PartialActivity {
            activity_type: Some("playing".into()),
            ..named_activity("first")
          },
        ),
        (
          "0".into(),
          PartialActivity {
            activity_type: Some("listening".into()),
            ..named_activity("second")
          },
        ),
        (
          "1".into(),
          PartialActivity {
            activity_type: Some("watching".into()),
            application_id: Some("99".into()),
            party_id: Some("party-x".into()),
            ..named_activity("third")
          },
        ),
      ]),
      vec![ACTIVITY_SINGULAR.into(), "0".into(), "1".into()],
    ))[0]
      .activities;
    assert_eq!(acts.len(), 3);
    let names: Vec<_> = acts.iter().map(|a| a.name.as_deref().unwrap()).collect();
    assert_eq!(names, ["first", "second", "third"]);
    assert_eq!(acts[0].activity_type, Some(ActivityType::Playing));
    assert_eq!(acts[1].activity_type, Some(ActivityType::Listening));
    assert_eq!(acts[2].activity_type, Some(ActivityType::Watching));
    assert_eq!(acts[0].application_id, "1");
    assert_eq!(acts[0].party.id, "1");
    assert_eq!(acts[1].application_id, "2");
    assert_eq!(acts[1].party.id, "2");
    assert_eq!(acts[2].application_id, "99");
    assert_eq!(acts[2].party.id, "party-x");
  }

  #[test]
  fn activity_without_name_skipped() {
    let accounts = resolve_ok(flat_with_activities(
      BTreeMap::from([
        (
          ACTIVITY_SINGULAR.into(),
          PartialActivity {
            details: Some("no name".into()),
            ..Default::default()
          },
        ),
        (
          "0".into(),
          PartialActivity {
            details: Some("also no name".into()),
            ..Default::default()
          },
        ),
        ("1".into(), named_activity("kept")),
      ]),
      vec![ACTIVITY_SINGULAR.into(), "0".into(), "1".into()],
    ));
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
      ..account_with_token("t")
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
      ..account_with_token("t")
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
      ..account_with_token("t")
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
  fn defaults_builtin_when_unset() {
    let cfg = resolve_config(partial_flat(account_with_token("t"))).unwrap();
    assert_eq!(cfg.defaults, product_defaults());
  }

  #[test]
  fn remaining_btree_keys_after_order() {
    let a = resolve_ok(PartialConfig {
      account_order: vec!["b".into()],
      accounts: BTreeMap::from([
        (
          "a".into(),
          PartialAccount {
            name: Some("A".into()),
            ..account_with_token("ta")
          },
        ),
        (
          "b".into(),
          PartialAccount {
            name: Some("B".into()),
            ..account_with_token("tb")
          },
        ),
        (
          "c".into(),
          PartialAccount {
            name: Some("C".into()),
            ..account_with_token("tc")
          },
        ),
      ]),
      ..Default::default()
    });
    assert_eq!(a.len(), 3);
    assert_eq!(a[0].name, "B");
    assert_eq!(a[1].name, "A");
    assert_eq!(a[2].name, "C");
  }
}
