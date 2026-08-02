use dka_presence::build_presence_data;
use serde_json::Value;
use tracing::{debug, info};

use crate::config::AccountConfig;

pub fn build_and_log_presence(account: &AccountConfig) -> Value {
  log_presence_summary(account);
  build_presence_data(
    account.status,
    account.custom_status.as_ref(),
    &account.activities,
    false,
    None,
    account.kind,
  )
}

fn log_presence_summary(account: &AccountConfig) {
  let status = account
    .status
    .map(|s| s.to_string())
    .unwrap_or_else(|| "-".into());
  let kind = account.kind.to_string();
  let device = account
    .device
    .map(|d| d.to_string())
    .unwrap_or_else(|| "-".into());

  let custom_text = account
    .custom_status
    .as_ref()
    .and_then(|c| c.text.as_deref())
    .filter(|s| !s.is_empty());

  let rich_summary: Vec<String> = account
    .activities
    .iter()
    .filter_map(|a| {
      let name = a.name.as_deref().filter(|s| !s.is_empty())?;
      let ty = a
        .activity_type
        .map(|t| t.to_string())
        .unwrap_or_else(|| "unknown".into());
      Some(format!("{ty} {name}"))
    })
    .collect();

  match (custom_text, rich_summary.is_empty()) {
    (Some(text), true) => {
      info!("presence {status} ({kind}/{device}), custom {text}");
    }
    (Some(text), false) => {
      info!(
        "presence {status} ({kind}/{device}), custom {text}, {}",
        rich_summary.join(", ")
      );
    }
    (None, false) => {
      info!(
        "presence {status} ({kind}/{device}), {}",
        rich_summary.join(", ")
      );
    }
    (None, true) => {
      info!("presence {status} ({kind}/{device})");
    }
  }

  if custom_text.is_none() && rich_summary.is_empty() {
    return;
  }

  if let Some(cs) = &account.custom_status {
    debug!(
      text = cs.text.as_deref().unwrap_or("-"),
      emoji = cs.emoji.as_deref().unwrap_or("-"),
      "custom status"
    );
  }

  for (i, act) in account.activities.iter().enumerate() {
    debug!(
      index = i,
      name = act.name.as_deref().unwrap_or("-"),
      r#type = act
        .activity_type
        .map(|t| t.to_string())
        .as_deref()
        .unwrap_or("-"),
      platform = act.platform.map(|p| p.to_string()).as_deref().unwrap_or("-"),
      timestamp = act.timestamp.as_deref().unwrap_or("-"),
      application_id = %act.application_id,
      details = act.details.as_deref().unwrap_or("-"),
      url = act.url.as_deref().unwrap_or("-"),
      large_image = act.large_image.image.as_deref().unwrap_or("-"),
      large_image_text = act.large_image.text.as_deref().unwrap_or("-"),
      small_image = act.small_image.image.as_deref().unwrap_or("-"),
      small_image_text = act.small_image.text.as_deref().unwrap_or("-"),
      button = act.button.name.as_deref().unwrap_or("-"),
      button_url = act.button.url.as_deref().unwrap_or("-"),
      button2 = act.button2.name.as_deref().unwrap_or("-"),
      button2_url = act.button2.url.as_deref().unwrap_or("-"),
      party_id = %act.party.id,
      party_current = act.party.current.as_deref().unwrap_or("-"),
      party_max = act.party.max.as_deref().unwrap_or("-"),
      "activity details"
    );
  }
}
