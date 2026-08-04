pub(crate) mod compress;
pub mod heartbeat;
pub mod payload;
pub mod properties;
pub mod reconnect;
pub mod session;

pub use properties::{ClientProperties, Defaults, identify_properties};
pub use session::{LiveSink, SessionParams, run_session};

pub const GATEWAY_VERSION: u8 = 10;
pub const GATEWAY_HOST: &str = "wss://gateway.discord.gg";

pub fn gateway_query() -> String {
  format!("v={GATEWAY_VERSION}&encoding=json&compress=zstd-stream")
}

pub fn gateway_url() -> String {
  format!("{GATEWAY_HOST}/?{}", gateway_query())
}
