use thiserror::Error;

#[derive(Error, Debug)]
pub enum Orb8Error {
    #[error("eBPF error: {0}")]
    EbpfError(String),

    #[error("Failed to load eBPF program: {0}")]
    ProgramLoadFailed(String),

    #[error("Failed to attach eBPF program: {0}")]
    AttachFailed(String),

    #[error("Kubernetes error: {0}")]
    KubernetesError(String),

    #[error("Pod not found: {name} in namespace {namespace}")]
    PodNotFound { name: String, namespace: String },

    #[error("Metrics error: {0}")]
    MetricsError(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Kernel version {version} is too old. Minimum required: {min_version}")]
    KernelVersionTooOld {
        version: String,
        min_version: String,
    },

    #[error("BTF not available. Ensure kernel is compiled with CONFIG_DEBUG_INFO_BTF")]
    BtfNotAvailable,

    #[error("Unsupported feature on this system: {0}")]
    UnsupportedFeature(String),
}

pub type Result<T> = std::result::Result<T, Orb8Error>;
