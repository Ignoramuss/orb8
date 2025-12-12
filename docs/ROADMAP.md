# orb8 Development Roadmap

## Current Status: Phase 3 (Network MVP)

orb8 follows an incremental development approach, delivering user value at each phase.

## Design Principles

**Data Collection**:
- Ring buffers over perf buffers (lower CPU overhead at high throughput)
- In-kernel filtering to minimize userspace load
- BPF maps for flow deduplication before userspace

**Container Identification**:
- cgroup v2 ID mapping for process-to-container correlation
- IP-based enrichment fallback when cgroup ID unavailable (TC classifiers)
- Async K8s API watch for metadata without blocking probes

**Cluster Aggregation**:
- Relay pattern: fan-out queries to per-node agents, merge results
- gRPC streaming for real-time event delivery
- Connection pooling with backpressure handling

---

## Phase 3: Network MVP (v0.0.3) - CURRENT

**Goal**: Single-node K8s network visibility via CLI

**Status**: In Progress

**Completed**:
- [x] eBPF TC probes (ingress/egress)
- [x] IPv4 5-tuple extraction
- [x] IP-based pod enrichment
- [x] gRPC API (QueryFlows, StreamEvents, GetStatus)
- [x] CLI commands (status, flows, trace network)
- [x] Smart interface discovery (eth0, cni0, docker0, br-*)

**Remaining**:
- [ ] GitHub release v0.0.3
- [ ] Quick-start documentation

---

## Phase 4: Production Deployment

### Phase 4a: Container Images
- Create Dockerfile for orb8-agent
- Set up GitHub Actions for ghcr.io publishing
- Multi-arch support (amd64, arm64)

### Phase 4b: Kubernetes Manifests
- DaemonSet with proper RBAC
- ConfigMap for configuration
- Headless service for discovery
- Kustomize base

---

## Phase 5: Prometheus Metrics

- Implement prom_exporter.rs
- Export: events_processed, events_dropped, packets, bytes
- Export: active_flows, pods_tracked
- ServiceMonitor for Prometheus Operator
- Example Grafana dashboard

---

## Phase 6: Cluster Mode (orb8-server)

- Agent discovery via K8s Endpoints API
- Query routing with fan-out/fan-in
- Result aggregation across nodes
- CLI --mode cluster flag

---

## Phase 7: Advanced Network Features

- IPv6 support
- DNS request/response parsing
- TCP connection state tracking (SYN/FIN/RST)
- TUI dashboard (ratatui)

---

## Phase 8: Syscall Monitoring

- syscall_probe.rs (tracepoint-based)
- `orb8 trace syscall` command
- Filtering by syscall type

---

## Phase 9: GPU Telemetry

- DCGM integration
- Pod-to-GPU mapping via kubelet pod-resources API
- Per-pod GPU metrics (utilization, memory, power)

---

## Prerequisites

All users need:
- Linux kernel 5.8+ with BTF enabled
- Kubernetes 1.20+ with containerd
- Root/privileged access for eBPF loading

Future compatibility improvements:
- BTF fallback for older kernels
- CRI-O and Docker runtime support
- Non-K8s standalone mode
