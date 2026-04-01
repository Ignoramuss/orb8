use crate::health::HealthState;
use dashmap::DashMap;
use orb8_common::NetworkFlowEvent;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

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

const CAPACITY_HIGH_WATERMARK: usize = 95;
const CAPACITY_LOW_WATERMARK: usize = 80;
const EVICTION_PERCENT: usize = 1;

#[derive(Clone)]
pub struct FlowAggregator {
    flows: Arc<DashMap<FlowKey, FlowStats>>,
    events_processed: Arc<AtomicU64>,
    flow_timeout: Duration,
    max_flows: usize,
    health: HealthState,
}

impl FlowAggregator {
    pub fn new(max_flows: usize, flow_timeout: Duration, health: HealthState) -> Self {
        Self {
            flows: Arc::new(DashMap::new()),
            events_processed: Arc::new(AtomicU64::new(0)),
            flow_timeout,
            max_flows,
            health,
        }
    }

    pub fn process_event(&self, event: &NetworkFlowEvent, namespace: &str, pod_name: &str) {
        self.events_processed.fetch_add(1, Ordering::Relaxed);

        let key = FlowKey {
            namespace: namespace.to_string(),
            pod_name: pod_name.to_string(),
            src_ip: event.src_ip,
            dst_ip: event.dst_ip,
            src_port: event.src_port,
            dst_port: event.dst_port,
            protocol: event.protocol,
            direction: event.direction,
        };

        if let Some(mut entry) = self.flows.get_mut(&key) {
            entry.update(event.timestamp_ns, event.packet_len);
            drop(entry);
            self.update_capacity_flag();
            return;
        }

        if self.flows.len() >= self.max_flows {
            self.evict_oldest_flows();
        }

        self.flows
            .entry(key)
            .and_modify(|stats| stats.update(event.timestamp_ns, event.packet_len))
            .or_insert_with(|| FlowStats::new(event.timestamp_ns, event.packet_len));

        self.update_capacity_flag();
    }

    fn evict_oldest_flows(&self) {
        let evict_count = std::cmp::max(self.max_flows * EVICTION_PERCENT / 100, 1);

        let mut candidates: Vec<_> = self
            .flows
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().last_seen))
            .collect();
        candidates.sort_by_key(|(_, last_seen)| *last_seen);

        let evicted = candidates
            .into_iter()
            .take(evict_count)
            .filter(|(key, _)| self.flows.remove(key).is_some())
            .count();

        if evicted > 0 {
            self.health.inc_flow_evictions(evicted as u64);
            log::warn!(
                "Evicted {} flows (table at capacity {})",
                evicted,
                self.max_flows
            );
        }
    }

    fn update_capacity_flag(&self) {
        let len = self.flows.len();
        let high = self.max_flows * CAPACITY_HIGH_WATERMARK / 100;
        if len >= high {
            self.health.set_flow_table_at_capacity(true);
        }
    }

    pub fn get_flows(&self, namespaces: &[String]) -> Vec<(FlowKey, FlowStats)> {
        self.flows
            .iter()
            .filter(|entry| namespaces.is_empty() || namespaces.contains(&entry.key().namespace))
            .map(|entry| (entry.key().clone(), entry.value().clone()))
            .collect()
    }

    pub fn active_flow_count(&self) -> usize {
        self.flows.len()
    }

    pub fn events_processed(&self) -> u64 {
        self.events_processed.load(Ordering::Relaxed)
    }

    pub fn expire_old_flows(&self) -> usize {
        let cutoff = Instant::now() - self.flow_timeout;
        let before = self.flows.len();
        self.flows.retain(|_, stats| stats.last_seen > cutoff);
        let expired = before - self.flows.len();

        let len = self.flows.len();
        let low = self.max_flows * CAPACITY_LOW_WATERMARK / 100;
        if len < low {
            self.health.set_flow_table_at_capacity(false);
        }

        expired
    }
}

impl Default for FlowAggregator {
    fn default() -> Self {
        Self::new(100_000, Duration::from_secs(30), HealthState::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_event(src_ip: u32, dst_ip: u32, src_port: u16, dst_port: u16) -> NetworkFlowEvent {
        NetworkFlowEvent {
            src_ip,
            dst_ip,
            src_port,
            dst_port,
            protocol: 6,
            direction: 1,
            packet_len: 100,
            cgroup_id: 0,
            timestamp_ns: 1_000_000,
        }
    }

    fn test_aggregator() -> FlowAggregator {
        FlowAggregator::default()
    }

    #[test]
    fn test_process_event_creates_flow() {
        let agg = test_aggregator();
        let event = make_event(0x0100000A, 0x0200000A, 8080, 443);

        agg.process_event(&event, "default", "nginx");

        assert_eq!(agg.active_flow_count(), 1);
        assert_eq!(agg.events_processed(), 1);

        let flows = agg.get_flows(&[]);
        assert_eq!(flows.len(), 1);
        assert_eq!(flows[0].0.namespace, "default");
        assert_eq!(flows[0].0.pod_name, "nginx");
        assert_eq!(flows[0].1.bytes, 100);
        assert_eq!(flows[0].1.packets, 1);
    }

    #[test]
    fn test_process_event_aggregates_same_flow() {
        let agg = test_aggregator();
        let event = make_event(0x0100000A, 0x0200000A, 8080, 443);

        agg.process_event(&event, "default", "nginx");
        agg.process_event(&event, "default", "nginx");
        agg.process_event(&event, "default", "nginx");

        assert_eq!(agg.active_flow_count(), 1);
        assert_eq!(agg.events_processed(), 3);

        let flows = agg.get_flows(&[]);
        assert_eq!(flows[0].1.bytes, 300);
        assert_eq!(flows[0].1.packets, 3);
    }

    #[test]
    fn test_different_pods_create_different_flows() {
        let agg = test_aggregator();
        let event = make_event(0x0100000A, 0x0200000A, 8080, 443);

        agg.process_event(&event, "default", "nginx");
        agg.process_event(&event, "default", "redis");

        assert_eq!(agg.active_flow_count(), 2);
    }

    #[test]
    fn test_get_flows_filters_by_namespace() {
        let agg = test_aggregator();
        let event = make_event(0x0100000A, 0x0200000A, 8080, 443);

        agg.process_event(&event, "default", "nginx");
        agg.process_event(&event, "kube-system", "coredns");

        let default_flows = agg.get_flows(&["default".to_string()]);
        assert_eq!(default_flows.len(), 1);
        assert_eq!(default_flows[0].0.namespace, "default");

        let all_flows = agg.get_flows(&[]);
        assert_eq!(all_flows.len(), 2);
    }

    #[test]
    fn test_expire_old_flows() {
        let agg = FlowAggregator {
            flows: Arc::new(DashMap::new()),
            events_processed: Arc::new(AtomicU64::new(0)),
            flow_timeout: Duration::from_millis(0),
            max_flows: 100_000,
            health: HealthState::default(),
        };

        let event = make_event(0x0100000A, 0x0200000A, 8080, 443);
        agg.process_event(&event, "default", "nginx");

        std::thread::sleep(Duration::from_millis(1));
        let expired = agg.expire_old_flows();

        assert_eq!(expired, 1);
        assert_eq!(agg.active_flow_count(), 0);
    }

    #[test]
    fn test_eviction_when_at_capacity() {
        let health = HealthState::new();
        let agg = FlowAggregator::new(5, Duration::from_secs(30), health.clone());

        for i in 0..5u16 {
            let event = make_event(0x0100000A, 0x0200000A, 8080, i);
            agg.process_event(&event, "default", "nginx");
        }
        assert_eq!(agg.active_flow_count(), 5);

        let event = make_event(0x0100000A, 0x0200000A, 8080, 999);
        agg.process_event(&event, "default", "nginx");

        assert!(agg.active_flow_count() <= 5);
        assert!(health.flow_evictions() > 0);
    }

    #[test]
    fn test_capacity_watermark() {
        let health = HealthState::new();
        health.set_probes_attached(true);
        let agg = FlowAggregator::new(10, Duration::from_secs(30), health.clone());

        for i in 0..10u16 {
            let event = make_event(0x0100000A, 0x0200000A, 8080, i);
            agg.process_event(&event, "default", "nginx");
        }

        assert!(health.health_message().contains("flow table at capacity"));
    }
}
