use std::ffi::OsString;

use crate::error::ConfigError;
use crate::load::{CONFIG_PATH_ENV, LoadedConfig, load_health_endpoint_with, load_with};
use crate::schema::fields::{
  AccountScalarField, ActivityField, ClientPropField, CustomStatusField, FieldSpec,
  GlobalDefaultField, RootField,
};
use crate::source::cli::Cli;
use crate::source::env::from_env_vars;

pub struct CatalogueEntry {
  pub group: &'static str,
  pub toml: &'static str,
  pub has_env: bool,
  pub has_cli: bool,
}

pub fn catalogue() -> Vec<CatalogueEntry> {
  let mut entries = Vec::new();
  let mut push = |group, spec: FieldSpec| {
    entries.push(CatalogueEntry {
      group,
      toml: spec.toml,
      has_env: spec.env_suffix.is_some(),
      has_cli: spec.cli_long.is_some(),
    });
  };
  for &field in RootField::ALL {
    push("root", field.spec());
  }
  for &field in AccountScalarField::ALL {
    push("account", field.spec());
  }
  for &field in CustomStatusField::ALL {
    push("custom_status", field.spec());
  }
  for &field in ActivityField::ALL {
    push("activity", field.spec());
  }
  for &field in GlobalDefaultField::ALL {
    push("defaults", field.spec());
  }
  for &field in ClientPropField::ALL {
    push("client_property", field.spec());
  }
  entries
}

pub fn load_with_env(
  cli: &Cli,
  vars: impl IntoIterator<Item = (OsString, OsString)>,
) -> Result<LoadedConfig, ConfigError> {
  let vars: Vec<_> = vars.into_iter().collect();
  let config_path = config_path_from(&vars);
  load_with(cli, config_path, from_env_vars(vars))
}

pub fn load_health_endpoint_with_env(
  cli: &Cli,
  vars: impl IntoIterator<Item = (OsString, OsString)>,
) -> Result<Option<String>, ConfigError> {
  let vars: Vec<_> = vars.into_iter().collect();
  let config_path = config_path_from(&vars);
  load_health_endpoint_with(cli, config_path, from_env_vars(vars).partial)
}

fn config_path_from(vars: &[(OsString, OsString)]) -> Option<OsString> {
  vars
    .iter()
    .find(|(key, _)| key == CONFIG_PATH_ENV)
    .map(|(_, value)| value.clone())
}
