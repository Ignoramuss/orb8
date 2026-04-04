use crate::aggregator::FlowAggregator;
use crate::health::HealthState;
use crate::net::{format_direction, format_ipv4, format_protocol};
use crate::pod_cache::PodCache;
use anyhow::Result;
use log::info;
use orb8_proto::{
    AgentStatus, GetStatusRequest, NetworkEvent, NetworkFlow, OrbitAgentService,
    OrbitAgentServiceServer, QueryFlowsRequest, QueryFlowsResponse, StreamEventsRequest,
};
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use tokio_stream::{wrappers::BroadcastStream, Stream, StreamExt};
use tokio_util::sync::CancellationToken;
use tonic::{Request, Response, Status};

pub struct AgentService {
    aggregator: FlowAggregator,
    pod_cache: PodCache,
    node_name: String,
    start_time: Instant,
    event_tx: broadcast::Sender<NetworkEvent>,
    events_dropped: Arc<AtomicU64>,
    health: HealthState,
    max_query_limit: usize,
}

impl AgentService {
    pub fn new(
        aggregator: FlowAggregator,
        pod_cache: PodCache,
        node_name: String,
        events_dropped: Arc<AtomicU64>,
        health: HealthState,
        broadcast_channel_size: usize,
        max_query_limit: usize,
    ) -> Self {
        let (event_tx, _) = broadcast::channel(broadcast_channel_size);

        Self {
            aggregator,
            pod_cache,
            node_name,
            start_time: Instant::now(),
            event_tx,
            events_dropped,
            health,
            max_query_limit,
        }
    }

    pub fn event_sender(&self) -> broadcast::Sender<NetworkEvent> {
        self.event_tx.clone()
    }
}

#[tonic::async_trait]
impl OrbitAgentService for AgentService {
    async fn query_flows(
        &self,
        request: Request<QueryFlowsRequest>,
    ) -> Result<Response<QueryFlowsResponse>, Status> {
        let req = request.into_inner();
        let limit = if req.limit == 0 || req.limit as usize > self.max_query_limit {
            self.max_query_limit
        } else {
            req.limit as usize
        };

        let mut flows: Vec<NetworkFlow> = self
            .aggregator
            .get_flows(&req.namespaces)
            .into_iter()
            .filter(|(key, _)| req.pod_names.is_empty() || req.pod_names.contains(&key.pod_name))
            .map(|(key, stats)| NetworkFlow {
                namespace: key.namespace,
                pod_name: key.pod_name,
                src_ip: format_ipv4(key.src_ip),
                dst_ip: format_ipv4(key.dst_ip),
                src_port: key.src_port as u32,
                dst_port: key.dst_port as u32,
                protocol: format_protocol(key.protocol).to_string(),
                direction: format_direction(key.direction).to_string(),
                bytes: stats.bytes,
                packets: stats.packets,
                first_seen_ns: stats.first_seen_ns as i64,
                last_seen_ns: stats.last_seen_ns as i64,
            })
            .collect();

        flows.sort_by(|a, b| b.bytes.cmp(&a.bytes));
        flows.truncate(limit);

        Ok(Response::new(QueryFlowsResponse { flows }))
    }

    type StreamEventsStream =
        Pin<Box<dyn Stream<Item = Result<NetworkEvent, Status>> + Send + 'static>>;

    async fn stream_events(
        &self,
        request: Request<StreamEventsRequest>,
    ) -> Result<Response<Self::StreamEventsStream>, Status> {
        let req = request.into_inner();
        let namespaces: Vec<String> = req.namespaces;

        let rx = self.event_tx.subscribe();
        let stream = BroadcastStream::new(rx).filter_map(move |result| match result {
            Ok(event) => {
                if namespaces.is_empty() || namespaces.contains(&event.namespace) {
                    Some(Ok(event))
                } else {
                    None
                }
            }
            Err(_) => None,
        });

        Ok(Response::new(Box::pin(stream)))
    }

    async fn get_status(
        &self,
        _request: Request<GetStatusRequest>,
    ) -> Result<Response<AgentStatus>, Status> {
        let uptime = self.start_time.elapsed().as_secs() as i64;

        Ok(Response::new(AgentStatus {
            node_name: self.node_name.clone(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            healthy: self.health.is_healthy(),
            health_message: self.health.health_message(),
            events_processed: self.aggregator.events_processed(),
            events_dropped: self.events_dropped.load(Ordering::Relaxed),
            pods_tracked: self.pod_cache.ip_entries_count() as u32,
            active_flows: self.aggregator.active_flow_count() as u32,
            uptime_seconds: uptime,
        }))
    }
}

pub struct ServerConfig {
    pub aggregator: FlowAggregator,
    pub pod_cache: PodCache,
    pub addr: std::net::SocketAddr,
    pub events_dropped: Arc<AtomicU64>,
    pub cancel: CancellationToken,
    pub health: HealthState,
    pub broadcast_channel_size: usize,
    pub max_query_limit: usize,
}

pub async fn start_server(config: ServerConfig) -> Result<(broadcast::Sender<NetworkEvent>, JoinHandle<()>)> {
    let node_name = std::env::var("NODE_NAME")
        .or_else(|_| hostname::get().map(|h| h.to_string_lossy().to_string()))
        .unwrap_or_else(|_| "unknown".to_string());

    let service = AgentService::new(
        config.aggregator,
        config.pod_cache,
        node_name,
        config.events_dropped,
        config.health,
        config.broadcast_channel_size,
        config.max_query_limit,
    );
    let event_tx = service.event_sender();

    info!("Starting gRPC server on {}", config.addr);

    let grpc_service = OrbitAgentServiceServer::new(service);

    let handle = tokio::spawn(async move {
        let server = tonic::transport::Server::builder()
            .add_service(grpc_service)
            .serve_with_shutdown(config.addr, config.cancel.cancelled());
        if let Err(e) = server.await {
            log::error!("gRPC server error: {}", e);
        }
    });

    Ok((event_tx, handle))
}
