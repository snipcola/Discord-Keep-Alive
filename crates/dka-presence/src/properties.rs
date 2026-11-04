use crate::constants::{AccountKind, Device};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ClientProperties {
  pub os: String,
  pub browser: Option<String>,
  pub device: String,
  pub user_agent: Option<String>,
  pub client_version: Option<String>,
  pub os_version: Option<String>,
  pub os_arch: Option<String>,
  pub app_arch: Option<String>,
  pub system_locale: Option<String>,
  pub release_channel: Option<String>,
  pub browser_version: Option<String>,
  pub os_sdk_version: Option<String>,
  pub client_build_number: Option<String>,
  pub native_build_number: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Defaults {
  pub bot: ClientProperties,
  pub web: ClientProperties,
  pub desktop: ClientProperties,
  pub mobile: ClientProperties,
  pub heartbeat_version: Option<String>,
  pub capabilities: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct UserOptions {
  pub qos_heartbeat_version: Option<u64>,
  pub capabilities: Option<u64>,
}

impl Defaults {
  pub fn user_options(&self, kind: AccountKind) -> UserOptions {
    match kind {
      AccountKind::User => UserOptions {
        qos_heartbeat_version: parse_u64(&self.heartbeat_version),
        capabilities: parse_u64(&self.capabilities),
      },
      AccountKind::Bot => UserOptions::default(),
    }
  }

  pub fn client_properties(&self, kind: AccountKind, device: Option<Device>) -> &ClientProperties {
    match kind {
      AccountKind::Bot => &self.bot,
      AccountKind::User => match device.unwrap_or(Device::Web) {
        Device::Web => &self.web,
        Device::Desktop => &self.desktop,
        Device::Mobile => &self.mobile,
      },
    }
  }
}

fn parse_u64(value: &Option<String>) -> Option<u64> {
  value.as_deref()?.parse().ok()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn user_options_are_user_only() {
    let defaults = |v: Option<&str>| Defaults {
      bot: ClientProperties::default(),
      web: ClientProperties::default(),
      desktop: ClientProperties::default(),
      mobile: ClientProperties::default(),
      heartbeat_version: v.map(str::to_string),
      capabilities: v.map(str::to_string),
    };
    assert_eq!(
      defaults(Some("31")).user_options(AccountKind::User),
      UserOptions {
        qos_heartbeat_version: Some(31),
        capabilities: Some(31),
      }
    );
    assert_eq!(
      defaults(Some("31")).user_options(AccountKind::Bot),
      UserOptions::default()
    );
    assert_eq!(
      defaults(None).user_options(AccountKind::User),
      UserOptions::default()
    );
  }
}
