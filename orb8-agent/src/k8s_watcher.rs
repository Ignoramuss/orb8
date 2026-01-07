//! Kubernetes pod watcher for tracking pod lifecycle events
//!
//! Watches all pods in the cluster and maintains the cgroup ID -> pod metadata mapping.

use crate::cgroup::CgroupResolver;
use crate::pod_cache::{PodCache, PodMetadata};
use anyhow::{Context, Result};
use futures::{StreamExt, TryStreamExt};
use k8s_openapi::api::core::v1::Pod;
use kube::{
    api::Api,
    runtime::watcher::{self, Event},
    Client,
};
use log::{debug, error, info, warn};
use std::time::Duration;

/// Kubernetes pod watcher that updates the pod cache
pub struct PodWatcher {
    client: Client,
    cache: PodCache,
    cgroup_resolver: CgroupResolver,
}

impl PodWatcher {
    /// Create a new PodWatcher
    pub async fn new(cache: PodCache) -> Result<Self> {
        let client = Client::try_default()
            .await
            .context("Failed to create Kubernetes client")?;

        Ok(Self {
            client,
            cache,
            cgroup_resolver: CgroupResolver::new(),
        })
    }

    /// Start watching pods and updating the cache
    /// This runs indefinitely and should be spawned as a task
    pub async fn run(&self) -> Result<()> {
        info!("Starting Kubernetes pod watcher...");

        let pods: Api<Pod> = Api::all(self.client.clone());

        let mut backoff = Duration::from_secs(1);
        let max_backoff = Duration::from_secs(30);

        loop {
            match self.watch_pods(&pods).await {
                Ok(_) => {
                    warn!("Pod watch stream ended, reconnecting...");
                    backoff = Duration::from_secs(1);
                }
                Err(e) => {
                    error!("Pod watch failed: {}, reconnecting in {:?}", e, backoff);
                    tokio::time::sleep(backoff).await;
                    backoff = std::cmp::min(backoff * 2, max_backoff);
                }
            }

            // Resync all pods after reconnection
            if let Err(e) = self.resync_all(&pods).await {
                error!("Failed to resync pods: {}", e);
            }
        }
    }

    /// Watch pod events and update the cache
    async fn watch_pods(&self, pods: &Api<Pod>) -> Result<()> {
        let config = watcher::Config::default();
        let mut stream = watcher::watcher(pods.clone(), config).boxed();

        while let Some(event) = stream.try_next().await? {
            match event {
                Event::Apply(pod) | Event::InitApply(pod) => {
                    self.handle_pod_apply(&pod);
                }
                Event::Delete(pod) => {
                    self.handle_pod_delete(&pod);
                }
                Event::Init => {
                    debug!("Pod watcher initialized");
                }
                Event::InitDone => {
                    info!(
                        "Pod watcher initial sync complete. Tracking {} pods (by IP)",
                        self.cache.ip_entries_count()
                    );
                }
            }
        }

        Ok(())
    }

    /// Resync all pods (used after reconnection)
    async fn resync_all(&self, pods: &Api<Pod>) -> Result<()> {
        info!("Resyncing all pods...");

        let pod_list = pods.list(&Default::default()).await?;

        for pod in pod_list {
            self.handle_pod_apply(&pod);
        }

        info!(
            "Resync complete. Tracking {} pods (by IP)",
            self.cache.ip_entries_count()
        );

        Ok(())
    }

    /// Handle a pod being created or updated
    fn handle_pod_apply(&self, pod: &Pod) {
        let namespace = pod.metadata.namespace.as_deref().unwrap_or("default");
        let name = pod.metadata.name.as_deref().unwrap_or("unknown");
        let pod_uid = pod.metadata.uid.as_deref().unwrap_or("");

        if pod_uid.is_empty() {
            return;
        }

        // Get container statuses
        let status = match &pod.status {
            Some(s) => s,
            None => return,
        };

        // Extract pod IP for IP-based enrichment
        let pod_ip = status.pod_ip.as_ref().and_then(|ip| parse_ipv4(ip));

        if let Some(ip) = pod_ip {
            debug!(
                "Pod {}/{} has IP {} (0x{:08x})",
                namespace,
                name,
                status.pod_ip.as_ref().unwrap(),
                ip
            );
        }

        let container_statuses = status.container_statuses.as_deref().unwrap_or(&[]);

        // If we have a pod IP, insert it for IP-based lookup (even without container info)
        if pod_ip.is_some() {
            let metadata = PodMetadata {
                namespace: namespace.to_string(),
                pod_name: name.to_string(),
                pod_uid: pod_uid.to_string(),
                container_name: String::new(),
                container_id: String::new(),
                pod_ip,
            };
            self.cache.insert_by_ip(metadata);
        }

        for cs in container_statuses {
            let container_id = match &cs.container_id {
                Some(id) => id,
                None => continue,
            };

            // Resolve cgroup ID for this container
            match self.cgroup_resolver.resolve(pod_uid, container_id) {
                Ok(cgroup_id) => {
                    let metadata = PodMetadata {
                        namespace: namespace.to_string(),
                        pod_name: name.to_string(),
                        pod_uid: pod_uid.to_string(),
                        container_name: cs.name.clone(),
                        container_id: container_id.clone(),
                        pod_ip,
                    };

                    self.cache.insert(cgroup_id, metadata);

                    debug!(
                        "Mapped cgroup {} -> {}/{}/{}",
                        cgroup_id, namespace, name, cs.name
                    );
                }
                Err(e) => {
                    debug!(
                        "Could not resolve cgroup for {}/{}/{}: {}",
                        namespace, name, cs.name, e
                    );
                }
            }
        }
    }

    /// Handle a pod being deleted
    fn handle_pod_delete(&self, pod: &Pod) {
        let namespace = pod.metadata.namespace.as_deref().unwrap_or("default");
        let name = pod.metadata.name.as_deref().unwrap_or("unknown");
        let pod_uid = pod.metadata.uid.as_deref().unwrap_or("");

        if !pod_uid.is_empty() {
            self.cache.remove_pod(pod_uid);
            debug!("Removed pod {}/{} from cache", namespace, name);
        }
    }
}

/// Parse an IPv4 address string to u32
/// Format: first octet in LSB position (consistent with how eBPF probe reads IPs)
fn parse_ipv4(ip_str: &str) -> Option<u32> {
    let parts: Vec<u8> = ip_str.split('.').filter_map(|p| p.parse().ok()).collect();

    if parts.len() == 4 {
        // Store with first octet in LSB (same as eBPF reads from packets)
        Some(u32::from_le_bytes([parts[0], parts[1], parts[2], parts[3]]))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ipv4_byte_order() {
        // 10.0.0.5: first octet (10) in LSB position
        // [10, 0, 0, 5] as little-endian = 0x0500000A
        assert_eq!(parse_ipv4("10.0.0.5"), Some(0x0500000A));

        // 192.168.1.100: [192, 168, 1, 100] as little-endian = 0x6401A8C0
        assert_eq!(parse_ipv4("192.168.1.100"), Some(0x6401A8C0));

        // 172.18.0.2: [172, 18, 0, 2] as little-endian = 0x020012AC
        assert_eq!(parse_ipv4("172.18.0.2"), Some(0x020012AC));

        // Loopback 127.0.0.1
        assert_eq!(parse_ipv4("127.0.0.1"), Some(0x0100007F));
    }

    #[test]
    fn test_parse_ipv4_invalid() {
        assert_eq!(parse_ipv4(""), None);
        assert_eq!(parse_ipv4("10.0.0"), None);
        assert_eq!(parse_ipv4("not.an.ip.addr"), None);
        assert_eq!(parse_ipv4("256.0.0.1"), None);
    }
}
