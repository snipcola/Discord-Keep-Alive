//! Config layers (later wins): defaults → file → env → CLI → resolve.
//! User-facing priority: CLI > ENV > File > hardcoded defaults.
//! `RUST_LOG` is logging-runtime only (`log::init`), not a config override of `log_level`.

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
const DEFAULT_LOG_LEVEL: &str = "info";

/// CLI surface only. Config value flags have no clap `env=`; the env provider owns those.
/// Flat-account / singular-activity long names stay aligned with `schema` (`cli_long`).
#[derive(Debug, Parser)]
#[command(
  name = "discord-keep-alive",
  about = "Keep Discord accounts online with optional presence"
)]
pub struct Cli {
  /// TOML config path.
  // No clap `env=`; path is process surface (see resolve_config_path). Env provider owns config values.
  #[arg(long, short = 'c', default_value = DEFAULT_CONFIG_PATH)]
  pub config: PathBuf,

  /// Log level: `error`, `warn`, `info`, `debug`, or `trace`.
  #[arg(long)]
  pub log_level: Option<String>,

  /// Health IPC endpoint (Unix path or Windows named pipe). Empty disables.
  #[arg(long)]
  pub health_socket: Option<String>,

  /// Flat-account token (prefer TOKEN env or config file).
  #[arg(long = "token")]
  pub token: Option<token::SecretString>,

  #[arg(long = "name")]
  pub name: Option<String>,

  /// `user` or `bot`.
  #[arg(long = "kind")]
  pub kind: Option<String>,

  /// `desktop`, `web`, or `mobile` (user accounts).
  #[arg(long = "device")]
  pub device: Option<String>,

  /// `online`, `idle`, `dnd`, or `invisible`.
  #[arg(long = "status")]
  pub status: Option<String>,

  /// Custom status text (user accounts).
  #[arg(long = "custom-status-text")]
  pub custom_status_text: Option<String>,

  /// Custom status emoji (user accounts).
  #[arg(long = "custom-status-emoji")]
  pub custom_status_emoji: Option<String>,

  #[arg(long = "activity")]
  pub activity: Option<String>,

  /// `playing`, `streaming`, `listening`, `watching`, or `competing`.
  #[arg(long = "activity-type")]
  pub activity_type: Option<String>,

  #[arg(long = "activity-platform")]
  pub activity_platform: Option<String>,

  /// Unix seconds (activity start).
  #[arg(long = "activity-timestamp")]
  pub activity_timestamp: Option<String>,

  #[arg(long = "activity-application-id")]
  pub activity_application_id: Option<String>,

  #[arg(long = "activity-details")]
  pub activity_details: Option<String>,

  /// Required for streaming.
  #[arg(long = "activity-url")]
  pub activity_url: Option<String>,

  #[arg(long = "activity-large-image")]
  pub activity_large_image: Option<String>,

  /// Large-image hover text.
  #[arg(long = "activity-large-image-text")]
  pub activity_large_image_text: Option<String>,

  #[arg(long = "activity-small-image")]
  pub activity_small_image: Option<String>,

  /// Small-image hover text.
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
  /// Probe health endpoint; exit 0/1/2.
  Health {
    /// Probe-only endpoint override. Empty disables.
    #[arg(long)]
    health_socket: Option<String>,

    /// TOML path when endpoint is not given on the CLI.
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

/// Hardcoded scalar defaults (client-property product defaults apply at resolve).
fn defaults_partial() -> PartialConfig {
  PartialConfig {
    log_level: Some(DEFAULT_LOG_LEVEL.into()),
    health_socket: None,
    ..Default::default()
  }
}

/// Compile-time check: clap `long = "..."` strings must equal catalog `cli_long()`.
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

  assert!(eq(AccountScalarField::Token.cli_long(), "token"));
  assert!(eq(AccountScalarField::Name.cli_long(), "name"));
  assert!(eq(AccountScalarField::Kind.cli_long(), "kind"));
  assert!(eq(AccountScalarField::Device.cli_long(), "device"));
  assert!(eq(AccountScalarField::Status.cli_long(), "status"));
  assert!(eq(CustomStatusField::Text.cli_long(), "custom-status-text"));
  assert!(eq(
    CustomStatusField::Emoji.cli_long(),
    "custom-status-emoji"
  ));
  assert!(eq(ActivityField::Name.cli_long(), "activity"));
  assert!(eq(ActivityField::Type.cli_long(), "activity-type"));
  assert!(eq(ActivityField::Platform.cli_long(), "activity-platform"));
  assert!(eq(
    ActivityField::Timestamp.cli_long(),
    "activity-timestamp"
  ));
  assert!(eq(
    ActivityField::ApplicationId.cli_long(),
    "activity-application-id"
  ));
  assert!(eq(ActivityField::Details.cli_long(), "activity-details"));
  assert!(eq(ActivityField::Url.cli_long(), "activity-url"));
  assert!(eq(
    ActivityField::LargeImage.cli_long(),
    "activity-large-image"
  ));
  assert!(eq(
    ActivityField::LargeImageText.cli_long(),
    "activity-large-image-text"
  ));
  assert!(eq(
    ActivityField::SmallImage.cli_long(),
    "activity-small-image"
  ));
  assert!(eq(
    ActivityField::SmallImageText.cli_long(),
    "activity-small-image-text"
  ));
  assert!(eq(ActivityField::Button.cli_long(), "activity-button"));
  assert!(eq(
    ActivityField::ButtonUrl.cli_long(),
    "activity-button-url"
  ));
  assert!(eq(ActivityField::Button2.cli_long(), "activity-button-2"));
  assert!(eq(
    ActivityField::Button2Url.cli_long(),
    "activity-button-2-url"
  ));
  assert!(eq(ActivityField::PartyId.cli_long(), "activity-party-id"));
  assert!(eq(
    ActivityField::PartyCurrent.cli_long(),
    "activity-party-current"
  ));
  assert!(eq(ActivityField::PartyMax.cli_long(), "activity-party-max"));
};

fn cli_partial(cli: &Cli) -> PartialConfig {
  let mut partial = PartialConfig {
    log_level: cli_opt(cli.log_level.as_deref()),
    health_socket: cli.health_socket.clone(),
    ..Default::default()
  };

  let mut account = PartialAccount::default();
  if let Some(v) = cli_opt(cli.token.as_deref()) {
    AccountScalarField::Token.set(&mut account, v);
  }
  set_cli_field(
    AccountScalarField::Name.get_mut(&mut account),
    cli.name.as_deref(),
  );
  set_cli_field(
    AccountScalarField::Kind.get_mut(&mut account),
    cli.kind.as_deref(),
  );
  set_cli_field(
    AccountScalarField::Device.get_mut(&mut account),
    cli.device.as_deref(),
  );
  set_cli_field(
    AccountScalarField::Status.get_mut(&mut account),
    cli.status.as_deref(),
  );

  let mut custom = PartialCustomStatus::default();
  set_cli_field(
    CustomStatusField::Text.get_mut(&mut custom),
    cli.custom_status_text.as_deref(),
  );
  set_cli_field(
    CustomStatusField::Emoji.get_mut(&mut custom),
    cli.custom_status_emoji.as_deref(),
  );
  if any_custom_status_field_set(&custom) {
    account.custom_status = Some(custom);
  }

  let mut activity = PartialActivity::default();
  set_cli_field(
    ActivityField::Name.get_mut(&mut activity),
    cli.activity.as_deref(),
  );
  set_cli_field(
    ActivityField::Type.get_mut(&mut activity),
    cli.activity_type.as_deref(),
  );
  set_cli_field(
    ActivityField::Platform.get_mut(&mut activity),
    cli.activity_platform.as_deref(),
  );
  set_cli_field(
    ActivityField::Timestamp.get_mut(&mut activity),
    cli.activity_timestamp.as_deref(),
  );
  set_cli_field(
    ActivityField::ApplicationId.get_mut(&mut activity),
    cli.activity_application_id.as_deref(),
  );
  set_cli_field(
    ActivityField::Details.get_mut(&mut activity),
    cli.activity_details.as_deref(),
  );
  set_cli_field(
    ActivityField::Url.get_mut(&mut activity),
    cli.activity_url.as_deref(),
  );
  set_cli_field(
    ActivityField::LargeImage.get_mut(&mut activity),
    cli.activity_large_image.as_deref(),
  );
  set_cli_field(
    ActivityField::LargeImageText.get_mut(&mut activity),
    cli.activity_large_image_text.as_deref(),
  );
  set_cli_field(
    ActivityField::SmallImage.get_mut(&mut activity),
    cli.activity_small_image.as_deref(),
  );
  set_cli_field(
    ActivityField::SmallImageText.get_mut(&mut activity),
    cli.activity_small_image_text.as_deref(),
  );
  set_cli_field(
    ActivityField::Button.get_mut(&mut activity),
    cli.activity_button.as_deref(),
  );
  set_cli_field(
    ActivityField::ButtonUrl.get_mut(&mut activity),
    cli.activity_button_url.as_deref(),
  );
  set_cli_field(
    ActivityField::Button2.get_mut(&mut activity),
    cli.activity_button_2.as_deref(),
  );
  set_cli_field(
    ActivityField::Button2Url.get_mut(&mut activity),
    cli.activity_button_2_url.as_deref(),
  );
  set_cli_field(
    ActivityField::PartyId.get_mut(&mut activity),
    cli.activity_party_id.as_deref(),
  );
  set_cli_field(
    ActivityField::PartyCurrent.get_mut(&mut activity),
    cli.activity_party_current.as_deref(),
  );
  set_cli_field(
    ActivityField::PartyMax.get_mut(&mut activity),
    cli.activity_party_max.as_deref(),
  );
  if any_activity_field_set(&activity) {
    account.activity = Some(activity);
  }

  partial.account = account;
  partial
}

fn cli_opt(raw: Option<&str>) -> Option<String> {
  raw
    .map(str::trim)
    .filter(|s| !s.is_empty())
    .map(str::to_string)
}

fn set_cli_field(dst: &mut Option<String>, raw: Option<&str>) {
  if let Some(v) = cli_opt(raw) {
    *dst = Some(v);
  }
}

/// CLI `--config`/`-c` wins; if still the clap default, honor `CONFIG_PATH` once.
/// Process surface only: not a merged config leaf, and not via clap `env=`.
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

/// Health endpoint only (no accounts required). Precedence: CLI > env > file > none.
/// Empty/whitespace CLI override disables the endpoint.
pub fn load_health_endpoint(
  config_path: &Path,
  cli_override: Option<&str>,
) -> Result<Option<String>, ConfigError> {
  let path = resolve_config_path_arg(config_path);
  load_health_endpoint_with(&path, cli_override, env::from_env())
}

/// Health endpoint load with injected env; path used as-is (no `CONFIG_PATH`).
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
  raw.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
  use super::*;
  use partial::{PartialAccount, PartialActivity};
  use std::fs;
  use std::sync::atomic::{AtomicU64, Ordering};

  static TEMP_SEQ: AtomicU64 = AtomicU64::new(0);

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

  #[test]
  fn env_overwrites_file_log_level() {
    let file = TempToml::write(
      r#"
log_level = "warn"
token = "t"
"#,
    );
    let env_layer = PartialConfig {
      log_level: Some("debug".into()),
      ..Default::default()
    };
    let app = load_with(file.path(), env_layer, PartialConfig::default()).unwrap();
    assert_eq!(app.log_level, "debug");
  }

  #[test]
  fn env_overwrites_file_health_socket() {
    let file = TempToml::write(
      r#"
health_socket = "/tmp/from-file"
token = "t"
"#,
    );
    let env_layer = PartialConfig {
      health_socket: Some("/tmp/from-env".into()),
      ..Default::default()
    };
    let app = load_with(file.path(), env_layer, PartialConfig::default()).unwrap();
    assert_eq!(app.health_socket.as_deref(), Some("/tmp/from-env"));
  }

  #[test]
  fn cli_overwrites_env_log_level() {
    let env_layer = with_token(PartialConfig {
      log_level: Some("debug".into()),
      ..Default::default()
    });
    let cli_layer = PartialConfig {
      log_level: Some("trace".into()),
      ..Default::default()
    };
    let file = TempToml::write("token = \"t\"\n");
    let app = load_with(file.path(), env_layer, cli_layer).unwrap();
    assert_eq!(app.log_level, "trace");
  }

  #[test]
  fn cli_overwrites_env_health_socket() {
    let file = TempToml::write("token = \"t\"\n");
    let env_layer = PartialConfig {
      health_socket: Some("/tmp/env".into()),
      ..Default::default()
    };
    let cli_layer = PartialConfig {
      health_socket: Some("/tmp/cli".into()),
      ..Default::default()
    };
    let app = load_with(file.path(), env_layer, cli_layer).unwrap();
    assert_eq!(app.health_socket.as_deref(), Some("/tmp/cli"));
  }

  #[test]
  fn load_health_endpoint_cli_wins_over_env_and_file() {
    let file = TempToml::write(r#"health_socket = "/tmp/file""#);
    let env_layer = PartialConfig {
      health_socket: Some("/tmp/env".into()),
      ..Default::default()
    };
    let ep = load_health_endpoint_with(file.path(), Some("/tmp/cli"), env_layer).unwrap();
    assert_eq!(ep.as_deref(), Some("/tmp/cli"));
  }

  #[test]
  fn load_health_endpoint_env_wins_over_file() {
    let file = TempToml::write(r#"health_socket = "/tmp/file""#);
    let env_layer = PartialConfig {
      health_socket: Some("/tmp/env".into()),
      ..Default::default()
    };
    let ep = load_health_endpoint_with(file.path(), None, env_layer).unwrap();
    assert_eq!(ep.as_deref(), Some("/tmp/env"));
  }

  #[test]
  fn missing_default_path_ok() {
    let path = Path::new(DEFAULT_CONFIG_PATH);
    if path.exists() {
      // Real config.toml in CWD; only check load does not hard-error on path.
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
    let path = Path::new("definitely-missing-config-xyz.toml");
    assert!(!path.exists());
    let err = load_with(path, PartialConfig::default(), PartialConfig::default()).unwrap_err();
    assert!(matches!(err, ConfigError::ConfigNotFound(_)));
  }

  #[test]
  fn explicit_missing_path_errors_for_health() {
    let path = Path::new("definitely-missing-health-config-xyz.toml");
    assert!(!path.exists());
    let err = load_health_endpoint_with(path, None, PartialConfig::default()).unwrap_err();
    assert!(matches!(err, ConfigError::ConfigNotFound(_)));
  }

  #[test]
  fn defaults_then_file_for_log_level() {
    let file = TempToml::write(
      r#"
log_level = "error"
token = "t"
"#,
    );
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
    merge_partial(
      &mut base,
      PartialConfig {
        log_level: Some("warn".into()),
        health_socket: Some("/tmp/file".into()),
        account: PartialAccount {
          token: Some("t".into()),
          ..Default::default()
        },
        ..Default::default()
      },
    );
    merge_partial(
      &mut base,
      PartialConfig {
        log_level: Some("debug".into()),
        health_socket: Some("/tmp/env".into()),
        ..Default::default()
      },
    );
    merge_partial(
      &mut base,
      PartialConfig {
        log_level: Some("trace".into()),
        ..Default::default()
      },
    );
    assert_eq!(base.log_level.as_deref(), Some("trace"));
    assert_eq!(base.health_socket.as_deref(), Some("/tmp/env"));
  }

  #[test]
  fn cli_partial_maps_flat_account_and_activity() {
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
    cli.activity_url = Some("https://example.com".into());
    cli.activity_platform = Some("desktop".into());
    cli.activity_timestamp = Some("123".into());
    cli.activity_application_id = Some("42".into());
    cli.activity_large_image = Some("li".into());
    cli.activity_large_image_text = Some("lit".into());
    cli.activity_small_image = Some("si".into());
    cli.activity_small_image_text = Some("sit".into());
    cli.activity_button = Some("b1".into());
    cli.activity_button_url = Some("https://b1".into());
    cli.activity_button_2 = Some("b2".into());
    cli.activity_button_2_url = Some("https://b2".into());
    cli.activity_party_id = Some("p".into());
    cli.activity_party_current = Some("1".into());
    cli.activity_party_max = Some("4".into());
    cli.log_level = Some("debug".into());
    cli.health_socket = Some("/tmp/cli-h".into());

    let p = cli_partial(&cli);
    assert_eq!(p.log_level.as_deref(), Some("debug"));
    assert_eq!(p.health_socket.as_deref(), Some("/tmp/cli-h"));
    assert_eq!(p.account.token.as_deref(), Some("cli-tok"));
    assert_eq!(p.account.name.as_deref(), Some("cli-name"));
    assert_eq!(p.account.kind.as_deref(), Some("user"));
    assert_eq!(p.account.device.as_deref(), Some("mobile"));
    assert_eq!(p.account.status.as_deref(), Some("dnd"));
    let cs = p.account.custom_status.as_ref().unwrap();
    assert_eq!(cs.text.as_deref(), Some("brb"));
    assert_eq!(cs.emoji.as_deref(), Some("💤"));
    let act = p.account.activity.as_ref().unwrap();
    assert_eq!(act.name.as_deref(), Some("Game"));
    assert_eq!(act.activity_type.as_deref(), Some("playing"));
    assert_eq!(act.details.as_deref(), Some("level 1"));
    assert_eq!(act.url.as_deref(), Some("https://example.com"));
    assert_eq!(act.platform.as_deref(), Some("desktop"));
    assert_eq!(act.timestamp.as_deref(), Some("123"));
    assert_eq!(act.application_id.as_deref(), Some("42"));
    assert_eq!(act.large_image.as_deref(), Some("li"));
    assert_eq!(act.large_image_text.as_deref(), Some("lit"));
    assert_eq!(act.small_image.as_deref(), Some("si"));
    assert_eq!(act.small_image_text.as_deref(), Some("sit"));
    assert_eq!(act.button.as_deref(), Some("b1"));
    assert_eq!(act.button_url.as_deref(), Some("https://b1"));
    assert_eq!(act.button2.as_deref(), Some("b2"));
    assert_eq!(act.button2_url.as_deref(), Some("https://b2"));
    assert_eq!(act.party_id.as_deref(), Some("p"));
    assert_eq!(act.party_current.as_deref(), Some("1"));
    assert_eq!(act.party_max.as_deref(), Some("4"));
    assert!(p.accounts.is_empty());
  }

  #[test]
  fn cli_partial_omits_unset_fields() {
    let p = cli_partial(&empty_cli());
    assert!(p.log_level.is_none());
    assert!(p.health_socket.is_none());
    assert!(p.account.token.is_none());
    assert!(p.account.activity.is_none());
    assert!(p.account.custom_status.is_none());
  }

  #[test]
  fn cli_token_overwrites_env_via_load_with() {
    let file = TempToml::write("token = \"file-tok\"\n");
    let env_layer = PartialConfig {
      account: PartialAccount {
        token: Some("env-tok".into()),
        status: Some("online".into()),
        ..Default::default()
      },
      ..Default::default()
    };
    let mut cli = empty_cli();
    cli.token = Some("cli-tok".into());
    let app = load_with(file.path(), env_layer, cli_partial(&cli)).unwrap();
    assert_eq!(app.accounts.len(), 1);
    assert_eq!(app.accounts[0].token, "cli-tok");
    assert_eq!(app.accounts[0].status, Some(dka_presence::Status::Online));
  }

  #[test]
  fn cli_status_overwrites_env_via_load_with() {
    let file = TempToml::write("token = \"t\"\n");
    let env_layer = PartialConfig {
      account: PartialAccount {
        token: Some("env-tok".into()),
        status: Some("online".into()),
        ..Default::default()
      },
      ..Default::default()
    };
    let mut cli = empty_cli();
    cli.status = Some("dnd".into());
    let app = load_with(file.path(), env_layer, cli_partial(&cli)).unwrap();
    assert_eq!(app.accounts[0].token, "env-tok");
    assert_eq!(app.accounts[0].status, Some(dka_presence::Status::Dnd));
  }

  #[test]
  fn cli_activity_field_merge_does_not_wipe_env_fields() {
    let file = TempToml::write("token = \"t\"\n");
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
    // CLI sets only name/type; details/url from env must survive field-level merge.
    let app = load_with(file.path(), env_layer, cli_partial(&cli)).unwrap();
    assert_eq!(app.accounts[0].activities.len(), 1);
    let act = &app.accounts[0].activities[0];
    assert_eq!(act.name.as_deref(), Some("from-cli"));
    assert_eq!(
      act.activity_type,
      Some(dka_presence::ActivityType::Watching)
    );
    assert_eq!(act.details.as_deref(), Some("env-details"));
    assert_eq!(act.url.as_deref(), Some("https://env.example"));
  }

  #[test]
  fn load_health_endpoint_ignores_accounts() {
    let file = TempToml::write(r#"health_socket = "/tmp/health-only""#);
    let ep = load_health_endpoint_with(file.path(), None, PartialConfig::default()).unwrap();
    assert_eq!(ep.as_deref(), Some("/tmp/health-only"));
  }

  #[test]
  fn cli_long_names_align_with_schema_catalog() {
    assert_eq!(AccountScalarField::Token.cli_long(), "token");
    assert_eq!(AccountScalarField::Status.cli_long(), "status");
    assert_eq!(CustomStatusField::Text.cli_long(), "custom-status-text");
    assert_eq!(CustomStatusField::Emoji.cli_long(), "custom-status-emoji");
    assert_eq!(ActivityField::Name.cli_long(), "activity");
    assert_eq!(ActivityField::Type.cli_long(), "activity-type");
    assert_eq!(ActivityField::Details.cli_long(), "activity-details");
    assert_eq!(ActivityField::Url.cli_long(), "activity-url");
    assert_eq!(ActivityField::Button2.cli_long(), "activity-button-2");
  }

  fn env_map(pairs: &[(&str, &str)]) -> std::collections::HashMap<String, String> {
    pairs
      .iter()
      .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
      .collect()
  }

  #[test]
  fn env_overwrites_file_account_token_and_status() {
    let file = TempToml::write(
      r#"
token = "file-tok"
status = "online"
"#,
    );
    let env_layer = env::from_env_map(&env_map(&[("TOKEN", "env-tok"), ("STATUS", "dnd")]));
    let app = load_with(file.path(), env_layer, PartialConfig::default()).unwrap();
    assert_eq!(app.accounts.len(), 1);
    assert_eq!(app.accounts[0].token, "env-tok");
    assert_eq!(app.accounts[0].status, Some(dka_presence::Status::Dnd));
  }

  #[test]
  fn sparse_activity_env_pads_then_resolve_filters_nameless() {
    let env_layer = env::from_env_map(&env_map(&[
      ("TOKEN", "t"),
      ("ACTIVITY_0", "zero"),
      ("ACTIVITY_0_TYPE", "playing"),
      ("ACTIVITY_2", "two"),
      ("ACTIVITY_2_TYPE", "watching"),
    ]));
    let file = TempToml::write("");
    let app = load_with(file.path(), env_layer, PartialConfig::default()).unwrap();
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
    let env_layer = env::from_env_map(&env_map(&[
      ("ACCOUNT_5_TOKEN", "tok-five"),
      ("ACCOUNT_5_NAME", "five"),
      ("ACCOUNT_5_STATUS", "idle"),
    ]));
    let file = TempToml::write("");
    let app = load_with(file.path(), env_layer, PartialConfig::default()).unwrap();
    assert_eq!(app.accounts.len(), 1);
    assert_eq!(app.accounts[0].name, "five");
    assert_eq!(app.accounts[0].token, "tok-five");
    assert_eq!(app.accounts[0].status, Some(dka_presence::Status::Idle));
  }

  #[test]
  fn account_env_dense_indices_merge_onto_file_slots() {
    let file = TempToml::write(
      r#"
[[accounts]]
name = "first"
token = "file-0"
status = "online"

[[accounts]]
name = "second"
token = "file-1"
status = "online"
"#,
    );
    let env_layer = env::from_env_map(&env_map(&[
      ("ACCOUNT_0_TOKEN", "env-0"),
      ("ACCOUNT_1_TOKEN", "env-1"),
      ("ACCOUNT_1_STATUS", "dnd"),
    ]));
    let app = load_with(file.path(), env_layer, PartialConfig::default()).unwrap();
    assert_eq!(app.accounts.len(), 2);
    assert_eq!(app.accounts[0].name, "first");
    assert_eq!(app.accounts[0].token, "env-0");
    assert_eq!(app.accounts[0].status, Some(dka_presence::Status::Online));
    assert_eq!(app.accounts[1].name, "second");
    assert_eq!(app.accounts[1].token, "env-1");
    assert_eq!(app.accounts[1].status, Some(dka_presence::Status::Dnd));
  }

  #[test]
  fn account_env_sparse_high_index_packs_to_slot_zero_on_empty_env_layer() {
    let file = TempToml::write(
      r#"
[[accounts]]
name = "first"
token = "file-0"

[[accounts]]
name = "second"
token = "file-1"
"#,
    );
    let env_layer = env::from_env_map(&env_map(&[("ACCOUNT_1_TOKEN", "env-1")]));
    let app = load_with(file.path(), env_layer, PartialConfig::default()).unwrap();
    assert_eq!(app.accounts.len(), 2);
    assert_eq!(app.accounts[0].token, "env-1");
    assert_eq!(app.accounts[0].name, "first");
    assert_eq!(app.accounts[1].token, "file-1");
    assert_eq!(app.accounts[1].name, "second");
  }

  #[test]
  fn account_env_discovery_sorted_by_token_anchor() {
    let env_layer = env::from_env_map(&env_map(&[
      ("ACCOUNT_2_TOKEN", "tok-2"),
      ("ACCOUNT_2_NAME", "two"),
      ("ACCOUNT_0_TOKEN", "tok-0"),
      ("ACCOUNT_0_NAME", "zero"),
    ]));
    let file = TempToml::write("");
    let app = load_with(file.path(), env_layer, PartialConfig::default()).unwrap();
    assert_eq!(app.accounts.len(), 2);
    assert_eq!(app.accounts[0].name, "zero");
    assert_eq!(app.accounts[0].token, "tok-0");
    assert_eq!(app.accounts[1].name, "two");
    assert_eq!(app.accounts[1].token, "tok-2");
  }

  #[test]
  fn flat_token_env_prepended_before_file_accounts() {
    let file = TempToml::write(
      r#"
[[accounts]]
name = "from-file"
token = "file-tok"
"#,
    );
    let env_layer = env::from_env_map(&env_map(&[("TOKEN", "flat-tok"), ("NAME", "from-env")]));
    let app = load_with(file.path(), env_layer, PartialConfig::default()).unwrap();
    assert_eq!(app.accounts.len(), 2);
    assert_eq!(app.accounts[0].name, "from-env");
    assert_eq!(app.accounts[0].token, "flat-tok");
    assert_eq!(app.accounts[1].name, "from-file");
    assert_eq!(app.accounts[1].token, "file-tok");
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
  fn resolve_config_path_cli_wins_over_config_path_env() {
    let explicit = Path::new("/tmp/explicit-cli-config.toml");
    let env = Some(std::ffi::OsString::from("/tmp/from-config-path-env.toml"));
    let resolved = resolve_config_path(explicit, env);
    assert_eq!(resolved, explicit);
  }

  #[test]
  fn resolve_config_path_default_uses_config_path_env() {
    let env = Some(std::ffi::OsString::from("/tmp/from-config-path-env.toml"));
    let resolved = resolve_config_path(Path::new(DEFAULT_CONFIG_PATH), env);
    assert_eq!(resolved, PathBuf::from("/tmp/from-config-path-env.toml"));
  }

  #[test]
  fn resolve_config_path_default_without_env() {
    let resolved = resolve_config_path(Path::new(DEFAULT_CONFIG_PATH), None);
    assert_eq!(resolved, PathBuf::from(DEFAULT_CONFIG_PATH));
  }

  #[test]
  fn load_health_endpoint_empty_cli_override_disables() {
    let file = TempToml::write(r#"health_socket = "/tmp/file""#);
    let env_layer = PartialConfig {
      health_socket: Some("/tmp/env".into()),
      ..Default::default()
    };
    let ep = load_health_endpoint_with(file.path(), Some(""), env_layer).unwrap();
    assert!(ep.is_none());

    let env_layer = PartialConfig {
      health_socket: Some("/tmp/env".into()),
      ..Default::default()
    };
    let ep = load_health_endpoint_with(file.path(), Some("   "), env_layer).unwrap();
    assert!(ep.is_none());
  }

  #[test]
  fn cli_debug_redacts_token() {
    let mut cli = empty_cli();
    cli.token = Some("super-secret-token".into());
    let rendered = format!("{cli:?}");
    assert!(rendered.contains("<redacted>"));
    assert!(!rendered.contains("super-secret-token"));
  }

  #[test]
  fn partial_account_debug_redacts_token() {
    let account = PartialAccount {
      name: Some("n".into()),
      token: Some("super-secret-token".into()),
      ..Default::default()
    };
    let rendered = format!("{account:?}");
    assert!(rendered.contains("<redacted>"));
    assert!(!rendered.contains("super-secret-token"));
  }
}
