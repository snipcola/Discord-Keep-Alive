//! Config layers (later wins): defaults -> TOML -> flat environment.

mod env;
mod file;
mod resolve;

use std::fmt;
use std::path::{Path, PathBuf};

use clap::Parser;
use dka_presence::{AccountKind, ActivityConfig, CustomStatusConfig, Device, Status};
use figment::{
  Figment,
  providers::{Format, Serialized, Toml},
};
use thiserror::Error;

pub use file::FileConfig;

use crate::gateway::properties::Defaults;

const DEFAULT_CONFIG_PATH: &str = "config.toml";

#[derive(Debug, Parser)]
#[command(
  name = "discord-keep-alive",
  about = "Keep Discord accounts online with optional presence"
)]
pub struct Cli {
  /// Path to the TOML config file.
  #[arg(long, short = 'c', env = "CONFIG_PATH", default_value = DEFAULT_CONFIG_PATH)]
  pub config: PathBuf,

  /// Override log level (`error`, `warn`, `info`, `debug`, or `trace`).
  #[arg(long, env = "LOG_LEVEL")]
  pub log_level: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AppConfig {
  pub log_level: String,
  pub defaults: Defaults,
  pub accounts: Vec<AccountConfig>,
}

#[derive(Clone)]
pub struct AccountConfig {
  pub name: String,
  pub token: String,
  pub kind: AccountKind,
  pub device: Option<Device>,
  pub status: Option<Status>,
  pub custom_status: Option<CustomStatusConfig>,
  pub activities: Vec<ActivityConfig>,
}

impl fmt::Debug for AccountConfig {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("AccountConfig")
      .field("name", &self.name)
      .field("token", &"[redacted]")
      .field("kind", &self.kind)
      .field("device", &self.device)
      .field("status", &self.status)
      .field("custom_status", &self.custom_status)
      .field("activities", &self.activities)
      .finish()
  }
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

pub fn load(cli: &Cli) -> Result<AppConfig, ConfigError> {
  let mut figment = Figment::from(Serialized::defaults(FileConfig::default()));

  let config_path = Path::new(&cli.config);
  let is_default_path = cli.config.as_os_str() == DEFAULT_CONFIG_PATH;
  if config_path.exists() {
    figment = figment.merge(Toml::file(config_path));
  } else if !is_default_path {
    return Err(ConfigError::ConfigNotFound(cli.config.clone()));
  }

  let mut file: FileConfig = figment.extract()?;
  env::apply_flat_env_overrides(&mut file);

  if let Some(level) = &cli.log_level {
    file.log_level = level.clone();
  }

  // Simple level names only; full EnvFilter syntax is handled in `log::init`.
  if let Ok(rust_log) = std::env::var("RUST_LOG") {
    match rust_log.to_ascii_lowercase().as_str() {
      "error" | "warn" | "info" | "debug" | "trace" => {
        file.log_level = rust_log;
      }
      _ => {}
    }
  }

  let (log_level, defaults, accounts) = resolve::resolve_config(file)?;
  Ok(AppConfig {
    log_level,
    defaults,
    accounts,
  })
}
