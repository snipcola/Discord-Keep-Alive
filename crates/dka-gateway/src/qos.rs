use std::time::{SystemTime, UNIX_EPOCH};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use prost::Message;

// Mirrors discord_protos.qos_token.v1, with only the fields we read or forward.
#[derive(Message)]
struct QosToken {
  #[prost(message, optional, tag = "1")]
  client_provided: Option<ClientProvidedQosData>,
  #[prost(message, optional, tag = "2")]
  derived: Option<DerivedQosData>,
}

#[derive(Message)]
#[cfg_attr(test, derive(PartialEq))]
struct ClientProvidedQosData {}

#[derive(Clone, Message)]
#[cfg_attr(test, derive(PartialEq))]
struct DerivedQosData {
  #[prost(bytes = "vec", tag = "1")]
  claims: Vec<u8>,
  #[prost(bytes = "vec", tag = "2")]
  signature: Vec<u8>,
  #[prost(uint32, tag = "3")]
  key_id: u32,
}

#[derive(Message)]
struct Claims {
  #[prost(fixed64, tag = "5")]
  expires_at: u64,
}

pub(crate) struct DerivedQos {
  data: DerivedQosData,
  expires_at_ms: u64,
}

impl DerivedQos {
  pub(crate) fn from_ready(b64: &str) -> Option<Self> {
    let data = DerivedQosData::decode(STANDARD.decode(b64).ok()?.as_slice()).ok()?;
    let expires_at_ms = Claims::decode(data.claims.as_slice()).ok()?.expires_at;
    (expires_at_ms != 0).then_some(Self {
      data,
      expires_at_ms,
    })
  }
}

pub(crate) fn identify_token(derived: Option<&DerivedQos>) -> String {
  let now_ms = SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .map_or(0, |d| d.as_millis() as u64);
  encode_identify(derived, now_ms)
}

fn encode_identify(derived: Option<&DerivedQos>, now_ms: u64) -> String {
  let token = QosToken {
    client_provided: Some(ClientProvidedQosData::default()),
    derived: derived
      .filter(|d| d.expires_at_ms > now_ms)
      .map(|d| d.data.clone()),
  };
  STANDARD.encode(token.encode_to_vec())
}

#[cfg(test)]
mod tests {
  use super::*;

  fn derived_data(expires_at: u64) -> DerivedQosData {
    DerivedQosData {
      claims: Claims { expires_at }.encode_to_vec(),
      signature: vec![1, 2, 3],
      key_id: 7,
    }
  }

  fn ready_token(expires_at: u64) -> String {
    STANDARD.encode(derived_data(expires_at).encode_to_vec())
  }

  #[test]
  fn fresh_identify_matches_official_client() {
    assert_eq!(encode_identify(None, 0), "CgA=");
  }

  #[test]
  fn live_token_is_forwarded_verbatim() {
    let derived = DerivedQos::from_ready(&ready_token(5_000)).expect("valid token");
    let bytes = STANDARD
      .decode(encode_identify(Some(&derived), 4_999))
      .unwrap();
    let token = QosToken::decode(bytes.as_slice()).unwrap();
    assert_eq!(
      token.client_provided,
      Some(ClientProvidedQosData::default())
    );
    assert_eq!(token.derived, Some(derived_data(5_000)));
  }

  #[test]
  fn expired_token_is_dropped() {
    let derived = DerivedQos::from_ready(&ready_token(5_000)).unwrap();
    assert_eq!(encode_identify(Some(&derived), 5_000), "CgA=");
  }

  #[test]
  fn malformed_tokens_are_rejected() {
    for input in ["not base64!", "CgU=", ""] {
      assert!(DerivedQos::from_ready(input).is_none(), "{input:?}");
    }
  }
}
