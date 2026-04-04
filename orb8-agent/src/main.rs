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
    use orb8_agent::config::AgentConfig;
    use orb8_agent::grpc_server;
    use orb8_agent::health::HealthState;
    use orb8_agent::health_server;
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
    use tokio::signal;
    use tokio::signal::unix::{signal as unix_signal, SignalKind};
    use tokio::task::JoinHandle;
    use tokio_util::sync::CancellationToken;

    let config = AgentConfig::from_env();

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    info!("orb8-agent starting...");
    config.log_config();

    let health = HealthState::new();
    let cancel = CancellationToken::new();
    let mut handles: Vec<JoinHandle<()>> = Vec::new();

    let pod_cache = PodCache::new(config.max_pod_cache_entries, health.clone());

    let k8s_enabled = match PodWatcher::new(
        pod_cache.clone(),
        cancel.child_token(),
        health.clone(),
        std::time::Duration::from_secs(1),
        std::time::Duration::from_secs(30),
    )
    .await
    {
        Ok(watcher) => {
            info!("Kubernetes API available - starting pod watcher");
            let watcher_health = health.clone();
            let handle = tokio::spawn(async move {
                if let Err(e) = watcher.run().await {
                    error!("Pod watcher terminated with error: {}", e);
                    watcher_health.set_k8s_watcher_connected(false);
                }
            });
            handles.push(handle);
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

    let aggregator = FlowAggregator::new(config.max_flows, config.flow_timeout, health.clone());

    let events_dropped = Arc::new(AtomicU64::new(0));

    let grpc_addr: SocketAddr = format!("0.0.0.0:{}", config.grpc_port).parse()?;
    let (event_tx, grpc_handle) = grpc_server::start_server(
        aggregator.clone(),
        pod_cache.clone(),
        grpc_addr,
        events_dropped.clone(),
        cancel.child_token(),
        health.clone(),
        config.broadcast_channel_size,
        config.max_query_limit,
    )
    .await?;
    handles.push(grpc_handle);

    // Health HTTP server
    let health_handle = tokio::spawn(health_server::run(
        health.clone(),
        config.health_port,
        cancel.child_token(),
    ));
    handles.push(health_handle);

    let mut manager = ProbeManager::new()?;

    if let Err(e) = EbpfLogger::init(manager.bpf_mut()) {
        warn!(
            "Failed to initialize EbpfLogger: {}. eBPF probe logs will not be visible.",
            e
        );
    }

    let interfaces = ProbeManager::discover_interfaces();
    manager.attach_to_interfaces(&interfaces)?;
    health.set_probes_attached(true);

    let local_ips = resolve_local_ips();
    if local_ips.is_empty() {
        warn!("Could not resolve local IPs; self-traffic filter will use port-only matching");
    } else {
        info!(
            "Self-traffic filter: port {} on {} local IPs",
            config.grpc_port,
            local_ips.len()
        );
    }

    let drop_counter_map = manager.events_dropped_reader();
    let mut ring_buf = manager.events_ring_buf()?;

    info!("orb8-agent running. Press Ctrl+C to exit.");
    info!(
        "gRPC server on :{}. Health server on :{}. K8s enrichment: {}",
        config.grpc_port,
        config.health_port,
        if k8s_enabled { "enabled" } else { "disabled" }
    );

    let expiration_aggregator = aggregator.clone();
    let expiration_cancel = cancel.child_token();
    let expiration_interval = config.expiration_interval;
    let expiration_handle = tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = expiration_cancel.cancelled() => break,
                _ = tokio::time::sleep(expiration_interval) => {
                    let expired = expiration_aggregator.expire_old_flows();
                    if expired > 0 {
                        debug!("Expired {} old flows", expired);
                    }
                }
            }
        }
    });
    handles.push(expiration_handle);

    let grpc_port = config.grpc_port;
    let max_batch_size = config.max_batch_size;
    let poll_interval = config.poll_interval;

    let mut sigterm =
        unix_signal(SignalKind::terminate()).expect("Failed to register SIGTERM handler");

    loop {
        tokio::select! {
            _ = signal::ctrl_c() => {
                info!("Received SIGINT, shutting down...");
                cancel.cancel();
                break;
            }
            _ = sigterm.recv() => {
                info!("Received SIGTERM, shutting down...");
                cancel.cancel();
                break;
            }
            _ = tokio::time::sleep(poll_interval) => {
                if let Some(ref map) = drop_counter_map {
                    events_dropped.store(read_events_dropped(map), Ordering::Relaxed);
                }

                let events = poll_events(&mut ring_buf, max_batch_size);
                for event in events {
                    if is_self_traffic(&event, grpc_port, &local_ips) {
                        continue;
                    }

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

                    if event_tx.send(network_event).is_err() {
                        health.inc_broadcast_drops();
                    }

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

    // Graceful shutdown: wait for tasks with timeout
    let shutdown_deadline = config.shutdown_timeout;
    match tokio::time::timeout(shutdown_deadline, async {
        for handle in handles {
            let _ = handle.await;
        }
    })
    .await
    {
        Ok(_) => info!("All tasks shut down cleanly"),
        Err(_) => warn!(
            "Shutdown timed out after {:?}, force-exiting",
            shutdown_deadline
        ),
    }

    manager.unload();

    info!("orb8-agent stopped");
    Ok(())
}
