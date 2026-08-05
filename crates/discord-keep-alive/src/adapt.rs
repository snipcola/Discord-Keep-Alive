use std::time::{SystemTime, UNIX_EPOCH};

use dka_gateway::SessionParams;
use dka_gateway::properties::Defaults;
use dka_presence::{build_presence_data, pin_default_activity_timestamps};
use tracing::{debug, info};

use dka_config::AccountConfig;

pub fn session_params(mut account: AccountConfig, defaults: &Defaults) -> SessionParams {
  let now = SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .map(|d| d.as_secs() as i64)
    .unwrap_or(0);
  pin_default_activity_timestamps(&mut account.activities, now);

  log_presence_summary(&account);

  let presence = build_presence_data(
    account.status,
    account.custom_status.as_ref(),
    &account.activities,
    false,
    None,
    account.kind,
  );

  let properties = defaults
    .client_properties(account.kind, account.device)
    .clone();

  SessionParams {
    name: account.name,
    token: account.token.into_inner(),
    kind: account.kind,
    presence,
    properties,
  }
}

fn log_presence_summary(account: &AccountConfig) {
  let account_name = account.name.as_str();
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

  let mut summary = format!("presence config: {status} ({kind}/{device})");
  if let Some(text) = custom_text {
    summary.push_str(", custom ");
    summary.push_str(text);
  }
  if !rich_summary.is_empty() {
    summary.push_str(", ");
    summary.push_str(&rich_summary.join(", "));
  }
  info!(account = %account_name, "{summary}");

  if custom_text.is_none() && rich_summary.is_empty() {
    return;
  }

  if let Some(cs) = &account.custom_status {
    debug!(
      account = %account_name,
      text = dash(cs.text.as_deref()),
      emoji = dash(cs.emoji.as_deref()),
      "custom status"
    );
  }

  for (i, act) in account.activities.iter().enumerate() {
    let ty = act.activity_type.map(|t| t.to_string());
    let platform = act.platform.map(|p| p.to_string());
    debug!(
      account = %account_name,
      index = i,
      name = dash(act.name.as_deref()),
      r#type = dash(ty.as_deref()),
      platform = dash(platform.as_deref()),
      timestamp = dash(act.timestamp.as_deref()),
      application_id = %act.application_id,
      details = dash(act.details.as_deref()),
      url = dash(act.url.as_deref()),
      large_image = dash(act.large_image.image.as_deref()),
      large_image_text = dash(act.large_image.text.as_deref()),
      small_image = dash(act.small_image.image.as_deref()),
      small_image_text = dash(act.small_image.text.as_deref()),
      button = dash(act.button.name.as_deref()),
      button_url = dash(act.button.url.as_deref()),
      button_2 = dash(act.button_2.name.as_deref()),
      button_2_url = dash(act.button_2.url.as_deref()),
      party_id = %act.party.id,
      party_current = dash(act.party.current.as_deref()),
      party_max = dash(act.party.max.as_deref()),
      "activity details"
    );
  }
}

fn dash(o: Option<&str>) -> &str {
  o.unwrap_or("-")
}
