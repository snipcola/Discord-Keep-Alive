use std::time::{SystemTime, UNIX_EPOCH};

use dka_gateway::SessionParams;
use dka_presence::{
  ActivityButton, ActivityConfig, ActivityParty, ActivityType, Defaults, ImageAsset, Status,
  build_presence_data, pin_default_activity_timestamps,
};
use tracing::{debug, info};

use dka_config::AccountConfig;

pub fn session_params(mut account: AccountConfig, defaults: &Defaults) -> SessionParams {
  let now = SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .map(|d| d.as_secs() as i64)
    .unwrap_or(0);
  pin_default_activity_timestamps(&mut account.activities, now);

  log_presence_summary(&account);

  let AccountConfig {
    name,
    token,
    kind,
    device,
    status,
    custom_status,
    activities,
  } = account;

  let presence = build_presence_data(
    status,
    custom_status.as_ref(),
    &activities,
    false,
    None,
    kind,
  );

  SessionParams {
    name,
    token: token.into_inner().into(),
    kind,
    presence,
    properties: defaults.client_properties(kind, device).clone(),
    user: defaults.user_options(kind),
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
    let ActivityConfig {
      name,
      activity_type,
      platform,
      timestamp,
      application_id,
      details,
      url,
      large_image: ImageAsset {
        image: large_image,
        text: large_image_text,
      },
      small_image: ImageAsset {
        image: small_image,
        text: small_image_text,
      },
      button: ActivityButton {
        name: button,
        url: button_url,
      },
      button_2: ActivityButton {
        name: button_2,
        url: button_2_url,
      },
      party:
        ActivityParty {
          id: party_id,
          current: party_current,
          max: party_max,
        },
    } = act;
    let fields = [
      ("name", name.as_deref()),
      ("type", activity_type.map(|t| t.as_str())),
      ("platform", platform.map(|p| p.as_str())),
      ("timestamp", timestamp.as_deref()),
      ("application_id", Some(application_id.as_str())),
      ("details", details.as_deref()),
      ("url", url.as_deref()),
      ("large_image", large_image.as_deref()),
      ("large_image_text", large_image_text.as_deref()),
      ("small_image", small_image.as_deref()),
      ("small_image_text", small_image_text.as_deref()),
      ("button", button.as_deref()),
      ("button_url", button_url.as_deref()),
      ("button_2", button_2.as_deref()),
      ("button_2_url", button_2_url.as_deref()),
      ("party_id", Some(party_id.as_str())),
      ("party_current", party_current.as_deref()),
      ("party_max", party_max.as_deref()),
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
