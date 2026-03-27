# orb8 Development Roadmap

## Current Status: Phase 4 (Deploy and Run Anywhere)

orb8 follows an incremental development approach, delivering user value at each phase. Phases are reordered from the original roadmap to prioritize **usability and value delivery**. Each phase is independently shippable.

## Design Principles

**Data Collection**:
- Ring buffers over perf buffers (lower CPU overhead at high throughput)
- In-kernel filtering to minimize userspace load
- BPF maps for flow deduplication before userspace

**Container Identification**:
- IP-based enrichment as primary path (TC classifiers cannot use `bpf_get_current_cgroup_id()`)
- cgroup v2 ID mapping as fallback for tracepoint-attached probes
- Async K8s API watch for metadata without blocking probes

**Cluster Aggregation**:
- Relay pattern: fan-out queries to per-node agents, merge results
- gRPC streaming for real-time event delivery
- Connection pooling with backpressure handling

---

## Phase 3: Network MVP (v0.0.3) - COMPLETE

**Goal**: Single-node K8s network visibility via CLI

**Completed**:
- [x] eBPF TC probes (ingress/egress)
- [x] IPv4 5-tuple extraction
- [x] IP-based pod enrichment
- [x] gRPC API (QueryFlows, StreamEvents, GetStatus)
- [x] CLI commands (status, flows, trace network)
- [x] Smart interface discovery (eth0, cni0, docker0, br-*)
- [x] Ring buffer drop counter surfaced in GetStatus
- [x] Compile-time little-endian assertion
- [x] Self-traffic filter (agent gRPC port excluded from captures)

---

## Phase 3.5: Structural Cleanup (v0.0.6) - COMPLETE

**Goal**: Fix what is broken, remove what is dead. Zero new features.

**User value**: `orb8 flows` returns correct pod names instead of `"unknown/cgroup-0"`.

**Problem**: The aggregator enriches events via `pod_cache.get(event.cgroup_id)`, but TC classifiers always produce `cgroup_id=0`. Every aggregated flow is attributed to `"unknown/cgroup-0"`. Meanwhile `main.rs` does correct IP-based enrichment for gRPC streams. So `orb8 flows` returns garbage while `orb8 trace network` works fine.

**Deliverables**:
- [x] Fix double enrichment bug: change `aggregator.process_event()` to accept pre-resolved `(namespace, pod_name)` from the caller
- [x] Delete entire `src/` directory (18 dead stub files)
- [x] Convert root `Cargo.toml` to virtual workspace (remove `[package]`, dead deps, `[[bin]]`)
- [x] Delete unused `EnrichedEvent` from `pod_cache.rs` and dead `events_dropped` field from aggregator
- [x] Delete `tests/integration_test.rs` (tests dead code)
- [x] Add unit tests for the aggregator
- [x] Consolidate `parse_ipv4` / `parse_ipv4_le` into one function in `net.rs`
- [x] Move `format_*` functions from `aggregator.rs` to `net.rs`

- [x] Ungate `aggregator` and `pod_cache` from `cfg(linux)` (pure data structures, enables macOS testing)
- [x] Dockerfile with multi-stage (CI) and local (fast) build targets
- [x] `deploy/daemonset.yaml` with RBAC (ServiceAccount, ClusterRole for pod list/watch)
- [x] `deploy/kind-config.yaml` for 2-node test cluster
- [x] `deploy/e2e-test-pods.yaml` (echo-server + Service + traffic-gen with nodeSelector)
- [x] `scripts/smoke-test.sh` — probe loading + traffic capture (no k8s)
- [x] `scripts/e2e-test.sh` — full kind cluster test across all network modes
- [x] `make smoke-test`, `make e2e-test`, `make docker-build` targets

**E2E test coverage** (9 assertions across 3 network modes):
- hostNetwork pods — agent's own traffic, pods tracked > 0
- Regular pods — cross-node pod-to-pod by IP, both agents show correct pod names
- Service ClusterIP — DNAT resolved to pod IP before TC hook, ClusterIP absent from flows

**Known limitation** (documented, not a test gap):
- Same-node pod-to-pod traffic stays on veth pairs and is invisible to eth0-only probes

**Acceptance criteria**:
- `orb8 flows` and `orb8 trace network` show identical pod attributions
- `cargo test` passes (18 tests), `cargo clippy --workspace -- -D warnings` clean
- No `src/` directory exists
- `make smoke-test` passes (6/6)
- `make e2e-test` passes (9/9)

---

## Phase 4: Deploy and Run Anywhere (v0.1.0)

**Goal**: One-command deployment to any Kubernetes cluster.

**User value**: `kubectl apply -k deploy/` gives network visibility on every node. First "installable product" milestone.

**Already delivered in Phase 3.5**:
- [x] `Dockerfile` with multi-stage (CI) and local (fast) build targets
- [x] `deploy/daemonset.yaml` with RBAC
- [x] `make docker-build`, `make e2e-test`, `make smoke-test` targets

**Remaining deliverables**:
- [ ] `deploy/kustomization.yaml` -- base overlay for production deployment
- [ ] `deploy/kubernetes/` -- namespace, headless Service, ConfigMap
- [ ] `orb8-agent/src/config.rs` -- `AgentConfig::from_env()` with defaults matching current hardcoded values
- [ ] GitHub Actions job to build + push container to ghcr.io on tagged releases
- [ ] Quick-start README section

**Acceptance criteria**:
- `kubectl apply -k deploy/` deploys agents that start, attach probes, capture traffic
- `orb8 --agent <node-ip>:9090 status` returns healthy from outside the cluster
- CI runs `make e2e-test` on every PR

---

## Phase 5: Prometheus Metrics (v0.2.0)

**Goal**: Prometheus-scrapable metrics from every agent.

**User value**: Grafana dashboards showing pod traffic, top talkers, agent health -- works without the CLI.

**Deliverables**:
- [ ] HTTP `/metrics` endpoint on port 9091 (using `prometheus` crate + `hyper`)
- [ ] Metrics: `orb8_network_bytes_total{namespace,pod,direction,protocol}`, `orb8_network_packets_total`, `orb8_active_flows`, `orb8_events_processed_total`, `orb8_events_dropped_total`, `orb8_pods_tracked`, `orb8_agent_uptime_seconds`
- [ ] `deploy/servicemonitor.yaml` for Prometheus Operator
- [ ] `deploy/grafana-dashboard.json` -- pre-built dashboard
- [ ] DaemonSet annotations for Prometheus scraping

**Acceptance criteria**:
- `curl <agent-ip>:9091/metrics` returns valid Prometheus exposition format
- Metrics are consistent with gRPC `GetStatus` values
- Grafana dashboard imports and shows live data

---

## Phase 6: Event Pipeline Architecture (v0.3.0)

**Goal**: Decompose monolithic main.rs into composable pipeline. Add JSON output.

**User value**: `orb8 trace network --output json` for scripting/piping. Foundation for all future sinks.

**Deliverables**:
- [ ] `orb8-util/` crate with shared formatting/parsing utilities
- [ ] `pipeline/` module with `EventSource`, `EventEnricher`, `EventProcessor`, `EventExporter` traits
- [ ] `export/grpc.rs` and `export/log.rs` sink implementations
- [ ] `filter.rs` with `EventFilter` trait and `SelfTrafficFilter`
- [ ] `event.rs` with canonical `EnrichedEvent` type
- [ ] `shutdown.rs` with `CancellationToken` + `JoinSet` coordinator
- [ ] Refactored `main.rs` under 100 lines
- [ ] CLI `--output json|table` flag
- [ ] Proto split: `common.proto`, `agent.proto`, `server.proto` (under `orb8/v1/`)

**Acceptance criteria**:
- All existing functionality unchanged (gRPC, CLI, E2E tests)
- `main.rs` under 100 lines
- Adding a new export sink = implementing one trait, no main.rs changes
- `orb8 trace network --output json | jq .pod_name` works

---

## Phase 7: Cluster Mode (v0.4.0)

**Goal**: Single endpoint to query all nodes.

**User value**: `orb8 flows --namespace ml-training` returns cluster-wide data from one command.

**Deliverables**:
- [ ] `orb8-server` implementation: agent discovery via K8s API, fan-out/fan-in gRPC
- [ ] Server Deployment + Service manifests
- [ ] CLI `--mode cluster` flag / auto-detection
- [ ] Partial failure handling (return results from healthy agents, warn about unreachable ones)

> **Design note**: The server's fan-out/aggregation protocol needs detailed
> design before implementation. The current per-agent API (QueryFlows,
> StreamEvents, GetStatus) must compose cleanly into multi-node responses.
> Consider: partial failure handling, result ordering, streaming back-pressure,
> and how AgentStatus rolls up into ClusterStatus.

---

## Phase 8: Syscall Monitoring (v0.5.0)

**Goal**: Per-pod syscall visibility.

**User value**: `orb8 trace syscall --pod my-app` -- security visibility into what syscalls pods are making.

**Rationale for ordering before GPU**: Uses existing eBPF infra (tracepoints), broader audience than GPU users, adds security value. `cgroup_id` IS available in tracepoint context (unlike TC), validating the cgroup enrichment path.

**Deliverables**:
- [ ] `orb8-probes/src/syscall_probe.rs` (tracepoint/raw_syscalls/sys_enter)
- [ ] `SyscallEvent` in `orb8-common`, new ring buffer
- [ ] Sampling for hot syscalls, always-capture for dangerous ones (ptrace, mount)
- [ ] CLI `orb8 trace syscall` + proto extensions
- [ ] Prometheus syscall metrics

---

## Phase 9: GPU Telemetry (v0.6.0)

**Goal**: Per-pod GPU utilization for ML workloads.

**User value**: `orb8 trace gpu --namespace ml-training` shows GPU utilization per pod.

**Deliverables**:
- [ ] NVML polling for utilization, memory, temperature
- [ ] Pod-to-GPU mapping via kubelet pod-resources API
- [ ] CLI `orb8 trace gpu` + Prometheus GPU metrics
- [ ] Grafana GPU dashboard panel

---

## Phase 10: Developer Experience (v0.7.0+)

**Goal**: Make orb8 the go-to tool for engineers debugging Kubernetes networking.

**Deliverables**:
- [ ] **TUI Dashboard** (ratatui) — real-time terminal UI showing top flows, pod traffic, agent health
- [ ] **Standalone Mode** — on-demand tracing without deploying a DaemonSet (`orb8 trace --standalone`)
- [ ] **DNS Tracing** — first-class `orb8 trace dns` command with query/response parsing (filter on port 53 of existing network probe)

---

## Deferred to Post-v1.0

| Feature | Reason |
|---------|--------|
| Historical Storage (TimescaleDB) | Out of scope. Prometheus handles retention; Thanos/Mimir for long-term. |
| YAML config file system | Env vars sufficient until cluster mode. |
| Multi-cluster support | Get single-cluster right first. |

## Deferred to Post-v1.0

- IPv6 support (NetworkFlowEvent struct migration)
- TCP connection state tracking (SYN/FIN/RST)
- eBPF GPU probes (closed-source driver, research only)
- OpenTelemetry export sink
- CRD-based policy/filter configuration

---

## Known Limitations

### Network visibility scope
TC probes attach to the node's primary interface (eth0). This sees cross-node pod
traffic and hostNetwork traffic, but **not same-node pod-to-pod traffic** which
stays on veth pairs/bridges and never reaches eth0. Attaching to container bridge
interfaces (cni0) or per-pod veth pairs would capture this traffic, but requires
probe attachment in each pod's network namespace or on a shared bridge — which
not all CNIs provide (kindnet uses direct routing with no bridge).

### hostNetwork pod attribution
Pods with `hostNetwork: true` share the node's IP address. All traffic from the
node IP is attributed to whichever hostNetwork pod was last indexed in the pod
cache. Host processes (kubelet, sshd) also share this IP and will be
mis-attributed. A hybrid approach (socket-level eBPF probe mapping 5-tuples to
cgroup IDs, shared with TC via an eBPF map) would solve this but adds complexity.

### Service ClusterIP transparency
kube-proxy applies DNAT before packets reach the TC hook on eth0, so flows show
the actual backend pod IP, not the Service ClusterIP. This means orb8 correctly
enriches the traffic but cannot tell you which Service was originally addressed.
This is validated by the e2e test (ClusterIP is absent from flows).

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

## Verification Plan

### Per-Phase Testing

| Phase | Test Command | What It Validates |
|-------|-------------|-------------------|
| 3.5 | `cargo test && cargo clippy --workspace -- -D warnings` | Bug fix, dead code removal, no regressions |
| 3.5 | `make e2e-test` (Lima VM) | `orb8 flows` returns correct pod names |
| 4 | `make docker-build && make e2e-test` | Container builds, DaemonSet deploys, captures traffic |
| 5 | `curl localhost:9091/metrics` + Grafana import | Metrics valid, dashboard works |
| 6 | `orb8 trace network --output json \| jq .` | JSON output valid |
| 6 | `cargo test -p orb8-agent` | Pipeline unit tests pass |
| 7 | `orb8 --mode cluster flows` on multi-node kind | Cross-node query works |
| 8 | `orb8 trace syscall --pod <test-pod>` | Syscalls captured with correct attribution |

### Continuous Validation

After every phase: `cargo fmt && cargo clippy --workspace -- -D warnings && cargo test && make e2e-test`

---

## Prerequisites

All users need:
- Linux kernel 5.8+ with BTF enabled
- Kubernetes 1.20+ with containerd
- Root/privileged access for eBPF loading

Future compatibility improvements:
- BTF fallback for older kernels
- CRI-O and Docker runtime support
