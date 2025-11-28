//! orb8-agent - Node agent for eBPF observability
//!
//! The agent runs on each Kubernetes node and:
//! - Loads eBPF probes into the kernel
//! - Attaches probes to network interfaces
//! - Polls ring buffers for events
//! - Enriches events with pod metadata
//! - Exposes metrics via gRPC and Prometheus

use anyhow::Result;

#[cfg(not(target_os = "linux"))]
fn main() -> Result<()> {
    eprintln!("Error: orb8-agent requires Linux to run eBPF programs");
    eprintln!("Please run the agent on Linux or in the Lima VM (make shell)");
    std::process::exit(1);
}

#[cfg(target_os = "linux")]
#[tokio::main]
async fn main() -> Result<()> {
    use aya_log::EbpfLogger;
    use log::{info, warn};
    use orb8_agent::probe_loader::ProbeManager;
    use tokio::signal;

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    info!("orb8-agent starting...");

    let mut manager = ProbeManager::new()?;

    if let Err(e) = EbpfLogger::init(manager.bpf_mut()) {
        warn!(
            "Failed to initialize EbpfLogger: {}. eBPF probe logs will not be visible.",
            e
        );
    }

    manager.attach_to_loopback()?;

    info!("orb8-agent running. Press Ctrl+C to exit.");

    signal::ctrl_c().await?;

    info!("Shutdown signal received");
    manager.unload();

    info!("orb8-agent stopped");
    Ok(())
}
