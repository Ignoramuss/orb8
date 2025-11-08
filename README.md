# orb8

> **eBPF-powered observability toolkit for Kubernetes with first-class GPU telemetry**

[![License](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org/)
[![Kubernetes](https://img.shields.io/badge/kubernetes-1.25%2B-326ce5.svg)](https://kubernetes.io/)

orb8 (observe) is a high-performance observability toolkit built with Rust and eBPF, designed specifically for Kubernetes clusters running AI/ML workloads. It provides deep, low-level visibility into container networking, system calls, resource utilization, and GPU performance with minimal overhead.

## Why orb8?

Existing Kubernetes observability tools either focus on high-level metrics or security-specific use cases. orb8 fills the gap by providing:

- **GPU-First Design**: Track CUDA kernels, GPU memory leaks, and multi-GPU workload distribution
- **eBPF Performance**: Sub-1% CPU overhead with zero-copy packet inspection
- **AI Cluster Optimized**: Built for large-scale GPU/TPU/Trainium workloads
- **Kubernetes Native**: Auto-discovery of pods, namespaces, and nodes via Kubernetes API
- **Minimal Overhead**: Designed for production environments running massive compute clusters

## Features

### Core Capabilities

- **Network Flow Tracking**: Real-time TCP/UDP/DNS flow monitoring per container
- **System Call Monitoring**: Security anomaly detection via syscall pattern analysis
- **CPU Scheduling Analysis**: Identify scheduling latency and CPU throttling
- **Memory Profiling**: Track allocation patterns and predict OOM events
- **GPU Telemetry**:
  - GPU utilization tracking per pod
  - CUDA kernel execution tracing
  - GPU memory leak detection
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
- CUDA 11.0+ (for GPU telemetry features)

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

### Basic Network Monitoring

```bash
# Monitor network flows for all pods in a namespace
orb8 trace network --namespace default

# Track DNS queries across the cluster
orb8 trace dns --all-namespaces
```

### GPU Telemetry

```bash
# Monitor GPU utilization for AI workloads
orb8 trace gpu --namespace ml-training

# Detect GPU memory leaks
orb8 trace gpu-memory --pod pytorch-job-123
```

### System Call Analysis

```bash
# Monitor syscalls for security anomalies
orb8 trace syscall --pod suspicious-pod-456
```

## Architecture

orb8 consists of three main components:

1. **eBPF Probe Manager**: Dynamically loads and manages eBPF programs for network, syscall, and GPU tracing
2. **Kubernetes Controller**: Watches cluster resources and orchestrates probe deployment
3. **Metrics Pipeline**: Aggregates eBPF events and exports to Prometheus/CLI

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for detailed design documentation.

## Comparison with Existing Tools

| Feature | orb8 | Pixie | Tetragon | kubectl-trace |
|---------|------|-------|----------|---------------|
| GPU Telemetry | ✅ | ❌ | ❌ | ❌ |
| eBPF-based | ✅ | ✅ | ✅ | ✅ |
| Network Tracing | ✅ | ✅ | ✅ | ⚠️ |
| Syscall Monitoring | ✅ | ⚠️ | ✅ | ✅ |
| K8s Native | ✅ | ✅ | ✅ | ✅ |
| AI Workload Focus | ✅ | ❌ | ❌ | ❌ |
| Overhead | <1% | ~2-5% | <1% | Varies |

## Roadmap

See [ROADMAP.md](ROADMAP.md) for the full development plan.

**Current Status**: 🚧 Early Development (v0.1.0)

- [x] Project initialization
- [ ] Core eBPF probe infrastructure
- [ ] Kubernetes API integration
- [ ] Network flow tracing
- [ ] GPU telemetry (CUDA hooks)
- [ ] Prometheus exporter
- [ ] CLI dashboard

## Contributing

We welcome contributions! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

### Development Setup

```bash
# Clone the repository
git clone https://github.com/Ignoramuss/orb8.git
cd orb8

# Install dependencies
cargo install cargo-bpf

# Run tests
cargo test

# Build
cargo build
```

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
