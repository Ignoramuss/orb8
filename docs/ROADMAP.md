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
- [x] Ring buffer drop counter surfaced in GetStatus
- [x] Compile-time little-endian assertion

**Remaining**:
- [ ] GitHub release v0.0.3
- [ ] Quick-start documentation

---

## Phase 4: Production Deployment

### Phase 4a: Container Images (partially complete)
- [x] Dockerfile for orb8-agent (multi-stage build with embedded probes)
- [ ] GitHub Actions for ghcr.io publishing
- [ ] Multi-arch support (amd64, arm64)

### Phase 4b: Kubernetes Manifests (partially complete)
- [x] DaemonSet with proper RBAC and security capabilities
- [x] E2E test infrastructure (kind cluster, traffic generation, gRPC verification)
- [ ] ConfigMap for configuration
- [ ] Headless service for agent discovery
- [ ] Kustomize base

### Phase 4c: Standalone Mode (optional)
- Skip K8s watcher, use local IP enrichment from `/proc/net`
- Allow agent to run without Kubernetes API access
- Lower barrier for single-node evaluation

---

## Phase 5: Prometheus Metrics

- Implement prom_exporter.rs
- Export: events_processed, events_dropped, packets, bytes
- Export: active_flows, pods_tracked
- ServiceMonitor for Prometheus Operator
- Example Grafana dashboard

> **Pre-requisite**: Before starting Phase 5, finalize the Phase 6 server
> protocol design. Ensure the per-agent gRPC API shape can be composed into
> multi-node responses without breaking changes to proto definitions or CLI.

---

## Phase 6: Cluster Mode (orb8-server)

- Agent discovery via K8s Endpoints API
- Query routing with fan-out/fan-in
- Result aggregation across nodes
- CLI --mode cluster flag

> **Design note**: The server's fan-out/aggregation protocol needs detailed
> design before implementation. The current per-agent API (QueryFlows,
> StreamEvents, GetStatus) must compose cleanly into multi-node responses.
> Consider: partial failure handling, result ordering, streaming back-pressure,
> and how AgentStatus rolls up into ClusterStatus.

---

## Phase 7: GPU Telemetry (lightweight MVP)

- NVML polling for per-GPU utilization, memory, temperature
- Pod-to-GPU mapping via kubelet pod-resources API
- `orb8 trace gpu` command with per-pod GPU metrics
- DCGM integration for advanced metrics (SM occupancy, NVLink)

> **Rationale**: GPU telemetry is orb8's headline differentiator for AI/ML
> workloads. A lightweight NVML-based MVP validates the GPU story early.
> Full DCGM integration can follow as an enhancement.

---

## Phase 8: Advanced Network Features

- IPv6 support (requires `NetworkFlowEvent` struct migration from `u32` to `[u8; 16]`)
- DNS request/response parsing
- TCP connection state tracking (SYN/FIN/RST)
- TUI dashboard (ratatui)

> **IPv6 migration path**: `NetworkFlowEvent` is a `#[repr(C)]` struct shared
> between kernel and userspace. Changing IP fields from `u32` to `[u8; 16]`
> is a breaking change affecting: probes, aggregator, pod_cache, gRPC proto,
> and CLI formatting. Consider a versioned event format or a union type to
> support both IPv4 and IPv6 in a single struct.

---

## Phase 9: Syscall Monitoring

- syscall_probe.rs (tracepoint-based)
- `orb8 trace syscall` command
- Filtering by syscall type

---

## Known Limitations

### Polling model
The main event loop polls the ring buffer at a fixed 100ms interval. Under high
throughput, this adds latency and can cause ring buffer drops. Under zero
traffic, it's wasted CPU. A future improvement will use epoll/async
notification from the ring buffer.

### FlowKey heap allocation
`FlowKey` includes `namespace` and `pod_name` (heap-allocated Strings). At high
packet rates, hashing these strings in the DashMap is measurably more expensive
than a pure integer 5-tuple key. A two-level lookup (integer key -> enriched
metadata) is planned for performance-critical deployments.

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
