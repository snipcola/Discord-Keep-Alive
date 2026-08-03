mod account;
mod format;

use std::io::IsTerminal;

use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

use self::account::AccountLayer;
use self::format::HumanFormat;

/// Init tracing: `RUST_LOG` wins when set; otherwise workspace crates at `log_level` with noisy deps muted. ANSI only on a TTY.
pub fn init(log_level: &str) {
  let filter =
    EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_filter(log_level)));

  let ansi = std::io::stderr().is_terminal();

  tracing_subscriber::registry()
    .with(filter)
    .with(AccountLayer)
    .with(tracing_subscriber::fmt::layer().event_format(HumanFormat { ansi }))
    .init();
}

fn default_filter(level: &str) -> String {
  let level = normalize_level(level);
  format!(
    "discord_keep_alive={level},dka_gateway={level},dka_runtime={level},tokio=warn,tokio_tungstenite=warn,tungstenite=warn,rustls=warn"
  )
}

fn normalize_level(level: &str) -> String {
  match level.to_ascii_lowercase().as_str() {
    "error" | "warn" | "info" | "debug" | "trace" => level.to_ascii_lowercase(),
    _ => "info".into(),
  }
}

mod util {
  use std::fmt;

  use tracing::field::{Field, Visit};

  pub(super) fn strip_debug_string(value: String) -> String {
    if value.len() >= 2 && value.starts_with('"') && value.ends_with('"') {
      value[1..value.len() - 1].to_string()
    } else {
      value
    }
  }

  pub(super) fn render_debug(value: &dyn fmt::Debug) -> String {
    strip_debug_string(format!("{value:?}"))
  }

  #[derive(Default)]
  pub(super) struct AccountField {
    pub account: Option<String>,
  }

  impl Visit for AccountField {
    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
      if field.name() == "account" {
        self.account = Some(render_debug(value));
      }
    }

    fn record_str(&mut self, field: &Field, value: &str) {
      if field.name() == "account" {
        self.account = Some(value.to_string());
      }
    }
  }

  #[derive(Default)]
  pub(super) struct EventFields {
    pub message: String,
    pub account: Option<String>,
    pub fields: Vec<(String, String)>,
  }

  impl EventFields {
    fn record(&mut self, name: &str, value: String) {
      match name {
        "message" => self.message = value,
        "account" => self.account = Some(value),
        _ => self.fields.push((name.to_string(), value)),
      }
    }
  }

  impl Visit for EventFields {
    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
      self.record(field.name(), render_debug(value));
    }

    fn record_str(&mut self, field: &Field, value: &str) {
      self.record(field.name(), value.to_string());
    }
  }
}
