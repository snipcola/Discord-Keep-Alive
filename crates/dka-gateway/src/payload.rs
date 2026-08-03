use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const OP_DISPATCH: u8 = 0;
pub const OP_HEARTBEAT: u8 = 1;
pub const OP_IDENTIFY: u8 = 2;
pub const OP_PRESENCE_UPDATE: u8 = 3;
pub const OP_RESUME: u8 = 6;
pub const OP_RECONNECT: u8 = 7;
pub const OP_INVALID_SESSION: u8 = 9;
pub const OP_HELLO: u8 = 10;
pub const OP_HEARTBEAT_ACK: u8 = 11;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayPayload {
  pub op: u8,
  #[serde(default)]
  pub d: Value,
  #[serde(default)]
  pub s: Option<i64>,
  #[serde(default)]
  pub t: Option<String>,
}

impl GatewayPayload {
  pub fn new(op: u8, d: Value) -> Self {
    Self {
      op,
      d,
      s: None,
      t: None,
    }
  }

  pub fn to_json(&self) -> Result<String, serde_json::Error> {
    serde_json::to_string(self)
  }

  pub fn from_json(text: &str) -> Result<Self, serde_json::Error> {
    serde_json::from_str(text)
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

impl ReadyInfo {
  pub fn display_name(&self) -> String {
    match &self.discriminator {
      Some(d) if d != "0" && !d.is_empty() => format!("{}#{}", self.username, d),
      _ => self.username.clone(),
    }
  }

  pub fn from_ready_data(d: &Value) -> Option<Self> {
    let user = d.get("user")?;
    let username = user.get("username")?.as_str()?.to_string();
    let discriminator = user
      .get("discriminator")
      .and_then(|v| v.as_str())
      .map(|s| s.to_string());
    let session_id = d.get("session_id")?.as_str()?.to_string();
    let resume_gateway_url = d
      .get("resume_gateway_url")
      .and_then(|v| v.as_str())
      .unwrap_or(crate::GATEWAY_HOST)
      .to_string();

    Some(Self {
      username,
      discriminator,
      session_id,
      resume_gateway_url,
    })
  }
}
