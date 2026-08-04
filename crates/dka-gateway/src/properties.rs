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

pub(crate) fn identify_properties(props: &ClientProperties) -> Value {
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
  fn client_property_templates() {
    let defaults = sample_defaults();
    let cases = [
      (
        "bot",
        AccountKind::Bot,
        Some(Device::Desktop),
        "FreeBSD",
        Some("lib"),
        "lib",
        Some("bot-ua"),
      ),
      (
        "web",
        AccountKind::User,
        Some(Device::Web),
        "Windows",
        Some("Firefox"),
        "",
        Some("web-ua"),
      ),
      (
        "desktop",
        AccountKind::User,
        Some(Device::Desktop),
        "Windows",
        Some("Discord Client"),
        "Discord Client",
        Some("desktop-ua"),
      ),
      (
        "mobile",
        AccountKind::User,
        Some(Device::Mobile),
        "iOS",
        Some("Discord iOS"),
        "iPhone",
        None,
      ),
    ];
    for (label, kind, device, os, browser, device_name, ua) in cases {
      let raw = defaults.client_properties(kind, device);
      let props = identify_properties(raw);
      assert_eq!(props["os"], os, "{label}");
      assert_eq!(props["browser"], browser.unwrap(), "{label}");
      assert_eq!(props["device"], device_name, "{label}");
      assert_eq!(raw.user_agent.as_deref(), ua, "{label}");
    }
  }

  #[test]
  fn web_omits_browser_when_cleared() {
    let mut defaults = sample_defaults();
    defaults.web.browser = None;
    let props = identify_properties(defaults.client_properties(AccountKind::User, None));
    assert!(props.get("browser").is_none());
  }
}
