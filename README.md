# orb8

> **eBPF-powered observability toolkit for Kubernetes with first-class GPU telemetry**

[![License](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org/)
[![Kubernetes](https://img.shields.io/badge/kubernetes-1.25%2B-326ce5.svg)](https://kubernetes.io/)
[![crates.io](https://img.shields.io/crates/v/orb8.svg)](https://crates.io/crates/orb8)
[![docs.rs](https://docs.rs/orb8/badge.svg)](https://docs.rs/orb8)

### Crate Documentation

| Crate | crates.io | docs.rs |
|-------|-----------|---------|
| `orb8` | [crates.io/crates/orb8](https://crates.io/crates/orb8) | [docs.rs/orb8](https://docs.rs/orb8) |
| `orb8-common` | [crates.io/crates/orb8-common](https://crates.io/crates/orb8-common) | [docs.rs/orb8-common](https://docs.rs/orb8-common) |
| `orb8-agent` | [crates.io/crates/orb8-agent](https://crates.io/crates/orb8-agent) | [docs.rs/orb8-agent](https://docs.rs/orb8-agent) |
| `orb8-cli` | [crates.io/crates/orb8-cli](https://crates.io/crates/orb8-cli) | [docs.rs/orb8-cli](https://docs.rs/orb8-cli) |

**orb8** (_orbit_) is a high-performance observability toolkit built with Rust and eBPF, designed specifically for Kubernetes clusters running AI/ML workloads. It provides deep, low-level visibility into container networking, system calls, resource utilization, and GPU performance with minimal overhead.

> The name "orb8" represents orbiting around your cluster infrastructure, continuously observing and monitoring from all angles.

## Why orb8?

Existing Kubernetes observability tools either focus on high-level metrics or security-specific use cases. orb8 fills the gap by providing:

- **AI Cluster Optimized**: Built for large-scale GPU/TPU/Trainium workloads (GPU telemetry planned for v0.4.0)
- **eBPF Performance**: Sub-1% CPU overhead with zero-copy packet inspection
- **Kubernetes Native**: Auto-discovery of pods, namespaces, and nodes via Kubernetes API
- **Minimal Overhead**: Designed for production environments running massive compute clusters

## Features

### Current (v0.0.2)

- **Network Flow Tracking**: TCP/UDP/ICMP flow monitoring with 5-tuple extraction
- **gRPC API**: Agent exposes QueryFlows, StreamEvents, GetStatus on port 9090
- **K8s Pod Enrichment**: Maps network events to pods via Kubernetes API
- **Flow Aggregation**: 5-tuple flow aggregation with 30-second expiration
- **Ring Buffer**: Efficient kernel-to-userspace event communication

### Planned

- **System Call Monitoring**: Security anomaly detection via syscall pattern analysis (v0.3.0 - Phase 6)
- **GPU Telemetry** (v0.4.0 - Phase 7):
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

**For Building:**
- Rust 1.75+ (stable)
- Rust nightly toolchain with `rust-src` component
- `bpf-linker` (install via `cargo install bpf-linker`)

**For Running eBPF Programs:**
- Linux kernel 5.8+ with BTF support
- Kubernetes cluster 1.25+ (for production deployment)

**Platform-Specific:**
- **macOS**: Lima + QEMU (for VM-based eBPF testing), 20GB free disk space
- **Linux**: Native support, no VM needed
- **Windows**: Use WSL2, follow Linux instructions

**Future Features:**
- CUDA 11.0+ (for GPU telemetry, v0.8.0+)

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

**Note**: orb8 v0.0.2 includes working network flow capture and gRPC API. The agent can be tested in the Lima VM.

### Testing the Agent (v0.0.2)

```bash
# Start the agent in Lima VM
make run-agent

# In another terminal, query agent status
grpcurl -plaintext -proto orb8-proto/proto/orb8.proto \
  localhost:9090 orb8.v1.OrbitAgentService/GetStatus

# Generate network traffic
ping -c 5 127.0.0.1

# Query captured flows
grpcurl -plaintext -proto orb8-proto/proto/orb8.proto \
  localhost:9090 orb8.v1.OrbitAgentService/QueryFlows
```

### Network Monitoring (Coming in v0.1.0)

```bash
# Monitor network flows for all pods in a namespace
orb8 trace network --namespace default

# Track DNS queries across the cluster
orb8 trace dns --all-namespaces
```

### System Call Analysis (Coming in v0.3.0)

```bash
# Monitor syscalls for security anomalies
orb8 trace syscall --pod suspicious-pod-456
```

### GPU Telemetry (Planned for v0.4.0)

```bash
# Monitor GPU utilization for AI workloads
orb8 trace gpu --namespace ml-training
```

## Testing

### Testing the eBPF Agent (Phase 1.2+)

Phase 1.2 implements the "Hello World" eBPF probe with full probe loading.

**What works:**
- eBPF probe compiles to bytecode
- Agent loads probe into kernel
- Probe attaches to network interfaces (loopback)
- eBPF logs are captured in userspace

**Testing on macOS (via Lima VM):**
```bash
# Build the agent (compiles eBPF probes automatically)
make build-agent

# Run the agent (requires sudo, use Ctrl+C to stop)
make run-agent
```

**Testing on Linux (native):**
```bash
# Build the agent
cargo build -p orb8-agent

# Run the agent (requires root for eBPF)
sudo ./target/debug/orb8-agent
```

**Verifying it works:**
1. Start the agent with `make run-agent`
2. In another terminal (inside VM if on macOS): `ping 127.0.0.1`
3. You should see logs like:
   ```
   [INFO  network_probe] Hello from eBPF! packet_len=98
   ```
4. Press Ctrl+C to stop the agent

### Linux Testing (Recommended: `make magic-local`)

```bash
# Verify your environment
make verify-setup

# Build, test, install (native, no VM)
make magic-local
```

**Why `magic-local`?** Direct and explicit. On Linux, `make magic` just redirects to `magic-local` anyway.

### macOS Testing

**For Phase 1.1 (build infrastructure only):**
```bash
# Verify your environment
make verify-setup

# Quick testing without VM (recommended for Phase 1.1)
make magic-local
```

**For Phase 1.2+ (when testing actual eBPF execution):**
```bash
# Full testing with VM (can load eBPF into kernel)
make magic
```

**What's the difference?**
- `make magic-local`: Builds on macOS, compiles eBPF to bytecode (fast, no VM)
- `make magic`: Uses Lima VM, can actually load eBPF programs (required for Phase 1.2+)

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

**Current Status**: Phase 2 Complete, Phase 3 In Progress

Completed:
- Phase 0: Foundation & Monorepo Setup
- Phase 1: eBPF Infrastructure (probe loading, ring buffer)
- Phase 2: Container Identification (K8s pod enrichment, gRPC API)

In Progress:
- Phase 3: Network Tracing MVP (CLI commands, public release)

Planned:
- v0.1.0: Network Tracing MVP (Phase 3)
- v0.2.0: Cluster Mode with Metrics (Phase 4-5)
- v0.3.0: Syscall Monitoring (Phase 6)
- v0.4.0: GPU Telemetry (Phase 7)
- v1.0.0: Production Ready (Phase 8)

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

**Step 1: Verify your environment**
```bash
make verify-setup
```

**Step 2: Build, test, and install**

**Linux:**
```bash
make magic-local    # Native build, test, install
cargo build         # Or build manually
cargo test          # Run tests
```

**macOS (Phase 1.1 - build infrastructure):**
```bash
make magic-local    # Fast local testing
# eBPF compiles to bytecode but doesn't load (no kernel)
```

**macOS (Phase 1.2+ - actual eBPF execution):**
```bash
make magic          # Full VM-based testing
make shell          # Enter VM
orb8 --help
```

**Expected output for Phase 1.1:**
Both platforms will show:
```
warning: target filter `bins` specified, but no targets matched
Finished `release` profile [optimized]
```
This is expected - probe binaries come in Phase 1.2.

### Manual Development Setup

**Linux:**
```bash
# Verify environment
make verify-setup

# Build and test
cargo build
cargo test

# For Phase 1.1 specifically
cargo build -p orb8-probes          # eBPF build infrastructure
cargo clippy -p orb8-probes -- -D warnings
```

**macOS:**
```bash
# Quick setup (no VM)
make verify-setup
cargo build -p orb8-probes

# Full setup (with VM for eBPF execution)
make dev            # Creates VM (5-10 min first time)
make shell          # Enter VM
cargo build
cargo test
```

See [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md) for detailed setup instructions and troubleshooting.

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

**Note**: This project is in early development. GPU telemetry features (Phase 7) will require specific hardware and driver configurations when implemented.
