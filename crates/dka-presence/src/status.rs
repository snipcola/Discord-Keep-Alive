use serde_json::{Value, json};

use crate::activity::{ActivityConfig, CustomStatusConfig};
use crate::constants::{AccountKind, Status};

pub fn build_presence_data(
  status: Option<Status>,
  custom: Option<&CustomStatusConfig>,
  activities: &[ActivityConfig],
  afk: bool,
  since: Option<i64>,
  kind: AccountKind,
) -> Value {
  let mut out = Vec::new();

  match kind {
    AccountKind::User => {
      if let Some(custom) = custom
        && let Some(v) = custom.to_activity()
      {
        out.push(v);
      }
      for act in activities {
        if let Some(v) = act.to_activity(kind) {
          out.push(v);
        }
      }
    }
    AccountKind::Bot => {
      // Bots keep only the first activity; Discord ignores the rest.
      if let Some(v) = activities.iter().find_map(|a| a.to_activity(kind)) {
        out.push(v);
      }
    }
  }

  json!({
    "since": since,
    "activities": out,
    "status": status.unwrap_or(Status::Online).as_str(),
    "afk": afk,
  })
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::activity::named;
  use crate::constants::{AccountKind, ActivityType, Status};

  fn presence(
    status: Option<Status>,
    custom: Option<&CustomStatusConfig>,
    activities: &[ActivityConfig],
    kind: AccountKind,
  ) -> Value {
    build_presence_data(status, custom, activities, false, None, kind)
  }

  #[test]
  fn empty_presence_online() {
    let d = presence(None, None, &[], AccountKind::User);
    assert_eq!(d["status"], "online");
    assert_eq!(d["activities"], json!([]));
    assert_eq!(d["afk"], false);
    assert!(d["since"].is_null());
  }

  #[test]
  fn with_custom_status_only() {
    let custom = CustomStatusConfig {
      text: Some("brb".into()),
      emoji: Some("🔥".into()),
    };
    let d = presence(Some(Status::Idle), Some(&custom), &[], AccountKind::User);
    assert_eq!(d["status"], "idle");
    assert_eq!(d["activities"].as_array().unwrap().len(), 1);
    assert_eq!(d["activities"][0]["type"], 4);
    assert_eq!(d["activities"][0]["state"], "brb");
    assert_eq!(d["activities"][0]["emoji"]["name"], "🔥");
  }

  #[test]
  fn custom_plus_multiple_rich() {
    let custom = CustomStatusConfig {
      text: Some("coding".into()),
      emoji: None,
    };
    let mut a = named("Rust");
    a.activity_type = Some(ActivityType::Playing);
    let mut b = named("Spotify");
    b.activity_type = Some(ActivityType::Listening);
    let d = presence(
      Some(Status::Online),
      Some(&custom),
      &[a, b],
      AccountKind::User,
    );
    let acts = d["activities"].as_array().unwrap();
    assert_eq!(acts.len(), 3);
    assert_eq!(acts[0]["type"], 4);
    assert_eq!(acts[0]["state"], "coding");
    assert_eq!(acts[1]["name"], "Rust");
    assert_eq!(acts[1]["type"], 0);
    assert_eq!(acts[2]["name"], "Spotify");
    assert_eq!(acts[2]["type"], 2);
  }

  #[test]
  fn bot_activity_omits_rich_fields() {
    let mut activity = named("Game");
    activity.activity_type = Some(ActivityType::Playing);
    activity.details = Some("Level 1".into());
    activity.application_id = "99".into();
    let d = presence(None, None, &[activity], AccountKind::Bot);
    let act = &d["activities"][0];
    assert_eq!(act["name"], "Game");
    assert_eq!(act["type"], 0);
    assert_eq!(act["state"], "Level 1");
    assert!(act.get("application_id").is_none());
  }

  #[test]
  fn bot_ignores_custom_and_extra_activities() {
    let custom = CustomStatusConfig {
      text: Some("ignored".into()),
      emoji: Some("x".into()),
    };
    let mut first = named("First");
    first.activity_type = Some(ActivityType::Playing);
    let mut second = named("Second");
    second.activity_type = Some(ActivityType::Watching);
    let d = presence(None, Some(&custom), &[first, second], AccountKind::Bot);
    let acts = d["activities"].as_array().unwrap();
    assert_eq!(acts.len(), 1);
    assert_eq!(acts[0]["name"], "First");
    assert_eq!(acts[0]["type"], 0);
  }

  #[test]
  fn nameless_activities_skipped() {
    let d = presence(
      None,
      None,
      &[ActivityConfig::new(), named("Only")],
      AccountKind::User,
    );
    assert_eq!(d["activities"].as_array().unwrap().len(), 1);
    assert_eq!(d["activities"][0]["name"], "Only");
  }
}
