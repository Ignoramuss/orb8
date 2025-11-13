use crate::{Orb8Error, Result};
use k8s_openapi::api::core::v1::Pod;
use kube::{Api, Client};
use tracing::{debug, info};

pub struct K8sClient {
    client: Client,
}

impl K8sClient {
    pub async fn try_default() -> Result<Self> {
        debug!("Initializing Kubernetes client");

        let client = Client::try_default().await.map_err(|e| {
            Orb8Error::KubernetesError(format!("Failed to create K8s client: {}", e))
        })?;

        info!("Successfully connected to Kubernetes cluster");

        Ok(Self { client })
    }

    pub fn pods(&self, namespace: &str) -> Api<Pod> {
        Api::namespaced(self.client.clone(), namespace)
    }

    pub fn pods_all(&self) -> Api<Pod> {
        Api::all(self.client.clone())
    }

    pub async fn get_pod(&self, name: &str, namespace: &str) -> Result<Pod> {
        let pods = self.pods(namespace);

        pods.get(name).await.map_err(|e| {
            if e.to_string().contains("NotFound") || e.to_string().contains("404") {
                Orb8Error::PodNotFound {
                    name: name.to_string(),
                    namespace: namespace.to_string(),
                }
            } else {
                Orb8Error::KubernetesError(format!(
                    "Failed to get pod {}/{}: {}",
                    namespace, name, e
                ))
            }
        })
    }

    pub async fn list_pods(&self, namespace: Option<&str>) -> Result<Vec<Pod>> {
        let pods = match namespace {
            Some(ns) => self.pods(ns),
            None => self.pods_all(),
        };

        let pod_list = pods
            .list(&Default::default())
            .await
            .map_err(|e| Orb8Error::KubernetesError(format!("Failed to list pods: {}", e)))?;

        Ok(pod_list.items)
    }
}
