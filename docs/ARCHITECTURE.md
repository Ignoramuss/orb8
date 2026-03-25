# orb8 Architecture

**Technical Design Document for eBPF-Powered Kubernetes Observability Platform**

---

## Table of Contents

1. [System Overview](#system-overview)
2. [Monorepo Structure](#monorepo-structure)
3. [Operating Modes](#operating-modes)
4. [Core Components](#core-components)
5. [Probe Architecture](#probe-architecture)
6. [Container Identification](#container-identification)
7. [GPU Telemetry Design](#gpu-telemetry-design)
8. [Data Flow](#data-flow)
9. [Communication Architecture](#communication-architecture)
10. [Deployment Models](#deployment-models)
11. [Security Model](#security-model)
12. [Performance Characteristics](#performance-characteristics)
13. [References](#references)

---

## System Overview

orb8 is a **DaemonSet-based observability platform** providing always-on cluster-wide monitoring (Pixie-style). It leverages **eBPF probes** written entirely in **Rust** using the aya framework for kernel-level telemetry with minimal overhead.

### High-Level Architecture

```
┌───────────────────────────────────────────────────────────────────┐
│                       Kubernetes Cluster                          │
│                                                                   │
│  ┌─────────────────────────────────────────────────────────────┐  │
│  │                          Node 1                             │  │
│  │                                                             │  │
│  │  ┌───────────────────────────────────────────────────────┐  │  │
│  │  │               orb8-agent DaemonSet Pod                │  │  │
│  │  │                                                       │  │  │
│  │  │  ┌─────────────────────────────────────────────────┐  │  │  │
│  │  │  │  KERNEL SPACE                                   │  │  │  │
│  │  │  │                                                 │  │  │  │
│  │  │  │  eBPF Probes (Rust):                            │  │  │  │
│  │  │  │    network_probe (tc hook)                      │  │  │  │
│  │  │  │    syscall_probe (tracepoint)                   │  │  │  │
│  │  │  │    gpu_probe (kprobe/uprobe)                    │  │  │  │
│  │  │  │                                                 │  │  │  │
│  │  │  │  eBPF Maps (shared kernel/user memory):         │  │  │  │
│  │  │  │    FLOW_EVENTS (ring buffer)                    │  │  │  │
│  │  │  │    SYSCALL_EVENTS (ring buffer)                 │  │  │  │
│  │  │  │    GPU_EVENTS (ring buffer)                     │  │  │  │
│  │  │  │    POD_METADATA (hashmap)                       │  │  │  │
│  │  │  └─────────────────────────────────────────────────┘  │  │  │
│  │  │                          ║                            │  │  │
│  │  │                          ║ (ring buffers)             │  │  │
│  │  │                          ▼                            │  │  │
│  │  │  ┌─────────────────────────────────────────────────┐  │  │  │
│  │  │  │  USER SPACE (Rust)                              │  │  │  │
│  │  │  │                                                 │  │  │  │
│  │  │  │  Probe Loader (aya)                             │  │  │  │
│  │  │  │  Event Collector (ring buffer reader)           │  │  │  │
│  │  │  │  Pod Metadata Manager (K8s watcher)             │  │  │  │
│  │  │  │  Metrics Aggregator (time-series)               │  │  │  │
│  │  │  │  Agent gRPC Server :9090                        │  │  │  │
│  │  │  │  Prometheus Exporter :9091/metrics              │  │  │  │
│  │  │  └─────────────────────────────────────────────────┘  │  │  │
│  │  └───────────────────────────────────────────────────────┘  │  │
│  │                                                             │  │
│  │  ┌───────────────────────────────────────────────────────┐  │  │
│  │  │  Workload Pods (being monitored)                      │  │  │
│  │  │    nginx-xyz (network traffic traced)                 │  │  │
│  │  │    pytorch-job (GPU usage monitored)                  │  │  │
│  │  └───────────────────────────────────────────────────────┘  │  │
│  └─────────────────────────────────────────────────────────────┘  │
│                               │                                   │
│                               │ gRPC                              │
│  ┌────────────────────────────▼────────────────────────────────┐  │
│  │  orb8-server (Central Control Plane)                        │  │
│  │    Cluster-wide aggregation                                 │  │
│  │    gRPC API :8080                                           │  │
│  │    Query routing to nodes                                   │  │
│  └─────────────────────────────────────────────────────────────┘  │
└───────────────────────────────────────────────────────────────────┘
                                │
                                │ gRPC
                                ▼
                      ┌──────────────────┐
                      │    orb8 CLI      │
                      │   (Developer)    │
                      └──────────────────┘
```

---

## Monorepo Structure

orb8 is organized as a **virtual Cargo workspace** (no root package) with multiple crates.

> **Note**: The structure below shows the **target architecture**. See the "Current Implementation Status" section at the end of this document for what is actually implemented in each phase.

```
orb8/
├── Cargo.toml                        # Virtual workspace ONLY (no [package])
│
├── orb8-probes/                      # eBPF probes (kernel space, no_std)
│   ├── Cargo.toml
│   ├── src/
│   │   ├── network_probe.rs          # Network flow tracing (TC classifier)
│   │   ├── syscall_probe.rs          # System call monitoring (Phase 8)
│   │   └── gpu_probe.rs              # GPU telemetry (Phase 9)
│   └── build.rs                      # eBPF compilation
│
├── orb8-common/                      # Shared types (no_std compatible)
│   ├── Cargo.toml
│   └── src/
│       └── lib.rs                    # NetworkFlowEvent, constants
│
├── orb8-util/                        # Userspace shared utilities (Phase 6)
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── format.rs                 # Display formatting (bytes, truncation)
│       ├── parse.rs                  # Duration parsing
│       └── net.rs                    # IP formatting/parsing
│
├── orb8-agent/                       # Node agent (DaemonSet)
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs                   # Entrypoint + event loop
│       ├── lib.rs                    # Module declarations
│       ├── net.rs                    # IP parsing/formatting (consolidated)
│       ├── aggregator.rs             # Flow aggregation
│       ├── grpc_server.rs            # gRPC service
│       ├── probe_loader.rs           # eBPF lifecycle
│       ├── cgroup.rs                 # Cgroup resolution
│       ├── pod_cache.rs              # Pod metadata cache
│       └── k8s_watcher.rs            # Pod lifecycle watcher
│
├── orb8-server/                      # Central API server (Phase 7)
│   ├── Cargo.toml
│   └── src/
│       └── lib.rs                    # Stub
│
├── orb8-cli/                         # CLI tool (produces `orb8` binary)
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs                   # CLI commands
│       └── lib.rs
│
├── orb8-proto/                       # gRPC protocol definitions
│   ├── Cargo.toml
│   ├── build.rs
│   └── proto/
│       └── orb8.proto                # Service definitions
│
├── deploy/                           # Dockerfile + K8s manifests (Phase 4)
│
├── tests/                            # Integration tests
│
├── docs/
│   ├── ARCHITECTURE.md               # This file
│   └── ROADMAP.md                    # Development roadmap
│
├── .lima/                            # macOS development VM
│   └── orb8-dev.yaml
│
└── scripts/
    └── setup-lima.sh
```

### Target Agent Structure (Phase 6)

The agent will be refactored into a composable pipeline architecture:

```
orb8-agent/src/
  main.rs                       # Slim entrypoint (~80 lines)
  lib.rs                        # Module declarations
  config.rs                     # AgentConfig struct, env var parsing
  event.rs                      # EnrichedEvent (canonical enriched type)
  net.rs                        # IP parsing, formatting (consolidated)
  filter.rs                     # EventFilter trait + SelfTrafficFilter
  shutdown.rs                   # CancellationToken + JoinSet coordinator
  probe_loader.rs               # eBPF lifecycle
  cgroup.rs                     # Cgroup resolution
  grpc_server.rs                # gRPC service
  pipeline/
    mod.rs                      # EventPipeline orchestrator
    source.rs                   # EventSource trait + RingBufSource
    enricher.rs                 # Single enrichment path (IP-first, cgroup fallback)
    processor.rs                # EventProcessor trait
    exporter.rs                 # EventExporter trait + fanout
  export/
    mod.rs
    grpc.rs                     # gRPC streaming exporter
    log.rs                      # Debug log exporter
  k8s/
    mod.rs
    watcher.rs                  # PodWatcher
    pod_cache.rs                # PodCache
  aggregator.rs                 # Flow aggregation (accepts pre-enriched events)
```

### Crate Dependency Graph

```
orb8-probes (no_std)
    |
    v
orb8-common (no_std compatible)
    |
  +-+--------+
  |          |
  v          v
orb8-util  orb8-proto
  |          |
  +--+--+    |
  |  |  |    |
  v  v  v    v
 cli agent server
```

### Distribution

**crates.io** (Rust library/binary distribution):
- `orb8-common` - Shared types between eBPF probes and userspace
- `orb8-cli` - CLI tool (installs `orb8` binary)
- `orb8-agent` - Node agent binary (`cargo install orb8-agent`, Linux-only)

**Not on crates.io**:
- `orb8-probes` - eBPF bytecode compiled for `bpfel-unknown-none` target; embedded in `orb8-agent` binary
- `orb8-server` - Central API server (stub, Phase 7)
- `orb8-proto` - gRPC protocol definitions

**Container Images** (Phase 4):
- `ghcr.io/ignoramuss/orb8-agent` - For Kubernetes DaemonSet deployment
- `ghcr.io/ignoramuss/orb8-server` - For central server deployment (Phase 7)

**Usage**:
```bash
# Install CLI binary
cargo install orb8-cli

# Install agent binary (Linux only)
cargo install orb8-agent

# Kubernetes deployment (Phase 4)
kubectl apply -k deploy/
```

---

## Operating Mode

orb8 operates as a **DaemonSet-based platform** providing always-on cluster-wide observability.

### Current: Single-Node Agent Mode

**Use case**: Per-node network visibility via direct agent connection

```bash
# Connect CLI directly to an agent
orb8 --agent <node-ip>:9090 status
orb8 --agent <node-ip>:9090 flows
orb8 --agent <node-ip>:9090 trace network
```

**Architecture**:
- DaemonSet runs `orb8-agent` on every node
- CLI connects directly to individual agents via gRPC
- Each agent provides visibility into its own node

### Future: Cluster Mode (Phase 7)

**Use case**: Cluster-wide observability via central server

```bash
# Install infrastructure
kubectl apply -k deploy/

# Query cluster-wide
orb8 flows --namespace ml-training
orb8 trace network --pod nginx-xyz
```

**Architecture**:
- DaemonSet runs `orb8-agent` on every node
- Central `orb8-server` discovers agents, routes queries, aggregates results
- CLI connects to server via gRPC

---

## Core Components

### Component 1: eBPF Probes (orb8-probes/)

eBPF programs written in **Rust** using the `aya-bpf` framework. These run in **kernel space**.

#### Build Process

```toml
# orb8-probes/Cargo.toml
[package]
name = "orb8-probes"
version = "0.1.0"
edition = "2021"

[dependencies]
aya-bpf = "0.1"
aya-log-ebpf = "0.1"
orb8-common = { path = "../orb8-common", default-features = false }

[profile.release]
opt-level = 3
lto = true
```

eBPF probes are compiled to `.o` object files using aya-bpf-compiler during build, then embedded into the agent binary for distribution.

### Component 2: Node Agent (orb8-agent/)

Rust user-space daemon running as DaemonSet pod.

**Responsibilities**:
- Load and manage eBPF probes
- Poll ring buffers for events
- Watch Kubernetes API for pod metadata
- Enrich events with pod/namespace context
- Aggregate metrics
- Expose gRPC API for queries
- Export Prometheus metrics

**Lifecycle**:
1. Initialize Kubernetes client
2. Start pod watcher (with reconnection on failure)
3. Load and attach eBPF probes
4. Start event collector (poll ring buffers)
5. Start gRPC API server (port 9090)
6. Start Prometheus exporter (port 9091)
7. Wait for shutdown signal
8. Cleanup: unload probes

### Component 3: Central API Server (orb8-server/)

Aggregates data from all node agents, provides cluster-wide query API.

**Responsibilities**:
- Discover and connect to all agents
- Route queries to appropriate nodes based on pod location
- Aggregate results from multiple agents
- Serve CLI queries via gRPC (port 8080)

### Component 4: CLI (orb8-cli/)

User-facing command-line interface.

**Cluster Mode**: Connects to central server via gRPC, queries cluster-wide data

**Standalone Mode**: Directly loads probes on target node via kubectl exec for ad-hoc tracing without DaemonSet

---

## Probe Architecture

All probes are written in **Rust** using `aya-bpf` and compiled to eBPF bytecode.

### Probe 1: network_probe

**Purpose**: Trace network flows per container

**eBPF Program Type**: `tc` (Traffic Control)

**Attachment Point**: tc ingress/egress hooks on veth interfaces

**Key Operations**:
1. Extract cgroup ID to identify container
2. Parse packet headers (Ethernet, IP, TCP/UDP)
3. Capture flow metadata (src/dst IP, ports, protocol, byte count)
4. Write event to ring buffer for userspace processing

**Event Data**: NetworkFlowEvent structure includes cgroup_id, timestamp, src/dst IP, protocol, and byte count.

### Probe 2: syscall_probe

**Purpose**: Monitor system calls for anomaly detection

**eBPF Program Type**: `tracepoint`

**Attachment Point**: `tracepoint/raw_syscalls/sys_enter`

**Key Operations**:
1. Extract cgroup ID and process ID
2. Read syscall ID from tracepoint arguments
3. Capture syscall metadata (syscall number, timestamp)
4. Apply sampling for high-frequency syscalls (read/write)
5. Write event to ring buffer

**Event Data**: SyscallEvent structure includes cgroup_id, pid, syscall_id, and timestamp.

### Probe 3: gpu_probe

**Purpose**: Track GPU usage per container

**Approach**: Multiple options (see [GPU Telemetry Design](#gpu-telemetry-design))

**Option A: DCGM Integration** (Recommended for MVP)
- Not an eBPF probe
- User-space polling of NVIDIA DCGM metrics
- Correlate with pod metadata

**Option B: eBPF ioctl Hooks** (Future Enhancement - Research Phase)
- Attach kprobe to NVIDIA driver ioctl functions
- Capture GPU memory allocation and kernel launch events
- Extract cgroup ID to correlate with containers
- Highly experimental due to closed-source driver instability

---

## Container Identification

**Critical Problem**: eBPF programs run in kernel space and have no direct knowledge of Kubernetes pods. How do we map kernel events to specific pods?

**Solution**: Dual enrichment strategy depending on the eBPF program type.

### Strategy 1: IP-Based Enrichment (Network Probes)

TC classifiers **cannot** call `bpf_get_current_cgroup_id()` because they execute in the network stack context (softirq), not in a process context. Network events always have `cgroup_id=0`.

Instead, the agent uses **IP-based pod lookup**:

1. **PodWatcher** watches K8s API for pod events, extracts pod IPs
2. **PodCache** maintains a `pod_ip (u32) → PodMetadata` map
3. On each event, the agent matches `src_ip` or `dst_ip` against the cache
4. Direction-aware attribution: ingress → dst IP is the pod, egress → src IP is the pod

### Strategy 2: Cgroup-Based Enrichment (Tracepoint Probes)

Tracepoint-attached probes (syscall monitoring, Phase 8) execute in process context where `bpf_get_current_cgroup_id()` IS available.

1. **eBPF probe** extracts cgroup ID via `bpf_get_current_cgroup_id()`
2. **CgroupResolver** maps pod UID + container ID → cgroup inode via filesystem
3. **PodCache** maintains a `cgroup_id (u64) → PodMetadata` map
4. Agent enriches events by looking up the cgroup ID

### PodWatcher Details

- Watches Kubernetes API for pod events (create, update, delete)
- Extracts pod UID, namespace, name, container IDs, and pod IP
- Resolves cgroup ID for each container (when filesystem is accessible)
- Populates both IP and cgroup maps in PodCache
- Implements reconnection with exponential backoff on watch failure

### Cgroup Hierarchy

Kubernetes uses cgroup v2 with this structure:

```
/sys/fs/cgroup/
└── kubepods.slice/
    ├── kubepods-burstable.slice/
    │   └── kubepods-burstable-pod<UID>.slice/
    │       └── cri-containerd-<container_id>.scope
    └── kubepods-besteffort.slice/
        └── kubepods-besteffort-pod<UID>.slice/
            └── cri-containerd-<container_id>.scope
```

The agent must handle all QoS classes (Guaranteed, Burstable, BestEffort).

### Runtime Compatibility Matrix

orb8 supports multiple container runtimes with different cgroup path formats:

| Runtime | Path Format | K8s Version | Status |
|---------|-------------|-------------|--------|
| **containerd** | `cri-containerd-{id}.scope` | 1.20+ | Primary |
| **CRI-O** | `crio-{id}.scope` | 1.20+ | Planned |
| **Docker** | `docker-{id}.scope` | <1.24 (deprecated) | Planned |

**Example Paths**:

```bash
# containerd (default in most K8s distros)
/sys/fs/cgroup/kubepods.slice/kubepods-burstable-pod{UID}.slice/cri-containerd-{container_id}.scope

# CRI-O (OpenShift default)
/sys/fs/cgroup/kubepods.slice/kubepods-burstable-pod{UID}.slice/crio-{container_id}.scope

# Docker (legacy, K8s <1.24)
/sys/fs/cgroup/kubepods.slice/kubepods-burstable-pod{UID}.slice/docker-{container_id}.scope
```

**Runtime Detection**: Agent auto-detects runtime by checking for socket presence (`/run/containerd/containerd.sock`, `/run/crio/crio.sock`, `/var/run/docker.sock`), falling back to trying all path formats.

---

## GPU Telemetry Design

GPU monitoring is a focus area for AI/ML workloads. This section documents the approach for per-pod GPU telemetry.

### Industry Landscape

**Standard tools:**
- [NVIDIA GPU Operator](https://docs.nvidia.com/datacenter/cloud-native/gpu-operator/latest/index.html) - Kubernetes operator bundling drivers, device plugin, and dcgm-exporter
- [dcgm-exporter](https://github.com/NVIDIA/dcgm-exporter) - Prometheus exporter using NVIDIA DCGM (Data Center GPU Manager)
- [Coroot](https://coroot.com/blog/working-with-gpus-on-kubernetes-and-making-them-observable/) - Uses NVML directly with zero instrumentation

**The per-pod attribution challenge:**

GPU metrics are naturally node-level (GPUs don't know about containers). Mapping GPU usage to specific pods requires either:
1. **Device plugin allocation tracking** - Query kubelet's pod-resources API (`/var/lib/kubelet/pod-resources`) for GPU device allocations, then match GPU UUIDs to pods
2. **Process-to-container mapping** - Use NVML's per-process metrics and map PIDs to containers via cgroup

| Tool | Approach | Per-Pod Attribution | Notes |
|------|----------|---------------------|-------|
| dcgm-exporter | DCGM daemon | Via kubelet pod-resources API | Industry standard, part of GPU Operator |
| Coroot | NVML direct | PID-to-container mapping | Zero-instrumentation, lightweight |
| eBPF (uprobes) | CUDA library tracing | Via cgroup ID | Can trace API calls, not GPU internals |

### DCGM Integration (Recommended for MVP)

Deploy [dcgm-exporter](https://github.com/NVIDIA/dcgm-exporter) as a sidecar and scrape its Prometheus metrics. Per-pod attribution via device plugin allocations.

**Architecture**:
```
┌───────────────────────────────────────────┐
│  orb8-agent Pod                           │
│                                           │
│  ┌─────────────────────────────────────┐  │
│  │  DCGM Sidecar Container             │  │
│  │    Runs dcgm-exporter               │  │
│  │    Exposes metrics on :9400         │  │
│  └─────────────────────────────────────┘  │
│                                           │
│  ┌─────────────────────────────────────┐  │
│  │  orb8-agent Container               │  │
│  │    Scrapes localhost:9400           │  │
│  │    Correlates GPU → Pod             │  │
│  │    Enriches with K8s metadata       │  │
│  └─────────────────────────────────────┘  │
└───────────────────────────────────────────┘
```

**Pros:** Production-proven, comprehensive metrics (utilization, memory, temperature, power), handles MIG partitioning

**Cons:** Requires DCGM installation, polling-based (1-10s granularity)

---

### NVML Direct Integration

Use [NVML](https://developer.nvidia.com/nvidia-management-library-nvml) (libnvidia-ml.so) directly from the agent, similar to nvidia-smi and Coroot.

**Pros:** No DCGM dependency, lighter weight, direct control over polling

**Cons:** Lower-level API, must implement per-pod attribution ourselves

---

### eBPF-Based GPU Tracing (Future Research)

Two sub-approaches under active research:

**CPU-side tracing:** Attach uprobes to CUDA libraries (libcuda.so, libcudart.so) to trace API calls like cudaMalloc, cudaLaunch, cudaMemcpy. Provides call-level visibility but cannot observe GPU-internal execution.

**GPU-side eBPF:** Academic research ([eGPU](https://dl.acm.org/doi/10.1145/3723851.3726984)) explores running eBPF bytecode on the GPU itself for memory access tracing. Not production-ready.

**Pros:** Event-driven (not polling), potential for CUDA kernel-level insights

**Cons:** Experimental, may break on driver updates, limited to CUDA workloads

---

### Recommended Strategy

| Phase | Approach | Rationale |
|-------|----------|-----------|
| MVP | DCGM Integration | Production-proven, covers 90% of use cases |
| Enhancement | NVML Direct | Lighter deployment for environments without DCGM |
| Research | eBPF uprobes on CUDA | Deeper observability for advanced users |

---

## Data Flow

### End-to-End Flow: Network Packet Tracing

```
1. [KERNEL] Packet arrives at veth interface
        ↓
2. [KERNEL] TC hook triggers network_probe (eBPF)
        ↓
3. [KERNEL] Probe extracts: src_ip, dst_ip, ports, protocol, packet_len
        ↓
4. [KERNEL] Writes NetworkFlowEvent to FLOW_EVENTS ring buffer
        ↓
5. [USER] Main loop polls ring buffer (100ms interval)
        ↓
6. [USER] Deserializes into NetworkFlowEvent struct
        ↓
7. [USER] Filters self-traffic (agent gRPC port on local IPs)
        ↓
8. [USER] IP-based enrichment: match src/dst IP against PodCache
        ↓
9. [USER] Direction-aware attribution:
    - ingress: dst IP is the local pod
    - egress: src IP is the local pod
        ↓
10. [USER] Aggregator updates flow stats with pre-enriched (namespace, pod_name)
    [USER] Broadcasts enriched NetworkEvent to gRPC stream subscribers
        ↓
11. [USER] Agent gRPC API serves QueryFlows and StreamEvents at :9090
        ↓
12. [EXTERNAL] CLI queries via gRPC → Agent
    [EXTERNAL] Prometheus scrapes /metrics (Phase 5)
```

> **Note**: TC classifiers cannot call `bpf_get_current_cgroup_id()`, so
> `cgroup_id` is always 0 in network events. Enrichment relies entirely on
> IP-based pod lookup. Cgroup-based enrichment will be used for tracepoint
> probes (syscall monitoring, Phase 8) where `bpf_get_current_cgroup_id()`
> is available.

### Memory Layout

```
┌──────────────────────────────────────────────────────────────┐
│  KERNEL SPACE                                                │
│                                                              │
│  eBPF Probes (.text section, read-only)                      │
│  ┌────────────────────────────────────────────────────────┐  │
│  │  network_probe.o:  ~8KB compiled bytecode              │  │
│  │  syscall_probe.o:  ~4KB compiled bytecode              │  │
│  │  gpu_probe.o:      ~4KB compiled bytecode              │  │
│  └────────────────────────────────────────────────────────┘  │
│                                                              │
│  eBPF Maps (kernel heap, accessible from user space)         │
│  ┌────────────────────────────────────────────────────────┐  │
│  │  FLOW_EVENTS:       RingBuf, 1MB                       │  │
│  │  SYSCALL_EVENTS:    RingBuf, 512KB                     │  │
│  │  GPU_EVENTS:        RingBuf, 512KB                     │  │
│  │  POD_METADATA:      HashMap, max 10k entries           │  │
│  │  CONFIG:            HashMap, ~1KB                      │  │
│  └────────────────────────────────────────────────────────┘  │
│                                                              │
│  Total kernel memory: ~2.5MB per node                        │
└──────────────────────────────────────────────────────────────┘
                             ║
                             ║ (ring buffers mmap'd into user space)
                             ▼
┌──────────────────────────────────────────────────────────────┐
│  USER SPACE                                                  │
│                                                              │
│  orb8-agent process: ~50-100MB RSS                           │
│  ┌────────────────────────────────────────────────────────┐  │
│  │  aya library:        manages eBPF lifecycle            │  │
│  │  Event buffers:      ring buffer readers               │  │
│  │  Aggregator cache:   last 5 min of metrics (~10MB)     │  │
│  │  gRPC server:        tokio runtime                     │  │
│  │  K8s client:         kube-rs API cache                 │  │
│  └────────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────────┘
```

---

## Communication Architecture

### Agent ↔ eBPF Probes

**Mechanism**: eBPF maps (shared kernel/user memory)

**Ring Buffers** (kernel → user):
```rust
// User-space reading
use aya::maps::RingBuf;

let mut flow_events = RingBuf::try_from(bpf.map_mut("FLOW_EVENTS")?)?;

loop {
    if let Some(event_data) = flow_events.next() {
        let event: NetworkFlowEvent = unsafe {
            std::ptr::read(event_data.as_ptr() as *const _)
        };

        process_event(event).await;
    }

    tokio::time::sleep(Duration::from_millis(10)).await;
}
```

**HashMaps** (bidirectional):
```rust
// User-space writing pod metadata
use aya::maps::HashMap as BpfHashMap;

let mut pod_map = BpfHashMap::try_from(bpf.map_mut("POD_METADATA")?)?;

pod_map.insert(
    cgroup_id,
    PodMetadata {
        namespace: "production".to_string(),
        pod_name: "nginx-xyz".to_string(),
    },
    0, // flags
)?;

// eBPF can now read this metadata (though typically doesn't need to)
```

### Agent ↔ Central Server

**Protocol**: gRPC over HTTP/2

**Service Definition**:
```protobuf
// orb8-proto/proto/orb8.proto
syntax = "proto3";

package orb8;

service OrbitService {
    rpc QueryFlows(FlowQuery) returns (FlowResponse);
    rpc QuerySyscalls(SyscallQuery) returns (SyscallResponse);
    rpc GetAgentStatus(StatusRequest) returns (StatusResponse);
    rpc StreamEvents(StreamRequest) returns (stream Event);
}

message FlowQuery {
    string namespace = 1;
    string pod_name = 2;
    optional int64 start_time_ns = 3;
    optional int64 end_time_ns = 4;
}

message FlowResponse {
    repeated NetworkFlow flows = 1;
}

message NetworkFlow {
    string namespace = 1;
    string pod_name = 2;
    string src_ip = 3;
    string dst_ip = 4;
    uint32 bytes = 5;
    int64 timestamp_ns = 6;
}
```

Clients (CLI, Server) connect to agents via gRPC using the OrbitService interface defined above.

---

## Deployment Models

### Cluster Mode Deployment

```yaml
# deploy/namespace.yaml
apiVersion: v1
kind: Namespace
metadata:
  name: orb8-system
```

```yaml
# deploy/daemonset.yaml
apiVersion: apps/v1
kind: DaemonSet
metadata:
  name: orb8-agent
  namespace: orb8-system
spec:
  selector:
    matchLabels:
      app: orb8-agent
  template:
    metadata:
      labels:
        app: orb8-agent
    spec:
      hostNetwork: true
      hostPID: true
      serviceAccountName: orb8-agent
      containers:
      - name: agent
        image: orb8/agent:latest
        securityContext:
          privileged: true
          capabilities:
            add:
            - SYS_ADMIN      # Load eBPF programs
            - SYS_RESOURCE   # Increase locked memory limit
            - NET_ADMIN      # Attach to network interfaces
        volumeMounts:
        - name: sys
          mountPath: /sys
          readOnly: true
        - name: cgroup
          mountPath: /sys/fs/cgroup
          readOnly: true
        - name: bpffs
          mountPath: /sys/fs/bpf
        env:
        - name: NODE_NAME
          valueFrom:
            fieldRef:
              fieldPath: spec.nodeName
        - name: RUST_LOG
          value: info
        ports:
        - containerPort: 9090
          name: grpc
        - containerPort: 9091
          name: metrics
        resources:
          requests:
            memory: "128Mi"
            cpu: "100m"
          limits:
            memory: "512Mi"
            cpu: "500m"
      volumes:
      - name: sys
        hostPath:
          path: /sys
      - name: cgroup
        hostPath:
          path: /sys/fs/cgroup
      - name: bpffs
        hostPath:
          path: /sys/fs/bpf
          type: DirectoryOrCreate
```

```yaml
# deploy/server.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: orb8-server
  namespace: orb8-system
spec:
  replicas: 2
  selector:
    matchLabels:
      app: orb8-server
  template:
    metadata:
      labels:
        app: orb8-server
    spec:
      serviceAccountName: orb8-server
      containers:
      - name: server
        image: orb8/server:latest
        ports:
        - containerPort: 8080
          name: grpc
        env:
        - name: RUST_LOG
          value: info
---
apiVersion: v1
kind: Service
metadata:
  name: orb8-server
  namespace: orb8-system
spec:
  selector:
    app: orb8-server
  ports:
  - port: 8080
    name: grpc
  type: ClusterIP
```

```yaml
# deploy/rbac.yaml
apiVersion: v1
kind: ServiceAccount
metadata:
  name: orb8-agent
  namespace: orb8-system
---
apiVersion: rbac.authorization.k8s.io/v1
kind: ClusterRole
metadata:
  name: orb8-agent
rules:
- apiGroups: [""]
  resources: ["pods", "nodes"]
  verbs: ["get", "list", "watch"]
---
apiVersion: rbac.authorization.k8s.io/v1
kind: ClusterRoleBinding
metadata:
  name: orb8-agent
roleRef:
  apiGroup: rbac.authorization.k8s.io
  kind: ClusterRole
  name: orb8-agent
subjects:
- kind: ServiceAccount
  name: orb8-agent
  namespace: orb8-system
```

---

## Security Model

### Capabilities Required

**Agent Container**:
- `CAP_SYS_ADMIN`: Load eBPF programs (required for bpf() syscall)
- `CAP_SYS_RESOURCE`: Increase `RLIMIT_MEMLOCK` for eBPF maps
- `CAP_NET_ADMIN`: Attach tc probes to network interfaces
- `CAP_BPF`: Explicit eBPF permission (Linux 5.8+)
- `CAP_PERFMON`: Access performance events (Linux 5.8+)

Modern kernels (5.8+) allow fine-grained capabilities instead of full `privileged: true`:

```yaml
securityContext:
  capabilities:
    add:
    - BPF
    - PERFMON
    - NET_ADMIN
    - SYS_RESOURCE
```

### eBPF Verifier

All eBPF programs are verified before loading:

**Guarantees**:
- No infinite loops (bounded execution)
- No out-of-bounds memory access
- No kernel crashes possible
- Limited stack size (512 bytes)
- No arbitrary kernel memory reads

**Limitations Enforced**:
- Cannot call arbitrary kernel functions (only whitelisted helpers)
- Cannot access kernel data structures without BTF type info
- Cannot modify return values of system calls

### RBAC

**Agent Permissions**:
```yaml
rules:
- apiGroups: [""]
  resources: ["pods", "nodes"]
  verbs: ["get", "list", "watch"]  # Read-only!
```

Agents cannot modify cluster state, only observe.

### Network Policies

Restrict agent-to-server communication:

```yaml
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata:
  name: orb8-agent-egress
  namespace: orb8-system
spec:
  podSelector:
    matchLabels:
      app: orb8-agent
  egress:
  - to:
    - podSelector:
        matchLabels:
          app: orb8-server
    ports:
    - protocol: TCP
      port: 8080
  - to:  # Allow Kubernetes API access
    - namespaceSelector: {}
      podSelector:
        matchLabels:
          k8s-app: kube-apiserver
```

---

## Performance Characteristics

### Resource Overhead

**Per-Node (Agent)**:
- **CPU**: 50-200m (0.05-0.2 cores) baseline, up to 500m under high load
- **Memory**: 128-256 MiB RSS
- **Network**: 1-5 MiB/s metrics traffic
- **Disk**: Negligible (no persistent storage)

**Per-Cluster (Server)**:
- **CPU**: 100-500m depending on cluster size
- **Memory**: 512 MiB - 2 GiB (scales with number of nodes)
- **Network**: Aggregate of all agents

### eBPF Overhead

**Network Probe**:
- Per-packet processing: <1μs
- Overhead on 10Gbps link: <0.5%
- Ring buffer full: drops events gracefully (no system impact)

**Syscall Probe**:
- Per-syscall overhead: <100ns
- High-frequency syscalls (read/write): sampled at 1:100 ratio
- Low-frequency syscalls (execve, open): 100% traced

**GPU Probe** (DCGM polling):
- Polling interval: 1-10 seconds (configurable)
- No overhead on GPU workloads
- DCGM itself: ~50 MiB memory, negligible CPU

### Scalability Targets

| Metric | Target |
|--------|--------|
| Pods per node | 500+ |
| Nodes per cluster | 1000+ |
| Events per second (per node) | 100,000+ |
| Ring buffer drops | <0.1% under normal load |
| Query latency (CLI) | <500ms (p99) |
| Prometheus scrape | <5s for full cluster |

### Optimization Strategies

1. **Event Sampling**: High-frequency events sampled probabilistically
2. **In-Kernel Filtering**: Drop uninteresting events in eBPF, not user-space
3. **Batch Processing**: Process ring buffer in batches, not per-event
4. **Zero-Copy**: Ring buffers avoid memory copies
5. **Prometheus Metrics**: Pre-aggregated, not raw events

### Ring Buffer Overflow Mitigation

**Problem Statement**:

At high event rates, ring buffers can overflow, causing event loss:

```
Event Rate: 1,000,000 events/sec
Event Size: 64 bytes
Required Bandwidth: 64 MB/sec
Ring Buffer Size: 1 MB
Time to Fill: 16 milliseconds
```

With a 1MB ring buffer processing 1M events/sec, the buffer fills in just 16ms. If userspace polling is delayed (e.g., CPU scheduling), significant event loss occurs.

**Mitigations**:

1. **Per-Flow Aggregation in eBPF**:
   - Use eBPF hashmap to aggregate packets into flows before sending to ring buffer
   - Reduces event rate by 100x (1M packets/sec → 10K flows/sec)
   - Trade-off: Higher eBPF memory usage (hashmap size)

2. **Adaptive Sampling**:
   - Monitor ring buffer utilization from userspace
   - Signal eBPF probe to increase sampling rate when buffer >80% full
   - Implementation:
     ```c
     // In eBPF probe
     if (ring_buffer_utilization() > 0.9) {
         // Drop 9 out of 10 events during backpressure
         if (bpf_get_prandom_u32() % 10 != 0) {
             return TC_ACT_OK;  // Skip this event
         }
     }
     ```

3. **Configurable Buffer Sizes**:
   - Default: 1MB (network), 512KB (syscall)
   - Maximum: 32MB per probe
   - Environment variable: `ORB8_RING_BUFFER_SIZE`
   - Must be power of 2 (eBPF requirement)

4. **Critical Event Preservation**:
   - Never sample TCP SYN, FIN, RST packets (flow state changes)
   - Always capture first N packets of new flows (connection establishment)
   - Implementation: check packet flags before sampling decision

5. **Monitoring and Alerting**:
   - Expose metrics: `orb8_ring_buffer_drops_total`, `orb8_ring_buffer_utilization`
   - Alert when drop rate >1% sustained for 5+ minutes
   - Log warnings when sustained backpressure detected

**Performance Impact**:

| Strategy | Event Loss | CPU Overhead | Memory Overhead |
|----------|-----------|--------------|-----------------|
| No mitigation | 50-99% | Minimal | Minimal |
| Sampling (1:10) | <0.1% | +5% | None |
| Per-flow aggregation | <0.01% | +10% | +16MB (hashmap) |
| Larger buffer (8MB) | <1% | None | +7MB |
| Combined (all above) | <0.01% | +15% | +23MB |

**Recommended Configuration**:

For high-traffic production clusters (>10Gbps per node):
- Ring buffer size: 8MB
- Per-flow aggregation: Enabled
- Adaptive sampling: Enabled (threshold: 80%)
- Preserve critical events: Enabled

---

## Current Implementation Status

This section documents what is actually implemented as of Phase 3.5 (v0.0.4).

### Implemented Components

| Component | Status | Key Files |
|-----------|--------|-----------|
| `orb8-probes` | Phase 3 | `src/network_probe.rs` - Full IPv4/TCP/UDP/ICMP packet parsing, ring buffer, drop counter |
| `orb8-common` | Phase 3 | `src/lib.rs` - `NetworkFlowEvent`, protocol/direction constants, LE assertion |
| `orb8-agent` | Phase 3.5 | `main.rs`, `lib.rs`, `net.rs`, `probe_loader.rs`, `aggregator.rs`, `grpc_server.rs`, `k8s_watcher.rs`, `pod_cache.rs`, `cgroup.rs` |
| `orb8-proto` | Phase 3 | `src/lib.rs`, `build.rs`, `proto/orb8.proto` - QueryFlows, StreamEvents, GetStatus |
| `orb8-server` | Stub | `src/lib.rs` - placeholder (Phase 7) |
| `orb8-cli` | Phase 3 | `src/main.rs` - status, flows, trace network commands |

### Phase Completion

- **Phase 0** (Foundation): ✅ Complete
- **Phase 1** (eBPF Infrastructure): ✅ Complete
  - eBPF probe compilation with aya-bpf
  - Probe loading and lifecycle management
  - Ring buffer kernel-to-userspace communication
  - Pre-flight system checks (kernel version, BTF, capabilities)
- **Phase 2** (Container Identification): ✅ Complete
  - Kubernetes pod watcher (kube-rs)
  - Pod cache with IP-based and cgroup-based lookup
  - gRPC API server (port 9090)
  - Flow aggregation with 30s expiration
- **Phase 3** (Network MVP): ✅ Complete (v0.0.3)
  - Full packet parsing (5-tuple extraction)
  - gRPC QueryFlows, StreamEvents, GetStatus
  - CLI trace network command
  - IP-based pod enrichment tested with kind cluster
  - Smart interface discovery (eth0, cni0, docker0, br-*)
  - Ring buffer drop counter (EVENTS_DROPPED eBPF map)
  - Self-traffic filter (agent gRPC port excluded)
- **Phase 3.5** (Structural Cleanup): 🔄 In Progress (v0.0.4)
  - Fixed double-enrichment bug (aggregator now accepts pre-resolved pod identity)
  - Removed dead root `src/` directory and converted to virtual workspace
  - Consolidated IP parsing/formatting into `net.rs`
  - Ungated aggregator/pod_cache from `cfg(linux)` (enables macOS testing, 18 tests)
  - Dockerfile (multi-stage + local build targets)
  - `deploy/` directory: DaemonSet with RBAC, kind config, e2e test pods
  - `scripts/smoke-test.sh` (probe loading, 6 assertions)
  - `scripts/e2e-test.sh` (3 network modes, 9 assertions: hostNetwork, regular pod, Service DNAT)
  - `make smoke-test`, `make e2e-test`, `make docker-build` targets

### What's Not Yet Implemented

- `orb8-agent/src/config.rs` (env var configuration) - Phase 4
- `deploy/kustomization.yaml` (production deployment overlay) - Phase 4
- Prometheus `/metrics` endpoint - Phase 5
- `orb8-util/` crate (shared userspace utilities) - Phase 6
- Pipeline architecture (`pipeline/`, `export/`, `filter.rs`, `event.rs`) - Phase 6
- `orb8-server` full implementation - Phase 7
- `orb8-probes/src/syscall_probe.rs` - Phase 8
- `orb8-probes/src/gpu_probe.rs` - Phase 9

### Known Network Limitations

- **Same-node pod traffic**: Invisible on eth0 (stays on veth pairs). Would require bridge/veth probe attachment.
- **hostNetwork pods**: Share node IP. Last-indexed hostNetwork pod "wins" attribution for all node IP traffic.
- **Service ClusterIP**: DNAT applied before TC hook, so flows show backend pod IP, not the Service address.

---

## References

### eBPF & Rust
- [eBPF Documentation](https://ebpf.io/)
- [aya Book](https://aya-rs.dev/book/)
- [gRPC in Rust (tonic)](https://github.com/hyperium/tonic)

### Kubernetes
- [Kubernetes API](https://kubernetes.io/docs/reference/)
- [Linux cgroup v2](https://www.kernel.org/doc/html/latest/admin-guide/cgroup-v2.html)

### Further Reading
- [Cilium Hubble](https://docs.cilium.io/en/stable/observability/hubble/)
- [Pixie](https://docs.px.dev/about-pixie/what-is-pixie/)
- [Tetragon](https://tetragon.io/)

### GPU
- [NVIDIA DCGM](https://developer.nvidia.com/dcgm)

---

**Document Version**: 2.0
**Last Updated**: 2026-02-13
**Authors**: orb8 maintainers
