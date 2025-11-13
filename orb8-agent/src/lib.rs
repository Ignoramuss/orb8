//! Node agent for orb8 (DaemonSet)
//!
//! Responsibilities:
//! - Load eBPF probes into kernel
//! - Poll ring buffers for events
//! - Watch Kubernetes API for pod metadata
//! - Map cgroup IDs to pods
//! - Aggregate metrics
//! - Expose gRPC API (:9090) and Prometheus metrics (:9091)
//!
//! Implementation will be added in Phase 1-4.
