pub mod events;
pub mod loader;
pub mod maps;

use crate::Result;
use async_trait::async_trait;

#[async_trait]
pub trait Probe: Send + Sync {
    async fn load(&mut self) -> Result<()>;

    async fn attach(&mut self) -> Result<()>;

    async fn detach(&mut self) -> Result<()>;

    async fn unload(&mut self) -> Result<()>;

    fn name(&self) -> &str;

    fn is_loaded(&self) -> bool;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeType {
    Network,
    Syscall,
    Gpu,
}

impl ProbeType {
    pub const fn as_str(&self) -> &'static str {
        match self {
            ProbeType::Network => "network",
            ProbeType::Syscall => "syscall",
            ProbeType::Gpu => "gpu",
        }
    }
}

impl std::fmt::Display for ProbeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}
