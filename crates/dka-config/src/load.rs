use std::ffi::OsString;
use std::path::PathBuf;

use crate::error::ConfigError;
use crate::merge::merge_partial;
use crate::model::partial::PartialConfig;
use crate::model::resolved::AppConfig;
use crate::resolve::resolve_config;
use crate::source::cli::{Cli, DEFAULT_CONFIG_PATH, cli_partial};
use crate::source::defaults::defaults_partial;
use crate::source::env::{self, EnvLayer};
use crate::source::file::load_file;
use crate::util::trim_owned;
use crate::warning::ConfigWarning;

pub(crate) const CONFIG_PATH_ENV: &str = "CONFIG_PATH";

struct ConfigFile {
  path: PathBuf,
  required: bool,
}

fn config_file(cli: &Cli, config_path_env: Option<OsString>) -> ConfigFile {
  match (&cli.config, config_path_env) {
    (Some(path), _) => ConfigFile {
      path: path.clone(),
      required: true,
    },
    (None, Some(path)) if !path.is_empty() => ConfigFile {
      path: path.into(),
      required: true,
    },
    (None, _) => ConfigFile {
      path: DEFAULT_CONFIG_PATH.into(),
      required: false,
    },
  }
}

fn load_file_layer(file: &ConfigFile) -> Result<PartialConfig, ConfigError> {
  if file.path.exists() {
    load_file(&file.path)
  } else if file.required {
    Err(ConfigError::ConfigNotFound(file.path.clone()))
  } else {
    Ok(PartialConfig::default())
  }
}

fn layers(
  cli: &Cli,
  config_path_env: Option<OsString>,
  env_layer: PartialConfig,
) -> Result<(PartialConfig, ConfigFile), ConfigError> {
  let file = config_file(cli, config_path_env);
  let mut partial = defaults_partial();
  merge_partial(&mut partial, load_file_layer(&file)?);
  merge_partial(&mut partial, env_layer);
  merge_partial(&mut partial, cli_partial(cli)?);
  Ok((partial, file))
}

fn resolve_layers(partial: PartialConfig, file: &ConfigFile) -> Result<AppConfig, ConfigError> {
  resolve_config(partial).map_err(|err| match err {
    ConfigError::NoAccounts if !file.path.exists() => {
      ConfigError::NoAccountsMissingConfig(file.path.clone())
    }
    err => err,
  })
}

#[derive(Debug)]
pub struct LoadedConfig {
  pub config: AppConfig,
  pub warnings: Vec<ConfigWarning>,
}

pub(crate) fn load_with(
  cli: &Cli,
  config_path_env: Option<OsString>,
  env_layer: EnvLayer,
) -> Result<LoadedConfig, ConfigError> {
  let EnvLayer { partial, warnings } = env_layer;
  let (partial, file) = layers(cli, config_path_env, partial)?;
  let config = resolve_layers(partial, &file)?;
  Ok(LoadedConfig { config, warnings })
}

pub(crate) fn load_health_endpoint_with(
  cli: &Cli,
  config_path_env: Option<OsString>,
  env_layer: PartialConfig,
) -> Result<Option<String>, ConfigError> {
  let (partial, _) = layers(cli, config_path_env, env_layer)?;
  Ok(trim_owned(partial.root.health_socket))
}

pub fn load(cli: &Cli) -> Result<LoadedConfig, ConfigError> {
  load_with(cli, std::env::var_os(CONFIG_PATH_ENV), env::from_env())
}

pub fn load_health_endpoint(cli: &Cli) -> Result<Option<String>, ConfigError> {
  load_health_endpoint_with(
    cli,
    std::env::var_os(CONFIG_PATH_ENV),
    env::from_env().partial,
  )
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::model::partial::PartialActivity;
  use crate::schema::id::{ACCOUNT_FLAT, ACTIVITY_SINGULAR};
  use crate::source::defaults::DEFAULT_LOG_LEVEL;
  use crate::source::env as env_src;
  use crate::test_support::*;
  use dka_presence::Status;
  use std::fs;
  use std::path::Path;
  use std::sync::atomic::{AtomicU64, Ordering};

  static TEMP_SEQ: AtomicU64 = AtomicU64::new(0);
  const TOKEN_T: &str = "token = \"t\"\n";

  fn unused_temp_path() -> PathBuf {
    let n = TEMP_SEQ.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("dka-cfg-{n}-{}.toml", std::process::id()))
  }

  struct TempToml {
    path: PathBuf,
  }

  impl TempToml {
    fn write(contents: &str) -> Self {
      let path = unused_temp_path();
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

  fn cli_for(file: &TempToml) -> Cli {
    let mut cli = empty_cli();
    cli.config = Some(file.path().to_path_buf());
    cli
  }

  fn no_env() -> PartialConfig {
    PartialConfig::default()
  }

  fn load_config(
    cli: &Cli,
    config_path_env: Option<OsString>,
    partial: PartialConfig,
  ) -> Result<AppConfig, ConfigError> {
    let env_layer = EnvLayer {
      partial,
      warnings: Vec::new(),
    };
    load_with(cli, config_path_env, env_layer).map(|loaded| loaded.config)
  }

  #[test]
  fn file_env_cli_precedence() {
    struct Case {
      label: &'static str,
      env: &'static [(&'static str, &'static str)],
      cli_log_level: Option<&'static str>,
      cli_token: Option<&'static str>,
      log_level: &'static str,
      token: &'static str,
      status: Option<Status>,
    }
    let file_toml = "log_level = \"warn\"\ntoken = \"file-tok\"\n";
    let env = &[
      ("LOG_LEVEL", "debug"),
      ("TOKEN", "env-tok"),
      ("STATUS", "online"),
    ];
    for case in [
      Case {
        label: "file only",
        env: &[],
        cli_log_level: None,
        cli_token: None,
        log_level: "warn",
        token: "file-tok",
        status: None,
      },
      Case {
        label: "env over file",
        env,
        cli_log_level: None,
        cli_token: None,
        log_level: "debug",
        token: "env-tok",
        status: Some(Status::Online),
      },
      Case {
        label: "cli over env",
        env,
        cli_log_level: Some("trace"),
        cli_token: Some("cli-tok"),
        log_level: "trace",
        token: "cli-tok",
        status: Some(Status::Online),
      },
    ] {
      let file = TempToml::write(file_toml);
      let mut cli = cli_for(&file);
      cli.log_level = case.cli_log_level.map(Into::into);
      cli.account.token = case.cli_token.map(Into::into);
      let app = load_config(&cli, None, env_src::from_env_map(&env_map(case.env)))
        .unwrap_or_else(|e| panic!("{}: {e}", case.label));
      assert_eq!(app.log_level, case.log_level, "{}", case.label);
      assert_eq!(app.accounts.len(), 1, "{}", case.label);
      assert_eq!(app.accounts[0].token, case.token, "{}", case.label);
      assert_eq!(app.accounts[0].status, case.status, "{}", case.label);
    }
  }

  #[test]
  fn health_endpoint_follows_the_full_precedence() {
    let file = TempToml::write(r#"health_socket = "/tmp/file""#);
    let env = env_src::from_env_map(&env_map(&[("HEALTH_SOCKET", "/tmp/env")]));
    for (label, flag, set, with_env, want) in [
      ("file only", None, None, false, Some("/tmp/file")),
      ("env over file", None, None, true, Some("/tmp/env")),
      (
        "flag over env",
        Some("/tmp/flag"),
        None,
        true,
        Some("/tmp/flag"),
      ),
      (
        "--set over env",
        None,
        Some("health_socket=/tmp/set"),
        true,
        Some("/tmp/set"),
      ),
      ("empty flag disables", Some(""), None, true, None),
      ("blank flag disables", Some("   "), None, true, None),
      (
        "empty --set disables",
        None,
        Some("health_socket="),
        true,
        None,
      ),
    ] {
      let mut cli = cli_for(&file);
      cli.health_socket = flag.map(Into::into);
      cli.set = set.map(Into::into).into_iter().collect();
      let env_layer = if with_env { env.clone() } else { no_env() };
      let got =
        load_health_endpoint_with(&cli, None, env_layer).unwrap_or_else(|e| panic!("{label}: {e}"));
      assert_eq!(got.as_deref(), want, "{label}");
    }
  }

  #[test]
  fn health_endpoint_fails_on_a_malformed_set_spec() {
    let file = TempToml::write("");
    let mut cli = cli_for(&file);
    cli.set = vec!["no-equals-sign".into()];
    let err = load_health_endpoint_with(&cli, None, no_env()).unwrap_err();
    assert!(
      matches!(err, ConfigError::InvalidPath(_)),
      "a malformed --set must fail the health probe: {err:?}"
    );
  }

  #[test]
  fn missing_default_file_is_skipped_but_an_explicit_one_is_required() {
    let missing = unused_temp_path();
    let optional = ConfigFile {
      path: missing.clone(),
      required: false,
    };
    let layer = load_file_layer(&optional).expect("optional missing file");
    assert_eq!(layer.root, Default::default(), "optional file");

    let required = ConfigFile {
      path: missing.clone(),
      required: true,
    };
    let err = load_file_layer(&required).unwrap_err();
    assert!(
      matches!(err, ConfigError::ConfigNotFound(ref p) if *p == missing),
      "required file: {err:?}"
    );
  }

  #[test]
  fn config_file_selection() {
    let from_env = || Some(OsString::from("/tmp/from-config-path-env.toml"));
    let mut flagged = empty_cli();
    flagged.config = Some("/tmp/explicit-cli-config.toml".into());
    let mut flagged_default_name = empty_cli();
    flagged_default_name.config = Some(DEFAULT_CONFIG_PATH.into());
    for (label, cli, env, want, required) in [
      (
        "-c equal to the default name still beats CONFIG_PATH",
        &flagged_default_name,
        from_env(),
        DEFAULT_CONFIG_PATH,
        true,
      ),
      (
        "-c alone",
        &flagged,
        None,
        "/tmp/explicit-cli-config.toml",
        true,
      ),
      (
        "empty CONFIG_PATH",
        &empty_cli(),
        Some(OsString::new()),
        DEFAULT_CONFIG_PATH,
        false,
      ),
      ("neither", &empty_cli(), None, DEFAULT_CONFIG_PATH, false),
    ] {
      let file = config_file(cli, env);
      assert_eq!(file.path, PathBuf::from(want), "{label}: path");
      assert_eq!(file.required, required, "{label}: required");
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
    let mut cli = cli_for(&file);
    cli.account_set = vec!["extra.token=cli-extra".into(), "extra.name=Extra".into()];
    cli.set = vec!["accounts.main.device=mobile".into()];
    let app = load_config(&cli, None, env_layer).unwrap();

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
    assert_eq!(main.status, Some(Status::Dnd));
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
    let mut cli = cli_for(&file);
    cli.activity.name = Some("from-cli".into());
    cli.activity.activity_type = Some("watching".into());
    let act = &load_config(&cli, None, env_layer).unwrap().accounts[0].activities;
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
    let err = load_config(&cli_for(&file), None, no_env()).unwrap_err();
    assert!(matches!(err, ConfigError::NoAccounts), "{err:?}");
  }

  #[test]
  fn no_accounts_reports_a_missing_default_config() {
    let file = ConfigFile {
      path: unused_temp_path(),
      required: false,
    };
    let err = resolve_layers(defaults_partial(), &file).unwrap_err();
    assert!(
      matches!(err, ConfigError::NoAccountsMissingConfig(ref p) if *p == file.path),
      "{err}"
    );
  }

  #[test]
  fn defaults_layer_supplies_the_default_log_level() {
    let file = ConfigFile {
      path: unused_temp_path(),
      required: false,
    };
    let env_layer = env_src::from_env_map(&env_map(&[("TOKEN", "t")]));
    let mut partial = defaults_partial();
    merge_partial(&mut partial, env_layer);
    let app = resolve_layers(partial, &file).unwrap();
    assert_eq!(app.log_level, DEFAULT_LOG_LEVEL);
    assert!(app.health_socket.is_none());
  }
}
