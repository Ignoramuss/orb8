//! orb8 CLI - eBPF-powered observability for Kubernetes
//!
//! Commands:
//! - `orb8 trace network` - Stream live network events from an agent
//! - `orb8 flows` - Query aggregated network flows
//! - `orb8 status` - Get agent status

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use futures::StreamExt;
use orb8_proto::{
    GetStatusRequest, OrbitAgentServiceClient, QueryFlowsRequest, StreamEventsRequest,
};

#[derive(Parser)]
#[command(name = "orb8")]
#[command(about = "eBPF-powered observability for Kubernetes", long_about = None)]
#[command(version)]
struct Cli {
    /// Agent address (host:port)
    #[arg(short, long, default_value = "localhost:9090", global = true)]
    agent: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Stream live network events
    Trace {
        #[command(subcommand)]
        kind: TraceKind,
    },
    /// Query aggregated network flows
    Flows {
        /// Filter by namespace(s)
        #[arg(short, long)]
        namespace: Vec<String>,

        /// Filter by pod name(s)
        #[arg(short, long)]
        pod: Vec<String>,

        /// Maximum number of flows to return
        #[arg(short, long, default_value = "20")]
        limit: u32,
    },
    /// Get agent status
    Status,
}

#[derive(Subcommand)]
enum TraceKind {
    /// Trace network events
    Network {
        /// Filter by namespace(s)
        #[arg(short, long)]
        namespace: Vec<String>,

        /// Duration to trace (e.g., "30s", "5m"). Runs indefinitely if not specified.
        #[arg(short, long)]
        duration: Option<String>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Trace { kind } => match kind {
            TraceKind::Network {
                namespace,
                duration,
            } => {
                trace_network(&cli.agent, namespace, duration).await?;
            }
        },
        Commands::Flows {
            namespace,
            pod,
            limit,
        } => {
            query_flows(&cli.agent, namespace, pod, limit).await?;
        }
        Commands::Status => {
            get_status(&cli.agent).await?;
        }
    }

    Ok(())
}

async fn trace_network(
    agent: &str,
    namespaces: Vec<String>,
    duration: Option<String>,
) -> Result<()> {
    let endpoint = format!("http://{}", agent);
    let mut client = OrbitAgentServiceClient::connect(endpoint)
        .await
        .context("Failed to connect to agent")?;

    let request = StreamEventsRequest {
        namespaces: namespaces.clone(),
    };

    println!(
        "Streaming network events from {}{}...",
        agent,
        if namespaces.is_empty() {
            String::new()
        } else {
            format!(" (namespaces: {})", namespaces.join(", "))
        }
    );
    println!(
        "{:<20} {:<15} {:>21} {:>21} {:>8} {:>9} {:>7}",
        "NAMESPACE/POD", "PROTOCOL", "SOURCE", "DESTINATION", "DIR", "BYTES", "TIME"
    );
    println!("{}", "-".repeat(110));

    let duration_ms = duration.map(|d| parse_duration(&d)).transpose()?;
    let start = std::time::Instant::now();

    let mut stream = client.stream_events(request).await?.into_inner();

    while let Some(result) = stream.next().await {
        if let Some(max_ms) = duration_ms {
            if start.elapsed().as_millis() as u64 >= max_ms {
                println!("\nDuration reached, stopping trace.");
                break;
            }
        }

        match result {
            Ok(event) => {
                let ns_pod = format!("{}/{}", event.namespace, truncate(&event.pod_name, 12));
                let src = format!("{}:{}", event.src_ip, event.src_port);
                let dst = format!("{}:{}", event.dst_ip, event.dst_port);
                let time = chrono::Local::now().format("%H:%M:%S%.3f");

                println!(
                    "{:<20} {:<15} {:>21} {:>21} {:>8} {:>9} {:>7}",
                    truncate(&ns_pod, 20),
                    event.protocol,
                    src,
                    dst,
                    event.direction,
                    format_bytes(event.bytes as u64),
                    time
                );
            }
            Err(e) => {
                eprintln!("Stream error: {}", e);
                break;
            }
        }
    }

    Ok(())
}

async fn query_flows(
    agent: &str,
    namespaces: Vec<String>,
    pod_names: Vec<String>,
    limit: u32,
) -> Result<()> {
    let endpoint = format!("http://{}", agent);
    let mut client = OrbitAgentServiceClient::connect(endpoint)
        .await
        .context("Failed to connect to agent")?;

    let request = QueryFlowsRequest {
        namespaces,
        pod_names,
        limit,
    };

    let response = client.query_flows(request).await?.into_inner();

    if response.flows.is_empty() {
        println!("No flows found.");
        return Ok(());
    }

    println!(
        "{:<20} {:<15} {:>21} {:>21} {:>8} {:>9} {:>8}",
        "NAMESPACE/POD", "PROTOCOL", "SOURCE", "DESTINATION", "DIR", "BYTES", "PACKETS"
    );
    println!("{}", "-".repeat(110));

    for flow in response.flows {
        let ns_pod = format!("{}/{}", flow.namespace, truncate(&flow.pod_name, 12));
        let src = format!("{}:{}", flow.src_ip, flow.src_port);
        let dst = format!("{}:{}", flow.dst_ip, flow.dst_port);

        println!(
            "{:<20} {:<15} {:>21} {:>21} {:>8} {:>9} {:>8}",
            truncate(&ns_pod, 20),
            flow.protocol,
            src,
            dst,
            flow.direction,
            format_bytes(flow.bytes),
            flow.packets
        );
    }

    Ok(())
}

async fn get_status(agent: &str) -> Result<()> {
    let endpoint = format!("http://{}", agent);
    let mut client = OrbitAgentServiceClient::connect(endpoint)
        .await
        .context("Failed to connect to agent")?;

    let response = client.get_status(GetStatusRequest {}).await?.into_inner();

    println!("Agent Status");
    println!("{}", "-".repeat(40));
    println!("Node:             {}", response.node_name);
    println!("Version:          {}", response.version);
    println!(
        "Health:           {}",
        if response.healthy { "OK" } else { "UNHEALTHY" }
    );
    println!("Health Message:   {}", response.health_message);
    println!("Uptime:           {}s", response.uptime_seconds);
    println!("Events Processed: {}", response.events_processed);
    println!("Events Dropped:   {}", response.events_dropped);
    println!("Pods Tracked:     {}", response.pods_tracked);
    println!("Active Flows:     {}", response.active_flows);

    Ok(())
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}

fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{}B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1}KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1}MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.1}GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

fn parse_duration(s: &str) -> Result<u64> {
    let s = s.trim();
    let (num, unit) = if s.ends_with("ms") {
        (s.trim_end_matches("ms"), 1u64)
    } else if s.ends_with('s') {
        (s.trim_end_matches('s'), 1000u64)
    } else if s.ends_with('m') {
        (s.trim_end_matches('m'), 60_000u64)
    } else if s.ends_with('h') {
        (s.trim_end_matches('h'), 3_600_000u64)
    } else {
        anyhow::bail!("Invalid duration format. Use: 30s, 5m, 1h, 500ms");
    };

    let value: u64 = num.parse().context("Invalid duration number")?;
    Ok(value * unit)
}
