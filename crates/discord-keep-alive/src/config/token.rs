use std::fmt;
use std::ops::Deref;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// Credential wrapper; `Debug` prints `<redacted>`.
#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SecretString(String);

impl SecretString {
  pub fn new(value: impl Into<String>) -> Self {
    Self(value.into())
  }

  pub fn into_inner(self) -> String {
    self.0
  }
}

impl fmt::Debug for SecretString {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str("<redacted>")
  }
}

impl Deref for SecretString {
  type Target = str;

  fn deref(&self) -> &str {
    &self.0
  }
}

impl From<String> for SecretString {
  fn from(value: String) -> Self {
    Self(value)
  }
}

impl From<&str> for SecretString {
  fn from(value: &str) -> Self {
    Self(value.to_string())
  }
}

impl FromStr for SecretString {
  type Err = std::convert::Infallible;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    Ok(Self(s.to_string()))
  }
}

impl PartialEq<str> for SecretString {
  fn eq(&self, other: &str) -> bool {
    self.0 == other
  }
}

impl PartialEq<&str> for SecretString {
  fn eq(&self, other: &&str) -> bool {
    self.0 == *other
  }
}
