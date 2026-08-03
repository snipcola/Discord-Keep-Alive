use dka_presence::{AccountKind, Device};
use serde_json::{Map, Value, json};

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

pub fn identify_properties(props: &ClientProperties) -> Value {
  let mut map = Map::new();
  map.insert("os".into(), json!(props.os));
  if let Some(browser) = &props.browser {
    map.insert("browser".into(), json!(browser));
  }
  map.insert("device".into(), json!(props.device));
  Value::Object(map)
}

#[cfg(test)]
mod tests {
  use super::*;

  fn sample_defaults() -> Defaults {
    Defaults {
      bot: ClientProperties {
        os: "FreeBSD".into(),
        browser: Some("lib".into()),
        device: "lib".into(),
        user_agent: Some("bot-ua".into()),
      },
      web: ClientProperties {
        os: "Windows".into(),
        browser: Some("Firefox".into()),
        device: String::new(),
        user_agent: Some("web-ua".into()),
      },
      desktop: ClientProperties {
        os: "Windows".into(),
        browser: Some("Discord Client".into()),
        device: "Discord Client".into(),
        user_agent: Some("desktop-ua".into()),
      },
      mobile: ClientProperties {
        os: "iOS".into(),
        browser: Some("Discord iOS".into()),
        device: "iPhone".into(),
        user_agent: None,
      },
    }
  }

  #[test]
  fn bot_uses_bot_defaults() {
    let defaults = sample_defaults();
    let props =
      identify_properties(defaults.client_properties(AccountKind::Bot, Some(Device::Desktop)));
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
  fn web_defaults_selected_for_user() {
    let defaults = sample_defaults();
    let props =
      identify_properties(defaults.client_properties(AccountKind::User, Some(Device::Web)));
    assert_eq!(props["os"], "Windows");
    assert_eq!(props["browser"], "Firefox");
    assert_eq!(props["device"], "");
    assert_eq!(
      defaults
        .client_properties(AccountKind::User, Some(Device::Web))
        .user_agent
        .as_deref(),
      Some("web-ua")
    );
  }

  #[test]
  fn web_omits_browser_when_cleared() {
    let mut defaults = sample_defaults();
    defaults.web.browser = None;
    let props = identify_properties(defaults.client_properties(AccountKind::User, None));
    assert!(props.get("browser").is_none());
  }

  #[test]
  fn desktop_and_mobile_templates() {
    let defaults = sample_defaults();
    let desktop =
      identify_properties(defaults.client_properties(AccountKind::User, Some(Device::Desktop)));
    assert_eq!(desktop["browser"], "Discord Client");
    assert_eq!(desktop["device"], "Discord Client");
    assert_eq!(
      defaults
        .client_properties(AccountKind::User, Some(Device::Desktop))
        .user_agent
        .as_deref(),
      Some("desktop-ua")
    );

    let mobile =
      identify_properties(defaults.client_properties(AccountKind::User, Some(Device::Mobile)));
    assert_eq!(mobile["os"], "iOS");
    assert_eq!(mobile["browser"], "Discord iOS");
    assert_eq!(mobile["device"], "iPhone");
    assert!(
      defaults
        .client_properties(AccountKind::User, Some(Device::Mobile))
        .user_agent
        .is_none()
    );
  }
}
