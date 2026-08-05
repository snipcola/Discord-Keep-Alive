use serde_json::{Map, Value, json};

use crate::constants::{
  AccountKind, ActivityPlatform, ActivityType, DEFAULT_APPLICATION_ID, DEFAULT_PARTY_ID,
};

// Rewrite Discord CDN/media URLs to mp:…. Leave known prefixes and numeric ids alone.
pub(crate) fn normalize_activity_image(image: &str) -> String {
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

pub fn pin_default_activity_timestamps(activities: &mut [ActivityConfig], now: i64) {
  let now = now.to_string();
  for activity in activities {
    if activity.timestamp.as_deref().is_none_or(|ts| ts.is_empty()) {
      activity.timestamp = Some(now.clone());
    }
  }
}

fn put_str(obj: &mut Value, key: &str, value: Option<&str>) {
  if let Some(v) = value.filter(|s| !s.is_empty()) {
    obj[key] = json!(v);
  }
}

fn put_str_map(map: &mut Map<String, Value>, key: &str, value: Option<&str>) {
  if let Some(v) = value.filter(|s| !s.is_empty()) {
    map.insert(key.into(), json!(v));
  }
}

fn put_image(map: &mut Map<String, Value>, key: &str, image: Option<&str>) {
  if let Some(image) = image.filter(|s| !s.is_empty()) {
    map.insert(key.into(), json!(normalize_activity_image(image)));
  }
}

// Bots may only send name, type, state, and url.
fn build_bot_activity(name: &str, cfg: &ActivityConfig) -> Value {
  let ty = match cfg.activity_type {
    None | Some(ActivityType::Custom) => ActivityType::Playing,
    Some(ty) => ty,
  };

  let mut activity = json!({
    "name": name,
    "type": ty.as_i64(),
  });
  put_str(&mut activity, "url", cfg.url.as_deref());
  put_str(&mut activity, "state", cfg.details.as_deref());
  activity
}

pub(crate) fn build_custom_status(state: &str, emoji: Option<&str>) -> Value {
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
  // Accept <:name:id>, <a:name:id>, a bare id, or a unicode name.
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

pub(crate) fn build_rich_presence(name: &str, cfg: &ActivityConfig) -> Value {
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
    && let Ok(start_secs) = ts.parse::<i64>()
  {
    activity["timestamps"] = json!({ "start": start_secs.saturating_mul(1000) });
  }

  put_str(&mut activity, "details", cfg.details.as_deref());
  put_str(&mut activity, "url", cfg.url.as_deref());

  let mut assets = Map::new();
  put_image(&mut assets, "large_image", cfg.large_image.image.as_deref());
  put_str_map(&mut assets, "large_text", cfg.large_image.text.as_deref());
  put_image(&mut assets, "small_image", cfg.small_image.image.as_deref());
  put_str_map(&mut assets, "small_text", cfg.small_image.text.as_deref());
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

  let mut party = Map::new();
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
pub(crate) fn named(name: &str) -> ActivityConfig {
  ActivityConfig {
    name: Some(name.into()),
    application_id: "1".into(),
    ..ActivityConfig::new()
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::constants::{ActivityPlatform, ActivityType};

  fn user_activity(cfg: ActivityConfig) -> Value {
    cfg.to_activity(AccountKind::User).unwrap()
  }

  #[test]
  fn custom_status_emoji_forms() {
    let cases: &[(&str, &str, Value)] = &[
      ("unicode", "🔥", json!({ "name": "🔥" })),
      ("numeric_id", "1234567890", json!({ "id": "1234567890" })),
      (
        "static_tag",
        "<:wave:123>",
        json!({ "name": "wave", "id": "123" }),
      ),
      (
        "animated_tag",
        "<a:party:456>",
        json!({ "name": "party", "id": "456", "animated": true }),
      ),
    ];
    for (label, emoji, expected) in cases {
      let v = build_custom_status("hi", Some(emoji));
      assert_eq!(v["type"], 4, "{label}");
      assert_eq!(v["name"], "Custom Status", "{label}");
      assert_eq!(v["state"], "hi", "{label}");
      assert_eq!(v["emoji"], *expected, "{label}");
    }
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
    assert!(CustomStatusConfig::default().to_activity().is_none());

    let v = CustomStatusConfig {
      text: Some("brb".into()),
      emoji: Some("💤".into()),
    }
    .to_activity()
    .unwrap();
    assert_eq!(v["type"], 4);
    assert_eq!(v["state"], "brb");
    assert_eq!(v["emoji"]["name"], "💤");
  }

  #[test]
  fn rich_presence_defaults_and_fields() {
    let mut cfg = named("Game");
    cfg.activity_type = Some(ActivityType::Playing);
    let v = user_activity(cfg);
    assert_eq!(
      (&v["name"], &v["type"], &v["application_id"]),
      (&json!("Game"), &json!(0), &json!("1")),
      "defaults"
    );

    let mut cfg = named("Song");
    cfg.activity_type = Some(ActivityType::Listening);
    cfg.platform = Some(ActivityPlatform::Xbox);
    cfg.timestamp = Some("1700000000".into());
    cfg.details = Some("Artist".into());
    cfg.application_id = "99".into();
    let v = user_activity(cfg);
    assert_eq!(v["platform"], "xbox", "platform_ts");
    assert_eq!(
      v["timestamps"]["start"], 1_700_000_000_000i64,
      "platform_ts"
    );
    assert_eq!(v["details"], "Artist", "platform_ts");
    assert_eq!(v["application_id"], "99", "platform_ts");
  }

  #[test]
  fn rich_presence_buttons_and_party() {
    let mut cfg = named("Stream");
    cfg.activity_type = Some(ActivityType::Streaming);
    cfg.url = Some("https://twitch.tv/x".into());
    cfg.button = ActivityButton {
      name: Some("Join".into()),
      url: Some("https://example.com/a".into()),
    };
    cfg.button2 = ActivityButton {
      name: Some("Watch".into()),
      url: Some("https://example.com/b".into()),
    };
    cfg.party = ActivityParty {
      id: "party1".into(),
      current: Some("2".into()),
      max: Some("5".into()),
    };
    let v = user_activity(cfg);
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
  fn partial_activity_fields() {
    let mut cfg = named("X");
    cfg.button = ActivityButton {
      name: Some("Only name".into()),
      url: None,
    };
    assert!(
      user_activity(cfg).get("buttons").is_none(),
      "partial_button"
    );

    let mut cfg = named("X");
    cfg.party = ActivityParty {
      id: "1".into(),
      current: Some("1".into()),
      max: None,
    };
    let v = user_activity(cfg);
    assert_eq!(v["party"]["id"], "1", "party_id_only");
    assert!(v["party"].get("size").is_none(), "party_id_only");

    let mut cfg = named("X");
    cfg.large_image = ImageAsset {
      image: None,
      text: Some("hover".into()),
    };
    let v = user_activity(cfg);
    assert_eq!(v["assets"]["large_text"], "hover", "assets_text_only");
    assert!(v["assets"].get("large_image").is_none(), "assets_text_only");
  }

  #[test]
  fn no_name_yields_none() {
    assert!(
      ActivityConfig::new()
        .to_activity(AccountKind::User)
        .is_none()
    );
  }

  #[test]
  fn pin_default_activity_timestamps_fills_missing_and_empty() {
    let mut activities = [
      named("A"),
      ActivityConfig {
        timestamp: Some(String::new()),
        ..named("B")
      },
      ActivityConfig {
        timestamp: Some("1700000000".into()),
        ..named("C")
      },
    ];

    pin_default_activity_timestamps(&mut activities, 1_800_000_000);

    assert_eq!(activities[0].timestamp.as_deref(), Some("1800000000"));
    assert_eq!(activities[1].timestamp.as_deref(), Some("1800000000"));
    assert_eq!(activities[2].timestamp.as_deref(), Some("1700000000"));
  }

  #[test]
  fn pin_default_activity_timestamps_is_stable() {
    let mut activities = [named("A")];

    pin_default_activity_timestamps(&mut activities, 1_800_000_000);
    pin_default_activity_timestamps(&mut activities, 1_900_000_000);

    assert_eq!(activities[0].timestamp.as_deref(), Some("1800000000"));
    let v = user_activity(activities[0].clone());
    assert_eq!(v["timestamps"]["start"], 1_800_000_000_000i64);
  }

  #[test]
  fn bot_activity_whitelist() {
    let mut cfg = named("Stream");
    cfg.activity_type = Some(ActivityType::Streaming);
    cfg.url = Some("https://twitch.tv/x".into());
    cfg.details = Some("Live".into());
    cfg.button = ActivityButton {
      name: Some("Join".into()),
      url: Some("https://example.com".into()),
    };
    let v = cfg.to_activity(AccountKind::Bot).unwrap();
    assert_eq!(
      (&v["name"], &v["type"], &v["url"], &v["state"]),
      (
        &json!("Stream"),
        &json!(1),
        &json!("https://twitch.tv/x"),
        &json!("Live")
      )
    );
    assert!(v.get("buttons").is_none() && v.get("application_id").is_none());
  }

  #[test]
  fn custom_type_maps_to_playing() {
    let mut cfg = named("brb");
    cfg.activity_type = Some(ActivityType::Custom);
    for kind in [AccountKind::Bot, AccountKind::User] {
      let v = cfg.to_activity(kind).unwrap();
      assert_eq!(v["name"], "brb", "{kind:?}");
      assert_eq!(v["type"], 0, "{kind:?}");
      assert!(v.get("emoji").is_none(), "{kind:?}");
      if kind == AccountKind::Bot {
        assert!(v.get("state").is_none(), "{kind:?}");
      }
    }
  }

  #[test]
  fn normalize_activity_image_inputs() {
    for (label, input, expected) in [
      (
        "cdn_query",
        "https://cdn.discordapp.com/app-assets/123/abc.png?size=128",
        "mp:app-assets/123/abc.png",
      ),
      (
        "media_external",
        "https://media.discordapp.net/external/hash/img.png",
        "mp:external/hash/img.png",
      ),
      ("mp_prefix", "mp:already", "mp:already"),
      ("numeric_id", "1234567890", "1234567890"),
      ("youtube", "youtube:dQw4w9WgXcQ", "youtube:dQw4w9WgXcQ"),
    ] {
      assert_eq!(normalize_activity_image(input), expected, "{label}");
    }
  }

  #[test]
  fn rich_presence_rewrites_large_image() {
    let mut cfg = named("X");
    cfg.large_image = ImageAsset {
      image: Some("https://cdn.discordapp.com/app-assets/1/2.png".into()),
      text: None,
    };
    assert_eq!(
      user_activity(cfg)["assets"]["large_image"],
      "mp:app-assets/1/2.png"
    );
  }
}
