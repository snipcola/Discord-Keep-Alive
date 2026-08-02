use serde_json::{Value, json};

use crate::constants::{
  AccountKind, ActivityPlatform, ActivityType, DEFAULT_APPLICATION_ID, DEFAULT_PARTY_ID,
};

/// Normalize an activity image for Discord.
///
/// CDN/media URLs become `mp:...`. Known prefixes (`mp:`, `youtube:`, `spotify:`,
/// `twitch:`, `external/`) and bare numeric asset ids are left unchanged.
pub fn normalize_activity_image(image: &str) -> String {
  let image = image.trim();
  if image.is_empty() {
    return String::new();
  }

  if image.starts_with("mp:")
    || image.starts_with("youtube:")
    || image.starts_with("spotify:")
    || image.starts_with("twitch:")
    || image.starts_with("external/")
    || image.chars().all(|c| c.is_ascii_digit())
  {
    return image.to_string();
  }

  let lower = image.to_ascii_lowercase();
  for host in [
    "https://cdn.discordapp.com/",
    "http://cdn.discordapp.com/",
    "https://media.discordapp.net/",
    "http://media.discordapp.net/",
    "https://media.discordapp.com/",
    "http://media.discordapp.com/",
  ] {
    if lower.starts_with(host) {
      let rest = &image[host.len()..];
      let path = rest.split('?').next().unwrap_or(rest);
      let path = path.trim_start_matches('/');
      if !path.is_empty() {
        return format!("mp:{path}");
      }
    }
  }

  image.to_string()
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ImageAsset {
  pub image: Option<String>,
  pub text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ActivityButton {
  pub name: Option<String>,
  pub url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivityParty {
  pub id: String,
  pub current: Option<String>,
  pub max: Option<String>,
}

impl Default for ActivityParty {
  fn default() -> Self {
    Self {
      id: DEFAULT_PARTY_ID.to_string(),
      current: None,
      max: None,
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CustomStatusConfig {
  pub text: Option<String>,
  pub emoji: Option<String>,
}

impl CustomStatusConfig {
  pub fn to_activity(&self) -> Option<Value> {
    let text = self.text.as_ref().filter(|t| !t.is_empty())?;
    Some(build_custom_status(text, self.emoji.as_deref()))
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivityConfig {
  pub name: Option<String>,
  pub activity_type: Option<ActivityType>,
  pub platform: Option<ActivityPlatform>,
  pub timestamp: Option<String>,
  pub application_id: String,
  pub details: Option<String>,
  pub url: Option<String>,
  pub large_image: ImageAsset,
  pub small_image: ImageAsset,
  pub button: ActivityButton,
  pub button2: ActivityButton,
  pub party: ActivityParty,
}

impl Default for ActivityConfig {
  fn default() -> Self {
    Self {
      name: None,
      activity_type: None,
      platform: None,
      timestamp: None,
      application_id: DEFAULT_APPLICATION_ID.to_string(),
      details: None,
      url: None,
      large_image: ImageAsset::default(),
      small_image: ImageAsset::default(),
      button: ActivityButton::default(),
      button2: ActivityButton::default(),
      party: ActivityParty::default(),
    }
  }
}

impl ActivityConfig {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn to_activity(&self, kind: AccountKind) -> Option<Value> {
    let name = self.name.as_ref().filter(|n| !n.is_empty())?;
    match kind {
      AccountKind::User => Some(build_rich_presence(name, self)),
      AccountKind::Bot => Some(build_bot_activity(name, self)),
    }
  }
}

// Discord only accepts name, type, state, and url for bot activities.
fn build_bot_activity(name: &str, cfg: &ActivityConfig) -> Value {
  let ty = match cfg.activity_type {
    None | Some(ActivityType::Custom) => ActivityType::Playing,
    Some(ty) => ty,
  };

  let mut activity = json!({
    "name": name,
    "type": ty.as_i64(),
  });
  if let Some(url) = cfg.url.as_deref().filter(|s| !s.is_empty()) {
    activity["url"] = json!(url);
  }
  if let Some(state) = cfg.details.as_deref().filter(|s| !s.is_empty()) {
    activity["state"] = json!(state);
  }
  activity
}

pub fn build_custom_status(state: &str, emoji: Option<&str>) -> Value {
  let mut activity = json!({
    "name": "Custom Status",
    "type": ActivityType::Custom.as_i64(),
    "state": state,
  });

  if let Some(emoji) = emoji.filter(|e| !e.is_empty()) {
    activity["emoji"] = parse_emoji(emoji);
  }

  activity
}

fn parse_emoji(raw: &str) -> Value {
  let raw = raw.trim();
  // <:name:id> / <a:name:id>, else bare id or unicode name.
  if let Some(inner) = raw.strip_prefix('<').and_then(|s| s.strip_suffix('>')) {
    let (animated, rest) = if let Some(rest) = inner.strip_prefix("a:") {
      (true, rest)
    } else if let Some(rest) = inner.strip_prefix(':') {
      (false, rest)
    } else {
      (false, "")
    };
    if let Some((name, id)) = rest.split_once(':')
      && !name.is_empty()
      && !id.is_empty()
      && id.chars().all(|c| c.is_ascii_digit())
    {
      let mut emoji = json!({ "name": name, "id": id });
      if animated {
        emoji["animated"] = json!(true);
      }
      return emoji;
    }
  }
  if !raw.is_empty() && raw.chars().all(|c| c.is_ascii_digit()) {
    json!({ "id": raw })
  } else {
    json!({ "name": raw })
  }
}

pub fn build_rich_presence(name: &str, cfg: &ActivityConfig) -> Value {
  let mut activity = json!({
    "name": name,
    "application_id": cfg.application_id,
  });

  if let Some(ty) = cfg.activity_type {
    let ty = if ty == ActivityType::Custom {
      ActivityType::Playing
    } else {
      ty
    };
    activity["type"] = json!(ty.as_i64());
  }

  if let Some(platform) = cfg.platform {
    activity["platform"] = json!(platform.as_str());
  }

  if let Some(ts) = cfg.timestamp.as_deref().filter(|s| !s.is_empty())
    && let Ok(start) = ts.parse::<i64>()
  {
    activity["timestamps"] = json!({ "start": start });
  }

  if let Some(details) = cfg.details.as_deref().filter(|s| !s.is_empty()) {
    activity["details"] = json!(details);
  }

  if let Some(url) = cfg.url.as_deref().filter(|s| !s.is_empty()) {
    activity["url"] = json!(url);
  }

  let mut assets = serde_json::Map::new();
  if let Some(image) = cfg.large_image.image.as_deref().filter(|s| !s.is_empty()) {
    assets.insert("large_image".into(), json!(normalize_activity_image(image)));
  }
  if let Some(text) = cfg.large_image.text.as_deref().filter(|s| !s.is_empty()) {
    assets.insert("large_text".into(), json!(text));
  }
  if let Some(image) = cfg.small_image.image.as_deref().filter(|s| !s.is_empty()) {
    assets.insert("small_image".into(), json!(normalize_activity_image(image)));
  }
  if let Some(text) = cfg.small_image.text.as_deref().filter(|s| !s.is_empty()) {
    assets.insert("small_text".into(), json!(text));
  }
  if !assets.is_empty() {
    activity["assets"] = Value::Object(assets);
  }

  let mut buttons = Vec::new();
  let mut button_urls = Vec::new();
  for btn in [&cfg.button, &cfg.button2] {
    match (&btn.name, &btn.url) {
      (Some(name), Some(url)) if !name.is_empty() && !url.is_empty() => {
        buttons.push(json!(name));
        button_urls.push(json!(url));
      }
      _ => {}
    }
  }
  if !buttons.is_empty() {
    activity["buttons"] = Value::Array(buttons);
    activity["metadata"] = json!({ "button_urls": button_urls });
  }

  let mut party = serde_json::Map::new();
  if !cfg.party.id.is_empty() {
    party.insert("id".into(), json!(cfg.party.id));
  }
  if let (Some(current), Some(max)) = (
    cfg.party.current.as_deref().filter(|s| !s.is_empty()),
    cfg.party.max.as_deref().filter(|s| !s.is_empty()),
  ) && let (Ok(current), Ok(max)) = (current.parse::<i64>(), max.parse::<i64>())
  {
    party.insert("size".into(), json!([current, max]));
  }
  if !party.is_empty() {
    activity["party"] = Value::Object(party);
  }

  activity
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::constants::{ActivityPlatform, ActivityType};

  #[test]
  fn custom_status_with_emoji() {
    let v = build_custom_status("hello", Some("🔥"));
    assert_eq!(v["type"], 4);
    assert_eq!(v["name"], "Custom Status");
    assert_eq!(v["state"], "hello");
    assert_eq!(v["emoji"]["name"], "🔥");
  }

  #[test]
  fn custom_status_numeric_emoji_id() {
    let v = build_custom_status("hi", Some("1234567890"));
    assert_eq!(v["emoji"]["id"], "1234567890");
    assert!(v["emoji"].get("name").is_none());
  }

  #[test]
  fn custom_status_message_form_emoji() {
    let v = build_custom_status("hi", Some("<:wave:123>"));
    assert_eq!(v["emoji"]["name"], "wave");
    assert_eq!(v["emoji"]["id"], "123");
    assert!(v["emoji"].get("animated").is_none());

    let animated = build_custom_status("hi", Some("<a:party:456>"));
    assert_eq!(animated["emoji"]["name"], "party");
    assert_eq!(animated["emoji"]["id"], "456");
    assert_eq!(animated["emoji"]["animated"], true);
  }

  #[test]
  fn activity_config_default_matches_new() {
    assert_eq!(ActivityConfig::default(), ActivityConfig::new());
    assert_eq!(
      ActivityConfig::default().application_id,
      DEFAULT_APPLICATION_ID
    );
  }

  #[test]
  fn custom_status_config_requires_text() {
    let empty = CustomStatusConfig::default();
    assert!(empty.to_activity().is_none());

    let cfg = CustomStatusConfig {
      text: Some("brb".into()),
      emoji: Some("💤".into()),
    };
    let v = cfg.to_activity().unwrap();
    assert_eq!(v["type"], 4);
    assert_eq!(v["state"], "brb");
    assert_eq!(v["emoji"]["name"], "💤");
  }

  #[test]
  fn rich_presence_defaults_application_id() {
    let cfg = ActivityConfig {
      name: Some("Game".into()),
      activity_type: Some(ActivityType::Playing),
      application_id: DEFAULT_APPLICATION_ID.into(),
      ..ActivityConfig::new()
    };
    let v = cfg.to_activity(AccountKind::User).unwrap();
    assert_eq!(v["name"], "Game");
    assert_eq!(v["type"], 0);
    assert_eq!(v["application_id"], "1");
  }

  #[test]
  fn rich_presence_buttons_and_party() {
    let cfg = ActivityConfig {
      name: Some("Stream".into()),
      activity_type: Some(ActivityType::Streaming),
      url: Some("https://twitch.tv/x".into()),
      button: ActivityButton {
        name: Some("Join".into()),
        url: Some("https://example.com/a".into()),
      },
      button2: ActivityButton {
        name: Some("Watch".into()),
        url: Some("https://example.com/b".into()),
      },
      party: ActivityParty {
        id: "party1".into(),
        current: Some("2".into()),
        max: Some("5".into()),
      },
      application_id: "1".into(),
      ..Default::default()
    };
    let v = cfg.to_activity(AccountKind::User).unwrap();
    assert_eq!(v["buttons"], json!(["Join", "Watch"]));
    assert_eq!(
      v["metadata"]["button_urls"],
      json!(["https://example.com/a", "https://example.com/b"])
    );
    assert_eq!(v["party"]["id"], "party1");
    assert_eq!(v["party"]["size"], json!([2, 5]));
    assert_eq!(v["url"], "https://twitch.tv/x");
  }

  #[test]
  fn partial_button_ignored() {
    let cfg = ActivityConfig {
      name: Some("X".into()),
      button: ActivityButton {
        name: Some("Only name".into()),
        url: None,
      },
      application_id: "1".into(),
      ..Default::default()
    };
    let v = cfg.to_activity(AccountKind::User).unwrap();
    assert!(v.get("buttons").is_none());
  }

  #[test]
  fn party_id_without_size() {
    let cfg = ActivityConfig {
      name: Some("X".into()),
      party: ActivityParty {
        id: "1".into(),
        current: Some("1".into()),
        max: None,
      },
      application_id: "1".into(),
      ..Default::default()
    };
    let v = cfg.to_activity(AccountKind::User).unwrap();
    assert_eq!(v["party"]["id"], "1");
    assert!(v["party"].get("size").is_none());
  }

  #[test]
  fn assets_text_without_image() {
    let cfg = ActivityConfig {
      name: Some("X".into()),
      large_image: ImageAsset {
        image: None,
        text: Some("hover".into()),
      },
      application_id: "1".into(),
      ..Default::default()
    };
    let v = cfg.to_activity(AccountKind::User).unwrap();
    assert_eq!(v["assets"]["large_text"], "hover");
    assert!(v["assets"].get("large_image").is_none());
  }

  #[test]
  fn no_name_yields_none() {
    let cfg = ActivityConfig::new();
    assert!(cfg.to_activity(AccountKind::User).is_none());
  }

  #[test]
  fn platform_and_timestamp() {
    let cfg = ActivityConfig {
      name: Some("Song".into()),
      activity_type: Some(ActivityType::Listening),
      platform: Some(ActivityPlatform::Xbox),
      timestamp: Some("1700000000".into()),
      details: Some("Artist".into()),
      application_id: "99".into(),
      ..Default::default()
    };
    let v = cfg.to_activity(AccountKind::User).unwrap();
    assert_eq!(v["platform"], "xbox");
    assert_eq!(v["timestamps"]["start"], 1700000000);
    assert_eq!(v["details"], "Artist");
    assert_eq!(v["application_id"], "99");
  }

  #[test]
  fn bot_activity_whitelist() {
    let cfg = ActivityConfig {
      name: Some("Stream".into()),
      activity_type: Some(ActivityType::Streaming),
      url: Some("https://twitch.tv/x".into()),
      details: Some("Live".into()),
      button: ActivityButton {
        name: Some("Join".into()),
        url: Some("https://example.com".into()),
      },
      application_id: "1".into(),
      ..Default::default()
    };
    let v = cfg.to_activity(AccountKind::Bot).unwrap();
    assert_eq!(v["name"], "Stream");
    assert_eq!(v["type"], 1);
    assert_eq!(v["url"], "https://twitch.tv/x");
    assert_eq!(v["state"], "Live");
    assert!(v.get("buttons").is_none());
    assert!(v.get("application_id").is_none());
  }

  #[test]
  fn bot_custom_maps_to_playing() {
    let cfg = ActivityConfig {
      name: Some("brb".into()),
      activity_type: Some(ActivityType::Custom),
      application_id: "1".into(),
      ..Default::default()
    };
    let v = cfg.to_activity(AccountKind::Bot).unwrap();
    assert_eq!(v["name"], "brb");
    assert_eq!(v["type"], 0);
    assert!(v.get("emoji").is_none());
    assert!(v.get("state").is_none());
  }

  #[test]
  fn user_custom_type_on_rich_maps_to_playing() {
    let cfg = ActivityConfig {
      name: Some("brb".into()),
      activity_type: Some(ActivityType::Custom),
      application_id: "1".into(),
      ..Default::default()
    };
    let v = cfg.to_activity(AccountKind::User).unwrap();
    assert_eq!(v["name"], "brb");
    assert_eq!(v["type"], 0);
    assert!(v.get("emoji").is_none());
  }

  #[test]
  fn normalize_cdn_url_to_mp() {
    assert_eq!(
      normalize_activity_image("https://cdn.discordapp.com/app-assets/123/abc.png?size=128"),
      "mp:app-assets/123/abc.png"
    );
    assert_eq!(
      normalize_activity_image("https://media.discordapp.net/external/hash/img.png"),
      "mp:external/hash/img.png"
    );
  }

  #[test]
  fn normalize_preserves_known_forms() {
    assert_eq!(normalize_activity_image("mp:already"), "mp:already");
    assert_eq!(normalize_activity_image("1234567890"), "1234567890");
    assert_eq!(
      normalize_activity_image("youtube:dQw4w9WgXcQ"),
      "youtube:dQw4w9WgXcQ"
    );
  }

  #[test]
  fn rich_presence_rewrites_large_image() {
    let cfg = ActivityConfig {
      name: Some("X".into()),
      large_image: ImageAsset {
        image: Some("https://cdn.discordapp.com/app-assets/1/2.png".into()),
        text: None,
      },
      application_id: "1".into(),
      ..Default::default()
    };
    let v = cfg.to_activity(AccountKind::User).unwrap();
    assert_eq!(v["assets"]["large_image"], "mp:app-assets/1/2.png");
  }
}
