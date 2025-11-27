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
    use orb8_agent::probe_loader::{poll_events, ProbeManager};
    use std::time::Duration;
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

    let mut ring_buf = manager.events_ring_buf()?;

    info!("orb8-agent running. Press Ctrl+C to exit.");
    info!("Polling ring buffer for events...");

    loop {
        tokio::select! {
            _ = signal::ctrl_c() => {
                info!("Shutdown signal received");
                break;
            }
            _ = tokio::time::sleep(Duration::from_millis(100)) => {
                let events = poll_events(&mut ring_buf);
                for event in events {
                    info!(
                        "Event: timestamp={}ns, packet_len={} bytes",
                        event.timestamp_ns,
                        event.packet_len
                    );
                }
            }
        }
    }

    manager.unload();

    info!("orb8-agent stopped");
    Ok(())
}
