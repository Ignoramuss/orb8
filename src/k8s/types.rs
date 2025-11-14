use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PodInfo {
    pub name: String,
    pub namespace: String,
    pub uid: String,
    pub node_name: Option<String>,
    pub pod_ip: Option<String>,
    pub phase: String,
    pub has_gpu: bool,
}

impl PodInfo {
    pub fn from_k8s_pod(pod: &k8s_openapi::api::core::v1::Pod) -> Self {
        let metadata = &pod.metadata;
        let spec = pod.spec.as_ref();
        let status = pod.status.as_ref();

        let has_gpu = spec
            .and_then(|s| s.containers.first())
            .and_then(|c| c.resources.as_ref())
            .and_then(|r| r.limits.as_ref())
            .map(|limits| {
                limits.contains_key("nvidia.com/gpu")
                    || limits.contains_key("amd.com/gpu")
                    || limits.contains_key("aws.amazon.com/neuron")
            })
            .unwrap_or(false);

        Self {
            name: metadata.name.clone().unwrap_or_default(),
            namespace: metadata.namespace.clone().unwrap_or_default(),
            uid: metadata.uid.clone().unwrap_or_default(),
            node_name: spec.and_then(|s| s.node_name.clone()),
            pod_ip: status.and_then(|s| s.pod_ip.clone()),
            phase: status
                .and_then(|s| s.phase.clone())
                .unwrap_or_else(|| "Unknown".to_string()),
            has_gpu,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    pub name: String,
    pub hostname: String,
    pub kernel_version: String,
}
