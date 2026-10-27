use crate::model::partial::PartialConfig;

pub const DEFAULT_LOG_LEVEL: &str = "info";
pub const LOG_LEVELS: [&str; 5] = ["error", "warn", "info", "debug", "trace"];

pub fn defaults_partial() -> PartialConfig {
  PartialConfig {
    log_level: Some(DEFAULT_LOG_LEVEL.into()),
    health_socket: None,
    ..Default::default()
  }
}
