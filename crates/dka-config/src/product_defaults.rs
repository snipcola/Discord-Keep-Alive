use dka_gateway::properties::{ClientProperties, Defaults};

const LOCALE: &str = "en-US";
const CHANNEL: &str = "stable";
const ARCH: &str = "x64";
const CLIENT_BUILD: &str = "582977";

const BOT_OS: &str = "linux";
const BOT_BROWSER: &str = "discord-keep-alive";
const BOT_DEVICE: &str = "discord-keep-alive";

const WEB_OS: &str = "Windows";
const WEB_BROWSER: &str = "Firefox";
const WEB_DEVICE: &str = "";
const WEB_BROWSER_VERSION: &str = "153.0";
const WEB_OS_VERSION: &str = "10";
const WEB_UA: &str = concat!(
  "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:153.0) ",
  "Gecko/20100101 Firefox/153.0",
);

const DESKTOP_OS: &str = "Windows";
const DESKTOP_BROWSER: &str = "Discord Client";
const DESKTOP_DEVICE: &str = "Discord Client";
const DESKTOP_CLIENT: &str = "1.0.9254";
const DESKTOP_ELECTRON: &str = "42.7.1";
const DESKTOP_OS_VERSION: &str = "10.0.26100";
const DESKTOP_SDK: &str = "26100";
const DESKTOP_NATIVE_BUILD: &str = "86777";
const DESKTOP_UA: &str = concat!(
  "Mozilla/5.0 (Windows NT 10.0; Win64; x64) ",
  "AppleWebKit/537.36 (KHTML, like Gecko) ",
  "discord/1.0.9254 Chrome/148.0.7778.280 Electron/42.7.1 Safari/537.36",
);

const MOBILE_OS: &str = "iOS";
const MOBILE_BROWSER: &str = "Discord iOS";
const MOBILE_DEVICE: &str = "iPhone";
const MOBILE_OS_VERSION: &str = "18.4";

fn s(value: &str) -> Option<String> {
  Some(value.into())
}

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
    ..Default::default()
  }
}

pub(crate) fn product_defaults() -> Defaults {
  Defaults {
    bot: props(BOT_OS, Some(BOT_BROWSER), BOT_DEVICE, None),
    web: ClientProperties {
      os_version: s(WEB_OS_VERSION),
      system_locale: s(LOCALE),
      release_channel: s(CHANNEL),
      browser_version: s(WEB_BROWSER_VERSION),
      client_build_number: s(CLIENT_BUILD),
      ..props(WEB_OS, Some(WEB_BROWSER), WEB_DEVICE, Some(WEB_UA))
    },
    desktop: ClientProperties {
      client_version: s(DESKTOP_CLIENT),
      os_version: s(DESKTOP_OS_VERSION),
      os_arch: s(ARCH),
      app_arch: s(ARCH),
      system_locale: s(LOCALE),
      release_channel: s(CHANNEL),
      browser_version: s(DESKTOP_ELECTRON),
      os_sdk_version: s(DESKTOP_SDK),
      client_build_number: s(CLIENT_BUILD),
      native_build_number: s(DESKTOP_NATIVE_BUILD),
      ..props(
        DESKTOP_OS,
        Some(DESKTOP_BROWSER),
        DESKTOP_DEVICE,
        Some(DESKTOP_UA),
      )
    },
    mobile: ClientProperties {
      os_version: s(MOBILE_OS_VERSION),
      system_locale: s(LOCALE),
      release_channel: s(CHANNEL),
      ..props(MOBILE_OS, Some(MOBILE_BROWSER), MOBILE_DEVICE, None)
    },
  }
}
