//! Layer order (later wins): defaults, file, env, CLI, then resolve.
//! Effective priority: CLI > env > file > defaults.
//! RUST_LOG only affects logging setup; it does not override log_level in config.

mod env;
mod file;
mod merge;
mod partial;
mod resolve;
mod schema;
mod token;

use std::path::{Path, PathBuf};

use clap::Parser;
use dka_presence::{AccountKind, ActivityConfig, CustomStatusConfig, Device, Status};
use thiserror::Error;

use dka_gateway::properties::Defaults;

use merge::merge_partial;
use partial::{
  PartialAccount, PartialActivity, PartialConfig, PartialCustomStatus, any_activity_field_set,
  any_custom_status_field_set,
};
use schema::{AccountScalarField, ActivityField, CustomStatusField};

const DEFAULT_CONFIG_PATH: &str = "config.toml";
pub(crate) const DEFAULT_LOG_LEVEL: &str = "info";

pub(crate) fn trim_nonempty(s: &str) -> Option<&str> {
  let t = s.trim();
  (!t.is_empty()).then_some(t)
}

pub(crate) fn trim_opt(s: Option<&str>) -> Option<&str> {
  s.and_then(trim_nonempty)
}

pub(crate) fn trim_owned(s: Option<String>) -> Option<String> {
  s.and_then(|v| trim_nonempty(&v).map(str::to_string))
}

fn trim_to_string(s: Option<&str>) -> Option<String> {
  trim_opt(s).map(str::to_string)
}

// CLI flags do not use clap env=; the env provider owns process env.
// Flag names match schema cli_long (see asserts below).
#[derive(Debug, Parser)]
#[command(
  name = "discord-keep-alive",
  about = "Keep Discord accounts online with optional presence"
)]
pub struct Cli {
  /// Path to the TOML config file.
  #[arg(long, short = 'c', default_value = DEFAULT_CONFIG_PATH)]
  pub config: PathBuf,

  /// Log level: error, warn, info, debug, or trace.
  #[arg(long)]
  pub log_level: Option<String>,

  /// Health socket path (Unix) or pipe name (Windows). Empty disables health.
  #[arg(long)]
  pub health_socket: Option<String>,

  /// Account token for a single flat account.
  #[arg(long = "token")]
  pub token: Option<token::SecretString>,

  #[arg(long = "account")]
  pub name: Option<String>,

  /// Account kind: user or bot.
  #[arg(long = "kind")]
  pub kind: Option<String>,

  /// Client device for user accounts: desktop, web, or mobile.
  #[arg(long = "device")]
  pub device: Option<String>,

  /// Presence status: online, idle, dnd, or invisible.
  #[arg(long = "status")]
  pub status: Option<String>,

  /// Custom status text (user accounts only).
  #[arg(long = "custom-status-text")]
  pub custom_status_text: Option<String>,

  /// Custom status emoji (user accounts only).
  #[arg(long = "custom-status-emoji")]
  pub custom_status_emoji: Option<String>,

  #[arg(long = "activity")]
  pub activity: Option<String>,

  /// Activity type: playing, streaming, listening, watching, or competing.
  #[arg(long = "activity-type")]
  pub activity_type: Option<String>,

  #[arg(long = "activity-platform")]
  pub activity_platform: Option<String>,

  /// Activity start time as Unix seconds.
  #[arg(long = "activity-timestamp")]
  pub activity_timestamp: Option<String>,

  #[arg(long = "activity-application-id")]
  pub activity_application_id: Option<String>,

  #[arg(long = "activity-details")]
  pub activity_details: Option<String>,

  /// Stream URL (required when type is streaming).
  #[arg(long = "activity-url")]
  pub activity_url: Option<String>,

  #[arg(long = "activity-large-image")]
  pub activity_large_image: Option<String>,

  /// Hover text for the large image.
  #[arg(long = "activity-large-image-text")]
  pub activity_large_image_text: Option<String>,

  #[arg(long = "activity-small-image")]
  pub activity_small_image: Option<String>,

  /// Hover text for the small image.
  #[arg(long = "activity-small-image-text")]
  pub activity_small_image_text: Option<String>,

  #[arg(long = "activity-button")]
  pub activity_button: Option<String>,

  #[arg(long = "activity-button-url")]
  pub activity_button_url: Option<String>,

  #[arg(long = "activity-button-2")]
  pub activity_button_2: Option<String>,

  #[arg(long = "activity-button-2-url")]
  pub activity_button_2_url: Option<String>,

  #[arg(long = "activity-party-id")]
  pub activity_party_id: Option<String>,

  #[arg(long = "activity-party-current")]
  pub activity_party_current: Option<String>,

  #[arg(long = "activity-party-max")]
  pub activity_party_max: Option<String>,

  #[command(subcommand)]
  pub command: Option<Command>,
}

#[derive(Debug, Clone, clap::Subcommand)]
pub enum Command {
  /// Check health and exit 0 (ok), 1 (unhealthy), or 2 (unreachable).
  Health {
    /// Health endpoint for this probe only. Empty disables.
    #[arg(long)]
    health_socket: Option<String>,

    /// Config file used when --health-socket is omitted.
    #[arg(long, short = 'c', default_value = DEFAULT_CONFIG_PATH)]
    config: PathBuf,
  },
}

#[derive(Debug, Clone)]
pub struct AppConfig {
  pub log_level: String,
  pub health_socket: Option<String>,
  pub defaults: Defaults,
  pub accounts: Vec<AccountConfig>,
}

#[derive(Debug, Clone)]
pub struct AccountConfig {
  pub name: String,
  pub token: token::SecretString,
  pub kind: AccountKind,
  pub device: Option<Device>,
  pub status: Option<Status>,
  pub custom_status: Option<CustomStatusConfig>,
  pub activities: Vec<ActivityConfig>,
}

#[derive(Debug, Error)]
pub enum ConfigError {
  #[error("failed to load config: {0}")]
  Figment(#[source] Box<figment::Error>),
  #[error("config file not found: {0}")]
  ConfigNotFound(PathBuf),
  #[error("no accounts configured (set TOKEN, ACCOUNT_N_TOKEN, or [[accounts]] in config)")]
  NoAccounts,
  #[error("account '{0}': token is required")]
  MissingToken(String),
  #[error("account '{0}': {1}")]
  Invalid(String, String),
}

impl From<figment::Error> for ConfigError {
  fn from(err: figment::Error) -> Self {
    Self::Figment(Box::new(err))
  }
}

fn defaults_partial() -> PartialConfig {
  PartialConfig {
    log_level: Some(DEFAULT_LOG_LEVEL.into()),
    health_socket: None,
    ..Default::default()
  }
}

// Clap names that differ from the field name (other names are covered by tests).
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

fn cli_partial(cli: &Cli) -> PartialConfig {
  let mut partial = PartialConfig {
    log_level: trim_to_string(cli.log_level.as_deref()),
    health_socket: cli.health_socket.clone(),
    ..Default::default()
  };

  let mut account = PartialAccount::default();
  if let Some(v) = trim_to_string(cli.token.as_deref()) {
    AccountScalarField::Token.set(&mut account, v);
  }
  for (field, raw) in [
    (AccountScalarField::Name, cli.name.as_deref()),
    (AccountScalarField::Kind, cli.kind.as_deref()),
    (AccountScalarField::Device, cli.device.as_deref()),
    (AccountScalarField::Status, cli.status.as_deref()),
  ] {
    set_cli_field(field.get_mut(&mut account), raw);
  }

  let mut custom = PartialCustomStatus::default();
  for (field, raw) in [
    (CustomStatusField::Text, cli.custom_status_text.as_deref()),
    (CustomStatusField::Emoji, cli.custom_status_emoji.as_deref()),
  ] {
    set_cli_field(field.get_mut(&mut custom), raw);
  }
  if any_custom_status_field_set(&custom) {
    account.custom_status = Some(custom);
  }

  let mut activity = PartialActivity::default();
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
    set_cli_field(field.get_mut(&mut activity), raw);
  }
  if any_activity_field_set(&activity) {
    account.activity = Some(activity);
  }

  partial.account = account;
  partial
}

fn set_cli_field(dst: &mut Option<String>, raw: Option<&str>) {
  if let Some(v) = trim_to_string(raw) {
    *dst = Some(v);
  }
}

// Prefer an explicit --config path; otherwise honor CONFIG_PATH when set.
fn resolve_config_path_arg(cli_path: &Path) -> PathBuf {
  resolve_config_path(cli_path, std::env::var_os("CONFIG_PATH"))
}

fn resolve_config_path(cli_path: &Path, config_path_env: Option<std::ffi::OsString>) -> PathBuf {
  if cli_path.as_os_str() != DEFAULT_CONFIG_PATH {
    return cli_path.to_path_buf();
  }
  match config_path_env {
    Some(p) if !p.is_empty() => PathBuf::from(p),
    _ => PathBuf::from(DEFAULT_CONFIG_PATH),
  }
}

fn is_default_config_path(path: &Path) -> bool {
  path.as_os_str() == DEFAULT_CONFIG_PATH
}

fn load_file_layer(path: &Path) -> Result<PartialConfig, ConfigError> {
  if path.exists() {
    file::load_toml(path)
  } else if is_default_config_path(path) {
    Ok(PartialConfig::default())
  } else {
    Err(ConfigError::ConfigNotFound(path.to_path_buf()))
  }
}

pub fn load(cli: &Cli) -> Result<AppConfig, ConfigError> {
  let path = resolve_config_path_arg(&cli.config);
  load_with(&path, env::from_env(), cli_partial(cli))
}

pub fn load_with(
  config_path: &Path,
  env_layer: PartialConfig,
  cli_layer: PartialConfig,
) -> Result<AppConfig, ConfigError> {
  let mut partial = defaults_partial();
  merge_partial(&mut partial, load_file_layer(config_path)?);
  merge_partial(&mut partial, env_layer);
  merge_partial(&mut partial, cli_layer);

  let (log_level, health_socket, defaults, accounts) = resolve::resolve_config(partial)?;
  Ok(AppConfig {
    log_level,
    health_socket,
    defaults,
    accounts,
  })
}

// Health endpoint only (accounts not required). Precedence: CLI, env, file.
// An empty CLI value turns health off.
pub fn load_health_endpoint(
  config_path: &Path,
  cli_override: Option<&str>,
) -> Result<Option<String>, ConfigError> {
  let path = resolve_config_path_arg(config_path);
  load_health_endpoint_with(&path, cli_override, env::from_env())
}

// Like load_health_endpoint, but takes an injected env layer and a fixed path.
pub fn load_health_endpoint_with(
  config_path: &Path,
  cli_override: Option<&str>,
  env_layer: PartialConfig,
) -> Result<Option<String>, ConfigError> {
  if let Some(raw) = cli_override {
    return Ok(normalize_health_socket(Some(raw.to_string())));
  }

  let mut partial = defaults_partial();
  merge_partial(&mut partial, load_file_layer(config_path)?);
  merge_partial(&mut partial, env_layer);
  Ok(normalize_health_socket(partial.health_socket))
}

pub(crate) fn normalize_health_socket(raw: Option<String>) -> Option<String> {
  trim_owned(raw)
}

#[cfg(test)]
mod tests {
  use super::*;
  use partial::{PartialAccount, PartialActivity};
  use std::fs;
  use std::sync::atomic::{AtomicU64, Ordering};

  static TEMP_SEQ: AtomicU64 = AtomicU64::new(0);
  const TOKEN_T: &str = "token = \"t\"\n";
  const SECRET: &str = "super-secret-token";

  struct TempToml {
    path: PathBuf,
  }

  impl TempToml {
    fn write(contents: &str) -> Self {
      let n = TEMP_SEQ.fetch_add(1, Ordering::Relaxed);
      let path = std::env::temp_dir().join(format!("dka-cfg-{n}-{}.toml", std::process::id()));
      fs::write(&path, contents).unwrap();
      Self { path }
    }

    fn path(&self) -> &Path {
      &self.path
    }
  }

  impl Drop for TempToml {
    fn drop(&mut self) {
      let _ = fs::remove_file(&self.path);
    }
  }

  fn with_token(mut p: PartialConfig) -> PartialConfig {
    if p.account.token.is_none() && p.accounts.is_empty() {
      p.account.token = Some("test-token".into());
    }
    p
  }

  fn empty_cli() -> Cli {
    Cli::try_parse_from(["discord-keep-alive"]).expect("empty cli parse")
  }

  fn env_map(pairs: &[(&str, &str)]) -> std::collections::HashMap<String, String> {
    pairs
      .iter()
      .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
      .collect()
  }

  fn load_env_toml(toml: &str, pairs: &[(&str, &str)]) -> AppConfig {
    let file = TempToml::write(toml);
    load_with(
      file.path(),
      env::from_env_map(&env_map(pairs)),
      PartialConfig::default(),
    )
    .unwrap()
  }

  fn load_layers(file: &Path, env: PartialConfig, cli: PartialConfig) -> AppConfig {
    load_with(file, env, cli).unwrap()
  }

  fn assert_redacts(debug: &str, secret: &str) {
    assert!(debug.contains("<redacted>"), "{debug}");
    assert!(!debug.contains(secret), "{debug}");
  }

  #[test]
  fn env_overwrites_file_scalars() {
    let app = load_env_toml(
      "log_level = \"warn\"\ntoken = \"t\"\n",
      &[("LOG_LEVEL", "debug")],
    );
    assert_eq!(app.log_level, "debug", "log_level");
    let app = load_env_toml(
      "health_socket = \"/tmp/from-file\"\ntoken = \"t\"\n",
      &[("HEALTH_SOCKET", "/tmp/from-env")],
    );
    assert_eq!(
      app.health_socket.as_deref(),
      Some("/tmp/from-env"),
      "health_socket"
    );
  }

  #[test]
  fn cli_overwrites_env_scalars() {
    let file = TempToml::write(TOKEN_T);
    let app = load_layers(
      file.path(),
      with_token(PartialConfig {
        log_level: Some("debug".into()),
        ..Default::default()
      }),
      PartialConfig {
        log_level: Some("trace".into()),
        ..Default::default()
      },
    );
    assert_eq!(app.log_level, "trace", "log_level");

    let app = load_layers(
      file.path(),
      PartialConfig {
        health_socket: Some("/tmp/env".into()),
        ..Default::default()
      },
      PartialConfig {
        health_socket: Some("/tmp/cli".into()),
        ..Default::default()
      },
    );
    assert_eq!(
      app.health_socket.as_deref(),
      Some("/tmp/cli"),
      "health_socket"
    );
  }

  #[test]
  fn load_health_endpoint_precedence_and_disable() {
    let file = TempToml::write(r#"health_socket = "/tmp/file""#);
    let env = PartialConfig {
      health_socket: Some("/tmp/env".into()),
      ..Default::default()
    };
    for (label, cli, want) in [
      ("cli_wins", Some("/tmp/cli"), Some("/tmp/cli")),
      ("env_over_file", None, Some("/tmp/env")),
      ("empty_cli", Some(""), None),
      ("ws_cli", Some("   "), None),
    ] {
      let ep = load_health_endpoint_with(file.path(), cli, env.clone()).unwrap();
      assert_eq!(ep.as_deref(), want, "{label}");
    }
    let file = TempToml::write(r#"health_socket = "/tmp/health-only""#);
    let ep = load_health_endpoint_with(file.path(), None, PartialConfig::default()).unwrap();
    assert_eq!(ep.as_deref(), Some("/tmp/health-only"), "ignores_accounts");
  }

  #[test]
  fn missing_default_path_ok() {
    let path = Path::new(DEFAULT_CONFIG_PATH);
    if path.exists() {
      let env_layer = with_token(PartialConfig::default());
      let _ = load_with(path, env_layer, PartialConfig::default());
      return;
    }
    let env_layer = with_token(PartialConfig::default());
    let app = load_with(path, env_layer, PartialConfig::default()).unwrap();
    assert_eq!(app.log_level, DEFAULT_LOG_LEVEL);
    assert!(app.health_socket.is_none());
  }

  #[test]
  fn explicit_missing_path_errors() {
    for (label, path, is_health) in [
      ("load", "definitely-missing-config-xyz.toml", false),
      ("health", "definitely-missing-health-config-xyz.toml", true),
    ] {
      let path = Path::new(path);
      assert!(!path.exists(), "{label}");
      let err = if is_health {
        load_health_endpoint_with(path, None, PartialConfig::default()).unwrap_err()
      } else {
        load_with(path, PartialConfig::default(), PartialConfig::default()).unwrap_err()
      };
      assert!(matches!(err, ConfigError::ConfigNotFound(_)), "{label}");
    }
  }

  #[test]
  fn defaults_then_file_for_log_level() {
    let file = TempToml::write("log_level = \"error\"\ntoken = \"t\"\n");
    let app = load_with(
      file.path(),
      PartialConfig::default(),
      PartialConfig::default(),
    )
    .unwrap();
    assert_eq!(app.log_level, "error");
  }

  #[test]
  fn env_from_lookup_sets_scalars() {
    let partial = env::from_env_lookup(|key| match key {
      "LOG_LEVEL" => Some("debug".into()),
      "HEALTH_SOCKET" => Some("/tmp/h".into()),
      "TOKEN" => Some("tok".into()),
      _ => None,
    });
    assert_eq!(partial.log_level.as_deref(), Some("debug"));
    assert_eq!(partial.health_socket.as_deref(), Some("/tmp/h"));
    assert_eq!(partial.account.token.as_deref(), Some("tok"));
  }

  #[test]
  fn merge_partial_cli_over_env() {
    let mut base = defaults_partial();
    for layer in [
      PartialConfig {
        log_level: Some("warn".into()),
        health_socket: Some("/tmp/file".into()),
        account: PartialAccount {
          token: Some("t".into()),
          ..Default::default()
        },
        ..Default::default()
      },
      PartialConfig {
        log_level: Some("debug".into()),
        health_socket: Some("/tmp/env".into()),
        ..Default::default()
      },
      PartialConfig {
        log_level: Some("trace".into()),
        ..Default::default()
      },
    ] {
      merge_partial(&mut base, layer);
    }
    assert_eq!(base.log_level.as_deref(), Some("trace"));
    assert_eq!(base.health_socket.as_deref(), Some("/tmp/env"));
  }

  #[test]
  fn cli_partial_maps_flat_account_and_activity() {
    let mut cli = empty_cli();
    macro_rules! set {
      ($($field:ident = $val:expr),* $(,)?) => {{ $(cli.$field = Some($val.into());)* }};
    }
    set!(
      token = "cli-tok",
      name = "cli-name",
      kind = "user",
      device = "mobile",
      status = "dnd",
      custom_status_text = "brb",
      custom_status_emoji = "💤",
      activity = "Game",
      activity_type = "playing",
      activity_details = "level 1",
      activity_url = "https://example.com",
      activity_platform = "desktop",
      activity_timestamp = "123",
      activity_application_id = "42",
      activity_large_image = "li",
      activity_large_image_text = "lit",
      activity_small_image = "si",
      activity_small_image_text = "sit",
      activity_button = "b1",
      activity_button_url = "https://b1",
      activity_button_2 = "b2",
      activity_button_2_url = "https://b2",
      activity_party_id = "p",
      activity_party_current = "1",
      activity_party_max = "4",
      log_level = "debug",
      health_socket = "/tmp/cli-h",
    );
    let p = cli_partial(&cli);
    let act = p.account.activity.as_ref().unwrap();
    let cs = p.account.custom_status.as_ref().unwrap();
    for (label, got, want) in [
      ("log_level", p.log_level.as_deref(), Some("debug")),
      (
        "health_socket",
        p.health_socket.as_deref(),
        Some("/tmp/cli-h"),
      ),
      ("token", p.account.token.as_deref(), Some("cli-tok")),
      ("name", p.account.name.as_deref(), Some("cli-name")),
      ("kind", p.account.kind.as_deref(), Some("user")),
      ("device", p.account.device.as_deref(), Some("mobile")),
      ("status", p.account.status.as_deref(), Some("dnd")),
      ("cs.text", cs.text.as_deref(), Some("brb")),
      ("cs.emoji", cs.emoji.as_deref(), Some("💤")),
      ("act.name", act.name.as_deref(), Some("Game")),
      ("act.type", act.activity_type.as_deref(), Some("playing")),
      ("act.details", act.details.as_deref(), Some("level 1")),
      ("act.url", act.url.as_deref(), Some("https://example.com")),
      ("act.platform", act.platform.as_deref(), Some("desktop")),
      ("act.timestamp", act.timestamp.as_deref(), Some("123")),
      ("act.app_id", act.application_id.as_deref(), Some("42")),
      ("act.li", act.large_image.as_deref(), Some("li")),
      ("act.lit", act.large_image_text.as_deref(), Some("lit")),
      ("act.si", act.small_image.as_deref(), Some("si")),
      ("act.sit", act.small_image_text.as_deref(), Some("sit")),
      ("act.btn", act.button.as_deref(), Some("b1")),
      ("act.btn_url", act.button_url.as_deref(), Some("https://b1")),
      ("act.btn2", act.button2.as_deref(), Some("b2")),
      (
        "act.btn2_url",
        act.button2_url.as_deref(),
        Some("https://b2"),
      ),
      ("act.party_id", act.party_id.as_deref(), Some("p")),
      ("act.party_cur", act.party_current.as_deref(), Some("1")),
      ("act.party_max", act.party_max.as_deref(), Some("4")),
    ] {
      assert_eq!(got, want, "{label}");
    }
    assert!(p.accounts.is_empty());
  }

  #[test]
  fn cli_partial_omits_unset_fields() {
    let p = cli_partial(&empty_cli());
    assert!(p.log_level.is_none() && p.health_socket.is_none());
    assert!(p.account.token.is_none());
    assert!(p.account.activity.is_none() && p.account.custom_status.is_none());
  }

  #[test]
  fn cli_fields_overwrite_env_via_load_with() {
    let env_layer = PartialConfig {
      account: PartialAccount {
        token: Some("env-tok".into()),
        status: Some("online".into()),
        ..Default::default()
      },
      ..Default::default()
    };
    let file = TempToml::write("token = \"file-tok\"\n");
    let mut cli = empty_cli();
    cli.token = Some("cli-tok".into());
    let app = load_layers(file.path(), env_layer.clone(), cli_partial(&cli));
    assert_eq!(app.accounts.len(), 1, "token");
    assert_eq!(app.accounts[0].token, "cli-tok", "token");
    assert_eq!(
      app.accounts[0].status,
      Some(dka_presence::Status::Online),
      "token"
    );

    let file = TempToml::write(TOKEN_T);
    let mut cli = empty_cli();
    cli.status = Some("dnd".into());
    let app = load_layers(file.path(), env_layer, cli_partial(&cli));
    assert_eq!(app.accounts[0].token, "env-tok", "status");
    assert_eq!(
      app.accounts[0].status,
      Some(dka_presence::Status::Dnd),
      "status"
    );
  }

  #[test]
  fn cli_activity_field_merge_does_not_wipe_env_fields() {
    let file = TempToml::write(TOKEN_T);
    let env_layer = PartialConfig {
      account: PartialAccount {
        token: Some("t".into()),
        activity: Some(PartialActivity {
          name: Some("from-env".into()),
          activity_type: Some("playing".into()),
          details: Some("env-details".into()),
          url: Some("https://env.example".into()),
          ..Default::default()
        }),
        ..Default::default()
      },
      ..Default::default()
    };
    let mut cli = empty_cli();
    cli.activity = Some("from-cli".into());
    cli.activity_type = Some("watching".into());
    let act = &load_layers(file.path(), env_layer, cli_partial(&cli)).accounts[0].activities;
    assert_eq!(act.len(), 1);
    assert_eq!(act[0].name.as_deref(), Some("from-cli"));
    assert_eq!(
      act[0].activity_type,
      Some(dka_presence::ActivityType::Watching)
    );
    assert_eq!(act[0].details.as_deref(), Some("env-details"));
    assert_eq!(act[0].url.as_deref(), Some("https://env.example"));
  }

  #[test]
  fn env_overwrites_file_account_token_and_status() {
    let app = load_env_toml(
      "token = \"file-tok\"\nstatus = \"online\"\n",
      &[("TOKEN", "env-tok"), ("STATUS", "dnd")],
    );
    assert_eq!(app.accounts.len(), 1);
    assert_eq!(app.accounts[0].token, "env-tok");
    assert_eq!(app.accounts[0].status, Some(dka_presence::Status::Dnd));
  }

  #[test]
  fn sparse_activity_env_pads_then_resolve_filters_nameless() {
    let app = load_env_toml(
      "",
      &[
        ("TOKEN", "t"),
        ("ACTIVITY_0", "zero"),
        ("ACTIVITY_0_TYPE", "playing"),
        ("ACTIVITY_2", "two"),
        ("ACTIVITY_2_TYPE", "watching"),
      ],
    );
    assert_eq!(app.accounts.len(), 1);
    let names: Vec<_> = app.accounts[0]
      .activities
      .iter()
      .map(|a| a.name.as_deref().unwrap())
      .collect();
    assert_eq!(names, ["zero", "two"]);
  }

  #[test]
  fn account_env_beyond_len_appends_without_pad() {
    let app = load_env_toml(
      "",
      &[
        ("ACCOUNT_5_TOKEN", "tok-five"),
        ("ACCOUNT_5", "five"),
        ("ACCOUNT_5_STATUS", "idle"),
      ],
    );
    assert_eq!(app.accounts.len(), 1);
    assert_eq!(app.accounts[0].name, "five");
    assert_eq!(app.accounts[0].token, "tok-five");
    assert_eq!(app.accounts[0].status, Some(dka_presence::Status::Idle));
  }

  #[test]
  fn account_env_dense_indices_merge_onto_file_slots() {
    let a = load_env_toml(
      "[[accounts]]\nname = \"first\"\ntoken = \"file-0\"\nstatus = \"online\"\n\n[[accounts]]\nname = \"second\"\ntoken = \"file-1\"\nstatus = \"online\"\n",
      &[
        ("ACCOUNT_0_TOKEN", "env-0"),
        ("ACCOUNT_1_TOKEN", "env-1"),
        ("ACCOUNT_1_STATUS", "dnd"),
      ],
    )
    .accounts;
    assert_eq!(a.len(), 2);
    assert_eq!((&*a[0].name, &*a[0].token), ("first", "env-0"));
    assert_eq!(a[0].status, Some(dka_presence::Status::Online));
    assert_eq!((&*a[1].name, &*a[1].token), ("second", "env-1"));
    assert_eq!(a[1].status, Some(dka_presence::Status::Dnd));
  }

  #[test]
  fn account_env_sparse_high_index_packs_to_slot_zero_on_empty_env_layer() {
    let a = load_env_toml(
      "[[accounts]]\nname = \"first\"\ntoken = \"file-0\"\n\n[[accounts]]\nname = \"second\"\ntoken = \"file-1\"\n",
      &[("ACCOUNT_1_TOKEN", "env-1")],
    )
    .accounts;
    assert_eq!(a.len(), 2);
    assert_eq!((&*a[0].token, &*a[0].name), ("env-1", "first"));
    assert_eq!((&*a[1].token, &*a[1].name), ("file-1", "second"));
  }

  #[test]
  fn account_env_discovery_sorted_by_token_anchor() {
    let a = load_env_toml(
      "",
      &[
        ("ACCOUNT_2_TOKEN", "tok-2"),
        ("ACCOUNT_2", "two"),
        ("ACCOUNT_0_TOKEN", "tok-0"),
        ("ACCOUNT_0", "zero"),
      ],
    )
    .accounts;
    assert_eq!(a.len(), 2);
    assert_eq!((&*a[0].name, &*a[0].token), ("zero", "tok-0"));
    assert_eq!((&*a[1].name, &*a[1].token), ("two", "tok-2"));
  }

  #[test]
  fn flat_token_env_prepended_before_file_accounts() {
    let a = load_env_toml(
      "[[accounts]]\nname = \"from-file\"\ntoken = \"file-tok\"\n",
      &[("TOKEN", "flat-tok"), ("ACCOUNT", "from-env")],
    )
    .accounts;
    assert_eq!(a.len(), 2);
    assert_eq!((&*a[0].name, &*a[0].token), ("from-env", "flat-tok"));
    assert_eq!((&*a[1].name, &*a[1].token), ("from-file", "file-tok"));
  }

  #[test]
  fn load_with_no_accounts_errors() {
    let file = TempToml::write(r#"log_level = "info""#);
    let err = load_with(
      file.path(),
      PartialConfig::default(),
      PartialConfig::default(),
    )
    .unwrap_err();
    assert!(matches!(err, ConfigError::NoAccounts));
  }

  #[test]
  fn resolve_config_path_cases() {
    let env = || Some(std::ffi::OsString::from("/tmp/from-config-path-env.toml"));
    for (label, cli, env, want) in [
      (
        "cli_wins",
        Path::new("/tmp/explicit-cli-config.toml"),
        env(),
        "/tmp/explicit-cli-config.toml",
      ),
      (
        "default_uses_env",
        Path::new(DEFAULT_CONFIG_PATH),
        env(),
        "/tmp/from-config-path-env.toml",
      ),
      (
        "default_without_env",
        Path::new(DEFAULT_CONFIG_PATH),
        None,
        DEFAULT_CONFIG_PATH,
      ),
    ] {
      assert_eq!(
        resolve_config_path(cli, env),
        PathBuf::from(want),
        "{label}"
      );
    }
  }

  #[test]
  fn cli_debug_redacts_token() {
    let mut cli = empty_cli();
    cli.token = Some(SECRET.into());
    assert_redacts(&format!("{cli:?}"), SECRET);
  }

  #[test]
  fn partial_account_debug_redacts_token() {
    let account = PartialAccount {
      name: Some("n".into()),
      token: Some(SECRET.into()),
      ..Default::default()
    };
    assert_redacts(&format!("{account:?}"), SECRET);
  }
}
