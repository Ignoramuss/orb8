use crate::cgroup::CgroupResolver;
use crate::health::HealthState;
use crate::net::parse_ipv4;
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
use std::time::{Duration, SystemTime};
use tokio_util::sync::CancellationToken;

pub struct PodWatcher {
    client: Client,
    cache: PodCache,
    cgroup_resolver: CgroupResolver,
    cancel: CancellationToken,
    health: HealthState,
    backoff_min: Duration,
    backoff_max: Duration,
}

impl PodWatcher {
    pub async fn new(
        cache: PodCache,
        cancel: CancellationToken,
        health: HealthState,
        backoff_min: Duration,
        backoff_max: Duration,
    ) -> Result<Self> {
        let client = Client::try_default()
            .await
            .context("Failed to create Kubernetes client")?;

        Ok(Self {
            client,
            cache,
            cgroup_resolver: CgroupResolver::new(),
            cancel,
            health,
            backoff_min,
            backoff_max,
        })
    }

    pub async fn run(&self) -> Result<()> {
        info!("Starting Kubernetes pod watcher...");

        let pods: Api<Pod> = Api::all(self.client.clone());

        let mut backoff = self.backoff_min;

        loop {
            tokio::select! {
                _ = self.cancel.cancelled() => {
                    info!("Pod watcher shutting down");
                    return Ok(());
                }
                result = self.watch_pods(&pods) => {
                    match result {
                        Ok(_) => {
                            warn!("Pod watch stream ended, reconnecting...");
                            backoff = self.backoff_min;
                        }
                        Err(e) => {
                            self.health.set_k8s_watcher_connected(false);
                            error!("Pod watch failed: {}, reconnecting in {:?}", e, backoff);

                            let jitter_ms = jitter_millis(backoff);
                            let sleep_duration = backoff + Duration::from_millis(jitter_ms);

                            tokio::select! {
                                _ = self.cancel.cancelled() => {
                                    info!("Pod watcher shutting down");
                                    return Ok(());
                                }
                                _ = tokio::time::sleep(sleep_duration) => {}
                            }

                            backoff = std::cmp::min(backoff * 2, self.backoff_max);
                        }
                    }
                }
            }

            if self.cancel.is_cancelled() {
                info!("Pod watcher shutting down");
                return Ok(());
            }

            if let Err(e) = self.resync_all(&pods).await {
                error!("Failed to resync pods: {}", e);
            }
        }
    }

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
                    self.health.set_k8s_watcher_connected(true);
                    info!(
                        "Pod watcher initial sync complete. Tracking {} pods (by IP)",
                        self.cache.ip_entries_count()
                    );
                }
            }
        }

        Ok(())
    }

    async fn resync_all(&self, pods: &Api<Pod>) -> Result<()> {
        info!("Resyncing all pods...");

        let pod_list = pods.list(&Default::default()).await?;

        for pod in pod_list {
            self.handle_pod_apply(&pod);
        }

        self.health.set_k8s_watcher_connected(true);
        info!(
            "Resync complete. Tracking {} pods (by IP)",
            self.cache.ip_entries_count()
        );

        Ok(())
    }

    fn handle_pod_apply(&self, pod: &Pod) {
        let namespace = pod.metadata.namespace.as_deref().unwrap_or("default");
        let name = pod.metadata.name.as_deref().unwrap_or("unknown");
        let pod_uid = pod.metadata.uid.as_deref().unwrap_or("");

        if pod_uid.is_empty() {
            return;
        }

        let status = match &pod.status {
            Some(s) => s,
            None => return,
        };

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

fn jitter_millis(backoff: Duration) -> u64 {
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    let max_jitter = (backoff.as_millis() / 4) as u64;
    if max_jitter == 0 {
        0
    } else {
        (nanos as u64) % max_jitter
    }
}
