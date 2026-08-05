use std::path::PathBuf;

use clap::Parser;

use crate::error::ConfigError;
use crate::model::partial::PartialConfig;
use crate::schema::fields::{
  AccountScalarCliArgs, AccountScalarField, ActivityCliArgs, ActivityField, ClientPropField,
  CustomStatusCliArgs, CustomStatusField, DefaultsProfile, apply_account_scalar_cli,
  apply_activity_cli, apply_custom_status_cli,
};
use crate::schema::id::{ACCOUNT_FLAT, ACTIVITY_SINGULAR};
use crate::schema::path::{
  AccountPath, ConfigPath, apply_path, parse_account_id, parse_activity_id,
};
use crate::util::trim_to_string;

pub const DEFAULT_CONFIG_PATH: &str = "config.toml";

// No clap env=; process env is the env source only.
// Flat leaves: flatten Args groups (macros cannot expand into struct fields).
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

  #[command(flatten)]
  pub account: AccountScalarCliArgs,

  #[command(flatten)]
  pub custom_status: CustomStatusCliArgs,

  #[command(flatten)]
  pub activity: ActivityCliArgs,

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

  apply_account_scalar_cli(partial, &cli.account);
  apply_custom_status_cli(partial, &cli.custom_status);
  apply_activity_cli(partial, &cli.activity);
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
    [seg] => parse_account_scalar_token(seg).map(|f| Ok(AccountPath::Scalar(f))),
    ["custom_status", field] => {
      Some(parse_custom_status_field(field).map(AccountPath::CustomStatus))
    }
    ["activity", field] => match parse_activity_field(field) {
      Ok(f) => Some(Ok(AccountPath::Activity(ACTIVITY_SINGULAR.into(), f))),
      Err(_) => None,
    },
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

fn parse_by_set_suffix<F: Copy>(
  all: &[F],
  raw: &str,
  set_suffix: impl Fn(F) -> &'static str,
  unknown: impl FnOnce() -> String,
) -> Result<F, ConfigError> {
  all
    .iter()
    .find(|&&f| set_suffix(f) == raw)
    .copied()
    .ok_or_else(|| ConfigError::UnknownField(unknown()))
}

fn parse_custom_status_field(raw: &str) -> Result<CustomStatusField, ConfigError> {
  parse_by_set_suffix(
    CustomStatusField::ALL,
    raw,
    |f| f.spec().set_suffix,
    || format!("custom_status.{raw}"),
  )
}

fn parse_activity_field(raw: &str) -> Result<ActivityField, ConfigError> {
  parse_by_set_suffix(
    ActivityField::ALL,
    raw,
    |f| f.spec().set_suffix,
    || format!("activity field '{raw}'"),
  )
}

fn parse_client_prop_field(raw: &str) -> Result<ClientPropField, ConfigError> {
  parse_by_set_suffix(
    ClientPropField::ALL,
    raw,
    |f| f.spec().set_suffix,
    || format!("client prop '{raw}'"),
  )
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
  use crate::schema::path::{account_scalar_path, activity_field_path, custom_status_path};
  use crate::test_support::*;

  #[test]
  fn flat_cli_flags_to_flat_account() {
    let mut cli = empty_cli();
    cli.account.token = Some("cli-tok".into());
    cli.account.name = Some("cli-name".into());
    cli.account.kind = Some("user".into());
    cli.account.device = Some("mobile".into());
    cli.account.status = Some("dnd".into());
    cli.custom_status.custom_status = Some("brb".into());
    cli.custom_status.custom_status_emoji = Some("💤".into());
    cli.activity.activity = Some("Game".into());
    cli.activity.activity_type = Some("playing".into());
    cli.activity.activity_details = Some("level 1".into());
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
    cli.account.status = Some("online".into());
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
    cli.account.token = Some("super-secret-token".into());
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

  /// Every catalog `cli_long` is a real clap long and applies to the flat account only.
  #[test]
  fn catalog_cli_longs_parse_and_apply_flat_account() {
    for_each_catalog_field(
      |field| {
        let Some(long) = field.spec().cli_long else {
          return;
        };
        let sample = catalog_sample_value(field.spec().set_suffix);
        let cli = Cli::try_parse_from(["discord-keep-alive", &format!("--{long}"), &sample])
          .unwrap_or_else(|e| panic!("--{long}: {e}"));
        let p = cli_partial(&cli).unwrap_or_else(|e| panic!("partial --{long}: {e}"));
        let mut acc = p
          .accounts
          .get(ACCOUNT_FLAT)
          .cloned()
          .unwrap_or_else(|| panic!("--{long}: no flat account"));
        assert_eq!(
          field.take(&mut acc).as_deref(),
          Some(sample.as_str()),
          "--{long}"
        );
      },
      |field| {
        let long = field.cli_long();
        let sample = catalog_sample_value(field.spec().set_suffix);
        let cli = Cli::try_parse_from(["discord-keep-alive", &format!("--{long}"), &sample])
          .unwrap_or_else(|e| panic!("--{long}: {e}"));
        let p = cli_partial(&cli).unwrap_or_else(|e| panic!("partial --{long}: {e}"));
        let mut cs = p
          .accounts
          .get(ACCOUNT_FLAT)
          .and_then(|a| a.custom_status.clone())
          .unwrap_or_else(|| panic!("--{long}: no custom_status"));
        assert_eq!(
          field.get_mut(&mut cs).as_deref(),
          Some(sample.as_str()),
          "--{long}"
        );
      },
      |field| {
        let long = field.cli_long();
        let sample = catalog_sample_value(field.spec().set_suffix);
        let cli = Cli::try_parse_from(["discord-keep-alive", &format!("--{long}"), &sample])
          .unwrap_or_else(|e| panic!("--{long}: {e}"));
        let p = cli_partial(&cli).unwrap_or_else(|e| panic!("partial --{long}: {e}"));
        let mut act = p
          .accounts
          .get(ACCOUNT_FLAT)
          .and_then(|a| a.activities.get(ACTIVITY_SINGULAR).cloned())
          .unwrap_or_else(|| panic!("--{long}: no singular activity"));
        assert_eq!(
          field.get_mut(&mut act).as_deref(),
          Some(sample.as_str()),
          "--{long}"
        );
      },
    );
  }

  #[test]
  fn catalog_defaults_set_paths_reachable() {
    for_each_defaults_field(|profile, field| {
      let path = format!("defaults.{}.{}", profile.toml(), field.spec().set_suffix);
      let got = parse_set_path(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
      assert_eq!(got, ConfigPath::Defaults(profile, field), "{path}");
      let sample = catalog_sample_value(field.spec().set_suffix);
      let mut cli = empty_cli();
      cli.set = vec![format!("{path}={sample}")];
      let mut p = cli_partial(&cli).unwrap_or_else(|e| panic!("{path}: {e}"));
      let props = profile.props_mut(&mut p.defaults);
      assert_eq!(
        field.get_mut(props).as_deref(),
        Some(sample.as_str()),
        "{path}"
      );
    });
  }

  #[test]
  fn invalid_path_mentions_allowed_roots() {
    let err = parse_set_path("nope.field").unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("allowed roots"), "{msg}");
    assert!(msg.contains("log_level"), "{msg}");
  }
}
