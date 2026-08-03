use std::path::Path;

use figment::{
  Figment,
  providers::{Format, Toml},
};

use super::ConfigError;
use super::partial::PartialConfig;

pub fn load_toml(path: &Path) -> Result<PartialConfig, ConfigError> {
  Figment::new()
    .merge(Toml::file(path))
    .extract()
    .map_err(ConfigError::from)
}
