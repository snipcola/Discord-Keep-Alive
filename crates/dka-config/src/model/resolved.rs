use dka_gateway::properties::Defaults;
use dka_presence::{AccountKind, ActivityConfig, CustomStatusConfig, Device, Status};

use crate::token::SecretString;

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
  pub token: SecretString,
  pub kind: AccountKind,
  pub device: Option<Device>,
  pub status: Option<Status>,
  pub custom_status: Option<CustomStatusConfig>,
  pub activities: Vec<ActivityConfig>,
}
