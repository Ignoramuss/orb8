//! Flow aggregator for grouping packet events into network flows
//!
//! Aggregates individual packet events into flows based on the 5-tuple:
//! (src_ip, dst_ip, src_port, dst_port, protocol)

use crate::pod_cache::PodCache;
use dashmap::DashMap;
use orb8_common::NetworkFlowEvent;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Key for flow aggregation
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct FlowKey {
    pub namespace: String,
    pub pod_name: String,
    pub src_ip: u32,
    pub dst_ip: u32,
    pub src_port: u16,
    pub dst_port: u16,
    pub protocol: u8,
    pub direction: u8,
}

/// Aggregated flow statistics
#[derive(Debug, Clone)]
pub struct FlowStats {
    pub bytes: u64,
    pub packets: u64,
    pub first_seen: Instant,
    pub last_seen: Instant,
    pub first_seen_ns: u64,
    pub last_seen_ns: u64,
}

impl FlowStats {
    fn new(timestamp_ns: u64, bytes: u16) -> Self {
        let now = Instant::now();
        Self {
            bytes: bytes as u64,
            packets: 1,
            first_seen: now,
            last_seen: now,
            first_seen_ns: timestamp_ns,
            last_seen_ns: timestamp_ns,
        }
    }

    fn update(&mut self, timestamp_ns: u64, bytes: u16) {
        self.bytes += bytes as u64;
        self.packets += 1;
        self.last_seen = Instant::now();
        self.last_seen_ns = timestamp_ns;
    }
}

/// Flow aggregator that groups events by flow key
#[derive(Clone)]
pub struct FlowAggregator {
    flows: Arc<DashMap<FlowKey, FlowStats>>,
    pod_cache: PodCache,
    events_processed: Arc<AtomicU64>,
    events_dropped: Arc<AtomicU64>,
    flow_timeout: Duration,
}

impl FlowAggregator {
    /// Create a new flow aggregator
    pub fn new(pod_cache: PodCache) -> Self {
        Self {
            flows: Arc::new(DashMap::new()),
            pod_cache,
            events_processed: Arc::new(AtomicU64::new(0)),
            events_dropped: Arc::new(AtomicU64::new(0)),
            flow_timeout: Duration::from_secs(30),
        }
    }

    /// Process a network flow event
    pub fn process_event(&self, event: &NetworkFlowEvent) {
        self.events_processed.fetch_add(1, Ordering::Relaxed);

        // Look up pod metadata
        let (namespace, pod_name) = match self.pod_cache.get(event.cgroup_id) {
            Some(meta) => (meta.namespace, meta.pod_name),
            None => ("unknown".to_string(), format!("cgroup-{}", event.cgroup_id)),
        };

        let key = FlowKey {
            namespace,
            pod_name,
            src_ip: event.src_ip,
            dst_ip: event.dst_ip,
            src_port: event.src_port,
            dst_port: event.dst_port,
            protocol: event.protocol,
            direction: event.direction,
        };

        // Update or insert flow
        self.flows
            .entry(key)
            .and_modify(|stats| stats.update(event.timestamp_ns, event.packet_len))
            .or_insert_with(|| FlowStats::new(event.timestamp_ns, event.packet_len));
    }

    /// Get all flows, optionally filtered by namespace
    pub fn get_flows(&self, namespaces: &[String]) -> Vec<(FlowKey, FlowStats)> {
        self.flows
            .iter()
            .filter(|entry| namespaces.is_empty() || namespaces.contains(&entry.key().namespace))
            .map(|entry| (entry.key().clone(), entry.value().clone()))
            .collect()
    }

    /// Get the number of active flows
    pub fn active_flow_count(&self) -> usize {
        self.flows.len()
    }

    /// Get the total number of events processed
    pub fn events_processed(&self) -> u64 {
        self.events_processed.load(Ordering::Relaxed)
    }

    /// Get the total number of events dropped
    pub fn events_dropped(&self) -> u64 {
        self.events_dropped.load(Ordering::Relaxed)
    }

    /// Expire old flows that haven't been seen recently
    pub fn expire_old_flows(&self) -> usize {
        let cutoff = Instant::now() - self.flow_timeout;
        let before = self.flows.len();

        self.flows.retain(|_, stats| stats.last_seen > cutoff);

        before - self.flows.len()
    }

    /// Get a reference to the pod cache
    pub fn pod_cache(&self) -> &PodCache {
        &self.pod_cache
    }
}

/// Format IPv4 address from u32 to dotted notation
/// IP addresses in network packets are big-endian, but read as native u32.
/// On little-endian systems, we need to reverse the byte order for display.
pub fn format_ipv4(ip: u32) -> String {
    format!(
        "{}.{}.{}.{}",
        ip & 0xFF,
        (ip >> 8) & 0xFF,
        (ip >> 16) & 0xFF,
        (ip >> 24) & 0xFF
    )
}

/// Format protocol number to string
pub fn format_protocol(protocol: u8) -> &'static str {
    match protocol {
        1 => "ICMP",
        6 => "TCP",
        17 => "UDP",
        _ => "OTHER",
    }
}

/// Format direction to string
pub fn format_direction(direction: u8) -> &'static str {
    match direction {
        0 => "ingress",
        1 => "egress",
        _ => "unknown",
    }
}
