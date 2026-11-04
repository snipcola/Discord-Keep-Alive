use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json, value::RawValue};

const REDACTED: &str = "<redacted>";

#[derive(Clone)]
pub struct Token(String);

impl Token {
  pub(crate) fn expose(&self) -> &str {
    &self.0
  }
}

impl From<String> for Token {
  fn from(value: String) -> Self {
    Self(value)
  }
}

impl fmt::Debug for Token {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(REDACTED)
  }
}

pub const OP_DISPATCH: u8 = 0;
pub const OP_HEARTBEAT: u8 = 1;
pub const OP_IDENTIFY: u8 = 2;
pub const OP_PRESENCE_UPDATE: u8 = 3;
pub const OP_RESUME: u8 = 6;
pub const OP_RECONNECT: u8 = 7;
pub const OP_INVALID_SESSION: u8 = 9;
pub const OP_HELLO: u8 = 10;
pub const OP_HEARTBEAT_ACK: u8 = 11;
pub const OP_QOS_HEARTBEAT: u8 = 40;

#[derive(Clone, Serialize)]
pub struct GatewayPayload {
  pub op: u8,
  pub d: Value,
}

impl fmt::Debug for GatewayPayload {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let mut d = self.d.clone();
    if let Some(token) = d.get_mut("token") {
      *token = Value::from(REDACTED);
    }
    f.debug_struct("GatewayPayload")
      .field("op", &self.op)
      .field("d", &d)
      .finish()
  }
}

impl GatewayPayload {
  pub fn new(op: u8, d: Value) -> Self {
    Self { op, d }
  }

  pub fn heartbeat(seq: Option<i64>, qos_version: Option<u64>) -> Self {
    match qos_version {
      Some(ver) => Self::new(
        OP_QOS_HEARTBEAT,
        json!({
          "seq": seq,
          "qos": { "ver": ver, "active": false, "reasons": [] },
        }),
      ),
      None => Self::new(OP_HEARTBEAT, json!(seq)),
    }
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

  pub fn invalid_session_resumable(&self) -> bool {
    self
      .d_str()
      .and_then(|d| serde_json::from_str(d).ok())
      .unwrap_or(false)
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
  pub qos_token: Option<String>,
}

#[derive(Deserialize)]
struct ReadyBody {
  session_id: String,
  #[serde(default)]
  resume_gateway_url: Option<String>,
  #[serde(default)]
  qos_token: Option<String>,
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
      qos_token: body.qos_token,
    })
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn qos_heartbeat_shape() {
    let hb = GatewayPayload::heartbeat(Some(7), Some(31));
    assert_eq!(hb.op, OP_QOS_HEARTBEAT);
    assert_eq!(
      hb.d,
      json!({"seq": 7, "qos": {"ver": 31, "active": false, "reasons": []}})
    );
    assert!(GatewayPayload::heartbeat(None, Some(31)).d["seq"].is_null());
  }

  #[test]
  fn plain_heartbeat_without_qos_version() {
    let hb = GatewayPayload::heartbeat(Some(7), None);
    assert_eq!(hb.op, OP_HEARTBEAT);
    assert_eq!(hb.d, json!(7));
    assert!(GatewayPayload::heartbeat(None, None).d.is_null());
  }

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
      r#"{{"op":0,"s":1,"t":"READY","d":{{"session_id":"abc","resume_gateway_url":"wss://resume.example","user":{{"username":"snip","discriminator":"1234"}},"qos_token":"CgA=","guilds":[{{"id":"1","blob":"{big}"}}]}}}}"#
    );
    let env = GatewayEnvelope::from_json(&text).unwrap();
    assert_eq!(env.t, Some("READY"));
    let info = ReadyInfo::from_ready_json(env.d_str().unwrap()).unwrap();
    assert_eq!(info.username, "snip");
    assert_eq!(info.discriminator.as_deref(), Some("1234"));
    assert_eq!(info.session_id, "abc");
    assert_eq!(info.resume_gateway_url, "wss://resume.example");
    assert_eq!(info.qos_token.as_deref(), Some("CgA="));
  }

  #[test]
  fn invalid_session_resumable_follows_boolean_payload() {
    for (label, text, expected) in [
      ("d true", r#"{"op":9,"d":true,"s":null,"t":null}"#, true),
      ("d false", r#"{"op":9,"d":false,"s":null,"t":null}"#, false),
      ("d null", r#"{"op":9,"d":null,"s":null,"t":null}"#, false),
      ("d absent", r#"{"op":9}"#, false),
      ("d string", r#"{"op":9,"d":"true"}"#, false),
      ("d object", r#"{"op":9,"d":{"resumable":true}}"#, false),
    ] {
      let env = GatewayEnvelope::from_json(text).unwrap();
      assert_eq!(
        env.invalid_session_resumable(),
        expected,
        "invalid session with {label}"
      );
    }
  }

  #[test]
  fn payload_debug_redacts_token() {
    let secret = "payload-secret-0451";
    let payload = GatewayPayload::new(
      OP_RESUME,
      json!({"token": secret, "session_id": "sid", "seq": 3}),
    );
    let debug = format!("{payload:?}");
    assert!(
      !debug.contains(secret),
      "GatewayPayload Debug leaked the token: {debug}"
    );
    assert!(
      debug.contains("sid") && debug.contains(REDACTED),
      "GatewayPayload Debug should keep other fields and mark the token: {debug}"
    );
    assert_eq!(
      payload.d["token"], secret,
      "redaction must not change the payload that is sent"
    );
  }

  #[test]
  fn ready_missing_resume_url_falls_back() {
    let d = r#"{"session_id":"sid","user":{"username":"u"}}"#;
    let info = ReadyInfo::from_ready_json(d).unwrap();
    assert_eq!(info.resume_gateway_url, crate::GATEWAY_HOST);
    assert!(info.qos_token.is_none());
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
        qos_token: None,
      };
      assert_eq!(info.display_name(), expected, "{label}");
    }
  }
}
