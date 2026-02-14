//! orb8-agent - Node agent for eBPF observability
//!
//! The agent runs on each Kubernetes node and:
//! - Loads eBPF probes into the kernel
//! - Attaches probes to network interfaces
//! - Polls ring buffers for events
//! - Enriches events with pod metadata
//! - Exposes metrics via gRPC and Prometheus

use anyhow::Result;

#[cfg(target_os = "linux")]
use std::collections::HashSet;

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
    use log::{debug, error, info, warn};
    use orb8_agent::aggregator::{format_direction, format_ipv4, format_protocol, FlowAggregator};
    use orb8_agent::grpc_server;
    use orb8_agent::k8s_watcher::PodWatcher;
    use orb8_agent::pod_cache::PodCache;
    use orb8_agent::probe_loader::{poll_events, read_events_dropped, ProbeManager};
    use orb8_proto::NetworkEvent;
    use std::net::SocketAddr;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::signal;

    // gRPC server port - used for binding and traffic filtering
    const GRPC_PORT: u16 = 9090;

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    info!("orb8-agent starting...");

    // Initialize pod cache for cgroup -> pod mapping
    let pod_cache = PodCache::new();

    // Try to start K8s watcher (optional - agent still works without K8s)
    let k8s_enabled = match PodWatcher::new(pod_cache.clone()).await {
        Ok(watcher) => {
            info!("Kubernetes API available - starting pod watcher");
            tokio::spawn(async move {
                if let Err(e) = watcher.run().await {
                    error!("Pod watcher terminated with error: {}", e);
                }
            });
            true
        }
        Err(e) => {
            warn!(
                "Kubernetes API not available: {}. Running without pod enrichment.",
                e
            );
            false
        }
    };

    // Initialize flow aggregator
    let aggregator = FlowAggregator::new(pod_cache.clone());

    // Shared counter for ring buffer drops, updated from eBPF map each poll cycle
    let events_dropped = Arc::new(AtomicU64::new(0));

    // Start gRPC server
    let grpc_addr: SocketAddr = format!("0.0.0.0:{}", GRPC_PORT).parse()?;
    let event_tx =
        grpc_server::start_server(aggregator.clone(), grpc_addr, events_dropped.clone()).await?;

    // Load and attach eBPF probes
    let mut manager = ProbeManager::new()?;

    if let Err(e) = EbpfLogger::init(manager.bpf_mut()) {
        warn!(
            "Failed to initialize EbpfLogger: {}. eBPF probe logs will not be visible.",
            e
        );
    }

    // Discover and attach to network interfaces
    let interfaces = ProbeManager::discover_interfaces();
    manager.attach_to_interfaces(&interfaces)?;

    // Collect local IPs for self-traffic filtering (only filter gRPC port
    // on the agent's own addresses, not on arbitrary remote endpoints)
    let local_ips = resolve_local_ips();
    if local_ips.is_empty() {
        warn!("Could not resolve local IPs; self-traffic filter will use port-only matching");
    } else {
        info!(
            "Self-traffic filter: port {} on {} local IPs",
            GRPC_PORT,
            local_ips.len()
        );
    }

    let drop_counter_map = manager.events_dropped_reader();
    let mut ring_buf = manager.events_ring_buf()?;

    info!("orb8-agent running. Press Ctrl+C to exit.");
    info!(
        "gRPC server listening on {}. K8s enrichment: {}",
        grpc_addr,
        if k8s_enabled { "enabled" } else { "disabled" }
    );

    // Spawn flow expiration task
    let expiration_aggregator = aggregator.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(10)).await;
            let expired = expiration_aggregator.expire_old_flows();
            if expired > 0 {
                debug!("Expired {} old flows", expired);
            }
        }
    });

    // Main event loop
    loop {
        tokio::select! {
            _ = signal::ctrl_c() => {
                info!("Shutdown signal received");
                break;
            }
            _ = tokio::time::sleep(Duration::from_millis(100)) => {
                // Sync ring buffer drop count from eBPF map
                if let Some(ref map) = drop_counter_map {
                    events_dropped.store(read_events_dropped(map), Ordering::Relaxed);
                }

                let events = poll_events(&mut ring_buf);
                for event in events {
                    // Filter out agent's own gRPC traffic (noise from CLI connections).
                    // Match on port AND local IP to avoid hiding legitimate app traffic on port 9090.
                    if is_self_traffic(&event, GRPC_PORT, &local_ips) {
                        continue;
                    }

                    // Process event for aggregation
                    aggregator.process_event(&event);

                    // IP-based pod enrichment
                    // Try to match src_ip or dst_ip to known pod IPs
                    let src_pod = pod_cache.get_by_ip(event.src_ip);
                    let dst_pod = pod_cache.get_by_ip(event.dst_ip);

                    // Determine which pod this traffic belongs to based on direction
                    // ingress: traffic coming TO a local pod (dst is the pod)
                    // egress: traffic going FROM a local pod (src is the pod)
                    let (namespace, pod_name) = if event.direction == orb8_common::direction::INGRESS {
                        dst_pod
                            .map(|p| (p.namespace, p.pod_name))
                            .or_else(|| src_pod.map(|p| (p.namespace, p.pod_name)))
                            .unwrap_or_else(|| ("external".to_string(), "unknown".to_string()))
                    } else {
                        src_pod
                            .map(|p| (p.namespace, p.pod_name))
                            .or_else(|| dst_pod.map(|p| (p.namespace, p.pod_name)))
                            .unwrap_or_else(|| ("external".to_string(), "unknown".to_string()))
                    };

                    // Broadcast to stream subscribers
                    let network_event = NetworkEvent {
                        namespace: namespace.clone(),
                        pod_name: pod_name.clone(),
                        src_ip: format_ipv4(event.src_ip),
                        dst_ip: format_ipv4(event.dst_ip),
                        src_port: event.src_port as u32,
                        dst_port: event.dst_port as u32,
                        protocol: format_protocol(event.protocol).to_string(),
                        direction: format_direction(event.direction).to_string(),
                        bytes: event.packet_len as u32,
                        timestamp_ns: event.timestamp_ns as i64,
                    };
                    let _ = event_tx.send(network_event);

                    // Log event
                    debug!(
                        "[{}/{}] {}:{} -> {}:{} {} {} len={}",
                        namespace, pod_name,
                        format_ipv4(event.src_ip), event.src_port,
                        format_ipv4(event.dst_ip), event.dst_port,
                        format_protocol(event.protocol),
                        format_direction(event.direction),
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

#[cfg(target_os = "linux")]
fn is_self_traffic(
    event: &orb8_common::NetworkFlowEvent,
    grpc_port: u16,
    local_ips: &HashSet<u32>,
) -> bool {
    if local_ips.is_empty() {
        // Fallback: port-only filter when local IPs couldn't be resolved
        return event.src_port == grpc_port || event.dst_port == grpc_port;
    }
    (event.src_port == grpc_port && local_ips.contains(&event.src_ip))
        || (event.dst_port == grpc_port && local_ips.contains(&event.dst_ip))
}

#[cfg(target_os = "linux")]
fn resolve_local_ips() -> HashSet<u32> {
    let mut ips = HashSet::new();
    ips.insert(u32::from_le_bytes([127, 0, 0, 1]));

    if let Ok(content) = std::fs::read_to_string("/proc/net/fib_trie") {
        // Parse local addresses from fib_trie. Lines matching "LOCAL" on the next
        // line indicate the preceding /32 host address is a local IP.
        let lines: Vec<&str> = content.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            if line.trim().starts_with("|-- ") || line.trim().starts_with("+-- ") {
                if let Some(next_line) = lines.get(i + 1) {
                    if next_line.contains("/32 host LOCAL") {
                        let ip_str = line
                            .trim()
                            .trim_start_matches("|-- ")
                            .trim_start_matches("+-- ");
                        if let Some(ip) = parse_ipv4_le(ip_str) {
                            ips.insert(ip);
                        }
                    }
                }
            }
        }
    }
    ips
}

#[cfg(target_os = "linux")]
fn parse_ipv4_le(ip_str: &str) -> Option<u32> {
    let parts: Vec<u8> = ip_str.split('.').filter_map(|p| p.parse().ok()).collect();
    if parts.len() == 4 {
        Some(u32::from_le_bytes([parts[0], parts[1], parts[2], parts[3]]))
    } else {
        None
    }
}
