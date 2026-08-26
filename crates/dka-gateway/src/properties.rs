use std::fmt::Write;

use dka_presence::{AccountKind, Device};
use serde_json::{Map, Value, json};

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

fn hex_bytes(bytes: &[u8]) -> String {
  let mut s = String::with_capacity(bytes.len() * 2);
  for b in bytes {
    let _ = write!(s, "{b:02x}");
  }
  s
}

fn uuid_v4() -> String {
  let mut b = rand::random::<[u8; 16]>();
  b[6] = (b[6] & 0x0f) | 0x40;
  b[8] = (b[8] & 0x3f) | 0x80;
  format!(
    "{}-{}-{}-{}-{}",
    hex_bytes(&b[..4]),
    hex_bytes(&b[4..6]),
    hex_bytes(&b[6..8]),
    hex_bytes(&b[8..10]),
    hex_bytes(&b[10..]),
  )
}

fn insert_nonempty(map: &mut Map<String, Value>, key: &str, value: &Option<String>) {
  if let Some(s) = value.as_deref().filter(|s| !s.is_empty()) {
    map.insert(key.into(), json!(s));
  }
}

fn insert_u64(map: &mut Map<String, Value>, key: &str, value: &Option<String>) {
  if let Some(s) = value.as_deref().filter(|s| !s.is_empty())
    && let Ok(n) = s.parse::<u64>()
  {
    map.insert(key.into(), json!(n));
  }
}

pub(crate) fn identify_properties(props: &ClientProperties, kind: AccountKind) -> Value {
  let mut map = Map::new();
  map.insert("os".into(), json!(props.os));
  if let Some(browser) = &props.browser {
    map.insert("browser".into(), json!(browser));
  }
  map.insert("device".into(), json!(props.device));
  if kind == AccountKind::User {
    let before = map.len();
    for (key, value) in [
      ("client_version", &props.client_version),
      ("os_version", &props.os_version),
      ("os_arch", &props.os_arch),
      ("app_arch", &props.app_arch),
      ("system_locale", &props.system_locale),
      ("release_channel", &props.release_channel),
      ("browser_version", &props.browser_version),
      ("os_sdk_version", &props.os_sdk_version),
    ] {
      insert_nonempty(&mut map, key, value);
    }
    insert_u64(&mut map, "client_build_number", &props.client_build_number);
    insert_u64(&mut map, "native_build_number", &props.native_build_number);
    if map.len() > before {
      map.insert("has_client_mods".into(), json!(false));
      map.insert("client_event_source".into(), Value::Null);
      map.insert("client_launch_id".into(), json!(uuid_v4()));
      insert_nonempty(&mut map, "browser_user_agent", &props.user_agent);
    }
  }
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
        ..Default::default()
      },
      web: ClientProperties {
        os: "Windows".into(),
        browser: Some("Firefox".into()),
        device: String::new(),
        user_agent: Some("web-ua".into()),
        ..Default::default()
      },
      desktop: ClientProperties {
        os: "Windows".into(),
        browser: Some("Discord Client".into()),
        device: "Discord Client".into(),
        user_agent: Some("desktop-ua".into()),
        ..Default::default()
      },
      mobile: ClientProperties {
        os: "iOS".into(),
        browser: Some("Discord iOS".into()),
        device: "iPhone".into(),
        user_agent: None,
        ..Default::default()
      },
    }
  }

  fn assert_uuid_v4(s: &str) {
    assert_eq!(s.len(), 36);
    let b = s.as_bytes();
    assert_eq!(&s[8..9], "-");
    assert_eq!(&s[13..14], "-");
    assert_eq!(&s[18..19], "-");
    assert_eq!(&s[23..24], "-");
    assert_eq!(b[14], b'4');
    assert!(matches!(b[19], b'8' | b'9' | b'a' | b'b'));
    for (i, c) in s.chars().enumerate() {
      if matches!(i, 8 | 13 | 18 | 23) {
        continue;
      }
      assert!(c.is_ascii_hexdigit() && !c.is_ascii_uppercase(), "{i} {c}");
    }
  }

  fn assert_rich_common(props: &Value, ua: Option<&str>) {
    assert_eq!(props["has_client_mods"], false);
    assert!(props["client_event_source"].is_null());
    assert_uuid_v4(
      props["client_launch_id"]
        .as_str()
        .expect("client_launch_id"),
    );
    assert!(props.get("launch_signature").is_none());
    match ua {
      Some(ua) => assert_eq!(props["browser_user_agent"], ua),
      None => assert!(props.get("browser_user_agent").is_none()),
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
      let props = identify_properties(raw, kind);
      assert_eq!(props["os"], os, "{label}");
      assert_eq!(props["browser"], browser.unwrap(), "{label}");
      assert_eq!(props["device"], device_name, "{label}");
      assert_eq!(raw.user_agent.as_deref(), ua, "{label}");
      assert!(props.get("has_client_mods").is_none(), "{label}");
    }
  }

  #[test]
  fn web_omits_browser_when_cleared() {
    let mut defaults = sample_defaults();
    defaults.web.browser = None;
    let props = identify_properties(
      defaults.client_properties(AccountKind::User, None),
      AccountKind::User,
    );
    assert!(props.get("browser").is_none());
  }

  #[test]
  fn identify_properties_rich_desktop() {
    let props = identify_properties(
      &ClientProperties {
        os: "Windows".into(),
        browser: Some("Discord Client".into()),
        device: "Discord Client".into(),
        user_agent: Some("desktop-ua".into()),
        client_version: Some("1.0.9000".into()),
        os_version: Some("10.0.26100".into()),
        os_arch: Some("x64".into()),
        app_arch: Some("x64".into()),
        system_locale: Some("en-US".into()),
        release_channel: Some("stable".into()),
        browser_version: Some("40.0.0".into()),
        os_sdk_version: Some("26100".into()),
        client_build_number: Some("500000".into()),
        native_build_number: Some("80000".into()),
      },
      AccountKind::User,
    );
    assert_eq!(props["os"], "Windows");
    assert_eq!(props["browser"], "Discord Client");
    assert_eq!(props["device"], "Discord Client");
    assert_eq!(props["client_version"], "1.0.9000");
    assert_eq!(props["os_version"], "10.0.26100");
    assert_eq!(props["os_arch"], "x64");
    assert_eq!(props["app_arch"], "x64");
    assert_eq!(props["system_locale"], "en-US");
    assert_eq!(props["release_channel"], "stable");
    assert_eq!(props["browser_version"], "40.0.0");
    assert_eq!(props["os_sdk_version"], "26100");
    assert_eq!(props["client_build_number"], 500000);
    assert_eq!(props["native_build_number"], 80000);
    assert_rich_common(&props, Some("desktop-ua"));
  }

  #[test]
  fn identify_properties_rich_web() {
    let props = identify_properties(
      &ClientProperties {
        os: "Windows".into(),
        browser: Some("Firefox".into()),
        device: String::new(),
        user_agent: Some("web-ua".into()),
        os_version: Some("10".into()),
        system_locale: Some("en-US".into()),
        release_channel: Some("stable".into()),
        browser_version: Some("140.0".into()),
        client_build_number: Some("500000".into()),
        ..Default::default()
      },
      AccountKind::User,
    );
    assert_eq!(props["os"], "Windows");
    assert_eq!(props["browser"], "Firefox");
    assert_eq!(props["device"], "");
    assert_eq!(props["os_version"], "10");
    assert_eq!(props["system_locale"], "en-US");
    assert_eq!(props["release_channel"], "stable");
    assert_eq!(props["browser_version"], "140.0");
    assert_eq!(props["client_build_number"], 500000);
    assert!(props.get("os_arch").is_none());
    assert!(props.get("app_arch").is_none());
    assert!(props.get("client_version").is_none());
    assert!(props.get("native_build_number").is_none());
    assert!(props.get("os_sdk_version").is_none());
    assert_rich_common(&props, Some("web-ua"));
  }

  #[test]
  fn identify_properties_rich_mobile() {
    let props = identify_properties(
      &ClientProperties {
        os: "iOS".into(),
        browser: Some("Discord iOS".into()),
        device: "iPhone".into(),
        user_agent: None,
        os_version: Some("18.0".into()),
        system_locale: Some("en-US".into()),
        release_channel: Some("stable".into()),
        ..Default::default()
      },
      AccountKind::User,
    );
    assert_eq!(props["os"], "iOS");
    assert_eq!(props["browser"], "Discord iOS");
    assert_eq!(props["device"], "iPhone");
    assert_eq!(props["os_version"], "18.0");
    assert_eq!(props["system_locale"], "en-US");
    assert_eq!(props["release_channel"], "stable");
    assert!(props.get("client_build_number").is_none());
    assert!(props.get("native_build_number").is_none());
    assert!(props.get("browser_version").is_none());
    assert!(props.get("client_version").is_none());
    assert_rich_common(&props, None);
  }

  #[test]
  fn identify_properties_bot_stays_minimal() {
    let props = identify_properties(
      &ClientProperties {
        os: "linux".into(),
        browser: Some("discord-keep-alive".into()),
        device: "discord-keep-alive".into(),
        user_agent: Some("bot-ua".into()),
        ..Default::default()
      },
      AccountKind::Bot,
    );
    let obj = props.as_object().expect("object");
    assert_eq!(obj.len(), 3);
    assert_eq!(props["os"], "linux");
    assert_eq!(props["browser"], "discord-keep-alive");
    assert_eq!(props["device"], "discord-keep-alive");
    assert!(props.get("has_client_mods").is_none());
    assert!(props.get("client_launch_id").is_none());
    assert!(props.get("browser_user_agent").is_none());
  }

  #[test]
  fn identify_properties_bot_extras_stay_minimal() {
    let props = identify_properties(
      &ClientProperties {
        os: "linux".into(),
        browser: Some("discord-keep-alive".into()),
        device: "discord-keep-alive".into(),
        user_agent: Some("bot-ua".into()),
        client_version: Some("1.0.0".into()),
        os_version: Some("6.8.0".into()),
        os_arch: Some("x64".into()),
        app_arch: Some("x64".into()),
        system_locale: Some("en-US".into()),
        release_channel: Some("stable".into()),
        browser_version: Some("1.0".into()),
        os_sdk_version: Some("0".into()),
        client_build_number: Some("1".into()),
        native_build_number: Some("1".into()),
      },
      AccountKind::Bot,
    );
    let obj = props.as_object().expect("object");
    assert_eq!(obj.len(), 3);
    assert_eq!(props["os"], "linux");
    assert_eq!(props["browser"], "discord-keep-alive");
    assert_eq!(props["device"], "discord-keep-alive");
    assert!(props.get("has_client_mods").is_none());
    assert!(props.get("client_event_source").is_none());
    assert!(props.get("client_launch_id").is_none());
    assert!(props.get("os_version").is_none());
    assert!(props.get("browser_user_agent").is_none());
  }

  #[test]
  fn identify_properties_omits_unparseable_build_numbers() {
    let props = identify_properties(
      &ClientProperties {
        os: "Windows".into(),
        device: String::new(),
        release_channel: Some("stable".into()),
        client_build_number: Some("not-a-number".into()),
        native_build_number: Some("-1".into()),
        ..Default::default()
      },
      AccountKind::User,
    );
    assert!(props.get("client_build_number").is_none());
    assert!(props.get("native_build_number").is_none());
    assert_rich_common(&props, None);
  }
}
