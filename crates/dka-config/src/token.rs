use std::fmt;
use std::ops::Deref;
use std::str::FromStr;

use secrecy::{ExposeSecret, SecretString as SecrecyString};
use serde::Deserialize;

/// Token wrapper: redacted Debug, zeroized on drop via `secrecy`.
#[derive(Clone, Default, Deserialize)]
#[serde(transparent)]
pub struct SecretString(SecrecyString);

impl SecretString {
  pub fn new(value: impl Into<String>) -> Self {
    Self(SecrecyString::from(value.into()))
  }

  pub fn into_inner(self) -> String {
    self.0.expose_secret().to_string()
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
    self.0.expose_secret()
  }
}

impl From<String> for SecretString {
  fn from(value: String) -> Self {
    Self::new(value)
  }
}

impl From<&str> for SecretString {
  fn from(value: &str) -> Self {
    Self::new(value)
  }
}

impl FromStr for SecretString {
  type Err = std::convert::Infallible;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    Ok(Self::new(s))
  }
}

impl PartialEq for SecretString {
  fn eq(&self, other: &Self) -> bool {
    self.0.expose_secret() == other.0.expose_secret()
  }
}

impl Eq for SecretString {}

impl PartialEq<str> for SecretString {
  fn eq(&self, other: &str) -> bool {
    self.0.expose_secret() == other
  }
}

impl PartialEq<&str> for SecretString {
  fn eq(&self, other: &&str) -> bool {
    self.0.expose_secret() == *other
  }
}
