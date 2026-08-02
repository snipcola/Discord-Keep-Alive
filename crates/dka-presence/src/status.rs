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
      // Bots only get one activity; extras are ignored by Discord.
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
  use crate::constants::{AccountKind, ActivityType, Status};

  #[test]
  fn empty_presence_online() {
    let d = build_presence_data(None, None, &[], false, None, AccountKind::User);
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
    let d = build_presence_data(
      Some(Status::Idle),
      Some(&custom),
      &[],
      false,
      None,
      AccountKind::User,
    );
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
    let a = ActivityConfig {
      name: Some("Rust".into()),
      activity_type: Some(ActivityType::Playing),
      application_id: "1".into(),
      ..ActivityConfig::new()
    };
    let b = ActivityConfig {
      name: Some("Spotify".into()),
      activity_type: Some(ActivityType::Listening),
      application_id: "1".into(),
      ..ActivityConfig::new()
    };
    let d = build_presence_data(
      Some(Status::Online),
      Some(&custom),
      &[a, b],
      false,
      None,
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
    let activity = ActivityConfig {
      name: Some("Game".into()),
      activity_type: Some(ActivityType::Playing),
      details: Some("Level 1".into()),
      application_id: "99".into(),
      ..ActivityConfig::new()
    };
    let d = build_presence_data(None, None, &[activity], false, None, AccountKind::Bot);
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
    let first = ActivityConfig {
      name: Some("First".into()),
      activity_type: Some(ActivityType::Playing),
      application_id: "1".into(),
      ..ActivityConfig::new()
    };
    let second = ActivityConfig {
      name: Some("Second".into()),
      activity_type: Some(ActivityType::Watching),
      application_id: "1".into(),
      ..ActivityConfig::new()
    };
    let d = build_presence_data(
      None,
      Some(&custom),
      &[first, second],
      false,
      None,
      AccountKind::Bot,
    );
    let acts = d["activities"].as_array().unwrap();
    assert_eq!(acts.len(), 1);
    assert_eq!(acts[0]["name"], "First");
    assert_eq!(acts[0]["type"], 0);
  }

  #[test]
  fn nameless_activities_skipped() {
    let empty = ActivityConfig::new();
    let named = ActivityConfig {
      name: Some("Only".into()),
      application_id: "1".into(),
      ..ActivityConfig::new()
    };
    let d = build_presence_data(None, None, &[empty, named], false, None, AccountKind::User);
    assert_eq!(d["activities"].as_array().unwrap().len(), 1);
    assert_eq!(d["activities"][0]["name"], "Only");
  }
}
