# orb8 Development Roadmap

## Current Status: Phase 4 (Production Hardening)

orb8 follows an incremental development approach, delivering user value at each phase. Phases are ordered to prioritize **product value and production readiness** over infrastructure. Each phase is independently shippable.

## Design Principles

**Data Collection**:
- Ring buffers over perf buffers (lower CPU overhead at high throughput)
- In-kernel filtering to minimize userspace load
- Lossy-by-design: accept event loss gracefully, track and expose drops as metrics
- Bounded data structures throughout (no unbounded growth)

**Container Identification**:
- IP-based enrichment as primary path (TC classifiers cannot use `bpf_get_current_cgroup_id()`)
- cgroup v2 ID mapping as fallback for tracepoint-attached probes
- Async K8s API watch for metadata without blocking probes

**Cluster Aggregation** (Hubble relay pattern):
- Three-tier architecture: data plane (per-node agent) -> aggregation (relay) -> presentation (CLI/UI)
- Persistent gRPC connections from relay to agents
- Streaming-first for real-time event delivery
- Partial failure handling: return results from healthy agents, warn about unreachable ones

---

## Completed Phases

### Phase 3: Network MVP (v0.0.3)

**Goal**: Single-node K8s network visibility via CLI

- [x] eBPF TC probes (ingress/egress)
- [x] IPv4 5-tuple extraction
- [x] IP-based pod enrichment
- [x] gRPC API (QueryFlows, StreamEvents, GetStatus)
- [x] CLI commands (status, flows, trace network)
- [x] Smart interface discovery (eth0, cni0, docker0, br-*)
- [x] Ring buffer drop counter surfaced in GetStatus
- [x] Compile-time little-endian assertion
- [x] Self-traffic filter (agent gRPC port excluded from captures)

### Phase 3.5: Structural Cleanup (v0.0.6)

**Goal**: Fix what is broken, remove what is dead. Zero new features.

- [x] Fix double enrichment bug (aggregator accepts pre-resolved pod identity)
- [x] Delete 18 dead stub files in `src/`
- [x] Convert root `Cargo.toml` to virtual workspace
- [x] Add unit tests for the aggregator
- [x] Consolidate `parse_ipv4` / `parse_ipv4_le` into one function
- [x] Ungate `aggregator` and `pod_cache` from `cfg(linux)`
- [x] Dockerfile with multi-stage (CI) and local (fast) build targets
- [x] DaemonSet with RBAC, kind-config, e2e-test-pods
- [x] `make smoke-test`, `make e2e-test`, `make docker-build` targets
- [x] E2E test coverage: 9 assertions across 3 network modes

---

## Phase 4: Production Hardening (v0.1.0) -- IN PROGRESS

**Goal**: Make the agent safe to run in production Kubernetes clusters.

**User value**: Operators can deploy orb8 with confidence that it won't OOM, won't silently fail, and will be restarted by Kubernetes on errors.

**Deliverables**:
- [x] `orb8-agent/src/config.rs` -- `AgentConfig::from_env()` replacing all hardcoded values
- [x] `orb8-agent/src/health.rs` -- `HealthState` with atomic flags and degradation counters
- [x] `orb8-agent/src/health_server.rs` -- HTTP `/healthz` and `/readyz` endpoints for K8s probes
- [x] Bounded flow table with batch eviction (default 100K flows, configurable)
- [x] Bounded pod cache with capacity check (default 10K entries, configurable)
- [x] Graceful shutdown via `CancellationToken` propagated to all spawned tasks
- [x] All spawned tasks tracked via `JoinHandle` with shutdown timeout
- [x] Real health status (computed from component state, not hardcoded `true`)
- [x] Broadcast drop tracking (no more silent `let _ = send()`)
- [x] DaemonSet: resource limits, liveness/readiness probes, terminationGracePeriodSeconds, seccompProfile
- [ ] `deploy/kustomization.yaml` -- base overlay for production deployment
- [ ] Quick-start README section

**Configuration knobs** (all via environment variables with sane defaults):

| Variable | Default | Description |
|----------|---------|-------------|
| `ORB8_GRPC_PORT` | 9090 | gRPC server port |
| `ORB8_HEALTH_PORT` | 9091 | Health HTTP endpoint port |
| `ORB8_MAX_FLOWS` | 100000 | Maximum flow table entries |
| `ORB8_FLOW_TIMEOUT_SECS` | 30 | Flow expiration timeout |
| `ORB8_MAX_POD_CACHE` | 10000 | Maximum pod cache entries |
| `ORB8_BROADCAST_CHANNEL_SIZE` | 1000 | gRPC event broadcast buffer |
| `ORB8_POLL_INTERVAL_MS` | 100 | Ring buffer poll interval |
| `ORB8_MAX_BATCH_SIZE` | 1024 | Max events per poll cycle |
| `ORB8_SHUTDOWN_TIMEOUT_SECS` | 10 | Graceful shutdown deadline |
| `ORB8_EXPIRATION_INTERVAL_SECS` | 10 | Flow expiration sweep interval |
| `ORB8_MAX_QUERY_LIMIT` | 10000 | Max flows returned per query |

**Acceptance criteria**:
- Agent memory stays bounded under sustained high event rates
- `kubectl rollout restart ds/orb8-agent` causes zero panics
- `curl <agent-ip>:9091/healthz` returns 200 when healthy, 503 when degraded
- All existing tests pass (34 unit tests + smoke + e2e)

---

## Phase 5: Hybrid Mode -- Standalone CLI (v0.2.0)

**Goal**: Let users run `orb8 trace network --standalone` on any Linux box for immediate network visibility, with no Kubernetes deployment required.

**User value**: Zero-install evaluation path. Someone can `cargo install orb8 --features standalone` and immediately see network flows.

**Architecture**:
- Extract agent core logic into `orb8-core` crate (probe loading, enrichment, aggregation)
- `orb8-agent` becomes a thin wrapper: `orb8-core` + gRPC server + K8s watcher
- CLI gains `--standalone` flag that wires `orb8-core` directly to output formatters
- Feature flag: `--features standalone` includes eBPF probes in CLI binary (~15MB)
- Without feature flag: CLI is remote-only (~5MB, cross-platform)

**Mode detection**:
```
orb8 trace network --standalone           # Load probes directly (Linux + root)
orb8 trace network --agent localhost:9090  # Connect to running agent via gRPC
orb8 trace network                        # Auto-detect: try agent, fall back to standalone
```

**Deliverables**:
- [ ] `orb8-core/` crate with `StandaloneEngine` (probe loading, event processing, enrichment)
- [ ] `EventSource` trait in CLI abstracting gRPC vs standalone event streams
- [ ] `--standalone` flag in CLI with feature-gated `orb8-core` dependency
- [ ] `node_name` field added to `NetworkEvent` and `NetworkFlow` proto messages
- [ ] Identical output format in both modes

**Acceptance criteria**:
- `sudo orb8 trace network --standalone` on Linux shows live flows
- `orb8 trace network --agent localhost:9090` works exactly as before
- Output is identical between standalone and remote modes
- `cargo install orb8` (no feature flag) works on macOS for remote-only usage

---

## Phase 6: Cluster Mode -- Relay (v0.3.0)

**Goal**: Single endpoint to query all nodes. `orb8 flows` returns cluster-wide data.

**User value**: No more port-forwarding to individual agents. One command, full cluster visibility.

**Architecture** (Hubble relay pattern):
- `orb8-server` implements relay: discovers agents via K8s API, maintains persistent gRPC connections
- Fans out queries to all agents in parallel, merges results
- Exposes same `ObserverService` gRPC API externally
- Partial failure: returns results from healthy agents, warns about unreachable ones
- Streaming: `StreamEvents` merges per-node streams ordered by timestamp

**Proto changes**:
- `ObserverService` (external API for clients)
- `PeerService` (internal relay-to-agent topology discovery)
- `Peer` message with `PeerState` (connected/unreachable)
- `StatusResponse` with per-node breakdown

**CLI changes**:
```
orb8 flows                                    # Auto-discover relay
orb8 flows --relay orb8-relay:8080           # Explicit relay address
orb8 flows --agent worker-1:9090             # Direct single agent
orb8 flows --standalone                       # Local only
```

**Deliverables**:
- [ ] `orb8-server` implementation (relay, peer discovery, fan-out/fan-in)
- [ ] Proto split: `ObserverService` + `PeerService`
- [ ] Relay Deployment + Service manifests
- [ ] CLI relay auto-discovery and `--relay` flag
- [ ] Partial failure handling with degraded results

**Acceptance criteria**:
- `orb8 flows` via relay returns flows from all nodes in a multi-node kind cluster
- `orb8 trace network` via relay streams events from all nodes
- `orb8 status` via relay shows per-node health
- If one agent is down, queries return results from remaining agents

---

## Phase 7: Rich Filtering and JSON Output (v0.4.0)

**Goal**: Composable filters for flows and events. JSON output for scripting.

**User value**: `orb8 flows --namespace ml-training --protocol TCP --dst-port 443 --output json | jq .`

**Deliverables**:
- [ ] `FlowFilter` proto message with namespace, pod, label, protocol, port, IP, direction fields
- [ ] Server-side filter evaluation (applied at agent before gRPC transfer)
- [ ] Pod label enrichment in pod cache (extend `PodMetadata`)
- [ ] CLI `--output json|table` flag
- [ ] CLI filter flags: `--namespace`, `--pod`, `--label`, `--protocol`, `--src-port`, `--dst-port`, `--src-ip`, `--dst-ip`

**Acceptance criteria**:
- All filter fields work individually and in combination (AND semantics)
- `--output json` produces valid NDJSON (one object per line for streaming)
- Filters applied server-side (verifiable by reduced gRPC bandwidth)

---

## Phase 8: Deploy and Operate (v0.5.0)

**Goal**: Production deployment infrastructure -- Helm chart, CI/CD, multi-arch images.

**User value**: `helm install orb8 ./charts/orb8` deploys a working cluster in minutes.

**Deliverables**:
- [ ] Helm chart with configurable values (image, resources, tolerations, relay replicas)
- [ ] GitHub Actions: build + push to `ghcr.io` on tagged releases (amd64 + arm64)
- [ ] GitHub Releases with pre-built CLI binaries (Linux standalone, macOS remote-only)
- [ ] `orb8-system` namespace isolation
- [ ] `deploy/kustomization.yaml` base overlay

**Acceptance criteria**:
- `helm install orb8 ./charts/orb8` deploys agents + relay that start and capture traffic
- CI builds images on every tag push
- CLI binaries downloadable from GitHub Releases

---

## Phase 9: Full Prometheus Integration and Grafana (v0.6.0)

**Goal**: Comprehensive Prometheus metrics and pre-built Grafana dashboards.

**User value**: Grafana dashboards showing pod traffic, top talkers, agent health -- works without the CLI.

**Deliverables**:
- [ ] HTTP `/metrics` endpoint on port 9091 (sharing health server, or separate port)
- [ ] Flow metrics: `orb8_network_bytes_total{namespace,pod,direction,protocol}`, `orb8_network_packets_total`, `orb8_active_flows`
- [ ] Agent metrics: `orb8_events_processed_total`, `orb8_events_dropped_total`, `orb8_pods_tracked`, `orb8_agent_uptime_seconds`
- [ ] `deploy/servicemonitor.yaml` for Prometheus Operator
- [ ] `deploy/grafana-dashboard.json` -- pre-built dashboard
- [ ] Cardinality management (limit label combinations)

**Acceptance criteria**:
- `curl <agent-ip>:9091/metrics` returns valid Prometheus exposition format
- Grafana dashboard imports and shows live data
- Cardinality stays bounded under high pod churn

---

## Phase 10+: Future (v0.7.0+)

Deferred in priority order:

1. **Syscall Monitoring** -- tracepoint probes with cgroup-based enrichment. `orb8 trace syscall --pod my-app`. Validates cgroup enrichment path.
2. **GPU Telemetry** -- NVML polling + kubelet pod-resources API. `orb8 trace gpu --namespace ml-training`.
3. **TUI Dashboard** -- ratatui-based real-time terminal UI (top flows, pod traffic, agent health).
4. **DNS Tracing** -- Parse DNS in network probe (port 53 filter). `orb8 trace dns`.
5. **Historical Storage** -- TimescaleDB or Thanos for long-term flow retention.

---

## Deferred to Post-v1.0

| Feature | Reason |
|---------|--------|
| IPv6 support | NetworkFlowEvent struct migration needed |
| TCP connection state tracking | SYN/FIN/RST tracking adds complexity |
| eBPF GPU probes | Closed-source NVIDIA driver, research only |
| OpenTelemetry export sink | Prometheus sufficient for initial users |
| CRD-based policy/filter config | Env vars + CLI flags sufficient initially |
| Multi-cluster support | Get single-cluster right first |
| YAML config file system | Env vars sufficient until complexity warrants it |

---

## Known Limitations

### Network visibility scope
TC probes attach to the node's primary interface (eth0). Pod-to-pod traffic on the same node stays on veth pairs and never reaches eth0, making it invisible. Attaching to container bridge interfaces (cni0) or per-pod veths would capture this traffic but requires probe attachment in each pod's network namespace or on a shared bridge.

### hostNetwork pod attribution
Pods with `hostNetwork: true` share the node's IP address. All traffic from the node IP is attributed to whichever hostNetwork pod was last indexed in the pod cache. A hybrid approach (socket-level eBPF probe mapping 5-tuples to cgroup IDs, shared with TC via an eBPF map) would solve this but adds complexity.

### Service ClusterIP transparency
kube-proxy applies DNAT before packets reach the TC hook on eth0, so flows show the actual backend pod IP, not the Service ClusterIP. orb8 correctly enriches the traffic but cannot tell you which Service was originally addressed.

### Polling model
The main event loop polls the ring buffer at a configurable interval (default 100ms). Under high throughput, this adds latency and can cause ring buffer drops. Under zero traffic, it is wasted CPU. A future improvement will use epoll/async notification from the ring buffer.

### FlowKey heap allocation
`FlowKey` includes `namespace` and `pod_name` (heap-allocated Strings). At high packet rates, hashing these strings in the DashMap is measurably more expensive than a pure integer 5-tuple key. A two-level lookup (integer key -> enriched metadata) is planned for performance-critical deployments.

---

## Verification Plan

### Per-Phase Testing

| Phase | Test Command | What It Validates |
|-------|-------------|-------------------|
| 4 | `cargo test && cargo clippy --workspace -- -D warnings` | Hardening, config, health, no regressions |
| 4 | `make e2e-test` (Lima VM) | DaemonSet deploys with probes, health checks work |
| 5 | `sudo orb8 trace network --standalone` | Standalone mode captures flows |
| 6 | `orb8 flows` (via relay on multi-node kind) | Cross-node query aggregation works |
| 7 | `orb8 flows --namespace X --output json \| jq .` | Filtering and JSON output valid |
| 8 | `helm install orb8 ./charts/orb8` | Helm deployment works end-to-end |
| 9 | `curl localhost:9091/metrics` + Grafana import | Metrics valid, dashboard works |

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
