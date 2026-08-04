pub(crate) mod compress;
pub(crate) mod heartbeat;
pub(crate) mod payload;
pub mod properties;
pub(crate) mod reconnect;
pub mod session;

pub use properties::{ClientProperties, Defaults};
pub use session::{LiveSink, SessionParams, run_session};

use tokio::sync::watch;

pub(crate) const GATEWAY_VERSION: u8 = 10;
pub(crate) const GATEWAY_HOST: &str = "wss://gateway.discord.gg";

pub(crate) fn gateway_query() -> String {
  format!("v={GATEWAY_VERSION}&encoding=json&compress=zstd-stream")
}

pub(crate) fn gateway_url() -> String {
  format!("{GATEWAY_HOST}/?{}", gateway_query())
}

pub(crate) fn is_shutdown(rx: &watch::Receiver<bool>) -> bool {
  *rx.borrow()
}
