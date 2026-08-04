//! Config load pipeline for Discord-Keep-Alive: sources → merge → resolve.

pub mod error;
pub(crate) mod load;
pub(crate) mod merge;
pub(crate) mod model;
pub(crate) mod product_defaults;
pub(crate) mod resolve;
pub(crate) mod schema;
pub(crate) mod source;
pub mod token;
pub(crate) mod util;

#[cfg(test)]
mod test_support;

pub use error::ConfigError;
pub use load::{load, load_health_endpoint};
pub use model::{AccountConfig, AppConfig};
pub use source::{Cli, Command, DEFAULT_LOG_LEVEL};
pub use token::SecretString;
