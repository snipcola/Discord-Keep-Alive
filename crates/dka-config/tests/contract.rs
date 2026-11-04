use std::ffi::OsString;
use std::sync::atomic::{AtomicU64, Ordering};

use clap::Parser;
use dka_config::contract_support::{catalogue, load_health_endpoint_with_env, load_with_env};
use dka_config::{AccountConfig, AppConfig, Cli};
use dka_presence::{ActivityConfig, CustomStatusConfig};

struct Field {
  toml: &'static str,
  env: Option<&'static str>,
  cli: Option<&'static str>,
  set: &'static str,
  value: &'static str,
  reached: fn(&AppConfig) -> String,
}

struct Section {
  group: &'static str,
  table: &'static str,
  siblings: &'static [(&'static str, &'static str)],
  fields: &'static [Field],
}

fn s(value: &Option<String>) -> String {
  value.clone().unwrap_or_default()
}

fn acct(c: &AppConfig) -> &AccountConfig {
  &c.accounts[0]
}

fn act(c: &AppConfig) -> ActivityConfig {
  acct(c).activities.first().cloned().unwrap_or_default()
}

fn cs(c: &AppConfig) -> CustomStatusConfig {
  acct(c).custom_status.clone().unwrap_or_default()
}

#[rustfmt::skip]
static SECTIONS: &[Section] = &[
  Section { group: "root", table: "", siblings: &[], fields: &[
    Field { toml: "log_level", env: Some("LOG_LEVEL"), cli: Some("log-level"), set: "log_level", value: "debug", reached: |c| c.log_level.clone() },
    Field { toml: "health_socket", env: Some("HEALTH_SOCKET"), cli: Some("health-socket"), set: "health_socket", value: "/tmp/contract.sock", reached: |c| s(&c.health_socket) },
  ] },
  Section { group: "account", table: "", siblings: &[], fields: &[
    Field { toml: "token", env: Some("TOKEN"), cli: Some("token"), set: "token", value: "contract-token", reached: |c| acct(c).token.to_string() },
    Field { toml: "name", env: Some("ACCOUNT"), cli: Some("account"), set: "name", value: "contract-name", reached: |c| acct(c).name.clone() },
    Field { toml: "kind", env: Some("KIND"), cli: Some("kind"), set: "kind", value: "bot", reached: |c| acct(c).kind.to_string() },
    Field { toml: "device", env: Some("DEVICE"), cli: Some("device"), set: "device", value: "mobile", reached: |c| acct(c).device.map(|d| d.to_string()).unwrap_or_default() },
    Field { toml: "status", env: Some("STATUS"), cli: Some("status"), set: "status", value: "dnd", reached: |c| acct(c).status.map(|v| v.to_string()).unwrap_or_default() },
  ] },
  Section { group: "custom_status", table: "custom_status", siblings: &[("text", "base-text")], fields: &[
    Field { toml: "text", env: Some("CUSTOM_STATUS"), cli: Some("custom-status"), set: "custom_status.text", value: "contract-text", reached: |c| s(&cs(c).text) },
    Field { toml: "emoji", env: Some("CUSTOM_STATUS_EMOJI"), cli: Some("custom-status-emoji"), set: "custom_status.emoji", value: "contract-emoji", reached: |c| s(&cs(c).emoji) },
  ] },
  Section { group: "activity", table: "activity", siblings: &[("name", "base-name")], fields: &[
    Field { toml: "name", env: Some("ACTIVITY"), cli: Some("activity"), set: "activity.name", value: "contract-name", reached: |c| s(&act(c).name) },
    Field { toml: "type", env: Some("ACTIVITY_TYPE"), cli: Some("activity-type"), set: "activity.type", value: "watching", reached: |c| act(c).activity_type.map(|t| t.to_string()).unwrap_or_default() },
    Field { toml: "platform", env: Some("ACTIVITY_PLATFORM"), cli: Some("activity-platform"), set: "activity.platform", value: "xbox", reached: |c| act(c).platform.map(|p| p.to_string()).unwrap_or_default() },
    Field { toml: "timestamp", env: Some("ACTIVITY_TIMESTAMP"), cli: Some("activity-timestamp"), set: "activity.timestamp", value: "1700000001", reached: |c| s(&act(c).timestamp) },
    Field { toml: "application_id", env: Some("ACTIVITY_APPLICATION_ID"), cli: Some("activity-application-id"), set: "activity.application_id", value: "777", reached: |c| act(c).application_id },
    Field { toml: "details", env: Some("ACTIVITY_DETAILS"), cli: Some("activity-details"), set: "activity.details", value: "contract-details", reached: |c| s(&act(c).details) },
    Field { toml: "url", env: Some("ACTIVITY_URL"), cli: Some("activity-url"), set: "activity.url", value: "https://contract.example/url", reached: |c| s(&act(c).url) },
    Field { toml: "large_image", env: Some("ACTIVITY_LARGE_IMAGE"), cli: Some("activity-large-image"), set: "activity.large_image", value: "contract-large", reached: |c| s(&act(c).large_image.image) },
    Field { toml: "large_image_text", env: Some("ACTIVITY_LARGE_IMAGE_TEXT"), cli: Some("activity-large-image-text"), set: "activity.large_image_text", value: "contract-large-text", reached: |c| s(&act(c).large_image.text) },
    Field { toml: "small_image", env: Some("ACTIVITY_SMALL_IMAGE"), cli: Some("activity-small-image"), set: "activity.small_image", value: "contract-small", reached: |c| s(&act(c).small_image.image) },
    Field { toml: "small_image_text", env: Some("ACTIVITY_SMALL_IMAGE_TEXT"), cli: Some("activity-small-image-text"), set: "activity.small_image_text", value: "contract-small-text", reached: |c| s(&act(c).small_image.text) },
    Field { toml: "button", env: Some("ACTIVITY_BUTTON"), cli: Some("activity-button"), set: "activity.button", value: "contract-button", reached: |c| s(&act(c).button.name) },
    Field { toml: "button_url", env: Some("ACTIVITY_BUTTON_URL"), cli: Some("activity-button-url"), set: "activity.button_url", value: "https://contract.example/button", reached: |c| s(&act(c).button.url) },
    Field { toml: "button_2", env: Some("ACTIVITY_BUTTON_2"), cli: Some("activity-button-2"), set: "activity.button_2", value: "contract-button-2", reached: |c| s(&act(c).button_2.name) },
    Field { toml: "button_2_url", env: Some("ACTIVITY_BUTTON_2_URL"), cli: Some("activity-button-2-url"), set: "activity.button_2_url", value: "https://contract.example/button-2", reached: |c| s(&act(c).button_2.url) },
    Field { toml: "party_id", env: Some("ACTIVITY_PARTY_ID"), cli: Some("activity-party-id"), set: "activity.party_id", value: "contract-party", reached: |c| act(c).party.id },
    Field { toml: "party_current", env: Some("ACTIVITY_PARTY_CURRENT"), cli: Some("activity-party-current"), set: "activity.party_current", value: "3", reached: |c| s(&act(c).party.current) },
    Field { toml: "party_max", env: Some("ACTIVITY_PARTY_MAX"), cli: Some("activity-party-max"), set: "activity.party_max", value: "5", reached: |c| s(&act(c).party.max) },
  ] },
  Section { group: "defaults", table: "defaults", siblings: &[], fields: &[
    Field { toml: "heartbeat_version", env: Some("DEFAULTS_HEARTBEAT_VERSION"), cli: None, set: "defaults.heartbeat_version", value: "77", reached: |c| s(&c.defaults.heartbeat_version) },
    Field { toml: "capabilities", env: Some("DEFAULTS_CAPABILITIES"), cli: None, set: "defaults.capabilities", value: "88", reached: |c| s(&c.defaults.capabilities) },
  ] },
  Section { group: "client_property", table: "defaults.web", siblings: &[], fields: &[
    Field { toml: "os", env: Some("DEFAULTS_WEB_OS"), cli: None, set: "defaults.web.os", value: "contract-os", reached: |c| c.defaults.web.os.clone() },
    Field { toml: "browser", env: Some("DEFAULTS_WEB_BROWSER"), cli: None, set: "defaults.web.browser", value: "contract-browser", reached: |c| s(&c.defaults.web.browser) },
    Field { toml: "device", env: Some("DEFAULTS_WEB_DEVICE"), cli: None, set: "defaults.web.device", value: "contract-device", reached: |c| c.defaults.web.device.clone() },
    Field { toml: "user_agent", env: Some("DEFAULTS_WEB_USER_AGENT"), cli: None, set: "defaults.web.user_agent", value: "contract-agent", reached: |c| s(&c.defaults.web.user_agent) },
    Field { toml: "client_version", env: Some("DEFAULTS_WEB_CLIENT_VERSION"), cli: None, set: "defaults.web.client_version", value: "contract-client-version", reached: |c| s(&c.defaults.web.client_version) },
    Field { toml: "os_version", env: Some("DEFAULTS_WEB_OS_VERSION"), cli: None, set: "defaults.web.os_version", value: "contract-os-version", reached: |c| s(&c.defaults.web.os_version) },
    Field { toml: "os_arch", env: Some("DEFAULTS_WEB_OS_ARCH"), cli: None, set: "defaults.web.os_arch", value: "contract-os-arch", reached: |c| s(&c.defaults.web.os_arch) },
    Field { toml: "app_arch", env: Some("DEFAULTS_WEB_APP_ARCH"), cli: None, set: "defaults.web.app_arch", value: "contract-app-arch", reached: |c| s(&c.defaults.web.app_arch) },
    Field { toml: "system_locale", env: Some("DEFAULTS_WEB_SYSTEM_LOCALE"), cli: None, set: "defaults.web.system_locale", value: "contract-locale", reached: |c| s(&c.defaults.web.system_locale) },
    Field { toml: "release_channel", env: Some("DEFAULTS_WEB_RELEASE_CHANNEL"), cli: None, set: "defaults.web.release_channel", value: "contract-channel", reached: |c| s(&c.defaults.web.release_channel) },
    Field { toml: "browser_version", env: Some("DEFAULTS_WEB_BROWSER_VERSION"), cli: None, set: "defaults.web.browser_version", value: "contract-browser-version", reached: |c| s(&c.defaults.web.browser_version) },
    Field { toml: "os_sdk_version", env: Some("DEFAULTS_WEB_OS_SDK_VERSION"), cli: None, set: "defaults.web.os_sdk_version", value: "contract-sdk", reached: |c| s(&c.defaults.web.os_sdk_version) },
    Field { toml: "client_build_number", env: Some("DEFAULTS_WEB_CLIENT_BUILD_NUMBER"), cli: None, set: "defaults.web.client_build_number", value: "contract-build", reached: |c| s(&c.defaults.web.client_build_number) },
    Field { toml: "native_build_number", env: Some("DEFAULTS_WEB_NATIVE_BUILD_NUMBER"), cli: None, set: "defaults.web.native_build_number", value: "contract-native-build", reached: |c| s(&c.defaults.web.native_build_number) },
  ] },
  Section { group: "profile", table: "defaults.bot", siblings: &[], fields: &[
    Field { toml: "os", env: Some("DEFAULTS_BOT_OS"), cli: None, set: "defaults.bot.os", value: "contract-bot-os", reached: |c| c.defaults.bot.os.clone() },
  ] },
  Section { group: "profile", table: "defaults.desktop", siblings: &[], fields: &[
    Field { toml: "os", env: Some("DEFAULTS_DESKTOP_OS"), cli: None, set: "defaults.desktop.os", value: "contract-desktop-os", reached: |c| c.defaults.desktop.os.clone() },
  ] },
  Section { group: "profile", table: "defaults.mobile", siblings: &[], fields: &[
    Field { toml: "os", env: Some("DEFAULTS_MOBILE_OS"), cli: None, set: "defaults.mobile.os", value: "contract-mobile-os", reached: |c| c.defaults.mobile.os.clone() },
  ] },
];

fn toml_doc(section: &Section, field: &Field, include_field: bool) -> String {
  let mut doc = String::new();
  if field.toml != "token" {
    doc.push_str("token = \"base-token\"\n");
  }
  let siblings: Vec<_> = section
    .siblings
    .iter()
    .filter(|(key, _)| *key != field.toml)
    .collect();
  if section.table.is_empty() {
    if include_field {
      doc.push_str(&format!("{} = \"{}\"\n", field.toml, field.value));
    }
    return doc;
  }
  if include_field || !siblings.is_empty() {
    doc.push_str(&format!("[{}]\n", section.table));
  }
  for (key, value) in siblings {
    doc.push_str(&format!("{key} = \"{value}\"\n"));
  }
  if include_field {
    doc.push_str(&format!("{} = \"{}\"\n", field.toml, field.value));
  }
  doc
}

static TEMP_SEQ: AtomicU64 = AtomicU64::new(0);

fn load_through(
  doc: &str,
  args: &[String],
  vars: Vec<(OsString, OsString)>,
) -> Result<AppConfig, String> {
  let n = TEMP_SEQ.fetch_add(1, Ordering::Relaxed);
  let path = std::env::temp_dir().join(format!("dka-contract-{n}-{}.toml", std::process::id()));
  std::fs::write(&path, doc).expect("write temp config");
  let mut argv = vec!["dka".to_string(), "-c".into(), path.display().to_string()];
  argv.extend(args.iter().cloned());
  let result = Cli::try_parse_from(argv)
    .map_err(|err| format!("rejected by the argument parser: {err}"))
    .and_then(|cli| {
      load_with_env(&cli, vars)
        .map(|loaded| loaded.config)
        .map_err(|err| err.to_string())
    });
  let _ = std::fs::remove_file(&path);
  result
}

struct TempConfig(std::path::PathBuf);

impl TempConfig {
  fn write(doc: &str) -> Self {
    let n = TEMP_SEQ.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("dka-p6-{n}-{}.toml", std::process::id()));
    std::fs::write(&path, doc).expect("write temp config");
    Self(path)
  }

  fn arg(&self) -> String {
    self.0.display().to_string()
  }
}

impl Drop for TempConfig {
  fn drop(&mut self) {
    let _ = std::fs::remove_file(&self.0);
  }
}

fn parse_cli(config: &str, args: &[String]) -> Result<Cli, String> {
  let mut argv = vec!["dka".to_string(), "-c".into(), config.to_string()];
  argv.extend(args.iter().cloned());
  Cli::try_parse_from(argv).map_err(|err| format!("rejected by the argument parser: {err}"))
}

#[test]
fn every_field_reaches_app_config_through_every_source() {
  let mut failures = Vec::new();
  for section in SECTIONS {
    for field in section.fields {
      let name = format!("{} field '{}'", section.group, field.toml);
      let value = field.value;
      let mut attempts = vec![(
        format!("TOML key '{}'", field.toml),
        load_through(&toml_doc(section, field, true), &[], vec![]),
      )];
      let base = toml_doc(section, field, false);
      if let Some(env) = field.env {
        attempts.push((
          format!("env {env}"),
          load_through(&base, &[], vec![(env.into(), value.into())]),
        ));
      }
      if let Some(flag) = field.cli {
        attempts.push((
          format!("flag --{flag}"),
          load_through(&base, &[format!("--{flag}"), value.into()], vec![]),
        ));
      }
      attempts.push((
        format!("--set {}", field.set),
        load_through(
          &base,
          &["--set".into(), format!("{}={value}", field.set)],
          vec![],
        ),
      ));

      for (source, loaded) in attempts {
        match loaded {
          Err(err) => failures.push(format!("{name} via {source}: load failed: {err}")),
          Ok(config) => {
            let got = (field.reached)(&config);
            if got != value {
              failures.push(format!(
                "{name} via {source}: expected '{value}' in AppConfig, got '{got}'"
              ));
            }
          }
        }
      }
    }
  }
  assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}

#[test]
fn every_catalogue_field_has_a_contract_row() {
  let mut problems = Vec::new();
  for entry in catalogue() {
    let row = SECTIONS
      .iter()
      .filter(|section| section.group == entry.group)
      .flat_map(|section| section.fields)
      .find(|field| field.toml == entry.toml);
    let name = format!("{} field '{}'", entry.group, entry.toml);
    match row {
      None => problems.push(format!(
        "{name}: no contract row (TOML, env, flag and --set sources are untested)"
      )),
      Some(field) => {
        if entry.has_env && field.env.is_none() {
          problems.push(format!(
            "{name}: catalogue has an env key, the contract row has none"
          ));
        }
        if entry.has_cli && field.cli.is_none() {
          problems.push(format!(
            "{name}: catalogue has a flag, the contract row has none"
          ));
        }
      }
    }
  }
  assert!(problems.is_empty(), "\n{}", problems.join("\n"));
}

fn vars(pairs: &[(&str, &str)]) -> Vec<(OsString, OsString)> {
  pairs
    .iter()
    .map(|&(key, value)| (key.into(), value.into()))
    .collect()
}

fn strings(args: &[&str]) -> Vec<String> {
  args.iter().map(|&arg| arg.to_string()).collect()
}

#[test]
fn log_level_behaves_the_same_from_every_source() {
  const ALLOWED: &str = "allowed: error, warn, info, debug, trace";
  let mut failures = Vec::new();
  for (input, outcome) in [
    ("DEBUG", Ok("debug")),
    ("Warn", Ok("warn")),
    ("trace", Ok("trace")),
    (
      "debgu",
      Err(format!(
        "invalid log_level 'debgu' (did you mean 'debug'? {ALLOWED})"
      )),
    ),
    (
      "bogus",
      Err(format!("invalid log_level 'bogus' ({ALLOWED})")),
    ),
  ] {
    let base = "token = \"base-token\"\n";
    let attempts = [
      (
        "TOML key",
        load_through(&format!("{base}log_level = \"{input}\"\n"), &[], vec![]),
      ),
      (
        "env LOG_LEVEL",
        load_through(base, &[], vars(&[("LOG_LEVEL", input)])),
      ),
      (
        "--set",
        load_through(
          base,
          &strings(&["--set", &format!("log_level={input}")]),
          vec![],
        ),
      ),
    ];
    for (source, loaded) in attempts {
      let got = loaded.map(|config| config.log_level);
      match (&outcome, &got) {
        (Ok(want), Ok(level)) if level == want => {}
        (Err(want), Err(msg)) if msg == want => {}
        _ => failures.push(format!(
          "log_level '{input}' via {source}: expected {outcome:?}, got {got:?}"
        )),
      }
    }
    let flag = load_through(base, &strings(&["--log-level", input]), vec![]);
    match (&outcome, flag) {
      (Ok(want), Ok(config)) if config.log_level == *want => {}
      (Err(_), Err(msg)) if msg.starts_with("rejected by the argument parser") => {}
      (_, got) => failures.push(format!(
        "log_level '{input}' via --log-level: expected {outcome:?}, got {:?}",
        got.map(|config| config.log_level)
      )),
    }
  }
  assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}

#[test]
fn health_probe_reads_the_same_layers_as_load() {
  let file = TempConfig::write("health_socket = \"/tmp/file.sock\"\n");
  let probe = |args: &[&str], env: &[(&str, &str)]| {
    let cli = parse_cli(&file.arg(), &strings(args)).expect("parse");
    load_health_endpoint_with_env(&cli, vars(env)).map_err(|err| err.to_string())
  };
  for (label, args, env, want) in [
    ("file", &[][..], &[][..], Some("/tmp/file.sock")),
    (
      "env over file",
      &[][..],
      &[("HEALTH_SOCKET", "/tmp/env.sock")][..],
      Some("/tmp/env.sock"),
    ),
    (
      "--set over env",
      &["--set", "health_socket=/tmp/set.sock"][..],
      &[("HEALTH_SOCKET", "/tmp/env.sock")][..],
      Some("/tmp/set.sock"),
    ),
    (
      "--set over flag",
      &[
        "--set",
        "health_socket=/tmp/set.sock",
        "--health-socket",
        "/tmp/flag.sock",
      ][..],
      &[][..],
      Some("/tmp/set.sock"),
    ),
    (
      "empty --set disables",
      &["--set", "health_socket="][..],
      &[][..],
      None,
    ),
    (
      "empty env disables",
      &[][..],
      &[("HEALTH_SOCKET", "")][..],
      None,
    ),
  ] {
    let got = probe(args, env).unwrap_or_else(|err| panic!("{label}: {err}"));
    assert_eq!(got.as_deref(), want, "health endpoint ({label})");
  }
}

#[test]
fn health_probe_fails_on_a_broken_config_file() {
  let broken = TempConfig::write("health_socket = [\n");
  let cli = parse_cli(
    &broken.arg(),
    &strings(&["--health-socket", "/tmp/flag.sock"]),
  )
  .expect("parse");
  let err = load_health_endpoint_with_env(&cli, vars(&[])).expect_err("broken file");
  assert!(
    err.to_string().contains(&broken.arg()),
    "a broken config must fail the probe even when --health-socket is given, naming the file: {err}"
  );
}

#[test]
fn explicit_config_flag_beats_config_path_env() {
  let flagged = TempConfig::write("token = \"t\"\nlog_level = \"warn\"\n");
  let from_env = TempConfig::write("token = \"t\"\nlog_level = \"error\"\n");
  let cli = parse_cli(&flagged.arg(), &[]).expect("parse");
  let config = load_with_env(&cli, vars(&[("CONFIG_PATH", &from_env.arg())]))
    .expect("load")
    .config;
  assert_eq!(config.log_level, "warn", "-c must beat CONFIG_PATH");

  let no_flag = Cli::try_parse_from(["dka"]).expect("parse");
  let config = load_with_env(&no_flag, vars(&[("CONFIG_PATH", &from_env.arg())]))
    .expect("load")
    .config;
  assert_eq!(config.log_level, "error", "CONFIG_PATH is used without -c");
}

#[test]
fn missing_explicit_config_is_an_error() {
  let missing =
    std::env::temp_dir().join(format!("dka-contract-missing-{}.toml", std::process::id()));
  let arg = missing.display().to_string();
  let cli = parse_cli(&arg, &strings(&["--token", "t"])).expect("parse");
  let load_err = load_with_env(&cli, vars(&[])).expect_err("missing -c on load");
  assert_eq!(
    load_err.to_string(),
    format!("config file not found: {arg}")
  );
  let probe_err = load_health_endpoint_with_env(&cli, vars(&[])).expect_err("missing -c on health");
  assert_eq!(
    probe_err.to_string(),
    format!("config file not found: {arg}")
  );

  let no_flag = Cli::try_parse_from(["dka", "--token", "t"]).expect("parse");
  let env_err =
    load_with_env(&no_flag, vars(&[("CONFIG_PATH", &arg)])).expect_err("missing CONFIG_PATH");
  assert_eq!(env_err.to_string(), format!("config file not found: {arg}"));
}

#[test]
fn set_and_account_set_values_never_reach_errors_or_debug() {
  const SECRET: &str = "mfa.CONTRACT-SECRET-0123456789";
  let file = TempConfig::write("token = \"t\"\n");
  for (label, args) in [
    (
      "--set unknown path",
      strings(&["--set", &format!("tokn={SECRET}")]),
    ),
    ("--set no equals", strings(&["--set", SECRET])),
    (
      "--account-set unknown field",
      strings(&["--account-set", &format!("main.tokn={SECRET}")]),
    ),
    (
      "--account-set no id",
      strings(&["--account-set", &format!("main={SECRET}")]),
    ),
    (
      "--account-set no equals",
      strings(&["--account-set", SECRET]),
    ),
    ("--set no path", strings(&["--set", &format!("={SECRET}")])),
    (
      "--set unknown nested path",
      strings(&["--set", &format!("activity.nope={SECRET}")]),
    ),
    (
      "--set bad defaults profile",
      strings(&["--set", &format!("defaults.nope.os={SECRET}")]),
    ),
    (
      "--account-set bad id",
      strings(&["--account-set", &format!("bad id.token={SECRET}")]),
    ),
    (
      "--account-set empty path",
      strings(&["--account-set", &format!("main.={SECRET}")]),
    ),
  ] {
    let cli = parse_cli(&file.arg(), &args).expect("parse");
    let err = load_with_env(&cli, vars(&[])).expect_err(label);
    assert!(
      !format!("{err} {err:?}").contains(SECRET),
      "{label}: the value leaked into the error: {err}"
    );
    assert!(
      !format!("{cli:?}").contains(SECRET),
      "{label}: the value leaked into Debug of Cli"
    );
  }
  let cli = parse_cli(
    &file.arg(),
    &strings(&[
      "--set",
      &format!("token={SECRET}"),
      "--account-set",
      &format!("main.token={SECRET}"),
      "--token",
      SECRET,
    ]),
  )
  .expect("parse");
  let debug = format!("{cli:?}");
  assert!(
    !debug.contains(SECRET),
    "valid --set, --account-set and --token values leaked into Debug of Cli: {debug}"
  );
  assert!(
    debug.contains("token=<redacted>") && debug.contains("main.token=<redacted>"),
    "Debug of Cli should keep the path part of --set and --account-set: {debug}"
  );
}

#[test]
fn empty_env_value_disables_defaults_like_file_and_set() {
  let doc = "token = \"t\"\n";
  let field = |config: AppConfig| {
    (
      config.defaults.capabilities,
      config.defaults.heartbeat_version,
    )
  };
  let set_up = |source: &str, loaded: Result<AppConfig, String>| {
    let config = loaded.unwrap_or_else(|err| panic!("{source}: {err}"));
    assert_eq!(
      field(config),
      (None, None),
      "empty value from {source} should disable capabilities and heartbeat_version"
    );
  };
  set_up(
    "TOML key",
    load_through(
      "token = \"t\"\n[defaults]\ncapabilities = \"\"\nheartbeat_version = \"\"\n",
      &[],
      vec![],
    ),
  );
  set_up(
    "--set",
    load_through(
      doc,
      &strings(&[
        "--set",
        "defaults.capabilities=",
        "--set",
        "defaults.heartbeat_version=",
      ]),
      vec![],
    ),
  );
  set_up(
    "env",
    load_through(
      doc,
      &[],
      vars(&[
        ("DEFAULTS_CAPABILITIES", ""),
        ("DEFAULTS_HEARTBEAT_VERSION", ""),
      ]),
    ),
  );
}

#[test]
fn field_errors_name_the_toml_key() {
  let doc = "token = \"t\"\n";
  for (label, args, env, needle) in [
    (
      "activity.type via --set",
      &["--set", "activity.name=x", "--set", "activity.type=nope"][..],
      &[][..],
      "invalid activity.type 'nope'",
    ),
    (
      "activity.party_current via env",
      &[][..],
      &[("ACTIVITY", "x"), ("ACTIVITY_PARTY_CURRENT", "many")][..],
      "invalid activity.party_current 'many' (expected an integer)",
    ),
    (
      "activity.timestamp via flag",
      &["--activity", "x", "--activity-timestamp", "soon"][..],
      &[][..],
      "invalid activity.timestamp 'soon' (expected an integer)",
    ),
  ] {
    let err = load_through(doc, &strings(args), vars(env)).expect_err(label);
    assert!(
      err.contains(needle),
      "{label}: expected {needle:?} in {err:?}"
    );
  }
}

#[test]
fn mistyped_env_keys_warn_once_and_never_echo_the_value() {
  const SECRET: &str = "mfa.ENV-WARNING-SECRET-0123";
  let file = TempConfig::write("token = \"t\"\n");
  let cli = parse_cli(&file.arg(), &[]).expect("parse");
  for (key, want) in [
    (
      "ACCOUNT_STAUTS",
      "environment variable ACCOUNT_STAUTS defines account 'STAUTS', not a setting (did you mean 'ACCOUNT_STATUS'?)",
    ),
    (
      "ACCOUNT_MAIN_STAUTS",
      "environment variable ACCOUNT_MAIN_STAUTS matches no setting and is ignored (did you mean 'ACCOUNT_MAIN_STATUS'?)",
    ),
    (
      "ACTIVITY_DETIALS",
      "environment variable ACTIVITY_DETIALS defines activity 'DETIALS', not a setting (did you mean 'ACTIVITY_DETAILS'?)",
    ),
    (
      "DEFAULTS_WEB_BROWSR",
      "environment variable DEFAULTS_WEB_BROWSR matches no setting and is ignored (did you mean 'DEFAULTS_WEB_BROWSER'?)",
    ),
    (
      "ACCOUNT_main_ACTIVITY_1_DETIALS",
      "environment variable ACCOUNT_main_ACTIVITY_1_DETIALS matches no setting and is ignored (did you mean 'ACCOUNT_main_ACTIVITY_1_DETAILS'?)",
    ),
    (
      "DEFAULTS_NOTHING_LIKE_IT",
      "environment variable DEFAULTS_NOTHING_LIKE_IT matches no setting and is ignored",
    ),
    (
      "ACCOUNT_STATUS",
      "environment variable ACCOUNT_STATUS matches no setting and is ignored",
    ),
  ] {
    let loaded =
      load_with_env(&cli, vars(&[(key, SECRET), ("ACCOUNT_STAUTS_TOKEN", "t2")])).expect("load");
    let texts: Vec<String> = loaded.warnings.iter().map(ToString::to_string).collect();
    assert_eq!(texts, [want], "{key}: exactly one warning");
    assert!(
      !format!("{texts:?} {:?}", loaded.warnings).contains(SECRET),
      "{key}: the value leaked into the warning"
    );
  }
}

#[test]
fn well_formed_env_keys_produce_no_warnings() {
  let file = TempConfig::write("token = \"t\"\n");
  let cli = parse_cli(&file.arg(), &[]).expect("parse");
  let loaded = load_with_env(
    &cli,
    vars(&[
      ("ACCOUNT_main_TOKEN", "a"),
      ("ACCOUNT_main_STATUS", "dnd"),
      ("ACCOUNT_main_ACTIVITY_1_DETAILS", "d"),
      ("ACTIVITY_foo_TYPE", "playing"),
      ("DEFAULTS_WEB_OS", "Linux"),
      ("PATH", "/usr/bin"),
    ]),
  )
  .expect("load");
  assert!(
    loaded.warnings.is_empty(),
    "warnings for valid keys: {:?}",
    loaded.warnings
  );
}
