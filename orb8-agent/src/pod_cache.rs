use crate::health::HealthState;
use dashmap::DashMap;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct PodMetadata {
    pub namespace: String,
    pub pod_name: String,
    pub pod_uid: String,
    pub container_name: String,
    pub container_id: String,
    pub pod_ip: Option<u32>,
}

#[derive(Clone)]
pub struct PodCache {
    by_cgroup: Arc<DashMap<u64, PodMetadata>>,
    by_ip: Arc<DashMap<u32, PodMetadata>>,
    max_entries: usize,
    health: HealthState,
}

impl PodCache {
    pub fn new(max_entries: usize, health: HealthState) -> Self {
        Self {
            by_cgroup: Arc::new(DashMap::new()),
            by_ip: Arc::new(DashMap::new()),
            max_entries,
            health,
        }
    }

    pub fn insert(&self, cgroup_id: u64, metadata: PodMetadata) {
        if let Some(ip) = metadata.pod_ip {
            self.insert_ip(ip, metadata.clone());
        }
        self.by_cgroup.insert(cgroup_id, metadata);
    }

    pub fn insert_by_ip(&self, metadata: PodMetadata) {
        if let Some(ip) = metadata.pod_ip {
            self.insert_ip(ip, metadata);
        }
    }

    fn insert_ip(&self, ip: u32, metadata: PodMetadata) {
        if self.by_ip.contains_key(&ip) {
            self.by_ip.insert(ip, metadata);
            return;
        }

        if self.by_ip.len() >= self.max_entries {
            log::warn!(
                "Pod cache at capacity ({}), skipping insert for {}/{}",
                self.max_entries,
                metadata.namespace,
                metadata.pod_name
            );
            self.health.set_pod_cache_at_capacity(true);
            self.health.inc_pod_cache_evictions();
            return;
        }

        self.by_ip.insert(ip, metadata);
    }

    pub fn get(&self, cgroup_id: u64) -> Option<PodMetadata> {
        self.by_cgroup.get(&cgroup_id).map(|r| r.clone())
    }

    pub fn get_by_ip(&self, ip: u32) -> Option<PodMetadata> {
        self.by_ip.get(&ip).map(|r| r.clone())
    }

    pub fn remove(&self, cgroup_id: u64) -> Option<PodMetadata> {
        self.by_cgroup.remove(&cgroup_id).map(|(_, v)| v)
    }

    pub fn remove_pod(&self, pod_uid: &str) {
        self.by_cgroup.retain(|_, v| v.pod_uid != pod_uid);
        self.by_ip.retain(|_, v| v.pod_uid != pod_uid);

        if self.by_ip.len() < self.max_entries {
            self.health.set_pod_cache_at_capacity(false);
        }
    }

    pub fn len(&self) -> usize {
        self.by_cgroup.len()
    }

    pub fn ip_entries_count(&self) -> usize {
        self.by_ip.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_cgroup.is_empty()
    }

    pub fn entries(&self) -> Vec<(u64, PodMetadata)> {
        self.by_cgroup
            .iter()
            .map(|r| (*r.key(), r.value().clone()))
            .collect()
    }
}

impl Default for PodCache {
    fn default() -> Self {
        Self::new(10_000, HealthState::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_cache() -> PodCache {
        PodCache::default()
    }

    #[test]
    fn test_pod_cache_insert_get() {
        let cache = test_cache();

        let metadata = PodMetadata {
            namespace: "default".to_string(),
            pod_name: "nginx".to_string(),
            pod_uid: "abc-123".to_string(),
            container_name: "nginx".to_string(),
            container_id: "container123".to_string(),
            pod_ip: Some(0x0A000005),
        };

        cache.insert(12345, metadata.clone());

        let retrieved = cache.get(12345).expect("Should find entry");
        assert_eq!(retrieved.namespace, "default");
        assert_eq!(retrieved.pod_name, "nginx");
    }

    #[test]
    fn test_pod_cache_get_by_ip() {
        let cache = test_cache();

        let metadata = PodMetadata {
            namespace: "default".to_string(),
            pod_name: "nginx".to_string(),
            pod_uid: "abc-123".to_string(),
            container_name: "nginx".to_string(),
            container_id: "container123".to_string(),
            pod_ip: Some(0x0A000005),
        };

        cache.insert_by_ip(metadata);

        let retrieved = cache.get_by_ip(0x0A000005).expect("Should find by IP");
        assert_eq!(retrieved.namespace, "default");
        assert_eq!(retrieved.pod_name, "nginx");

        assert!(cache.get_by_ip(0x0A000099).is_none());
    }

    #[test]
    fn test_pod_cache_remove_pod() {
        let cache = test_cache();

        let metadata1 = PodMetadata {
            namespace: "default".to_string(),
            pod_name: "nginx".to_string(),
            pod_uid: "pod-1".to_string(),
            container_name: "nginx".to_string(),
            container_id: "c1".to_string(),
            pod_ip: Some(0x0A000001),
        };

        let metadata2 = PodMetadata {
            namespace: "default".to_string(),
            pod_name: "nginx".to_string(),
            pod_uid: "pod-1".to_string(),
            container_name: "sidecar".to_string(),
            container_id: "c2".to_string(),
            pod_ip: Some(0x0A000001),
        };

        let metadata3 = PodMetadata {
            namespace: "other".to_string(),
            pod_name: "redis".to_string(),
            pod_uid: "pod-2".to_string(),
            container_name: "redis".to_string(),
            container_id: "c3".to_string(),
            pod_ip: Some(0x0A000002),
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
        assert!(cache.get_by_ip(0x0A000001).is_none());
        assert!(cache.get_by_ip(0x0A000002).is_some());
    }

    #[test]
    fn test_pod_cache_capacity() {
        let health = HealthState::new();
        let cache = PodCache::new(3, health.clone());

        for i in 1..=3u32 {
            let metadata = PodMetadata {
                namespace: "default".to_string(),
                pod_name: format!("pod-{}", i),
                pod_uid: format!("uid-{}", i),
                container_name: "main".to_string(),
                container_id: format!("c-{}", i),
                pod_ip: Some(i),
            };
            cache.insert_by_ip(metadata);
        }
        assert_eq!(cache.ip_entries_count(), 3);

        let overflow = PodMetadata {
            namespace: "default".to_string(),
            pod_name: "pod-4".to_string(),
            pod_uid: "uid-4".to_string(),
            container_name: "main".to_string(),
            container_id: "c-4".to_string(),
            pod_ip: Some(4),
        };
        cache.insert_by_ip(overflow);

        assert_eq!(cache.ip_entries_count(), 3);
        assert!(cache.get_by_ip(4).is_none());
        assert_eq!(health.pod_cache_evictions(), 1);
    }

    #[test]
    fn test_pod_cache_allows_update_at_capacity() {
        let health = HealthState::new();
        let cache = PodCache::new(2, health);

        for i in 1..=2u32 {
            let metadata = PodMetadata {
                namespace: "default".to_string(),
                pod_name: format!("pod-{}", i),
                pod_uid: format!("uid-{}", i),
                container_name: "main".to_string(),
                container_id: format!("c-{}", i),
                pod_ip: Some(i),
            };
            cache.insert_by_ip(metadata);
        }

        let update = PodMetadata {
            namespace: "updated".to_string(),
            pod_name: "pod-1-updated".to_string(),
            pod_uid: "uid-1".to_string(),
            container_name: "main".to_string(),
            container_id: "c-1".to_string(),
            pod_ip: Some(1),
        };
        cache.insert_by_ip(update);

        let retrieved = cache.get_by_ip(1).expect("Should find updated entry");
        assert_eq!(retrieved.namespace, "updated");
    }
}
