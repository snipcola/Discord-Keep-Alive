use std::path::{Path, PathBuf};

use crate::error::ConfigError;
use crate::merge::merge_partial;
use crate::model::partial::PartialConfig;
use crate::model::resolved::AppConfig;
use crate::resolve::resolve_config;
use crate::source::cli::{Cli, DEFAULT_CONFIG_PATH, cli_partial};
use crate::source::defaults::defaults_partial;
use crate::source::env;
use crate::source::file::load_file;
use crate::util::trim_owned;

// --config wins; if it is still the default path, use CONFIG_PATH when set.
fn resolve_config_path_arg(cli_path: &Path) -> PathBuf {
  resolve_config_path(cli_path, std::env::var_os("CONFIG_PATH"))
}

pub(crate) fn resolve_config_path(
  cli_path: &Path,
  config_path_env: Option<std::ffi::OsString>,
) -> PathBuf {
  if cli_path.as_os_str() != DEFAULT_CONFIG_PATH {
    return cli_path.to_path_buf();
  }
  match config_path_env {
    Some(p) if !p.is_empty() => PathBuf::from(p),
    _ => PathBuf::from(DEFAULT_CONFIG_PATH),
  }
}

fn is_default_config_path(path: &Path) -> bool {
  path.as_os_str() == DEFAULT_CONFIG_PATH
}

fn load_file_layer(path: &Path) -> Result<PartialConfig, ConfigError> {
  if path.exists() {
    load_file(path)
  } else if is_default_config_path(path) {
    Ok(PartialConfig::default())
  } else {
    Err(ConfigError::ConfigNotFound(path.to_path_buf()))
  }
}

pub fn load(cli: &Cli) -> Result<AppConfig, ConfigError> {
  let path = resolve_config_path_arg(&cli.config);
  load_with(&path, env::from_env(), cli_partial(cli)?)
}

fn merge_partial_layers(
  config_path: &Path,
  layers: impl IntoIterator<Item = PartialConfig>,
) -> Result<PartialConfig, ConfigError> {
  let mut partial = defaults_partial();
  merge_partial(&mut partial, load_file_layer(config_path)?);
  for layer in layers {
    merge_partial(&mut partial, layer);
  }
  Ok(partial)
}

/// Merge order (last wins): defaults → file → env → CLI, then resolve.
pub(crate) fn load_with(
  config_path: &Path,
  env_layer: PartialConfig,
  cli_layer: PartialConfig,
) -> Result<AppConfig, ConfigError> {
  resolve_config(merge_partial_layers(config_path, [env_layer, cli_layer])?)
}

// Health socket only (skips account resolve). CLI override first; empty string disables.
pub fn load_health_endpoint(
  config_path: &Path,
  cli_override: Option<&str>,
) -> Result<Option<String>, ConfigError> {
  let path = resolve_config_path_arg(config_path);
  load_health_endpoint_with(&path, cli_override, env::from_env())
}

pub(crate) fn load_health_endpoint_with(
  config_path: &Path,
  cli_override: Option<&str>,
  env_layer: PartialConfig,
) -> Result<Option<String>, ConfigError> {
  if let Some(raw) = cli_override {
    return Ok(trim_owned(Some(raw.to_string())));
  }

  let partial = merge_partial_layers(config_path, [env_layer])?;
  Ok(trim_owned(partial.health_socket))
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::model::partial::{PartialAccount, PartialActivity};
  use crate::schema::id::{ACCOUNT_FLAT, ACTIVITY_SINGULAR};
  use crate::source::cli::cli_partial;
  use crate::source::defaults::DEFAULT_LOG_LEVEL;
  use crate::source::env as env_src;
  use crate::test_support::*;
  use std::fs;
  use std::sync::atomic::{AtomicU64, Ordering};

  static TEMP_SEQ: AtomicU64 = AtomicU64::new(0);
  const TOKEN_T: &str = "token = \"t\"\n";

  struct TempToml {
    path: PathBuf,
  }

  impl TempToml {
    fn write(contents: &str) -> Self {
      let n = TEMP_SEQ.fetch_add(1, Ordering::Relaxed);
      let path = std::env::temp_dir().join(format!("dka-cfg-{n}-{}.toml", std::process::id()));
      fs::write(&path, contents).unwrap();
      Self { path }
    }

    fn path(&self) -> &Path {
      &self.path
    }
  }

  impl Drop for TempToml {
    fn drop(&mut self) {
      let _ = fs::remove_file(&self.path);
    }
  }

  fn with_token(mut p: PartialConfig) -> PartialConfig {
    if !p.accounts.values().any(|a| a.token.is_some()) {
      p.accounts.entry(ACCOUNT_FLAT.into()).or_default().token = Some("test-token".into());
      if !p.account_order.iter().any(|id| id == ACCOUNT_FLAT) {
        p.account_order.push(ACCOUNT_FLAT.into());
      }
    }
    p
  }

  #[test]
  fn load_with_file_env_cli_precedence() {
    let file = TempToml::write("log_level = \"warn\"\ntoken = \"file-tok\"\n");
    let env_layer = PartialConfig {
      log_level: Some("debug".into()),
      accounts: {
        let mut m = std::collections::BTreeMap::new();
        m.insert(
          ACCOUNT_FLAT.into(),
          PartialAccount {
            token: Some("env-tok".into()),
            status: Some("online".into()),
            ..Default::default()
          },
        );
        m
      },
      account_order: vec![ACCOUNT_FLAT.into()],
      ..Default::default()
    };
    let mut cli = empty_cli();
    cli.log_level = Some("trace".into());
    cli.token = Some("cli-tok".into());
    let app = load_with(file.path(), env_layer, cli_partial(&cli).unwrap()).unwrap();
    assert_eq!(app.log_level, "trace");
    assert_eq!(app.accounts.len(), 1);
    assert_eq!(app.accounts[0].token, "cli-tok");
    assert_eq!(app.accounts[0].status, Some(dka_presence::Status::Online));
  }

  #[test]
  fn health_endpoint_cli_empty_disables() {
    let file = TempToml::write(r#"health_socket = "/tmp/file""#);
    let env = PartialConfig {
      health_socket: Some("/tmp/env".into()),
      ..Default::default()
    };
    for (label, cli, want) in [
      ("cli_wins", Some("/tmp/cli"), Some("/tmp/cli")),
      ("env_over_file", None, Some("/tmp/env")),
      ("empty_cli", Some(""), None),
      ("ws_cli", Some("   "), None),
    ] {
      let ep = load_health_endpoint_with(file.path(), cli, env.clone()).unwrap();
      assert_eq!(ep.as_deref(), want, "{label}");
    }
    let file = TempToml::write(r#"health_socket = "/tmp/health-only""#);
    let ep = load_health_endpoint_with(file.path(), None, PartialConfig::default()).unwrap();
    assert_eq!(ep.as_deref(), Some("/tmp/health-only"));
  }

  #[test]
  fn missing_default_path_ok() {
    let path = Path::new(DEFAULT_CONFIG_PATH);
    if path.exists() {
      let env_layer = with_token(PartialConfig::default());
      let _ = load_with(path, env_layer, PartialConfig::default());
      return;
    }
    let env_layer = with_token(PartialConfig::default());
    let app = load_with(path, env_layer, PartialConfig::default()).unwrap();
    assert_eq!(app.log_level, DEFAULT_LOG_LEVEL);
    assert!(app.health_socket.is_none());
  }

  #[test]
  fn explicit_missing_path_errors() {
    for (label, path, is_health) in [
      ("load", "definitely-missing-config-xyz.toml", false),
      ("health", "definitely-missing-health-config-xyz.toml", true),
    ] {
      let path = Path::new(path);
      assert!(!path.exists(), "{label}");
      let err = if is_health {
        load_health_endpoint_with(path, None, PartialConfig::default()).unwrap_err()
      } else {
        load_with(path, PartialConfig::default(), PartialConfig::default()).unwrap_err()
      };
      assert!(matches!(err, ConfigError::ConfigNotFound(_)), "{label}");
    }
  }

  #[test]
  fn resolve_config_path_cases() {
    let env = || Some(std::ffi::OsString::from("/tmp/from-config-path-env.toml"));
    for (label, cli, env, want) in [
      (
        "cli_wins",
        Path::new("/tmp/explicit-cli-config.toml"),
        env(),
        "/tmp/explicit-cli-config.toml",
      ),
      (
        "default_uses_env",
        Path::new(DEFAULT_CONFIG_PATH),
        env(),
        "/tmp/from-config-path-env.toml",
      ),
      (
        "default_without_env",
        Path::new(DEFAULT_CONFIG_PATH),
        None,
        DEFAULT_CONFIG_PATH,
      ),
    ] {
      assert_eq!(
        resolve_config_path(cli, env),
        PathBuf::from(want),
        "{label}"
      );
    }
  }

  #[test]
  fn multi_source_unique_string_ids() {
    let file = TempToml::write(
      r#"
token = "flat-file"
status = "online"

[[accounts]]
name = "from-file"
token = "file-0"
"#,
    );
    let env_layer = env_src::from_env_map(&env_map(&[
      ("ACCOUNT_main_TOKEN", "env-main"),
      ("ACCOUNT_main", "Main"),
      ("ACCOUNT_main_STATUS", "dnd"),
    ]));
    let mut cli = empty_cli();
    cli.account_set = vec!["extra.token=cli-extra".into(), "extra.name=Extra".into()];
    cli.set = vec!["accounts.main.device=mobile".into()];
    let app = load_with(file.path(), env_layer, cli_partial(&cli).unwrap()).unwrap();

    let names: Vec<_> = app.accounts.iter().map(|a| a.name.as_str()).collect();
    assert!(names.contains(&"from-file"));
    assert!(names.contains(&"Main"));
    assert!(names.contains(&"Extra"));
    assert_eq!(app.accounts.len(), 4);

    let main = app
      .accounts
      .iter()
      .find(|a| a.name == "Main")
      .expect("main");
    assert_eq!(main.token, "env-main");
    assert_eq!(main.status, Some(dka_presence::Status::Dnd));
    assert_eq!(main.device, Some(dka_presence::Device::Mobile));

    let extra = app
      .accounts
      .iter()
      .find(|a| a.name == "Extra")
      .expect("extra");
    assert_eq!(extra.token, "cli-extra");
  }

  #[test]
  fn cli_activity_field_merge_does_not_wipe_env_fields() {
    let file = TempToml::write(TOKEN_T);
    let mut env_acc = account_with_token("t");
    env_acc.activities.insert(
      ACTIVITY_SINGULAR.into(),
      PartialActivity {
        name: Some("from-env".into()),
        activity_type: Some("playing".into()),
        details: Some("env-details".into()),
        url: Some("https://env.example".into()),
        ..Default::default()
      },
    );
    env_acc.activity_order.push(ACTIVITY_SINGULAR.into());
    let env_layer = PartialConfig {
      accounts: {
        let mut m = std::collections::BTreeMap::new();
        m.insert(ACCOUNT_FLAT.into(), env_acc);
        m
      },
      account_order: vec![ACCOUNT_FLAT.into()],
      ..Default::default()
    };
    let mut cli = empty_cli();
    cli.activity = Some("from-cli".into());
    cli.activity_type = Some("watching".into());
    let act = &load_with(file.path(), env_layer, cli_partial(&cli).unwrap())
      .unwrap()
      .accounts[0]
      .activities;
    assert_eq!(act.len(), 1);
    assert_eq!(act[0].name.as_deref(), Some("from-cli"));
    assert_eq!(
      act[0].activity_type,
      Some(dka_presence::ActivityType::Watching)
    );
    assert_eq!(act[0].details.as_deref(), Some("env-details"));
    assert_eq!(act[0].url.as_deref(), Some("https://env.example"));
  }

  #[test]
  fn load_with_no_accounts_errors() {
    let file = TempToml::write(r#"log_level = "info""#);
    let err = load_with(
      file.path(),
      PartialConfig::default(),
      PartialConfig::default(),
    )
    .unwrap_err();
    assert!(matches!(err, ConfigError::NoAccounts));
  }

  #[test]
  fn env_overwrites_file_scalars() {
    let file = TempToml::write("log_level = \"warn\"\ntoken = \"t\"\n");
    let app = load_with(
      file.path(),
      env_src::from_env_map(&env_map(&[("LOG_LEVEL", "debug")])),
      PartialConfig::default(),
    )
    .unwrap();
    assert_eq!(app.log_level, "debug");
  }
}
