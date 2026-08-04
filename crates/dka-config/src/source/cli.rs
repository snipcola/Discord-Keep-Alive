use std::path::PathBuf;

use clap::Parser;

use crate::error::ConfigError;
use crate::model::partial::PartialConfig;
use crate::schema::fields::{
  AccountScalarField, ActivityField, ClientPropField, CustomStatusField, DefaultsProfile,
};
use crate::schema::id::{ACCOUNT_FLAT, ACTIVITY_SINGULAR};
use crate::schema::path::{
  AccountPath, ConfigPath, account_scalar_path, activity_field_path, apply_path,
  custom_status_path, parse_account_id, parse_activity_id,
};
use crate::token::SecretString;
use crate::util::trim_to_string;

pub const DEFAULT_CONFIG_PATH: &str = "config.toml";

// No clap env= bindings; process env is applied only via the env source.
#[derive(Debug, Parser)]
#[command(
  name = "discord-keep-alive",
  about = "Keep Discord accounts online with optional presence"
)]
pub struct Cli {
  /// TOML config path.
  #[arg(long, short = 'c', default_value = DEFAULT_CONFIG_PATH)]
  pub config: PathBuf,

  /// Log level (`error`, `warn`, `info`, `debug`, `trace`).
  #[arg(long)]
  pub log_level: Option<String>,

  /// Health socket (Unix) or pipe name (Windows). Empty disables.
  #[arg(long)]
  pub health_socket: Option<String>,

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

  /// Custom status text (users only).
  #[arg(long = CustomStatusField::Text.cli_long())]
  pub custom_status_text: Option<String>,

  /// Custom status emoji (users only).
  #[arg(long = CustomStatusField::Emoji.cli_long())]
  pub custom_status_emoji: Option<String>,

  /// Flat activity name.
  #[arg(long = ActivityField::Name.cli_long())]
  pub activity: Option<String>,

  /// Activity type (`playing`, `streaming`, `listening`, `watching`, `competing`).
  #[arg(long = ActivityField::Type.cli_long())]
  pub activity_type: Option<String>,

  /// Activity platform string.
  #[arg(long = ActivityField::Platform.cli_long())]
  pub activity_platform: Option<String>,

  /// Activity start time (Unix seconds).
  #[arg(long = ActivityField::Timestamp.cli_long())]
  pub activity_timestamp: Option<String>,

  /// Discord application id.
  #[arg(long = ActivityField::ApplicationId.cli_long())]
  pub activity_application_id: Option<String>,

  /// Activity details line.
  #[arg(long = ActivityField::Details.cli_long())]
  pub activity_details: Option<String>,

  /// Stream URL (required when type is `streaming`).
  #[arg(long = ActivityField::Url.cli_long())]
  pub activity_url: Option<String>,

  /// Large image asset key.
  #[arg(long = ActivityField::LargeImage.cli_long())]
  pub activity_large_image: Option<String>,

  /// Large image hover text.
  #[arg(long = ActivityField::LargeImageText.cli_long())]
  pub activity_large_image_text: Option<String>,

  /// Small image asset key.
  #[arg(long = ActivityField::SmallImage.cli_long())]
  pub activity_small_image: Option<String>,

  /// Small image hover text.
  #[arg(long = ActivityField::SmallImageText.cli_long())]
  pub activity_small_image_text: Option<String>,

  /// Button 1 label.
  #[arg(long = ActivityField::Button.cli_long())]
  pub activity_button: Option<String>,

  /// Button 1 URL.
  #[arg(long = ActivityField::ButtonUrl.cli_long())]
  pub activity_button_url: Option<String>,

  /// Button 2 label.
  #[arg(long = ActivityField::Button2.cli_long())]
  pub activity_button_2: Option<String>,

  /// Button 2 URL.
  #[arg(long = ActivityField::Button2Url.cli_long())]
  pub activity_button_2_url: Option<String>,

  /// Party id.
  #[arg(long = ActivityField::PartyId.cli_long())]
  pub activity_party_id: Option<String>,

  /// Party current size.
  #[arg(long = ActivityField::PartyCurrent.cli_long())]
  pub activity_party_current: Option<String>,

  /// Party max size.
  #[arg(long = ActivityField::PartyMax.cli_long())]
  pub activity_party_max: Option<String>,

  /// Account override: `id.path=value` (e.g. `main.token=AAA`).
  #[arg(long = "account-set", value_name = "SPEC", action = clap::ArgAction::Append)]
  pub account_set: Vec<String>,

  /// Path override: `PATH=VALUE` (e.g. `log_level=debug`).
  #[arg(long = "set", value_name = "PATH=VALUE", action = clap::ArgAction::Append)]
  pub set: Vec<String>,

  #[command(subcommand)]
  pub command: Option<Command>,
}

#[derive(Debug, Clone, clap::Subcommand)]
pub enum Command {
  /// Probe health: exit 0 ok, 1 unhealthy, 2 unreachable.
  Health {
    /// Endpoint for this probe only. Empty disables.
    #[arg(long)]
    health_socket: Option<String>,

    /// Config path when `--health-socket` is omitted.
    #[arg(long, short = 'c', default_value = DEFAULT_CONFIG_PATH)]
    config: PathBuf,
  },
}

const _: () = {
  const fn eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
      return false;
    }
    let mut i = 0;
    while i < a.len() {
      if a[i] != b[i] {
        return false;
      }
      i += 1;
    }
    true
  }
  assert!(eq(AccountScalarField::Name.cli_long(), "account"));
  assert!(eq(ActivityField::Name.cli_long(), "activity"));
  assert!(eq(CustomStatusField::Text.cli_long(), "custom-status-text"));
};

/// CLI partial. Apply order (last wins): flat flags → `--account-set` → `--set`.
pub fn cli_partial(cli: &Cli) -> Result<PartialConfig, ConfigError> {
  let mut partial = PartialConfig::default();

  apply_flat_flags(&mut partial, cli);

  for spec in &cli.account_set {
    let (path, value) = parse_account_set_spec(spec)?;
    apply_path(&mut partial, &path, value);
  }

  for spec in &cli.set {
    let (path, value) = parse_set_spec(spec)?;
    apply_path(&mut partial, &path, value);
  }

  Ok(partial)
}

fn apply_flat_flags(partial: &mut PartialConfig, cli: &Cli) {
  if let Some(v) = trim_to_string(cli.log_level.as_deref()) {
    apply_path(partial, &ConfigPath::LogLevel, v);
  }
  // Keep empty health_socket (disables). Do not drop it via trim.
  if let Some(v) = cli.health_socket.clone() {
    apply_path(partial, &ConfigPath::HealthSocket, v);
  }

  for (field, raw) in [
    (AccountScalarField::Token, cli.token.as_deref()),
    (AccountScalarField::Name, cli.name.as_deref()),
    (AccountScalarField::Kind, cli.kind.as_deref()),
    (AccountScalarField::Device, cli.device.as_deref()),
    (AccountScalarField::Status, cli.status.as_deref()),
  ] {
    if let Some(v) = trim_to_string(raw) {
      apply_path(partial, &account_scalar_path(ACCOUNT_FLAT, field), v);
    }
  }

  for (field, raw) in [
    (CustomStatusField::Text, cli.custom_status_text.as_deref()),
    (CustomStatusField::Emoji, cli.custom_status_emoji.as_deref()),
  ] {
    if let Some(v) = trim_to_string(raw) {
      apply_path(partial, &custom_status_path(ACCOUNT_FLAT, field), v);
    }
  }

  for (field, raw) in [
    (ActivityField::Name, cli.activity.as_deref()),
    (ActivityField::Type, cli.activity_type.as_deref()),
    (ActivityField::Platform, cli.activity_platform.as_deref()),
    (ActivityField::Timestamp, cli.activity_timestamp.as_deref()),
    (
      ActivityField::ApplicationId,
      cli.activity_application_id.as_deref(),
    ),
    (ActivityField::Details, cli.activity_details.as_deref()),
    (ActivityField::Url, cli.activity_url.as_deref()),
    (
      ActivityField::LargeImage,
      cli.activity_large_image.as_deref(),
    ),
    (
      ActivityField::LargeImageText,
      cli.activity_large_image_text.as_deref(),
    ),
    (
      ActivityField::SmallImage,
      cli.activity_small_image.as_deref(),
    ),
    (
      ActivityField::SmallImageText,
      cli.activity_small_image_text.as_deref(),
    ),
    (ActivityField::Button, cli.activity_button.as_deref()),
    (ActivityField::ButtonUrl, cli.activity_button_url.as_deref()),
    (ActivityField::Button2, cli.activity_button_2.as_deref()),
    (
      ActivityField::Button2Url,
      cli.activity_button_2_url.as_deref(),
    ),
    (ActivityField::PartyId, cli.activity_party_id.as_deref()),
    (
      ActivityField::PartyCurrent,
      cli.activity_party_current.as_deref(),
    ),
    (ActivityField::PartyMax, cli.activity_party_max.as_deref()),
  ] {
    if let Some(v) = trim_to_string(raw) {
      apply_path(
        partial,
        &activity_field_path(ACCOUNT_FLAT, ACTIVITY_SINGULAR, field),
        v,
      );
    }
  }
}

fn split_kv(spec: &str) -> Result<(&str, String), ConfigError> {
  let Some((path, value)) = spec.split_once('=') else {
    return Err(ConfigError::InvalidPath(format!(
      "expected PATH=VALUE, got '{spec}'"
    )));
  };
  if path.is_empty() {
    return Err(ConfigError::InvalidPath(format!("empty path in '{spec}'")));
  }
  Ok((path, value.to_string()))
}

pub fn parse_set_spec(spec: &str) -> Result<(ConfigPath, String), ConfigError> {
  let (path, value) = split_kv(spec)?;
  Ok((parse_set_path(path)?, value))
}

pub fn parse_account_set_spec(spec: &str) -> Result<(ConfigPath, String), ConfigError> {
  let (path, value) = split_kv(spec)?;
  let Some((id, rest)) = path.split_once('.') else {
    return Err(ConfigError::InvalidPath(format!(
      "account-set expects id.path=value, got '{spec}'"
    )));
  };
  if rest.is_empty() {
    return Err(ConfigError::InvalidPath(format!(
      "account-set missing path after id in '{spec}'"
    )));
  }
  let account_id = parse_account_id(id)?;
  let account_path = parse_account_relative_path(rest)?;
  Ok((ConfigPath::Account(account_id, account_path), value))
}

const PATH_ROOTS_HELP: &str = "allowed roots: log_level, health_socket, token/account/name/kind/device/status, \
   custom_status.*, activity.*, activities.*, defaults.*, accounts.*";

const ACCOUNT_RELATIVE_HELP: &str =
  "account-relative: name/token/kind/device/status, custom_status.*, activity.*, activities.*";

pub fn parse_set_path(path: &str) -> Result<ConfigPath, ConfigError> {
  let segs: Vec<&str> = path.split('.').collect();
  match segs.as_slice() {
    ["log_level"] => Ok(ConfigPath::LogLevel),
    ["health_socket"] => Ok(ConfigPath::HealthSocket),

    ["defaults", profile, field] => {
      let p = parse_defaults_profile(profile)?;
      let f = parse_client_prop_field(field)?;
      Ok(ConfigPath::Defaults(p, f))
    }

    ["accounts", id, rest @ ..] if !rest.is_empty() => {
      let account_id = parse_account_id(id)?;
      let relative = rest.join(".");
      let account_path = parse_account_relative_path(&relative)?;
      Ok(ConfigPath::Account(account_id, account_path))
    }

    // Bare paths (token, activity.name, …) attach to the flat account.
    _ => match parse_account_relative_segs(&segs) {
      Some(Ok(account_path)) => Ok(ConfigPath::Account(ACCOUNT_FLAT.into(), account_path)),
      Some(Err(e)) => Err(e),
      None => Err(ConfigError::InvalidPath(format!(
        "{path} ({PATH_ROOTS_HELP})"
      ))),
    },
  }
}

fn parse_account_relative_path(path: &str) -> Result<AccountPath, ConfigError> {
  let segs: Vec<&str> = path.split('.').collect();
  match parse_account_relative_segs(&segs) {
    Some(Ok(p)) => Ok(p),
    Some(Err(e)) => Err(e),
    None => Err(ConfigError::InvalidPath(format!(
      "{path} ({ACCOUNT_RELATIVE_HELP})"
    ))),
  }
}

/// Account-relative path segments. `None` = not this grammar;
/// `Some(Err)` keeps bad field/id errors distinct from unknown roots.
fn parse_account_relative_segs(segs: &[&str]) -> Option<Result<AccountPath, ConfigError>> {
  match segs {
    [seg] if parse_account_scalar_token(seg).is_some() => Some(Ok(AccountPath::Scalar(
      parse_account_scalar_token(seg).expect("checked"),
    ))),
    ["custom_status", field] => {
      Some(parse_custom_status_field(field).map(AccountPath::CustomStatus))
    }
    ["activity", field] if parse_activity_field(field).is_ok() => Some(Ok(AccountPath::Activity(
      ACTIVITY_SINGULAR.into(),
      parse_activity_field(field).expect("checked"),
    ))),
    ["activity", aid, field] => Some(
      parse_activity_id(aid)
        .and_then(|id| parse_activity_field(field).map(|f| AccountPath::Activity(id, f))),
    ),
    ["activities", aid, field] => Some(
      parse_activity_id(aid)
        .and_then(|id| parse_activity_field(field).map(|f| AccountPath::Activity(id, f))),
    ),
    _ => None,
  }
}

/// Catalog set_suffix, or the CLI long `account` → Name.
fn parse_account_scalar_token(raw: &str) -> Option<AccountScalarField> {
  if raw == "account" {
    return Some(AccountScalarField::Name);
  }
  AccountScalarField::ALL
    .iter()
    .find(|&&f| f.spec().set_suffix == raw)
    .copied()
}

fn parse_custom_status_field(raw: &str) -> Result<CustomStatusField, ConfigError> {
  CustomStatusField::ALL
    .iter()
    .find(|&&f| f.spec().set_suffix == raw)
    .copied()
    .ok_or_else(|| ConfigError::UnknownField(format!("custom_status.{raw}")))
}

fn parse_activity_field(raw: &str) -> Result<ActivityField, ConfigError> {
  ActivityField::ALL
    .iter()
    .find(|&&f| f.spec().set_suffix == raw)
    .copied()
    .ok_or_else(|| ConfigError::UnknownField(format!("activity field '{raw}'")))
}

fn parse_client_prop_field(raw: &str) -> Result<ClientPropField, ConfigError> {
  ClientPropField::ALL
    .iter()
    .find(|&&f| f.spec().set_suffix == raw)
    .copied()
    .ok_or_else(|| ConfigError::UnknownField(format!("client prop '{raw}'")))
}

fn parse_defaults_profile(raw: &str) -> Result<DefaultsProfile, ConfigError> {
  for &p in DefaultsProfile::ALL {
    if p.toml() == raw {
      return Ok(p);
    }
  }
  Err(ConfigError::UnknownField(format!(
    "defaults profile '{raw}'"
  )))
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::schema::id::{ACCOUNT_FLAT, ACTIVITY_SINGULAR};
  use crate::test_support::*;

  #[test]
  fn flat_cli_flags_to_flat_account() {
    let mut cli = empty_cli();
    cli.token = Some("cli-tok".into());
    cli.name = Some("cli-name".into());
    cli.kind = Some("user".into());
    cli.device = Some("mobile".into());
    cli.status = Some("dnd".into());
    cli.custom_status_text = Some("brb".into());
    cli.custom_status_emoji = Some("💤".into());
    cli.activity = Some("Game".into());
    cli.activity_type = Some("playing".into());
    cli.activity_details = Some("level 1".into());
    cli.log_level = Some("debug".into());
    cli.health_socket = Some("/tmp/cli-h".into());

    let p = cli_partial(&cli).unwrap();
    assert_eq!(p.log_level.as_deref(), Some("debug"));
    assert_eq!(p.health_socket.as_deref(), Some("/tmp/cli-h"));
    let acc = p.accounts.get(ACCOUNT_FLAT).unwrap();
    assert_eq!(acc.token.as_deref(), Some("cli-tok"));
    assert_eq!(acc.name.as_deref(), Some("cli-name"));
    assert_eq!(acc.kind.as_deref(), Some("user"));
    assert_eq!(acc.device.as_deref(), Some("mobile"));
    assert_eq!(acc.status.as_deref(), Some("dnd"));
    assert_eq!(
      acc.custom_status.as_ref().and_then(|c| c.text.as_deref()),
      Some("brb")
    );
    assert_eq!(
      acc.custom_status.as_ref().and_then(|c| c.emoji.as_deref()),
      Some("💤")
    );
    let act = acc.activities.get(ACTIVITY_SINGULAR).unwrap();
    assert_eq!(act.name.as_deref(), Some("Game"));
    assert_eq!(act.activity_type.as_deref(), Some("playing"));
    assert_eq!(act.details.as_deref(), Some("level 1"));
    assert_eq!(p.account_order, vec![ACCOUNT_FLAT.to_string()]);
  }

  #[test]
  fn set_path_forms() {
    type Case = (
      &'static str,
      &'static [&'static str],
      &'static [&'static str],
      fn(&PartialConfig),
    );
    let cases: &[Case] = &[
      ("log_level", &["log_level=debug"], &[], |p| {
        assert_eq!(p.log_level.as_deref(), Some("debug"));
      }),
      (
        "accounts_main_token",
        &["accounts.main.token=x"],
        &[],
        |p| {
          assert_eq!(token_of(p, "main"), Some("x"));
          assert_eq!(p.account_order, vec!["main".to_string()]);
        },
      ),
      ("account_set_main_status", &[], &["main.status=dnd"], |p| {
        assert_eq!(status_of(p, "main"), Some("dnd"));
      }),
      (
        "activities_foo_name",
        &["activities.foo.name=Rust"],
        &[],
        |p| {
          let flat = p.accounts.get(ACCOUNT_FLAT).unwrap();
          assert_eq!(
            flat.activities.get("foo").and_then(|a| a.name.as_deref()),
            Some("Rust")
          );
        },
      ),
      ("defaults_web_os", &["defaults.web.os=Linux"], &[], |p| {
        assert_eq!(p.defaults.web.os.as_deref(), Some("Linux"));
      }),
      (
        "set_value_with_extra_eq",
        &["activity.details=a=b=c"],
        &[],
        |p| {
          let act = p
            .accounts
            .get(ACCOUNT_FLAT)
            .and_then(|a| a.activities.get(ACTIVITY_SINGULAR))
            .unwrap();
          assert_eq!(act.details.as_deref(), Some("a=b=c"));
        },
      ),
    ];
    for &(label, sets, account_sets, check) in cases {
      let mut cli = empty_cli();
      cli.set = sets.iter().map(|s| (*s).to_string()).collect();
      cli.account_set = account_sets.iter().map(|s| (*s).to_string()).collect();
      let p = cli_partial(&cli).unwrap_or_else(|e| panic!("{label}: {e}"));
      check(&p);
    }
  }

  #[test]
  fn account_set_activity_forms() {
    let mut cli = empty_cli();
    cli.account_set = vec![
      "main.activities.game.type=playing".into(),
      "main.activity.game.details=d".into(),
      "main.activity.name=Singular".into(),
      "0.status=idle".into(),
    ];
    let p = cli_partial(&cli).unwrap();
    let main = p.accounts.get("main").unwrap();
    assert_eq!(
      main
        .activities
        .get("game")
        .and_then(|a| a.activity_type.as_deref()),
      Some("playing")
    );
    assert_eq!(
      main
        .activities
        .get("game")
        .and_then(|a| a.details.as_deref()),
      Some("d")
    );
    assert_eq!(
      main
        .activities
        .get(ACTIVITY_SINGULAR)
        .and_then(|a| a.name.as_deref()),
      Some("Singular")
    );
    assert_eq!(status_of(&p, "0"), Some("idle"));
  }

  #[test]
  fn set_overrides_flat_last_wins() {
    let mut cli = empty_cli();
    cli.status = Some("online".into());
    cli.set = vec!["status=dnd".into()];
    let p = cli_partial(&cli).unwrap();
    assert_eq!(status_of(&p, ACCOUNT_FLAT), Some("dnd"));
  }

  #[test]
  fn set_unknown_path_errors() {
    let mut cli = empty_cli();
    cli.set = vec!["nope.field=1".into()];
    let err = cli_partial(&cli).unwrap_err();
    assert!(matches!(err, ConfigError::InvalidPath(_)));
  }

  #[test]
  fn cli_partial_omits_unset() {
    let p = cli_partial(&empty_cli()).unwrap();
    assert!(p.log_level.is_none());
    assert!(p.health_socket.is_none());
    assert!(p.accounts.is_empty());
  }

  #[test]
  fn cli_debug_redacts_token() {
    let mut cli = empty_cli();
    cli.token = Some("super-secret-token".into());
    let debug = format!("{cli:?}");
    assert!(debug.contains("<redacted>"), "{debug}");
    assert!(!debug.contains("super-secret-token"), "{debug}");
  }

  #[test]
  fn catalog_set_suffixes_and_cli_longs_reachable_via_path_parser() {
    for_each_catalog_field(
      |field| {
        let suffix = field.spec().set_suffix;
        let path = parse_set_path(suffix).unwrap_or_else(|e| panic!("{suffix}: {e}"));
        assert_eq!(path, account_scalar_path(ACCOUNT_FLAT, field));
        let relative =
          parse_account_relative_path(suffix).unwrap_or_else(|e| panic!("relative {suffix}: {e}"));
        assert_eq!(relative, AccountPath::Scalar(field));
        if let Some(cli_long) = field.spec().cli_long {
          let flat_token = if cli_long == "account" {
            "account"
          } else {
            suffix
          };
          let path = parse_set_path(flat_token).unwrap_or_else(|e| panic!("{flat_token}: {e}"));
          assert_eq!(path, account_scalar_path(ACCOUNT_FLAT, field));
        }
      },
      |field| {
        let suffix = field.spec().set_suffix;
        let path = format!("custom_status.{suffix}");
        let got = parse_set_path(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
        assert_eq!(got, custom_status_path(ACCOUNT_FLAT, field));
        let relative =
          parse_account_relative_path(&path).unwrap_or_else(|e| panic!("relative {path}: {e}"));
        assert_eq!(relative, AccountPath::CustomStatus(field));
      },
      |field| {
        let suffix = field.spec().set_suffix;
        let path = format!("activity.{suffix}");
        let got = parse_set_path(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
        assert_eq!(
          got,
          activity_field_path(ACCOUNT_FLAT, ACTIVITY_SINGULAR, field)
        );
        let relative =
          parse_account_relative_path(&path).unwrap_or_else(|e| panic!("relative {path}: {e}"));
        assert_eq!(
          relative,
          AccountPath::Activity(ACTIVITY_SINGULAR.into(), field)
        );
        let path = format!("activities.foo.{suffix}");
        let got = parse_set_path(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
        assert_eq!(got, activity_field_path(ACCOUNT_FLAT, "foo", field));
      },
    );
  }

  #[test]
  fn invalid_path_mentions_allowed_roots() {
    let err = parse_set_path("nope.field").unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("allowed roots"), "{msg}");
    assert!(msg.contains("log_level"), "{msg}");
  }
}
