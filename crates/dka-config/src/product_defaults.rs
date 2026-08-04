use dka_gateway::properties::{ClientProperties, Defaults};

macro_rules! join_space {
  ($first:literal $(, $rest:literal)* $(,)?) => {
    concat!($first $(, " ", $rest)*)
  };
}

const DEFAULT_BOT_OS: &str = "linux";
const DEFAULT_BOT_BROWSER: &str = "discord-keep-alive";
const DEFAULT_BOT_DEVICE: &str = "discord-keep-alive";

const DEFAULT_WEB_OS: &str = "Windows";
const DEFAULT_WEB_BROWSER: &str = "Firefox";
const DEFAULT_WEB_DEVICE: &str = "";
const DEFAULT_WEB_UA: &str = join_space!(
  "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:153.0)",
  "Gecko/20100101 Firefox/153.0",
);

const DEFAULT_DESKTOP_OS: &str = "Windows";
const DEFAULT_DESKTOP_BROWSER: &str = "Discord Client";
const DEFAULT_DESKTOP_DEVICE: &str = "Discord Client";
const DEFAULT_DESKTOP_UA: &str = join_space!(
  "Mozilla/5.0 (Windows NT 10.0; Win64; x64)",
  "AppleWebKit/537.36 (KHTML, like Gecko)",
  "discord/1.0.9250 Chrome/148.0.7778.280",
  "Electron/42.7.1 Safari/537.36",
);

const DEFAULT_MOBILE_OS: &str = "iOS";
const DEFAULT_MOBILE_BROWSER: &str = "Discord iOS";
const DEFAULT_MOBILE_DEVICE: &str = "iPhone";

fn props(
  os: &str,
  browser: Option<&str>,
  device: &str,
  user_agent: Option<&str>,
) -> ClientProperties {
  ClientProperties {
    os: os.into(),
    browser: browser.map(str::to_string),
    device: device.into(),
    user_agent: user_agent.map(str::to_string),
  }
}

pub(crate) fn product_defaults() -> Defaults {
  Defaults {
    bot: props(
      DEFAULT_BOT_OS,
      Some(DEFAULT_BOT_BROWSER),
      DEFAULT_BOT_DEVICE,
      None,
    ),
    web: props(
      DEFAULT_WEB_OS,
      Some(DEFAULT_WEB_BROWSER),
      DEFAULT_WEB_DEVICE,
      Some(DEFAULT_WEB_UA),
    ),
    desktop: props(
      DEFAULT_DESKTOP_OS,
      Some(DEFAULT_DESKTOP_BROWSER),
      DEFAULT_DESKTOP_DEVICE,
      Some(DEFAULT_DESKTOP_UA),
    ),
    mobile: props(
      DEFAULT_MOBILE_OS,
      Some(DEFAULT_MOBILE_BROWSER),
      DEFAULT_MOBILE_DEVICE,
      None,
    ),
  }
}
