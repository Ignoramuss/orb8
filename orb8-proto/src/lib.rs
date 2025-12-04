//! gRPC protocol definitions for orb8
//!
//! Defines:
//! - `OrbitAgentService` - gRPC service interface for agents
//! - Query and response message types
//! - Streaming event types
//!
//! Generated from `proto/orb8.proto`.

pub mod v1 {
    tonic::include_proto!("orb8.v1");
}

pub use v1::orbit_agent_service_client::OrbitAgentServiceClient;
pub use v1::orbit_agent_service_server::{OrbitAgentService, OrbitAgentServiceServer};
pub use v1::*;
