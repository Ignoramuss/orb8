//! gRPC server implementation for the agent
//!
//! Implements `OrbitAgentService` to expose flow data and status via gRPC.

use crate::aggregator::{format_direction, format_ipv4, format_protocol, FlowAggregator};
use anyhow::Result;
use log::info;
use orb8_proto::{
    AgentStatus, GetStatusRequest, NetworkEvent, NetworkFlow, OrbitAgentService,
    OrbitAgentServiceServer, QueryFlowsRequest, QueryFlowsResponse, StreamEventsRequest,
};
use std::pin::Pin;
use std::time::Instant;
use tokio::sync::broadcast;
use tokio_stream::{wrappers::BroadcastStream, Stream, StreamExt};
use tonic::{Request, Response, Status};

/// gRPC service implementation
pub struct AgentService {
    aggregator: FlowAggregator,
    node_name: String,
    start_time: Instant,
    event_tx: broadcast::Sender<NetworkEvent>,
}

impl AgentService {
    /// Create a new agent service
    pub fn new(aggregator: FlowAggregator, node_name: String) -> Self {
        let (event_tx, _) = broadcast::channel(1000);

        Self {
            aggregator,
            node_name,
            start_time: Instant::now(),
            event_tx,
        }
    }

    /// Get a sender for broadcasting events to stream subscribers
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
        let limit = if req.limit == 0 {
            1000
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

        // Sort by bytes descending
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
        let stream = BroadcastStream::new(rx).filter_map(move |result| {
            match result {
                Ok(event) => {
                    // Filter by namespace if specified
                    if namespaces.is_empty() || namespaces.contains(&event.namespace) {
                        Some(Ok(event))
                    } else {
                        None
                    }
                }
                Err(_) => None, // Skip lagged events
            }
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
            healthy: true,
            health_message: "OK".to_string(),
            events_processed: self.aggregator.events_processed(),
            events_dropped: self.aggregator.events_dropped(),
            pods_tracked: self.aggregator.pod_cache().len() as u32,
            active_flows: self.aggregator.active_flow_count() as u32,
            uptime_seconds: uptime,
        }))
    }
}

/// Start the gRPC server
pub async fn start_server(
    aggregator: FlowAggregator,
    addr: std::net::SocketAddr,
) -> Result<broadcast::Sender<NetworkEvent>> {
    let node_name = std::env::var("NODE_NAME")
        .or_else(|_| hostname::get().map(|h| h.to_string_lossy().to_string()))
        .unwrap_or_else(|_| "unknown".to_string());

    let service = AgentService::new(aggregator, node_name);
    let event_tx = service.event_sender();

    info!("Starting gRPC server on {}", addr);

    let server = tonic::transport::Server::builder()
        .add_service(OrbitAgentServiceServer::new(service))
        .serve(addr);

    tokio::spawn(async move {
        if let Err(e) = server.await {
            log::error!("gRPC server error: {}", e);
        }
    });

    Ok(event_tx)
}
