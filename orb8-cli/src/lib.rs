//! Command-line interface for orb8
//!
//! Commands:
//! - `orb8 trace network` - Stream live network events
//! - `orb8 flows` - Query aggregated network flows
//! - `orb8 status` - Get agent status
//!
//! Usage:
//! ```bash
//! # Stream network events from agent on localhost:9090
//! orb8 trace network
//!
//! # Stream events from specific agent, filtering by namespace
//! orb8 -a 10.0.0.5:9090 trace network -n default
//!
//! # Query top flows
//! orb8 flows --limit 50
//!
//! # Get agent status
//! orb8 status
//! ```

pub use orb8_proto::{AgentStatus, NetworkEvent, NetworkFlow};
