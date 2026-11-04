use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigWarning {
  UnknownEnvKey {
    key: String,
    suggestion: Option<String>,
  },
  SuspectEnvKey {
    key: String,
    kind: &'static str,
    id: String,
    suggestion: String,
  },
}

impl fmt::Display for ConfigWarning {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::UnknownEnvKey { key, suggestion } => {
        write!(
          f,
          "environment variable {key} matches no setting and is ignored"
        )?;
        match suggestion {
          Some(suggestion) => write!(f, " (did you mean '{suggestion}'?)"),
          None => Ok(()),
        }
      }
      Self::SuspectEnvKey {
        key,
        kind,
        id,
        suggestion,
      } => write!(
        f,
        "environment variable {key} defines {kind} '{id}', not a setting (did you mean '{suggestion}'?)"
      ),
    }
  }
}
