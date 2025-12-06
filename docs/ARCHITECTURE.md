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

orb8 is a **dual-mode observability platform** that combines:
- **Cluster Mode**: Always-on DaemonSet-based monitoring (Pixie-style)
- **Standalone Mode**: On-demand CLI tracing (kubectl-trace-style)

Both modes leverage **eBPF probes** written entirely in **Rust** using the aya framework for kernel-level telemetry with minimal overhead.

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

orb8 is organized as a **Cargo workspace** with multiple crates.

> **Note**: The structure below shows the **target architecture**. See the "Current Implementation Status" section at the end of this document for what is actually implemented in each phase.

```
orb8/
├── Cargo.toml                        # Workspace definition
│
├── orb8-probes/                      # eBPF probes (kernel space)
│   ├── Cargo.toml
│   ├── src/
│   │   ├── network_probe.rs          # Network flow tracing
│   │   ├── syscall_probe.rs          # System call monitoring
│   │   └── gpu_probe.rs              # GPU telemetry
│   └── build.rs                      # eBPF compilation
│
├── orb8-common/                      # Shared types
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── events.rs                 # Event definitions (shared kernel/user)
│       └── types.rs                  # Common data structures
│
├── orb8-agent/                       # Node agent (DaemonSet)
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs
│       ├── probe_loader.rs           # Load eBPF probes
│       ├── collector.rs              # Poll ring buffers
│       ├── enricher.rs               # Add pod metadata to events
│       ├── aggregator.rs             # Time-series aggregation
│       ├── api_server.rs             # gRPC server
│       ├── prom_exporter.rs          # Prometheus metrics
│       └── k8s/
│           ├── mod.rs
│           ├── watcher.rs            # Watch pod lifecycle
│           └── cgroup.rs             # cgroup ID resolution
│
├── orb8-server/                      # Central API server
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs
│       ├── api.rs                    # gRPC service implementation
│       ├── aggregator.rs             # Cluster-wide aggregation
│       └── query.rs                  # Query routing to agents
│
├── orb8-cli/                         # CLI tool
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs
│       ├── commands/
│       │   ├── mod.rs
│       │   ├── trace.rs              # Tracing commands
│       │   ├── query.rs              # Query commands
│       │   └── dashboard.rs          # TUI dashboard
│       ├── client.rs                 # gRPC client
│       └── standalone.rs             # Standalone mode (direct eBPF)
│
├── orb8-proto/                       # Protocol definitions
│   ├── Cargo.toml
│   ├── build.rs
│   └── proto/
│       └── orb8.proto                # gRPC service definitions
│
├── tests/
│   ├── integration/                  # End-to-end tests
│   └── fixtures/                     # Test manifests
│
├── docs/
│   ├── ARCHITECTURE.md               # This file
│   └── ROADMAP.md                    # Development roadmap
│
├── deploy/
│   ├── daemonset.yaml                # Agent DaemonSet
│   ├── server.yaml                   # Central server deployment
│   └── rbac.yaml                     # RBAC configuration
│
├── .lima/                            # macOS development VM
│   └── orb8-dev.yaml
│
└── scripts/
    └── setup-lima.sh
```

### Workspace Dependencies

The workspace crates have the following dependency graph:

```
orb8-cli ─────┐
              ├──> orb8-proto ──> orb8-common
orb8-server ──┤
              │
orb8-agent ───┴──> orb8-common <─── orb8-probes
```

### Distribution

orb8 crates are distributed via multiple channels:

**crates.io** (Rust library/binary distribution):
- `orb8` - Root crate re-exporting `orb8-common` and `orb8-cli` as optional features
- `orb8-common` - Shared types between eBPF probes and userspace
- `orb8-cli` - CLI command definitions (library)
- `orb8-agent` - Node agent binary (`cargo install orb8-agent`, Linux-only)

**Not on crates.io**:
- `orb8-probes` - eBPF bytecode compiled for `bpfel-unknown-none` target; embedded in `orb8-agent` binary
- `orb8-server` - Central API server (stub, Phase 4)
- `orb8-proto` - gRPC protocol definitions (stub, Phase 4)

**Container Images** (planned):
- `ghcr.io/ignoramuss/orb8-agent` - For Kubernetes DaemonSet deployment
- `ghcr.io/ignoramuss/orb8-server` - For central server deployment

**Usage**:
```bash
# Add as Rust dependency
cargo add orb8

# Install agent binary (Linux only)
cargo install orb8-agent

# Kubernetes deployment (future)
kubectl apply -f https://raw.githubusercontent.com/Ignoramuss/orb8/main/deploy/
```

---

## Operating Modes

orb8 supports two distinct operating modes, selectable via CLI flags.

### Mode 1: Cluster Mode (Platform)

**Use case**: Continuous, cluster-wide observability

```bash
# Install infrastructure (one-time)
kubectl apply -f deploy/

# Use CLI to query
orb8 --mode=cluster query pods --namespace ml-training
orb8 --mode=cluster trace network --pod nginx-xyz --duration 60s
```

**Architecture**:
- DaemonSet runs `orb8-agent` on every node
- Central `orb8-server` aggregates data
- CLI connects to server via gRPC
- Always-on monitoring with historical data

**Deployment**:
```yaml
apiVersion: apps/v1
kind: DaemonSet
metadata:
  name: orb8-agent
spec:
  selector:
    matchLabels:
      app: orb8-agent
  template:
    spec:
      hostNetwork: true
      hostPID: true
      containers:
      - name: agent
        image: orb8/agent:latest
        securityContext:
          privileged: true
          capabilities:
            add: ["SYS_ADMIN", "SYS_RESOURCE", "NET_ADMIN"]
```

### Mode 2: Standalone Mode (On-Demand)

**Use case**: Ad-hoc tracing without cluster installation

```bash
# No installation required
orb8 --mode=standalone trace network --node worker-1 --duration 30s
```

**Architecture**:
- CLI directly SSH/kubectl-exec to target node
- Temporarily loads eBPF probes
- Collects data locally
- Cleans up on exit
- No DaemonSet or server required

**How it works**:
1. CLI uses `kubectl exec` or SSH to access node
2. Transfers probe binaries to `/tmp/orb8/`
3. Loads probes using aya
4. Streams events back to CLI
5. Unloads probes and cleans up

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

**Solution**: cgroup v2 ID extraction + user-space mapping

### Step 1: Extract cgroup ID in eBPF

eBPF probes call `bpf_get_current_cgroup_id()` helper to obtain a unique 64-bit cgroup ID for the task.

### Step 2: Map cgroup ID to Pod (User-Space)

Agent's CgroupResolver:
- Constructs cgroup filesystem path from pod UID and container ID
- Handles runtime-specific path formats (containerd, CRI-O, Docker)
- Supports all QoS classes (Guaranteed, Burstable, BestEffort)
- Returns cgroup inode number as unique identifier

Agent's PodWatcher:
- Watches Kubernetes API for pod events
- Extracts pod UID, namespace, name, and container IDs
- Resolves cgroup ID for each container
- Maintains shared eBPF map: `cgroup_id → PodMetadata`
- Implements reconnection on watch failure

### Step 3: Enrich Events with Pod Metadata

EventEnricher looks up cgroup ID in the metadata map and attaches namespace, pod name, and container name to each event.

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
3. [KERNEL] Probe extracts: cgroup_id=12345, src_ip, dst_ip, bytes
        ↓
4. [KERNEL] Writes to FLOW_EVENTS ring buffer (shared memory)
        ↓
5. [USER] EventCollector.poll_events() reads ring buffer (async loop)
        ↓
6. [USER] Deserializes into NetworkFlowEvent struct
        ↓
7. [USER] EventEnricher looks up cgroup_id=12345 in POD_METADATA map
        ↓
8. [USER] Finds: pod=nginx-xyz, namespace=production, container=nginx
        ↓
9. [USER] Creates EnrichedNetworkFlow with K8s context
        ↓
10. [USER] Aggregator updates time-series:
    - network_bytes{pod="nginx-xyz",namespace="production",direction="egress"} += bytes
        ↓
11. [USER] PrometheusExporter exposes metric at :9091/metrics
    [USER] Agent gRPC API makes available for queries at :9090
        ↓
12. [EXTERNAL] Prometheus scrapes metrics
    [EXTERNAL] CLI queries via gRPC → API Server → Agent
```

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

### Standalone Mode Execution

No installation required:

```bash
# CLI uses kubectl exec to access node
orb8 --mode=standalone trace network \
  --node worker-1 \
  --namespace production \
  --pod nginx-xyz \
  --duration 30s
```

**Under the hood**:
1. CLI finds node running target pod: `kubectl get pod nginx-xyz -o jsonpath='{.spec.nodeName}'`
2. Creates temporary pod on that node with host privileges
3. Transfers probe binaries
4. Loads probes, collects events
5. Streams results back to CLI
6. Cleans up temporary pod

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

This section documents what is actually implemented as of Phase 2 (v0.0.2).

### Implemented Components

| Component | Status | Files Implemented |
|-----------|--------|-------------------|
| `orb8-probes` | Phase 2 | `src/network_probe.rs` - Full IPv4/TCP/UDP/ICMP packet parsing, ring buffer |
| `orb8-common` | Phase 2 | `src/lib.rs` - `NetworkFlowEvent`, `PacketEvent`, protocol/direction constants |
| `orb8-agent` | Phase 2 | `main.rs`, `lib.rs`, `probe_loader.rs`, `aggregator.rs`, `grpc_server.rs`, `k8s_watcher.rs`, `pod_cache.rs`, `cgroup.rs` |
| `orb8-proto` | Phase 2 | `src/lib.rs`, `build.rs`, `proto/orb8.proto` - gRPC service definitions |
| `orb8-server` | Stub | `src/lib.rs` - placeholder (Phase 4) |
| `orb8-cli` | Phase 2 | `src/lib.rs`, `src/main.rs` - Basic structure |

### Phase Completion

- **Phase 0** (Foundation): ✅ Complete
- **Phase 1** (eBPF Infrastructure): ✅ Complete
  - eBPF probe compilation with aya-bpf
  - Probe loading and lifecycle management
  - Ring buffer kernel-to-userspace communication
  - Pre-flight system checks (kernel version, BTF, capabilities)
- **Phase 2** (Container Identification): ✅ Complete (MVP)
  - Kubernetes pod watcher (kube-rs)
  - Pod cache with cgroup ID mapping
  - Event enrichment with pod metadata
  - gRPC API server (port 9090)
  - Flow aggregation with 30s expiration

  > **Note**: `bpf_get_current_cgroup_id()` not available for TC classifiers.
  > Using K8s API-based enrichment with cgroup_id=0 fallback.

- **Phase 3** (Network MVP): 🔄 In Progress
  - ✅ Full packet parsing (5-tuple extraction)
  - ✅ gRPC QueryFlows, StreamEvents, GetStatus
  - ⏳ CLI trace network command
  - ⏳ Public release

### What's Not Yet Implemented

The following components exist in the target architecture but are not yet implemented:

- `orb8-probes/src/syscall_probe.rs` (Phase 6)
- `orb8-probes/src/gpu_probe.rs` (Phase 7)
- `orb8-agent/src/prom_exporter.rs` (Phase 5)
- `orb8-server` full implementation (Phase 4)
- `orb8-cli` full trace commands (Phase 3)

---

## References

- [eBPF Documentation](https://ebpf.io/)
- [aya Book](https://aya-rs.dev/book/)
- [Kubernetes API](https://kubernetes.io/docs/reference/)
- [Linux cgroup v2](https://www.kernel.org/doc/html/latest/admin-guide/cgroup-v2.html)
- [NVIDIA DCGM](https://developer.nvidia.com/dcgm)
- [gRPC in Rust](https://github.com/hyperium/tonic)
- [BTF and CO-RE](https://nakryiko.com/posts/bpf-portability-and-co-re/)

---

**Document Version**: 1.2
**Last Updated**: 2025-12-04
**Authors**: orb8 maintainers
