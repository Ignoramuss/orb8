# orb8

> **eBPF-powered observability toolkit for Kubernetes with first-class GPU telemetry**

[![License](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org/)
[![Kubernetes](https://img.shields.io/badge/kubernetes-1.25%2B-326ce5.svg)](https://kubernetes.io/)

**orb8** (_orbit_) is a high-performance observability toolkit built with Rust and eBPF, designed specifically for Kubernetes clusters running AI/ML workloads. It provides deep, low-level visibility into container networking, system calls, resource utilization, and GPU performance with minimal overhead.

> The name "orb8" represents orbiting around your cluster infrastructure, continuously observing and monitoring from all angles.

## Why orb8?

Existing Kubernetes observability tools either focus on high-level metrics or security-specific use cases. orb8 fills the gap by providing:

- **AI Cluster Optimized**: Built for large-scale GPU/TPU/Trainium workloads (GPU telemetry planned for v0.8.0)
- **eBPF Performance**: Sub-1% CPU overhead with zero-copy packet inspection
- **Kubernetes Native**: Auto-discovery of pods, namespaces, and nodes via Kubernetes API
- **Minimal Overhead**: Designed for production environments running massive compute clusters

## Features

### Current (In Development)

- **Network Flow Tracking**: Real-time TCP/UDP/DNS flow monitoring per container (v0.4.0)
- **System Call Monitoring**: Security anomaly detection via syscall pattern analysis (v0.5.0)
- **CPU Scheduling Analysis**: Identify scheduling latency and CPU throttling (v0.6.0)
- **Memory Profiling**: Track allocation patterns and predict OOM events (v0.6.0)

### Planned

- **GPU Telemetry** (v0.8.0 - Research Phase):
  - GPU utilization tracking per pod (via DCGM)
  - GPU memory usage monitoring
  - Experimental: CUDA kernel execution tracing (feasibility TBD)
  - Multi-GPU workload balancing insights

### Kubernetes Integration

- Auto-discovery of cluster resources
- CRD-based configuration for selective tracing
- Prometheus metrics exporter
- Real-time CLI dashboard
- Namespace and pod-level filtering

## Installation

### Prerequisites

- Linux kernel 5.8+ with BTF support
- Kubernetes cluster 1.25+
- Rust 1.75+ (for building from source)
- 20GB free disk space (for development VM on macOS)
- CUDA 11.0+ (for future GPU telemetry features, v0.8.0+)

### From Source

```bash
git clone https://github.com/Ignoramuss/orb8.git
cd orb8
cargo build --release
```

### Deploy to Kubernetes

```bash
kubectl apply -f deploy/orb8-daemonset.yaml
```

## Quick Start

**Note**: orb8 is currently in Phase 0 (Foundation). eBPF probes will be functional in v0.2.0+.

### Network Monitoring (Coming in v0.4.0)

```bash
# Monitor network flows for all pods in a namespace
orb8 trace network --namespace default

# Track DNS queries across the cluster
orb8 trace dns --all-namespaces
```

### System Call Analysis (Coming in v0.5.0)

```bash
# Monitor syscalls for security anomalies
orb8 trace syscall --pod suspicious-pod-456
```

### GPU Telemetry (Planned for v0.8.0)

```bash
# Monitor GPU utilization for AI workloads
orb8 trace gpu --namespace ml-training
```

## Architecture

orb8 consists of three main components:

1. **eBPF Probe Manager**: Dynamically loads and manages eBPF programs for network and syscall tracing (GPU planned)
2. **Kubernetes Controller**: Watches cluster resources and orchestrates probe deployment
3. **Metrics Pipeline**: Aggregates eBPF events and exports to Prometheus/CLI

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for detailed design documentation.

## Comparison with Existing Tools

**Note**: orb8 is in active development. The table shows planned capabilities (see [Roadmap](#roadmap) for timeline).

| Feature | orb8 | Pixie | Tetragon | kubectl-trace |
|---------|------|-------|----------|---------------|
| GPU Telemetry | Yes (planned) | No | No | No |
| eBPF-based | Yes | Yes | Yes | Yes |
| Network Tracing | Yes (planned) | Yes | Yes | Partial |
| Syscall Monitoring | Yes (planned) | Partial | Yes | Yes |
| K8s Native | Yes | Yes | Yes | Yes |
| AI Workload Focus | Yes | No | No | No |
| Overhead | <1% (target) | ~2-5% | <1% | Varies |

## Roadmap

See [ROADMAP.md](ROADMAP.md) for the full development plan.

**Current Status**: Pre-Alpha (Early Development)

In Progress:
- Project initialization and scaffolding (v0.1.0)

Planned:
- Core eBPF probe infrastructure (v0.2.0)
- Kubernetes API integration (v0.3.0)
- Network flow tracing (v0.4.0)
- GPU telemetry (v0.8.0)
- Prometheus exporter (v0.6.0)
- CLI dashboard (v0.7.0)

## Platform Support

orb8 requires Linux for eBPF functionality.

### macOS
Development uses Lima/QEMU to provide a Linux VM with full eBPF support. Your code is automatically mounted from macOS into the VM.

### Linux
Native development with direct kernel access. No VM required.

### Windows
Use WSL2 and follow Linux instructions.

## Contributing

We welcome contributions! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

### Quick Development Workflow

**One command to build, test, and install:**

```bash
git clone https://github.com/Ignoramuss/orb8.git
cd orb8
make magic        # Builds, tests, installs orb8
```

On macOS, this uses a Lima VM. On Linux, it runs natively.

After `make magic` completes:

**macOS:**
```bash
make shell        # Enter VM
orb8 --help       # orb8 is now in PATH
orb8 trace network --namespace default
```

**Linux:**
```bash
orb8 --help       # orb8 is now in PATH
sudo orb8 trace network --namespace default  # eBPF requires root
```

### Manual Development Setup

**macOS:**
```bash
make dev          # Creates Linux VM (5-10 min first time)
make shell        # Enter VM
cargo build
cargo test
```

**Linux:**
```bash
cargo build
cargo test
```

See [DEVELOPMENT.md](DEVELOPMENT.md) for detailed setup instructions and troubleshooting.

## License

Apache License 2.0 - see [LICENSE](LICENSE) for details.

## Acknowledgments

Built with:
- [aya](https://github.com/aya-rs/aya) - Rust eBPF library
- [kube-rs](https://github.com/kube-rs/kube) - Kubernetes API client
- [ratatui](https://github.com/ratatui-org/ratatui) - Terminal UI framework

## Contact

- GitHub: [@Ignoramuss](https://github.com/Ignoramuss)
- Issues: [GitHub Issues](https://github.com/Ignoramuss/orb8/issues)

---

**Note**: This project is in early development. GPU telemetry features require specific hardware and driver configurations. See [docs/GPU_SETUP.md](docs/GPU_SETUP.md) for details (coming soon).
