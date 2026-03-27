# orb8

> **eBPF-powered network observability for Kubernetes, built for AI/ML clusters**

[![CI](https://github.com/Ignoramuss/orb8/actions/workflows/ci.yml/badge.svg)](https://github.com/Ignoramuss/orb8/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.94%2B-orange.svg)](https://www.rust-lang.org/)
[![crates.io](https://img.shields.io/crates/v/orb8.svg)](https://crates.io/crates/orb8)

| Crate | crates.io | docs.rs |
|-------|-----------|---------|
| `orb8` | [![crates.io](https://img.shields.io/crates/v/orb8.svg)](https://crates.io/crates/orb8) | [![docs.rs](https://docs.rs/orb8/badge.svg)](https://docs.rs/orb8) |
| `orb8-common` | [![crates.io](https://img.shields.io/crates/v/orb8-common.svg)](https://crates.io/crates/orb8-common) | [![docs.rs](https://docs.rs/orb8-common/badge.svg)](https://docs.rs/orb8-common) |
| `orb8-agent` | [![crates.io](https://img.shields.io/crates/v/orb8-agent.svg)](https://crates.io/crates/orb8-agent) | [![docs.rs](https://docs.rs/orb8-agent/badge.svg)](https://docs.rs/orb8-agent) |
| `orb8-cli` | [![crates.io](https://img.shields.io/crates/v/orb8-cli.svg)](https://crates.io/crates/orb8-cli) | [![docs.rs](https://docs.rs/orb8-cli/badge.svg)](https://docs.rs/orb8-cli) |

orb8 is a lightweight observability toolkit that uses eBPF to capture network flows inside Kubernetes clusters with zero application changes. It maps every packet to the pod that sent or received it, giving you real-time visibility into how your workloads communicate.

Built entirely in Rust using the [aya](https://github.com/aya-rs/aya) eBPF framework. Designed for production AI/ML clusters where overhead matters and GPU telemetry is on the roadmap.

## What works today (v0.0.6)

- **Network flow capture** — eBPF TC classifiers on ingress/egress, IPv4 5-tuple extraction (TCP/UDP/ICMP)
- **Pod enrichment** — Maps packet IPs to Kubernetes pod names via the K8s API. Works for regular pods, cross-node traffic, and Service ClusterIP (DNAT-resolved)
- **gRPC API** — QueryFlows (aggregated), StreamEvents (real-time), GetStatus
- **CLI** — `orb8 status`, `orb8 flows`, `orb8 trace network` with namespace/pod filtering and duration control
- **DaemonSet deployment** — Dockerfile, RBAC, capabilities-based security (not privileged)
- **Tested** — Smoke test (probe loading) + e2e test (3 network modes, 9 assertions on a kind cluster)

## Quick start

### 1. Install

```bash
cargo install orb8
```

### 2. Deploy to Kubernetes

```bash
kubectl apply -f https://raw.githubusercontent.com/Ignoramuss/orb8/main/deploy/daemonset.yaml
```

The DaemonSet includes a ServiceAccount with ClusterRole for pod list/watch. The agent runs with specific Linux capabilities (`BPF`, `NET_ADMIN`, `SYS_ADMIN`, `PERFMON`, `SYS_RESOURCE`) — not as a privileged container.

### 3. Observe

```bash
# Check agent health
orb8 --agent <node-ip>:9090 status

# Stream live network events
orb8 --agent <node-ip>:9090 trace network --namespace default

# Query aggregated flows
orb8 --agent <node-ip>:9090 flows --limit 10
```

Example output from a real cluster:

```
NAMESPACE/POD        PROTOCOL                       SOURCE           DESTINATION      DIR     BYTES  PACKETS
--------------------------------------------------------------------------------------------------------------
default/traffic-gen  TCP                     10.244.0.5:80      10.244.1.2:42270  ingress     1.4KB        5
default/traffic-gen  TCP                  10.244.1.2:42270         10.244.0.5:80   egress      544B        7
default/traffic-gen  UDP                     10.244.0.3:53      10.244.1.2:48459  ingress      426B        2
```

## Installation

### From crates.io

```bash
# Install the CLI (produces the `orb8` binary)
cargo install orb8
```

### From source

```bash
git clone https://github.com/Ignoramuss/orb8.git
cd orb8
cargo build --release

# The CLI binary is at target/release/orb8
# The agent binary is at target/release/orb8-agent (requires Linux)
```

### Kubernetes deployment

The agent requires Linux kernel 5.8+ with BTF enabled. Check with:

```bash
# Verify BTF is available on your nodes
ls /sys/kernel/btf/vmlinux
```

Deploy:

```bash
kubectl apply -f deploy/daemonset.yaml
```

This creates:
- A `ServiceAccount` with a `ClusterRole` granting pod list/watch
- A `DaemonSet` running the agent on every node with `hostNetwork: true`
- Volume mounts for `/sys`, `/sys/kernel/debug`, `/sys/fs/cgroup`

Verify:

```bash
# Check all agents are running
kubectl get ds orb8-agent

# Check agent logs for probe attachment
kubectl logs -l app=orb8-agent | grep "Attached.*probe"

# Port-forward and query
kubectl port-forward ds/orb8-agent 19090:9090
orb8 --agent localhost:19090 status
```

## Usage

### Check agent status

```bash
orb8 --agent localhost:9090 status
```

```
Agent Status
----------------------------------------
Node:             worker-1
Version:          0.0.6
Health:           OK
Uptime:           3600s
Events Processed: 48201
Events Dropped:   0
Pods Tracked:     12
Active Flows:     34
```

### Query aggregated flows

```bash
# All flows, sorted by bytes
orb8 --agent localhost:9090 flows

# Filter by namespace
orb8 --agent localhost:9090 flows --namespace kube-system

# Filter by pod name
orb8 --agent localhost:9090 flows --pod coredns --limit 50
```

### Stream live events

```bash
# Stream all events
orb8 --agent localhost:9090 trace network

# Filter by namespace, stop after 30 seconds
orb8 --agent localhost:9090 trace network --namespace default --duration 30s
```

## Architecture

```
┌──────────────────────────────────────────────────────┐
│                   Kubernetes Node                     │
│                                                       │
│  ┌─────────────────────────────────────────────────┐ │
│  │  Kernel Space                                    │ │
│  │                                                  │ │
│  │  TC Classifier (eBPF)                            │ │
│  │    ingress + egress on network interfaces        │ │
│  │    extracts: IPs, ports, protocol, direction     │ │
│  │    writes to: ring buffer (1MB, ~32K events)     │ │
│  └──────────────────────┬──────────────────────────┘ │
│                         │ ring buffer                 │
│  ┌──────────────────────▼──────────────────────────┐ │
│  │  User Space (orb8-agent)                         │ │
│  │                                                  │ │
│  │  Poll events → Filter self-traffic               │ │
│  │  → Look up pod by IP (K8s API watcher)           │ │
│  │  → Aggregate into flows                          │ │
│  │  → Serve via gRPC (:9090)                        │ │
│  └──────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────┘
         │
         │ gRPC
         ▼
   ┌────────────┐
   │  orb8 CLI  │
   └────────────┘
```

eBPF probes are written in Rust (`#![no_std]`) and embedded in the agent binary at compile time. No sidecar, no kernel module, no application changes required.

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for the full technical design.

## Comparison

### What works today

| Feature | orb8 | Pixie | Cilium Hubble | Tetragon |
|---------|------|-------|---------------|----------|
| Network flow capture | Yes | Yes | Yes | Yes |
| Pod enrichment | Yes (IP-based) | Yes | Yes | Yes |
| eBPF-based | Yes | Yes | Yes (via Cilium) | Yes |
| K8s native | Yes | Yes | Yes | Yes |
| Pure Rust | Yes | No (Go/C++) | No (Go/C) | No (Go/C) |
| Overhead target | <1% CPU | ~2-5% | <1% | <1% |

### What's on the roadmap

| Feature | orb8 | Pixie | Cilium Hubble | Tetragon |
|---------|------|-------|---------------|----------|
| GPU telemetry | Phase 9 | No | No | No |
| Syscall monitoring | Phase 8 | Partial | No | Yes |
| Prometheus metrics | Phase 5 | Yes | Yes | Yes |
| Cluster-wide queries | Phase 7 | Yes | Yes | Partial |
| TUI dashboard | Phase 10 | Yes | No | No |
| Standalone tracing | Phase 10 | No | No | Partial |
| DNS query tracing | Phase 10 | Yes | Yes | No |

orb8's differentiator: purpose-built for AI/ML clusters with GPU telemetry on the roadmap. Most observability tools ignore GPU workloads entirely.

## Known limitations

These are documented and tracked:

- **Same-node pod traffic** is invisible when probes attach to eth0 only ([#36](https://github.com/Ignoramuss/orb8/issues/36))
- **hostNetwork pods** share the node IP, causing traffic mis-attribution ([#37](https://github.com/Ignoramuss/orb8/issues/37))
- **Service ClusterIP** is resolved by kube-proxy before the TC hook, so flows show the backend pod IP, not the Service address ([#38](https://github.com/Ignoramuss/orb8/issues/38))
- **IPv4 only** — IPv6 support is deferred to post-v1.0
- **Single-agent queries** — no cluster-wide aggregation yet (Phase 7)

## Roadmap

Development follows a phase-based approach. Each phase is independently shippable.

| Phase | Version | Status | What it delivers |
|-------|---------|--------|-----------------|
| 3 | v0.0.3 | Done | Network flow capture, gRPC API, CLI |
| 3.5 | v0.0.6 | Done | Enrichment fix, test infrastructure, DaemonSet |
| 4 | v0.1.0 | Next | Kustomize overlays, CI image builds, env config |
| 5 | v0.2.0 | Planned | Prometheus `/metrics` endpoint, Grafana dashboard |
| 6 | v0.3.0 | Planned | Event pipeline refactor, JSON output, `--output json` |
| 7 | v0.4.0 | Planned | Cluster mode — `orb8-server` for multi-node queries |
| 8 | v0.5.0 | Planned | Syscall monitoring via tracepoints |
| 9 | v0.6.0 | Planned | GPU telemetry via NVML (per-pod GPU utilization) |
| 10 | v0.7.0+ | Planned | TUI dashboard, standalone mode, DNS tracing |

See [docs/ROADMAP.md](docs/ROADMAP.md) for detailed deliverables per phase.

## Testing

### Unit tests

```bash
cargo test              # 18 tests (runs on macOS and Linux)
```

### Smoke test (no Kubernetes required)

```bash
make smoke-test         # Loads eBPF probes, captures traffic, queries via CLI
```

Runs the agent directly with sudo. Verifies probes load, attach, and capture real packets. All traffic shows as "external/unknown" (expected — no pod watcher without K8s). 6 assertions.

### E2E test (full Kubernetes pipeline)

```bash
make e2e-test           # Creates kind cluster, deploys DaemonSet, verifies enrichment
```

Builds a Docker image, deploys to a 2-node kind cluster, and tests three network modes:

1. **hostNetwork pods** — agent's own K8s API traffic
2. **Regular pods** — cross-node pod-to-pod traffic by IP (traffic-gen on worker, echo-server on control-plane)
3. **Service ClusterIP** — verifies DNAT resolves to pod IP before the TC hook

Queries both the worker and control-plane agents to verify enrichment from both sides. 9 assertions.

## Development

### Prerequisites

- **Rust** 1.94+ stable + nightly with `rust-src`
- **bpf-linker**: `cargo install bpf-linker`
- **Linux kernel** 5.8+ with BTF (for running eBPF; 5.15+ recommended)
- **macOS**: Lima + QEMU for a Linux VM (eBPF requires a real kernel)

### macOS setup

```bash
make dev              # Create and start Lima VM (first run: ~5-10 min)
make shell            # Enter the VM
make verify-setup     # Verify Rust, bpf-linker, BTF are available
```

The project directory is auto-mounted at the same path inside the VM. All `make` commands (build, test, smoke-test, e2e-test) automatically delegate to the VM on macOS.

### Linux setup

Native development — no VM needed. Just ensure kernel 5.8+ with BTF:

```bash
ls /sys/kernel/btf/vmlinux    # Should exist
make verify-setup              # Checks all dependencies
```

### Build commands

| Command | Description |
|---------|-------------|
| `make magic` | Build, test, install (VM on macOS, native on Linux) |
| `make build` | Build all crates (release) |
| `make test` | Run cargo test in Lima VM |
| `make smoke-test` | Probe loading + traffic capture test |
| `make e2e-test` | Full kind cluster e2e test |
| `make docker-build` | Build `orb8-agent:test` Docker image |
| `make run-agent` | Build and run agent with sudo |
| `make fmt` | Format all code |
| `make clippy` | Run linter (`-D warnings`) |
| `make dev` | Setup Lima VM (macOS) |
| `make shell` | Enter Lima VM (macOS) |
| `make verify-setup` | Check development environment |

### Code quality

All PRs must pass:

```bash
cargo fmt --check
cargo clippy --workspace -- -D warnings
cargo test
```

E2E tests run in the Lima VM and are required before releases.

## Contributing

We welcome contributions. Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

Before submitting a PR:
1. Check [docs/ROADMAP.md](docs/ROADMAP.md) for phase dependencies
2. Run `cargo fmt && cargo clippy --workspace -- -D warnings && cargo test`
3. Add tests for new functionality
4. Update documentation if you change behavior

## License

Apache License 2.0 — see [LICENSE](LICENSE) for details.

## Acknowledgments

Built with:
- [aya](https://github.com/aya-rs/aya) — Rust eBPF library
- [kube-rs](https://github.com/kube-rs/kube) — Kubernetes API client
- [tonic](https://github.com/hyperium/tonic) — gRPC framework
- [Lima](https://github.com/lima-vm/lima) — Linux VMs on macOS
