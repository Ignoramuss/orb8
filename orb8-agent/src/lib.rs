//! Node agent for orb8 (DaemonSet)
//!
//! Responsibilities:
//! - Load eBPF probes into kernel
//! - Poll ring buffers for events
//! - Watch Kubernetes API for pod metadata
//! - Map cgroup IDs to pods
//! - Aggregate metrics
//! - Expose gRPC API (:9090) and Prometheus metrics (:9091)

#[cfg(target_os = "linux")]
pub mod aggregator;
#[cfg(target_os = "linux")]
pub mod cgroup;
#[cfg(target_os = "linux")]
pub mod grpc_server;
#[cfg(target_os = "linux")]
pub mod k8s_watcher;
#[cfg(target_os = "linux")]
pub mod pod_cache;
#[cfg(target_os = "linux")]
pub mod probe_loader;
