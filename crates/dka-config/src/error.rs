use std::path::PathBuf;

use thiserror::Error;

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
  #[error("invalid config path: {0}")]
  InvalidPath(String),
  #[error("invalid id: {0}")]
  InvalidId(String),
  #[error("unknown field: {0}")]
  UnknownField(String),
}

impl ConfigError {
  pub(crate) fn invalid(account: impl Into<String>, msg: impl Into<String>) -> Self {
    Self::Invalid(account.into(), msg.into())
  }

  pub(crate) fn invalid_field(account: impl Into<String>, field: &str, v: &str) -> Self {
    Self::Invalid(account.into(), format!("invalid {field} '{v}'"))
  }
}

impl From<figment::Error> for ConfigError {
  fn from(err: figment::Error) -> Self {
    Self::Figment(Box::new(err))
  }
}
