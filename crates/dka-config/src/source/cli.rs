use std::fmt;
use std::path::PathBuf;

use clap::Parser;
use clap::builder::PossibleValuesParser;

use crate::error::ConfigError;
use crate::model::partial::PartialConfig;
use crate::schema::fields::{
  ACCOUNT_KEYS, AccountScalarField, AccountScalars, ActivityField, ClientPropField,
  CustomStatusField, DefaultsProfile, GlobalDefaultField, PartialActivity, PartialCustomStatus,
  ROOT_TABLES, RootField,
};
use crate::schema::id::{ACCOUNT_FLAT, ACTIVITY_SINGULAR};
use crate::schema::path::{
  AccountPath, ConfigPath, account_scalar_path, activity_field_path, apply_path,
  custom_status_path, parse_account_id, parse_activity_id,
};
use crate::source::defaults::LOG_LEVELS;
use crate::util::trim_to_string;

pub const DEFAULT_CONFIG_PATH: &str = "config.toml";

#[derive(Parser)]
#[command(
  name = "discord-keep-alive",
  about = "Keep Discord accounts online with optional presence",
  version
)]
pub struct Cli {
  #[arg(
    long,
    short = 'c',
    global = true,
    help = format!("TOML config path [default: {DEFAULT_CONFIG_PATH}]")
  )]
  pub config: Option<PathBuf>,

  /// Log level.
  #[arg(long, value_parser = PossibleValuesParser::new(LOG_LEVELS), ignore_case = true)]
  pub log_level: Option<String>,

  /// Health socket (Unix) or pipe name (Windows). Empty disables.
  #[arg(long, global = true)]
  pub health_socket: Option<String>,

  #[command(flatten, next_help_heading = "Account")]
  pub account: AccountScalars,

  #[command(flatten, next_help_heading = "Custom status")]
  pub custom_status: PartialCustomStatus,

  #[command(flatten, next_help_heading = "Activity")]
  pub activity: PartialActivity,

  /// Set a field on a specific account: `ID.PATH=VALUE` (e.g. `main.token=AAA`).
  #[arg(
    long = "account-set",
    value_name = "SPEC",
    action = clap::ArgAction::Append,
    help_heading = "Overrides"
  )]
  pub account_set: Vec<String>,

  /// Set any config field: `PATH=VALUE` (e.g. `log_level=debug`).
  #[arg(
    long = "set",
    value_name = "PATH=VALUE",
    action = clap::ArgAction::Append,
    help_heading = "Overrides"
  )]
  pub set: Vec<String>,

  #[command(subcommand)]
  pub command: Option<Command>,
}

impl fmt::Debug for Cli {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let Self {
      config,
      log_level,
      health_socket,
      account,
      custom_status,
      activity,
      account_set,
      set,
      command,
    } = self;
    let redact = |specs: &[String]| specs.iter().map(|s| redacted_spec(s)).collect::<Vec<_>>();
    f.debug_struct("Cli")
      .field("config", config)
      .field("log_level", log_level)
      .field("health_socket", health_socket)
      .field("account", account)
      .field("custom_status", custom_status)
      .field("activity", activity)
      .field("account_set", &redact(account_set))
      .field("set", &redact(set))
      .field("command", command)
      .finish()
  }
}

fn redacted_spec(spec: &str) -> String {
  match spec.split_once('=') {
    Some((path, _)) => format!("{path}=<redacted>"),
    None => "<redacted>".to_string(),
  }
}

#[derive(Debug, Clone, clap::Subcommand)]
pub enum Command {
  /// Check that a running instance is healthy
  ///
  /// Exit codes:
  ///   0  healthy
  ///   1  one or more accounts offline
  ///   2  probe failed (not configured or unreachable)
  #[command(verbatim_doc_comment)]
  Health,
}

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
    apply_path(partial, &ConfigPath::Root(RootField::LogLevel), v);
  }
  if let Some(v) = cli.health_socket.clone() {
    apply_path(partial, &ConfigPath::Root(RootField::HealthSocket), v);
  }

  let mut account = cli.account.clone();
  for &field in AccountScalarField::ALL {
    if let Some(v) = trim_to_string(field.take(&mut account).as_deref()) {
      apply_path(partial, &account_scalar_path(ACCOUNT_FLAT, field), v);
    }
  }
  let mut custom_status = cli.custom_status.clone();
  for &field in CustomStatusField::ALL {
    if let Some(v) = trim_to_string(field.take(&mut custom_status).as_deref()) {
      apply_path(partial, &custom_status_path(ACCOUNT_FLAT, field), v);
    }
  }
  let mut activity = cli.activity.clone();
  for &field in ActivityField::ALL {
    if let Some(v) = trim_to_string(field.take(&mut activity).as_deref()) {
      let path = activity_field_path(ACCOUNT_FLAT, ACTIVITY_SINGULAR, field);
      apply_path(partial, &path, v);
    }
  }
}

fn split_kv(spec: &str) -> Option<(&str, String)> {
  let (path, value) = spec.split_once('=')?;
  (!path.is_empty()).then(|| (path, value.to_string()))
}

pub fn parse_set_spec(spec: &str) -> Result<(ConfigPath, String), ConfigError> {
  let (path, value) = split_kv(spec).ok_or_else(|| malformed_spec("--set", spec, "PATH=VALUE"))?;
  Ok((parse_set_path(path)?, value))
}

fn malformed_spec(flag: &str, spec: &str, expected: &str) -> ConfigError {
  let subject = match split_kv(spec) {
    Some((path, _)) => format!("{flag} '{path}'"),
    None => flag.to_string(),
  };
  ConfigError::InvalidPath(format!("{subject}: expected {expected}"))
}

pub fn parse_account_set_spec(spec: &str) -> Result<(ConfigPath, String), ConfigError> {
  let malformed = || malformed_spec("--account-set", spec, "ID.PATH=VALUE");
  let (path, value) = split_kv(spec).ok_or_else(malformed)?;
  let (id, rest) = path
    .split_once('.')
    .filter(|(_, rest)| !rest.is_empty())
    .ok_or_else(malformed)?;
  let account_id = parse_account_id(id)?;
  let account_path = parse_account_relative_path(rest)?;
  Ok((ConfigPath::Account(account_id, account_path), value))
}

fn account_paths() -> impl Iterator<Item = &'static str> {
  AccountScalarField::keys().chain(ACCOUNT_KEYS)
}

fn root_paths() -> impl Iterator<Item = &'static str> {
  RootField::keys().chain(ROOT_TABLES).chain(account_paths())
}

pub fn parse_set_path(path: &str) -> Result<ConfigPath, ConfigError> {
  let segs: Vec<&str> = path.split('.').collect();
  if let [key] = segs.as_slice()
    && let Some(field) = RootField::from_toml(key)
  {
    return Ok(ConfigPath::Root(field));
  }
  match segs.as_slice() {
    ["defaults", field] => parse_field(
      GlobalDefaultField::from_toml,
      GlobalDefaultField::keys,
      field,
      "defaults field",
    )
    .map(ConfigPath::GlobalDefault),

    ["defaults", profile, field] => {
      let p = parse_defaults_profile(profile)?;
      let f = parse_field(
        ClientPropField::from_toml,
        ClientPropField::keys,
        field,
        "client property",
      )?;
      Ok(ConfigPath::Defaults(p, f))
    }

    ["accounts", id, rest @ ..] if !rest.is_empty() => {
      let account_id = parse_account_id(id)?;
      let relative = rest.join(".");
      let account_path = parse_account_relative_path(&relative)?;
      Ok(ConfigPath::Account(account_id, account_path))
    }

    _ => match parse_account_relative_segs(&segs) {
      Some(Ok(account_path)) => Ok(ConfigPath::Account(ACCOUNT_FLAT.into(), account_path)),
      Some(Err(e)) => Err(e),
      None => Err(ConfigError::unknown("path", path, root_paths())),
    },
  }
}

fn parse_account_relative_path(path: &str) -> Result<AccountPath, ConfigError> {
  let segs: Vec<&str> = path.split('.').collect();
  parse_account_relative_segs(&segs)
    .unwrap_or_else(|| Err(ConfigError::unknown("path", path, account_paths())))
}

fn parse_account_relative_segs(segs: &[&str]) -> Option<Result<AccountPath, ConfigError>> {
  match segs {
    [seg] => parse_account_scalar_token(seg).map(|f| Ok(AccountPath::Scalar(f))),
    ["custom_status", field] => Some(
      parse_field(
        CustomStatusField::from_toml,
        CustomStatusField::keys,
        field,
        "custom_status field",
      )
      .map(AccountPath::CustomStatus),
    ),
    ["activity", field] => {
      Some(parse_activity_field(field).map(|f| AccountPath::Activity(ACTIVITY_SINGULAR.into(), f)))
    }
    ["activity", aid, field] | ["activities", aid, field] => Some(
      parse_activity_id(aid)
        .and_then(|id| parse_activity_field(field).map(|f| AccountPath::Activity(id, f))),
    ),
    _ => None,
  }
}

fn parse_account_scalar_token(raw: &str) -> Option<AccountScalarField> {
  match raw {
    "account" => Some(AccountScalarField::Name),
    _ => AccountScalarField::from_toml(raw),
  }
}

fn parse_field<F, K: Iterator<Item = &'static str>>(
  from_toml: fn(&str) -> Option<F>,
  keys: fn() -> K,
  raw: &str,
  kind: &str,
) -> Result<F, ConfigError> {
  from_toml(raw).ok_or_else(|| ConfigError::unknown(kind, raw, keys()))
}

fn parse_activity_field(raw: &str) -> Result<ActivityField, ConfigError> {
  parse_field(
    ActivityField::from_toml,
    ActivityField::keys,
    raw,
    "activity field",
  )
}

fn parse_defaults_profile(raw: &str) -> Result<DefaultsProfile, ConfigError> {
  DefaultsProfile::ALL
    .iter()
    .copied()
    .find(|p| p.toml() == raw)
    .ok_or_else(|| {
      ConfigError::unknown(
        "defaults profile",
        raw,
        DefaultsProfile::ALL.iter().map(|p| p.toml()),
      )
    })
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
    cli.custom_status.text = Some("brb".into());
    cli.custom_status.emoji = Some("💤".into());
    cli.activity.name = Some("Game".into());
    cli.activity.activity_type = Some("playing".into());
    cli.activity.details = Some("level 1".into());
    cli.log_level = Some("debug".into());
    cli.health_socket = Some("/tmp/cli-h".into());

    let p = cli_partial(&cli).unwrap();
    assert_eq!(p.root.log_level.as_deref(), Some("debug"));
    assert_eq!(p.root.health_socket.as_deref(), Some("/tmp/cli-h"));
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
        assert_eq!(p.root.log_level.as_deref(), Some("debug"));
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
  fn set_unknown_path_lists_roots() {
    let mut cli = empty_cli();
    cli.set = vec!["nope.field=1".into()];
    let err = cli_partial(&cli).unwrap_err();
    assert!(matches!(err, ConfigError::UnknownField(_)), "{err:?}");
    let msg = err.to_string();
    assert!(
      msg.starts_with("unknown path 'nope.field' (expected "),
      "{msg}"
    );
    assert!(msg.contains("log_level"), "{msg}");
  }

  #[test]
  fn malformed_specs_name_their_flag() {
    let msg = |err: ConfigError| err.to_string();
    for (spec, want) in [
      ("log_level", "--set: expected PATH=VALUE"),
      ("=x", "--set: expected PATH=VALUE"),
    ] {
      assert_eq!(msg(parse_set_spec(spec).unwrap_err()), want, "--set {spec}");
    }
    for (spec, want) in [
      ("main=1", "--account-set 'main': expected ID.PATH=VALUE"),
      ("main.=1", "--account-set 'main.': expected ID.PATH=VALUE"),
      ("x", "--account-set: expected ID.PATH=VALUE"),
    ] {
      assert_eq!(
        msg(parse_account_set_spec(spec).unwrap_err()),
        want,
        "--account-set {spec}"
      );
    }
  }

  #[test]
  fn unknown_fields_suggest_the_closest_name() {
    let msg = |path: &str| parse_set_path(path).unwrap_err().to_string();
    assert_eq!(
      msg("activity.nme"),
      "unknown activity field 'nme' (did you mean 'name'?)"
    );
    assert_eq!(msg("tokn"), "unknown path 'tokn' (did you mean 'token'?)");
    assert_eq!(
      msg("defaults.wb.os"),
      "unknown defaults profile 'wb' (did you mean 'web'?)"
    );
  }

  #[test]
  fn enum_flags_reject_unknown_values() {
    let err = Cli::try_parse_from(["dka", "--status", "busy"]).unwrap_err();
    assert_eq!(err.kind(), clap::error::ErrorKind::InvalidValue);
    let cli = Cli::try_parse_from(["dka", "--status", "DND"]).unwrap();
    assert_eq!(cli.account.status.as_deref(), Some("DND"));
  }

  #[test]
  fn config_and_health_socket_apply_before_or_after_subcommand() {
    for args in [
      ["dka", "-c", "a.toml", "--health-socket", "s", "health"],
      ["dka", "health", "-c", "a.toml", "--health-socket", "s"],
    ] {
      let cli = Cli::try_parse_from(args).unwrap();
      assert_eq!(cli.config, Some(PathBuf::from("a.toml")));
      assert_eq!(cli.health_socket.as_deref(), Some("s"));
      assert!(matches!(cli.command, Some(Command::Health)));
    }
  }

  #[test]
  fn cli_partial_omits_unset() {
    let p = cli_partial(&empty_cli()).unwrap();
    assert!(p.root.log_level.is_none());
    assert!(p.root.health_socket.is_none());
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
  fn catalogue_set_suffixes_and_cli_longs_reachable_via_path_parser() {
    for &field in AccountScalarField::ALL {
      let suffix = field.spec().toml;
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
    }
    for &field in CustomStatusField::ALL {
      let suffix = field.spec().toml;
      let path = format!("custom_status.{suffix}");
      let got = parse_set_path(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
      assert_eq!(got, custom_status_path(ACCOUNT_FLAT, field));
      let relative =
        parse_account_relative_path(&path).unwrap_or_else(|e| panic!("relative {path}: {e}"));
      assert_eq!(relative, AccountPath::CustomStatus(field));
    }
    for &field in ActivityField::ALL {
      let suffix = field.spec().toml;
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
    }
  }

  fn parse_flag(long: &str, sample: &str) -> PartialConfig {
    let cli = Cli::try_parse_from(["discord-keep-alive", &format!("--{long}"), sample])
      .unwrap_or_else(|e| panic!("--{long}: {e}"));
    cli_partial(&cli).unwrap_or_else(|e| panic!("partial --{long}: {e}"))
  }

  #[test]
  fn catalogue_cli_longs_parse_and_apply_flat_account() {
    for &field in AccountScalarField::ALL {
      let long = field.spec().cli_long.expect("account flag");
      let sample = account_sample(field);
      let mut acc = parse_flag(long, &sample)
        .accounts
        .get(ACCOUNT_FLAT)
        .cloned()
        .unwrap_or_else(|| panic!("--{long}: no flat account"));
      assert_eq!(
        field.take(&mut acc).as_deref(),
        Some(sample.as_str()),
        "--{long}"
      );
    }
    for &field in CustomStatusField::ALL {
      let long = field.spec().cli_long.expect("custom_status flag");
      let sample = plain_sample(field.spec().toml);
      let mut cs = parse_flag(long, &sample)
        .accounts
        .get(ACCOUNT_FLAT)
        .and_then(|a| a.custom_status.clone())
        .unwrap_or_else(|| panic!("--{long}: no custom_status"));
      assert_eq!(
        field.take(&mut cs).as_deref(),
        Some(sample.as_str()),
        "--{long}"
      );
    }
    for &field in ActivityField::ALL {
      let long = field.spec().cli_long.expect("activity flag");
      let sample = activity_sample(field);
      let mut act = parse_flag(long, &sample)
        .accounts
        .get(ACCOUNT_FLAT)
        .and_then(|a| a.activities.get(ACTIVITY_SINGULAR).cloned())
        .unwrap_or_else(|| panic!("--{long}: no singular activity"));
      assert_eq!(
        field.take(&mut act).as_deref(),
        Some(sample.as_str()),
        "--{long}"
      );
    }
  }

  #[test]
  fn catalogue_defaults_set_paths_reachable() {
    for &profile in DefaultsProfile::ALL {
      for &field in ClientPropField::ALL {
        let path = format!("defaults.{}.{}", profile.toml(), field.spec().toml);
        let got = parse_set_path(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
        assert_eq!(got, ConfigPath::Defaults(profile, field), "{path}");
        let sample = plain_sample(field.spec().toml);
        let mut cli = empty_cli();
        cli.set = vec![format!("{path}={sample}")];
        let mut p = cli_partial(&cli).unwrap_or_else(|e| panic!("{path}: {e}"));
        let props = profile.props_mut(&mut p.defaults);
        assert_eq!(
          field.take(props).as_deref(),
          Some(sample.as_str()),
          "{path}"
        );
      }
    }
  }

  #[test]
  fn catalogue_global_defaults_set_paths_reachable() {
    for &field in GlobalDefaultField::ALL {
      let path = format!("defaults.{}", field.spec().toml);
      let got = parse_set_path(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
      assert_eq!(got, ConfigPath::GlobalDefault(field), "{path}");
      let sample = plain_sample(field.spec().toml);
      let mut cli = empty_cli();
      cli.set = vec![format!("{path}={sample}")];
      let mut p = cli_partial(&cli).unwrap_or_else(|e| panic!("{path}: {e}"));
      assert_eq!(
        field.take(&mut p.defaults.global).as_deref(),
        Some(sample.as_str()),
        "{path}"
      );
    }
    assert!(parse_set_path("defaults.nope").is_err());
  }
}
