//! Pod metadata cache for correlating cgroup IDs to Kubernetes pods
//!
//! This module maintains a concurrent map from cgroup IDs to pod metadata,
//! allowing the agent to enrich eBPF events with Kubernetes context.

use dashmap::DashMap;
use std::sync::Arc;

/// Metadata about a Kubernetes pod container
#[derive(Debug, Clone)]
pub struct PodMetadata {
    pub namespace: String,
    pub pod_name: String,
    pub pod_uid: String,
    pub container_name: String,
    pub container_id: String,
}

/// Thread-safe cache mapping cgroup IDs to pod metadata
#[derive(Clone)]
pub struct PodCache {
    inner: Arc<DashMap<u64, PodMetadata>>,
}

impl PodCache {
    /// Create a new empty pod cache
    pub fn new() -> Self {
        Self {
            inner: Arc::new(DashMap::new()),
        }
    }

    /// Insert or update a mapping from cgroup ID to pod metadata
    pub fn insert(&self, cgroup_id: u64, metadata: PodMetadata) {
        self.inner.insert(cgroup_id, metadata);
    }

    /// Look up pod metadata by cgroup ID
    pub fn get(&self, cgroup_id: u64) -> Option<PodMetadata> {
        self.inner.get(&cgroup_id).map(|r| r.clone())
    }

    /// Remove a cgroup ID mapping
    pub fn remove(&self, cgroup_id: u64) -> Option<PodMetadata> {
        self.inner.remove(&cgroup_id).map(|(_, v)| v)
    }

    /// Remove all entries matching a pod UID
    pub fn remove_pod(&self, pod_uid: &str) {
        self.inner.retain(|_, v| v.pod_uid != pod_uid);
    }

    /// Get the number of entries in the cache
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Check if the cache is empty
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Get all entries (for debugging/metrics)
    pub fn entries(&self) -> Vec<(u64, PodMetadata)> {
        self.inner
            .iter()
            .map(|r| (*r.key(), r.value().clone()))
            .collect()
    }
}

impl Default for PodCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Enriched event with pod metadata resolved from cgroup ID
#[derive(Debug, Clone)]
pub struct EnrichedEvent {
    pub timestamp_ns: u64,
    pub namespace: String,
    pub pod_name: String,
    pub container_name: String,
    pub src_ip: u32,
    pub dst_ip: u32,
    pub src_port: u16,
    pub dst_port: u16,
    pub protocol: u8,
    pub direction: u8,
    pub packet_len: u16,
}

impl EnrichedEvent {
    /// Create an enriched event from a raw network flow event and pod metadata
    pub fn from_flow(
        event: &orb8_common::NetworkFlowEvent,
        metadata: Option<&PodMetadata>,
    ) -> Self {
        let (namespace, pod_name, container_name) = match metadata {
            Some(m) => (
                m.namespace.clone(),
                m.pod_name.clone(),
                m.container_name.clone(),
            ),
            None => (
                "unknown".to_string(),
                format!("cgroup-{}", event.cgroup_id),
                "unknown".to_string(),
            ),
        };

        Self {
            timestamp_ns: event.timestamp_ns,
            namespace,
            pod_name,
            container_name,
            src_ip: event.src_ip,
            dst_ip: event.dst_ip,
            src_port: event.src_port,
            dst_port: event.dst_port,
            protocol: event.protocol,
            direction: event.direction,
            packet_len: event.packet_len,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pod_cache_insert_get() {
        let cache = PodCache::new();

        let metadata = PodMetadata {
            namespace: "default".to_string(),
            pod_name: "nginx".to_string(),
            pod_uid: "abc-123".to_string(),
            container_name: "nginx".to_string(),
            container_id: "container123".to_string(),
        };

        cache.insert(12345, metadata.clone());

        let retrieved = cache.get(12345).expect("Should find entry");
        assert_eq!(retrieved.namespace, "default");
        assert_eq!(retrieved.pod_name, "nginx");
    }

    #[test]
    fn test_pod_cache_remove_pod() {
        let cache = PodCache::new();

        let metadata1 = PodMetadata {
            namespace: "default".to_string(),
            pod_name: "nginx".to_string(),
            pod_uid: "pod-1".to_string(),
            container_name: "nginx".to_string(),
            container_id: "c1".to_string(),
        };

        let metadata2 = PodMetadata {
            namespace: "default".to_string(),
            pod_name: "nginx".to_string(),
            pod_uid: "pod-1".to_string(),
            container_name: "sidecar".to_string(),
            container_id: "c2".to_string(),
        };

        let metadata3 = PodMetadata {
            namespace: "other".to_string(),
            pod_name: "redis".to_string(),
            pod_uid: "pod-2".to_string(),
            container_name: "redis".to_string(),
            container_id: "c3".to_string(),
        };

        cache.insert(1, metadata1);
        cache.insert(2, metadata2);
        cache.insert(3, metadata3);

        assert_eq!(cache.len(), 3);

        cache.remove_pod("pod-1");

        assert_eq!(cache.len(), 1);
        assert!(cache.get(1).is_none());
        assert!(cache.get(2).is_none());
        assert!(cache.get(3).is_some());
    }
}
