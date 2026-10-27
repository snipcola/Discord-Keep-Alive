use std::fmt::Display;
use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::util::hint;

#[derive(Debug, Error)]
pub enum ConfigError {
  #[error("{}: {message}", file.display())]
  InvalidFile { file: PathBuf, message: String },
  #[error("config file not found: {0}")]
  ConfigNotFound(PathBuf),
  #[error("no accounts configured (use --token, TOKEN or an [[accounts]] entry in the config)")]
  NoAccounts,
  #[error("no accounts configured and {} was not found (use --token, TOKEN or --config)", .0.display())]
  NoAccountsMissingConfig(PathBuf),
  #[error("account '{0}': token is required")]
  MissingToken(String),
  #[error("account '{0}': {1}")]
  Invalid(String, String),
  #[error("{0}")]
  InvalidPath(String),
  #[error("{0}")]
  InvalidId(String),
  #[error("{0}")]
  UnknownField(String),
}

impl ConfigError {
  pub(crate) fn invalid(account: impl Into<String>, msg: impl Into<String>) -> Self {
    Self::Invalid(account.into(), msg.into())
  }

  pub(crate) fn invalid_field(
    account: impl Into<String>,
    field: &str,
    value: &str,
    hint: impl Display,
  ) -> Self {
    Self::invalid(account, format!("invalid {field} '{value}' ({hint})"))
  }

  pub(crate) fn invalid_id(id: &str, reason: &str) -> Self {
    Self::InvalidId(format!("invalid id '{id}': {reason}"))
  }

  pub(crate) fn unknown(
    kind: &str,
    name: &str,
    candidates: impl IntoIterator<Item = impl AsRef<str>>,
  ) -> Self {
    Self::UnknownField(format!(
      "unknown {kind} '{name}' ({})",
      hint(name, candidates)
    ))
  }

  pub(crate) fn unknown_key(
    file: &Path,
    scope: &str,
    key: &str,
    candidates: impl IntoIterator<Item = impl AsRef<str>>,
  ) -> Self {
    let location = if scope.is_empty() {
      String::new()
    } else {
      format!(" in {scope}")
    };
    Self::InvalidFile {
      file: file.to_path_buf(),
      message: format!("unknown key '{key}'{location} ({})", hint(key, candidates)),
    }
  }

  pub(crate) fn from_figment(file: &Path, err: &figment::Error) -> Self {
    let message = if err.path.is_empty() {
      err.kind.to_string()
    } else {
      format!("{}: {}", err.path.join("."), err.kind)
    };
    Self::InvalidFile {
      file: file.to_path_buf(),
      message,
    }
  }
}
