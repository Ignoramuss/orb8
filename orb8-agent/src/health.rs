use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

#[derive(Clone)]
pub struct HealthState {
    inner: Arc<Inner>,
}

struct Inner {
    probes_attached: AtomicBool,
    k8s_watcher_connected: AtomicBool,
    flow_table_at_capacity: AtomicBool,
    pod_cache_at_capacity: AtomicBool,
    broadcast_drops: AtomicU64,
    flow_evictions: AtomicU64,
    pod_cache_evictions: AtomicU64,
}

impl HealthState {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Inner {
                probes_attached: AtomicBool::new(false),
                k8s_watcher_connected: AtomicBool::new(false),
                flow_table_at_capacity: AtomicBool::new(false),
                pod_cache_at_capacity: AtomicBool::new(false),
                broadcast_drops: AtomicU64::new(0),
                flow_evictions: AtomicU64::new(0),
                pod_cache_evictions: AtomicU64::new(0),
            }),
        }
    }

    pub fn is_healthy(&self) -> bool {
        self.inner.probes_attached.load(Ordering::Relaxed)
            && !self.inner.flow_table_at_capacity.load(Ordering::Relaxed)
    }

    pub fn is_ready(&self) -> bool {
        self.inner.probes_attached.load(Ordering::Relaxed)
    }

    pub fn health_message(&self) -> String {
        let mut issues = Vec::new();

        if !self.inner.probes_attached.load(Ordering::Relaxed) {
            issues.push("probes not attached".to_string());
        }
        if self.inner.flow_table_at_capacity.load(Ordering::Relaxed) {
            issues.push("flow table at capacity".to_string());
        }
        if !self.inner.k8s_watcher_connected.load(Ordering::Relaxed) {
            issues.push("k8s watcher disconnected".to_string());
        }
        if self.inner.pod_cache_at_capacity.load(Ordering::Relaxed) {
            issues.push("pod cache at capacity".to_string());
        }

        let drops = self.broadcast_drops();
        let flow_evictions = self.flow_evictions();
        let cache_evictions = self.pod_cache_evictions();

        if issues.is_empty() {
            let mut msg = "OK".to_string();
            if drops > 0 || flow_evictions > 0 || cache_evictions > 0 {
                msg.push_str(&format!(
                    "; broadcast_drops={}, flow_evictions={}, pod_cache_evictions={}",
                    drops, flow_evictions, cache_evictions
                ));
            }
            msg
        } else {
            let severity = if self.is_healthy() {
                "DEGRADED"
            } else {
                "UNHEALTHY"
            };
            format!("{}: {}", severity, issues.join("; "))
        }
    }

    pub fn set_probes_attached(&self, val: bool) {
        self.inner.probes_attached.store(val, Ordering::Relaxed);
    }

    pub fn set_k8s_watcher_connected(&self, val: bool) {
        self.inner
            .k8s_watcher_connected
            .store(val, Ordering::Relaxed);
    }

    pub fn set_flow_table_at_capacity(&self, val: bool) {
        self.inner
            .flow_table_at_capacity
            .store(val, Ordering::Relaxed);
    }

    pub fn set_pod_cache_at_capacity(&self, val: bool) {
        self.inner
            .pod_cache_at_capacity
            .store(val, Ordering::Relaxed);
    }

    pub fn inc_broadcast_drops(&self) {
        self.inner.broadcast_drops.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_flow_evictions(&self, count: u64) {
        self.inner
            .flow_evictions
            .fetch_add(count, Ordering::Relaxed);
    }

    pub fn inc_pod_cache_evictions(&self) {
        self.inner
            .pod_cache_evictions
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn broadcast_drops(&self) -> u64 {
        self.inner.broadcast_drops.load(Ordering::Relaxed)
    }

    pub fn flow_evictions(&self) -> u64 {
        self.inner.flow_evictions.load(Ordering::Relaxed)
    }

    pub fn pod_cache_evictions(&self) -> u64 {
        self.inner.pod_cache_evictions.load(Ordering::Relaxed)
    }
}

impl Default for HealthState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_state_not_healthy() {
        let health = HealthState::new();
        assert!(!health.is_healthy());
        assert!(!health.is_ready());
    }

    #[test]
    fn test_probes_attached_makes_ready() {
        let health = HealthState::new();
        health.set_probes_attached(true);
        assert!(health.is_ready());
        assert!(health.is_healthy());
    }

    #[test]
    fn test_flow_table_at_capacity_makes_unhealthy() {
        let health = HealthState::new();
        health.set_probes_attached(true);
        assert!(health.is_healthy());

        health.set_flow_table_at_capacity(true);
        assert!(!health.is_healthy());
        assert!(health.is_ready());
    }

    #[test]
    fn test_health_message_ok() {
        let health = HealthState::new();
        health.set_probes_attached(true);
        health.set_k8s_watcher_connected(true);
        assert_eq!(health.health_message(), "OK");
    }

    #[test]
    fn test_health_message_with_counters() {
        let health = HealthState::new();
        health.set_probes_attached(true);
        health.set_k8s_watcher_connected(true);
        health.inc_broadcast_drops();
        health.inc_broadcast_drops();
        assert!(health.health_message().contains("broadcast_drops=2"));
    }

    #[test]
    fn test_health_message_unhealthy() {
        let health = HealthState::new();
        let msg = health.health_message();
        assert!(msg.starts_with("UNHEALTHY"));
        assert!(msg.contains("probes not attached"));
    }

    #[test]
    fn test_health_message_degraded() {
        let health = HealthState::new();
        health.set_probes_attached(true);
        let msg = health.health_message();
        assert!(msg.starts_with("DEGRADED"));
        assert!(msg.contains("k8s watcher disconnected"));
    }

    #[test]
    fn test_counters() {
        let health = HealthState::new();
        assert_eq!(health.broadcast_drops(), 0);
        assert_eq!(health.flow_evictions(), 0);
        assert_eq!(health.pod_cache_evictions(), 0);

        health.inc_broadcast_drops();
        health.inc_flow_evictions(5);
        health.inc_pod_cache_evictions();

        assert_eq!(health.broadcast_drops(), 1);
        assert_eq!(health.flow_evictions(), 5);
        assert_eq!(health.pod_cache_evictions(), 1);
    }

    #[test]
    fn test_clone_shares_state() {
        let health = HealthState::new();
        let clone = health.clone();

        health.set_probes_attached(true);
        assert!(clone.is_ready());

        clone.inc_broadcast_drops();
        assert_eq!(health.broadcast_drops(), 1);
    }
}
