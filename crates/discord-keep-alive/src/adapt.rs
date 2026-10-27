use std::time::{SystemTime, UNIX_EPOCH};

use dka_gateway::SessionParams;
use dka_gateway::properties::Defaults;
use dka_presence::{ActivityType, Status, build_presence_data, pin_default_activity_timestamps};
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
    user: defaults.user_options(account.kind),
  }
}

fn log_presence_summary(account: &AccountConfig) {
  let account_name = account.name.as_str();
  let identity = match account.device {
    Some(device) => format!("{} on {device}", account.kind),
    None => account.kind.to_string(),
  };
  let mut parts = vec![format!(
    "{} as {identity}",
    account.status.unwrap_or(Status::DEFAULT)
  )];

  if let Some(custom) = &account.custom_status
    && let Some(text) = custom.text.as_deref().filter(|s| !s.is_empty())
  {
    let label = match custom.emoji.as_deref().filter(|s| !s.is_empty()) {
      Some(emoji) => format!("{emoji} {text}"),
      None => text.to_string(),
    };
    parts.push(format!("custom status {label:?}"));
  }

  parts.extend(account.activities.iter().filter_map(|a| {
    let name = a.name.as_deref().filter(|s| !s.is_empty())?;
    Some(format!("{} {name}", activity_verb(a.activity_type)))
  }));
  info!(account = %account_name, "{}", parts.join(", "));

  for (i, act) in account.activities.iter().enumerate() {
    let fields = [
      ("name", act.name.as_deref()),
      ("type", act.activity_type.map(|t| t.as_str())),
      ("platform", act.platform.map(|p| p.as_str())),
      ("timestamp", act.timestamp.as_deref()),
      ("application_id", Some(act.application_id.as_str())),
      ("details", act.details.as_deref()),
      ("url", act.url.as_deref()),
      ("large_image", act.large_image.image.as_deref()),
      ("large_image_text", act.large_image.text.as_deref()),
      ("small_image", act.small_image.image.as_deref()),
      ("small_image_text", act.small_image.text.as_deref()),
      ("button", act.button.name.as_deref()),
      ("button_url", act.button.url.as_deref()),
      ("button_2", act.button_2.name.as_deref()),
      ("button_2_url", act.button_2.url.as_deref()),
      ("party_id", Some(act.party.id.as_str())),
      ("party_current", act.party.current.as_deref()),
      ("party_max", act.party.max.as_deref()),
    ];
    let set: Vec<String> = fields
      .into_iter()
      .filter_map(|(key, value)| Some(format!("{key}={:?}", value?)))
      .collect();
    debug!(account = %account_name, "activity {}: {}", i + 1, set.join(" "));
  }
}

fn activity_verb(kind: Option<ActivityType>) -> &'static str {
  match kind {
    None => ActivityType::Playing.as_str(),
    Some(ActivityType::Listening) => "listening to",
    Some(ActivityType::Competing) => "competing in",
    Some(kind) => kind.as_str(),
  }
}
