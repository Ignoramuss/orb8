pub mod cli;
pub mod ebpf;
pub mod error;
pub mod k8s;
pub mod metrics;
pub mod ui;

pub use error::{Orb8Error, Result};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(feature = "common")]
pub use orb8_common as common;

#[cfg(feature = "cli")]
pub use orb8_cli as cli_crate;
