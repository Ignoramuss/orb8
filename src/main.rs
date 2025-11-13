use clap::{Parser, Subcommand};
use std::process;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

#[derive(Parser)]
#[command(name = "orb8")]
#[command(author = "Mayank Dighe")]
#[command(version = "0.1.0")]
#[command(about = "eBPF-powered observability toolkit for Kubernetes with GPU telemetry", long_about = None)]
struct Cli {
    #[arg(short, long, global = true, help = "Enable verbose logging")]
    verbose: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "Start tracing operations")]
    Trace {
        #[command(subcommand)]
        trace_type: TraceType,
    },
    #[command(about = "Display cluster information")]
    Info {
        #[arg(short, long, help = "Kubernetes namespace")]
        namespace: Option<String>,
    },
    #[command(about = "Export metrics to various formats")]
    Export {
        #[arg(short, long, help = "Output format (json, yaml, prometheus)")]
        format: String,

        #[arg(short, long, help = "Output file path")]
        output: Option<String>,
    },
}

#[derive(Subcommand)]
enum TraceType {
    #[command(about = "Trace network flows")]
    Network {
        #[arg(short, long, help = "Kubernetes namespace to monitor")]
        namespace: Option<String>,

        #[arg(short, long, help = "Specific pod name")]
        pod: Option<String>,

        #[arg(short, long, help = "Monitor all namespaces")]
        all_namespaces: bool,
    },
    #[command(about = "Trace DNS queries")]
    Dns {
        #[arg(short, long, help = "Kubernetes namespace to monitor")]
        namespace: Option<String>,

        #[arg(short, long, help = "Monitor all namespaces")]
        all_namespaces: bool,
    },
    #[command(about = "Trace system calls")]
    Syscall {
        #[arg(short, long, help = "Specific pod name")]
        pod: String,

        #[arg(short, long, help = "Kubernetes namespace")]
        namespace: Option<String>,
    },
    #[command(about = "Monitor GPU utilization")]
    Gpu {
        #[arg(short, long, help = "Kubernetes namespace to monitor")]
        namespace: Option<String>,

        #[arg(short, long, help = "Specific pod name")]
        pod: Option<String>,
    },
    #[command(about = "Detect GPU memory leaks")]
    GpuMemory {
        #[arg(short, long, help = "Specific pod name")]
        pod: String,

        #[arg(short, long, help = "Kubernetes namespace")]
        namespace: Option<String>,
    },
}

fn main() {
    let cli = Cli::parse();

    let log_level = if cli.verbose {
        Level::DEBUG
    } else {
        Level::INFO
    };

    let subscriber = FmtSubscriber::builder().with_max_level(log_level).finish();

    tracing::subscriber::set_global_default(subscriber).expect("Failed to set tracing subscriber");

    info!("Starting orb8 v{}", env!("CARGO_PKG_VERSION"));

    let result = match cli.command {
        Some(Commands::Trace { trace_type }) => handle_trace(trace_type),
        Some(Commands::Info { namespace }) => handle_info(namespace),
        Some(Commands::Export { format, output }) => handle_export(format, output),
        None => {
            eprintln!("No command specified. Use --help for usage information.");
            process::exit(1);
        }
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        process::exit(1);
    }
}

fn handle_trace(trace_type: TraceType) -> Result<(), Box<dyn std::error::Error>> {
    match trace_type {
        TraceType::Network {
            namespace,
            pod,
            all_namespaces,
        } => {
            info!("Network tracing requested");
            if all_namespaces {
                info!("Monitoring all namespaces");
            } else if let Some(ns) = namespace {
                info!("Monitoring namespace: {}", ns);
            }
            if let Some(p) = pod {
                info!("Monitoring pod: {}", p);
            }
            println!("🚧 Network tracing coming in v0.4.0 - See ROADMAP.md");
        }
        TraceType::Dns {
            namespace,
            all_namespaces,
        } => {
            info!("DNS tracing requested");
            if all_namespaces {
                info!("Monitoring all namespaces");
            } else if let Some(ns) = namespace {
                info!("Monitoring namespace: {}", ns);
            }
            println!("🚧 DNS tracing coming in v0.4.0 - See ROADMAP.md");
        }
        TraceType::Syscall { pod, namespace } => {
            info!("Syscall tracing requested for pod: {}", pod);
            if let Some(ns) = namespace {
                info!("Namespace: {}", ns);
            }
            println!("🚧 Syscall tracing coming in v0.5.0 - See ROADMAP.md");
        }
        TraceType::Gpu { namespace, pod } => {
            info!("GPU monitoring requested");
            if let Some(ns) = namespace {
                info!("Monitoring namespace: {}", ns);
            }
            if let Some(p) = pod {
                info!("Monitoring pod: {}", p);
            }
            println!("🚧 GPU monitoring coming in v0.8.0 - See ROADMAP.md");
        }
        TraceType::GpuMemory { pod, namespace } => {
            info!("GPU memory leak detection requested for pod: {}", pod);
            if let Some(ns) = namespace {
                info!("Namespace: {}", ns);
            }
            println!("🚧 GPU memory leak detection coming in v0.9.0 - See ROADMAP.md");
        }
    }
    Ok(())
}

fn handle_info(namespace: Option<String>) -> Result<(), Box<dyn std::error::Error>> {
    info!("Cluster info requested");
    if let Some(ns) = namespace {
        info!("Namespace filter: {}", ns);
    }
    println!("🚧 Cluster info coming in v0.3.0 - See ROADMAP.md");
    Ok(())
}

fn handle_export(format: String, output: Option<String>) -> Result<(), Box<dyn std::error::Error>> {
    info!("Export requested - format: {}", format);
    if let Some(out) = output {
        info!("Output file: {}", out);
    }
    println!("🚧 Metrics export coming in v0.6.0 - See ROADMAP.md");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_parsing() {
        let cli = Cli::parse_from(["orb8", "--help"]);
        assert_eq!(cli.verbose, false);
    }
}
