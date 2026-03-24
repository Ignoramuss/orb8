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
    use log::{debug, error, info, warn};
    use orb8_agent::aggregator::FlowAggregator;
    use orb8_agent::grpc_server;
    use orb8_agent::k8s_watcher::PodWatcher;
    use orb8_agent::net::{
        format_direction, format_ipv4, format_protocol, is_self_traffic, resolve_local_ips,
    };
    use orb8_agent::pod_cache::PodCache;
    use orb8_agent::probe_loader::{poll_events, read_events_dropped, ProbeManager};
    use orb8_proto::NetworkEvent;
    use std::net::SocketAddr;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::signal;

    const GRPC_PORT: u16 = 9090;

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    info!("orb8-agent starting...");

    let pod_cache = PodCache::new();

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

    let aggregator = FlowAggregator::new();

    let events_dropped = Arc::new(AtomicU64::new(0));

    let grpc_addr: SocketAddr = format!("0.0.0.0:{}", GRPC_PORT).parse()?;
    let event_tx = grpc_server::start_server(
        aggregator.clone(),
        pod_cache.clone(),
        grpc_addr,
        events_dropped.clone(),
    )
    .await?;

    let mut manager = ProbeManager::new()?;

    if let Err(e) = EbpfLogger::init(manager.bpf_mut()) {
        warn!(
            "Failed to initialize EbpfLogger: {}. eBPF probe logs will not be visible.",
            e
        );
    }

    let interfaces = ProbeManager::discover_interfaces();
    manager.attach_to_interfaces(&interfaces)?;

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

    loop {
        tokio::select! {
            _ = signal::ctrl_c() => {
                info!("Shutdown signal received");
                break;
            }
            _ = tokio::time::sleep(Duration::from_millis(100)) => {
                if let Some(ref map) = drop_counter_map {
                    events_dropped.store(read_events_dropped(map), Ordering::Relaxed);
                }

                let events = poll_events(&mut ring_buf);
                for event in events {
                    if is_self_traffic(&event, GRPC_PORT, &local_ips) {
                        continue;
                    }

                    // IP-based pod enrichment (single path for both aggregator and streams)
                    let src_pod = pod_cache.get_by_ip(event.src_ip);
                    let dst_pod = pod_cache.get_by_ip(event.dst_ip);

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

                    aggregator.process_event(&event, &namespace, &pod_name);

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
