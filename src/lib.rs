pub mod cli;
pub mod ebpf;
pub mod error;
pub mod k8s;
pub mod metrics;
pub mod ui;

pub use error::{Orb8Error, Result};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
