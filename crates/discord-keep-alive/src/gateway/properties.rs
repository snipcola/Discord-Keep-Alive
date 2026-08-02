use dka_presence::{AccountKind, Device};
use serde_json::{Map, Value, json};

macro_rules! join_space {
  ($first:literal $(, $rest:literal)* $(,)?) => {
    concat!($first $(, " ", $rest)*)
  };
}

pub const DEFAULT_BOT_OS: &str = "linux";
pub const DEFAULT_BOT_BROWSER: &str = "discord-keep-alive";
pub const DEFAULT_BOT_DEVICE: &str = "discord-keep-alive";

pub const DEFAULT_WEB_OS: &str = "Windows";
pub const DEFAULT_WEB_BROWSER: &str = "Firefox";
pub const DEFAULT_WEB_DEVICE: &str = "";
pub const DEFAULT_WEB_UA: &str = join_space!(
  "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:153.0)",
  "Gecko/20100101 Firefox/153.0",
);

pub const DEFAULT_DESKTOP_OS: &str = "Windows";
pub const DEFAULT_DESKTOP_BROWSER: &str = "Discord Client";
pub const DEFAULT_DESKTOP_DEVICE: &str = "Discord Client";
pub const DEFAULT_DESKTOP_UA: &str = join_space!(
  "Mozilla/5.0 (Windows NT 10.0; Win64; x64)",
  "AppleWebKit/537.36 (KHTML, like Gecko)",
  "discord/1.0.9250 Chrome/148.0.7778.280",
  "Electron/42.7.1 Safari/537.36",
);

pub const DEFAULT_MOBILE_OS: &str = "iOS";
pub const DEFAULT_MOBILE_BROWSER: &str = "Discord iOS";
pub const DEFAULT_MOBILE_DEVICE: &str = "iPhone";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientProperties {
  pub os: String,
  pub browser: Option<String>,
  pub device: String,
  pub user_agent: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Defaults {
  pub bot: ClientProperties,
  pub web: ClientProperties,
  pub desktop: ClientProperties,
  pub mobile: ClientProperties,
}

impl Defaults {
  pub fn builtin() -> Self {
    Self {
      bot: ClientProperties {
        os: DEFAULT_BOT_OS.into(),
        browser: Some(DEFAULT_BOT_BROWSER.into()),
        device: DEFAULT_BOT_DEVICE.into(),
        user_agent: None,
      },
      web: ClientProperties {
        os: DEFAULT_WEB_OS.into(),
        browser: Some(DEFAULT_WEB_BROWSER.into()),
        device: DEFAULT_WEB_DEVICE.into(),
        user_agent: Some(DEFAULT_WEB_UA.into()),
      },
      desktop: ClientProperties {
        os: DEFAULT_DESKTOP_OS.into(),
        browser: Some(DEFAULT_DESKTOP_BROWSER.into()),
        device: DEFAULT_DESKTOP_DEVICE.into(),
        user_agent: Some(DEFAULT_DESKTOP_UA.into()),
      },
      mobile: ClientProperties {
        os: DEFAULT_MOBILE_OS.into(),
        browser: Some(DEFAULT_MOBILE_BROWSER.into()),
        device: DEFAULT_MOBILE_DEVICE.into(),
        user_agent: None,
      },
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

pub fn identify_properties(
  kind: AccountKind,
  device: Option<Device>,
  defaults: &Defaults,
) -> Value {
  let set = defaults.client_properties(kind, device);

  let mut props = Map::new();
  props.insert("os".into(), json!(set.os));
  if let Some(browser) = &set.browser {
    props.insert("browser".into(), json!(browser));
  }
  props.insert("device".into(), json!(set.device));
  Value::Object(props)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn bot_uses_bot_defaults() {
    let mut defaults = Defaults::builtin();
    defaults.bot = ClientProperties {
      os: "FreeBSD".into(),
      browser: Some("lib".into()),
      device: "lib".into(),
      user_agent: Some("bot-ua".into()),
    };
    let props = identify_properties(AccountKind::Bot, Some(Device::Desktop), &defaults);
    assert_eq!(props["os"], "FreeBSD");
    assert_eq!(props["browser"], "lib");
    assert_eq!(props["device"], "lib");
    assert_eq!(
      defaults
        .client_properties(AccountKind::Bot, None)
        .user_agent
        .as_deref(),
      Some("bot-ua")
    );
  }

  #[test]
  fn web_defaults_to_firefox() {
    let defaults = Defaults::builtin();
    let props = identify_properties(AccountKind::User, Some(Device::Web), &defaults);
    assert_eq!(props["os"], DEFAULT_WEB_OS);
    assert_eq!(props["browser"], DEFAULT_WEB_BROWSER);
    assert_eq!(props["device"], DEFAULT_WEB_DEVICE);
    assert_eq!(
      defaults
        .client_properties(AccountKind::User, Some(Device::Web))
        .user_agent
        .as_deref(),
      Some(DEFAULT_WEB_UA)
    );
  }

  #[test]
  fn web_omits_browser_when_cleared() {
    let mut defaults = Defaults::builtin();
    defaults.web.browser = None;
    let props = identify_properties(AccountKind::User, None, &defaults);
    assert!(props.get("browser").is_none());
  }

  #[test]
  fn desktop_and_mobile_templates() {
    let defaults = Defaults::builtin();
    let desktop = identify_properties(AccountKind::User, Some(Device::Desktop), &defaults);
    assert_eq!(desktop["browser"], DEFAULT_DESKTOP_BROWSER);
    assert_eq!(desktop["device"], DEFAULT_DESKTOP_DEVICE);
    assert_eq!(
      defaults
        .client_properties(AccountKind::User, Some(Device::Desktop))
        .user_agent
        .as_deref(),
      Some(DEFAULT_DESKTOP_UA)
    );

    let mobile = identify_properties(AccountKind::User, Some(Device::Mobile), &defaults);
    assert_eq!(mobile["os"], DEFAULT_MOBILE_OS);
    assert_eq!(mobile["browser"], DEFAULT_MOBILE_BROWSER);
    assert_eq!(mobile["device"], DEFAULT_MOBILE_DEVICE);
    assert!(
      defaults
        .client_properties(AccountKind::User, Some(Device::Mobile))
        .user_agent
        .is_none()
    );
  }
}
