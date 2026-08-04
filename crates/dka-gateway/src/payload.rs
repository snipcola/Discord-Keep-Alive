use serde::{Deserialize, Serialize};
use serde_json::{Value, value::RawValue};

pub const OP_DISPATCH: u8 = 0;
pub const OP_HEARTBEAT: u8 = 1;
pub const OP_IDENTIFY: u8 = 2;
pub const OP_PRESENCE_UPDATE: u8 = 3;
pub const OP_RESUME: u8 = 6;
pub const OP_RECONNECT: u8 = 7;
pub const OP_INVALID_SESSION: u8 = 9;
pub const OP_HELLO: u8 = 10;
pub const OP_HEARTBEAT_ACK: u8 = 11;

#[derive(Debug, Clone, Serialize)]
pub struct GatewayPayload {
  pub op: u8,
  pub d: Value,
}

impl GatewayPayload {
  pub fn new(op: u8, d: Value) -> Self {
    Self { op, d }
  }

  pub fn to_json(&self) -> Result<String, serde_json::Error> {
    serde_json::to_string(self)
  }
}

#[derive(Debug, Deserialize)]
pub struct GatewayEnvelope<'a> {
  pub op: u8,
  #[serde(default, borrow)]
  pub d: Option<&'a RawValue>,
  #[serde(default)]
  pub s: Option<i64>,
  #[serde(default, borrow)]
  pub t: Option<&'a str>,
}

impl<'a> GatewayEnvelope<'a> {
  pub fn from_json(text: &'a str) -> Result<Self, serde_json::Error> {
    serde_json::from_str(text)
  }

  pub fn d_str(&self) -> Option<&'a str> {
    self.d.map(RawValue::get)
  }
}

#[derive(Debug, Clone, Deserialize)]
pub struct HelloData {
  pub heartbeat_interval: u64,
}

#[derive(Debug, Clone)]
pub struct ReadyInfo {
  pub username: String,
  pub discriminator: Option<String>,
  pub session_id: String,
  pub resume_gateway_url: String,
}

#[derive(Deserialize)]
struct ReadyBody {
  session_id: String,
  #[serde(default)]
  resume_gateway_url: Option<String>,
  user: ReadyUser,
}

#[derive(Deserialize)]
struct ReadyUser {
  username: String,
  #[serde(default)]
  discriminator: Option<String>,
}

impl ReadyInfo {
  pub fn display_name(&self) -> String {
    match &self.discriminator {
      Some(d) if d != "0" && !d.is_empty() => format!("{}#{}", self.username, d),
      _ => self.username.clone(),
    }
  }

  pub fn from_ready_json(d: &str) -> Option<Self> {
    let body: ReadyBody = serde_json::from_str(d).ok()?;
    Some(Self {
      username: body.user.username,
      discriminator: body.user.discriminator,
      session_id: body.session_id,
      resume_gateway_url: body
        .resume_gateway_url
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| crate::GATEWAY_HOST.to_string()),
    })
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn hello_envelope_keeps_raw_d() {
    let text = r#"{"op":10,"d":{"heartbeat_interval":41250},"s":null,"t":null}"#;
    let env = GatewayEnvelope::from_json(text).unwrap();
    assert_eq!(env.op, OP_HELLO);
    let d = env.d_str().unwrap();
    assert!(d.contains("heartbeat_interval"));
    let hello: HelloData = serde_json::from_str(d).unwrap();
    assert_eq!(hello.heartbeat_interval, 41250);
  }

  #[test]
  fn ready_extracts_fields_ignoring_junk() {
    let big = "x".repeat(10_000);
    let text = format!(
      r#"{{"op":0,"s":1,"t":"READY","d":{{"session_id":"abc","resume_gateway_url":"wss://resume.example","user":{{"username":"snip","discriminator":"1234"}},"guilds":[{{"id":"1","blob":"{big}"}}]}}}}"#
    );
    let env = GatewayEnvelope::from_json(&text).unwrap();
    assert_eq!(env.t, Some("READY"));
    let info = ReadyInfo::from_ready_json(env.d_str().unwrap()).unwrap();
    assert_eq!(info.username, "snip");
    assert_eq!(info.discriminator.as_deref(), Some("1234"));
    assert_eq!(info.session_id, "abc");
    assert_eq!(info.resume_gateway_url, "wss://resume.example");
  }

  #[test]
  fn invalid_session_bool() {
    for (label, d, expected) in [("true", "true", true), ("false", "false", false)] {
      let text = format!(r#"{{"op":9,"d":{d},"s":null,"t":null}}"#);
      let env = GatewayEnvelope::from_json(&text).unwrap();
      let resumable = env
        .d_str()
        .and_then(|raw| serde_json::from_str(raw).ok())
        .unwrap_or(false);
      assert_eq!(resumable, expected, "{label}");
    }
  }

  #[test]
  fn ready_missing_resume_url_falls_back() {
    let d = r#"{"session_id":"sid","user":{"username":"u"}}"#;
    let info = ReadyInfo::from_ready_json(d).unwrap();
    assert_eq!(info.resume_gateway_url, crate::GATEWAY_HOST);
  }

  #[test]
  fn display_name_discriminator() {
    for (label, disc, expected) in [
      ("zero", Some("0"), "user"),
      ("num", Some("1234"), "user#1234"),
    ] {
      let info = ReadyInfo {
        username: "user".into(),
        discriminator: disc.map(str::to_string),
        session_id: "s".into(),
        resume_gateway_url: crate::GATEWAY_HOST.into(),
      };
      assert_eq!(info.display_name(), expected, "{label}");
    }
  }
}
