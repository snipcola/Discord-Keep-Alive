use dka_gateway::properties::{ClientProperties, Defaults};

const LOCALE: &str = "en-US";
const CHANNEL: &str = "stable";
const ARCH: &str = "x64";
const CLIENT_BUILD: &str = "649389";
const QOS_HEARTBEAT_VERSION: &str = "31";
const CAPABILITIES: &str = "1734653";
const WINDOWS_UA_PLATFORM: &str = "Windows NT 10.0; Win64; x64";

const BOT_OS: &str = "linux";
const BOT_BROWSER: &str = "discord-keep-alive";
const BOT_DEVICE: &str = "discord-keep-alive";

const WEB_OS: &str = "Windows";
const WEB_BROWSER: &str = "Firefox";
const WEB_DEVICE: &str = "";
const WEB_BROWSER_VERSION: &str = "156.0";
const WEB_OS_VERSION: &str = "10";

const DESKTOP_OS: &str = "Windows";
const DESKTOP_BROWSER: &str = "Discord Client";
const DESKTOP_DEVICE: &str = "Discord Client";
const DESKTOP_CLIENT: &str = "1.0.9265";
const DESKTOP_NATIVE_BUILD: &str = "96160";
const DESKTOP_ELECTRON: &str = "42.11.15";
const DESKTOP_CHROME: &str = "148.0.7778.280";
const DESKTOP_OS_VERSION: &str = "10.0.26200";
const DESKTOP_SDK: &str = "26200";

const MOBILE_OS: &str = "iOS";
const MOBILE_BROWSER: &str = "Discord iOS";
const MOBILE_DEVICE: &str = "iPhone";
const MOBILE_OS_VERSION: &str = "27.0.1";

fn web_ua() -> String {
  format!(
    "Mozilla/5.0 ({WINDOWS_UA_PLATFORM}; rv:{WEB_BROWSER_VERSION}) \
     Gecko/20100101 Firefox/{WEB_BROWSER_VERSION}"
  )
}

fn desktop_ua() -> String {
  format!(
    "Mozilla/5.0 ({WINDOWS_UA_PLATFORM}) \
     AppleWebKit/537.36 (KHTML, like Gecko) \
     discord/{DESKTOP_CLIENT} Chrome/{DESKTOP_CHROME} \
     Electron/{DESKTOP_ELECTRON} Safari/537.36"
  )
}

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
      ..props(WEB_OS, Some(WEB_BROWSER), WEB_DEVICE, Some(&web_ua()))
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
        Some(&desktop_ua()),
      )
    },
    mobile: ClientProperties {
      os_version: s(MOBILE_OS_VERSION),
      system_locale: s(LOCALE),
      release_channel: s(CHANNEL),
      ..props(MOBILE_OS, Some(MOBILE_BROWSER), MOBILE_DEVICE, None)
    },
    heartbeat_version: s(QOS_HEARTBEAT_VERSION),
    capabilities: s(CAPABILITIES),
  }
}
