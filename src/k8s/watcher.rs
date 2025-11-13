use crate::Result;
use futures::StreamExt;
use k8s_openapi::api::core::v1::Pod;
use kube::runtime::{watcher, WatchStreamExt};
use kube::Api;
use tracing::{debug, info};

pub struct PodWatcher {
    api: Api<Pod>,
}

impl PodWatcher {
    pub fn new(api: Api<Pod>) -> Self {
        Self { api }
    }

    pub async fn watch<F>(&self, mut handler: F) -> Result<()>
    where
        F: FnMut(PodEvent) -> Result<()>,
    {
        info!("Starting pod watcher");

        let watcher_config = watcher::Config::default();
        let mut stream = watcher(self.api.clone(), watcher_config)
            .applied_objects()
            .boxed();

        while let Some(pod_result) = stream.next().await {
            match pod_result {
                Ok(pod) => {
                    let event = PodEvent::Applied(Box::new(pod));
                    handler(event)?;
                }
                Err(e) => {
                    debug!("Watcher error: {}", e);
                }
            }
        }

        Ok(())
    }
}

#[derive(Debug)]
pub enum PodEvent {
    Applied(Box<Pod>),
    Deleted(String),
}
