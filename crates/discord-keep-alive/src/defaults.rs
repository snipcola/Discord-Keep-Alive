use dka_gateway::properties::{ClientProperties, Defaults};

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

pub fn product_defaults() -> Defaults {
  Defaults {
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
