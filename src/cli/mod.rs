pub mod commands;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "orb8")]
#[command(author = "Ignoramuss")]
#[command(version = env!("CARGO_PKG_VERSION"))]
#[command(about = "eBPF-powered observability toolkit for Kubernetes with GPU telemetry", long_about = None)]
pub struct Cli {
    #[arg(short, long, global = true, help = "Enable verbose logging")]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
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
pub enum TraceType {
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
