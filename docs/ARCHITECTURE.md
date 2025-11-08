# orb8 Architecture

This document provides a technical overview of orb8's architecture, design decisions, and implementation details.

## Table of Contents

1. [System Overview](#system-overview)
2. [Core Components](#core-components)
3. [eBPF Infrastructure](#ebpf-infrastructure)
4. [Kubernetes Integration](#kubernetes-integration)
5. [GPU Telemetry](#gpu-telemetry)
6. [Data Flow](#data-flow)
7. [Performance Considerations](#performance-considerations)
8. [Security Model](#security-model)

## System Overview

orb8 is a distributed observability system consisting of:

```
┌─────────────────────────────────────────────────────┐
│                  Kubernetes Cluster                  │
│  ┌─────────────────────────────────────────────┐   │
│  │              orb8 DaemonSet                  │   │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────┐  │   │
│  │  │ Node 1   │  │ Node 2   │  │ Node N   │  │   │
│  │  │          │  │          │  │          │  │   │
│  │  │ eBPF     │  │ eBPF     │  │ eBPF     │  │   │
│  │  │ Probes   │  │ Probes   │  │ Probes   │  │   │
│  │  └─────┬────┘  └─────┬────┘  └─────┬────┘  │   │
│  │        │             │             │        │   │
│  │        └─────────────┼─────────────┘        │   │
│  │                      │                      │   │
│  │              ┌───────▼────────┐             │   │
│  │              │  Aggregator    │             │   │
│  │              │  Service       │             │   │
│  │              └───────┬────────┘             │   │
│  └──────────────────────┼──────────────────────┘   │
│                         │                          │
│                  ┌──────▼──────┐                   │
│                  │ Prometheus  │                   │
│                  │  Exporter   │                   │
│                  └─────────────┘                   │
└─────────────────────────────────────────────────────┘
```

## Core Components

### 1. CLI (Command Line Interface)

**Location**: `src/main.rs`

The CLI provides user interaction via `clap`:

```rust
orb8 trace network --namespace ml-training
orb8 trace gpu --pod pytorch-job-123
orb8 export --format prometheus
```

**Responsibilities**:
- Parse user commands
- Configure logging and tracing
- Initialize runtime components
- Display TUI dashboard (future)

### 2. eBPF Probe Manager

**Location**: `src/ebpf/`

Manages eBPF program lifecycle using the `aya` library.

**Components**:
- **Loader**: Dynamically loads eBPF programs into the kernel
- **Map Manager**: Manages eBPF maps for data exchange
- **Event Handler**: Processes events from ring buffers
- **Probe Registry**: Tracks active probes per pod/namespace

**Key Features**:
- BTF-based CO-RE (Compile Once, Run Everywhere)
- Ring buffer for efficient event streaming
- Support for multiple probe types (kprobe, tracepoint, XDP)

### 3. Kubernetes Controller

**Location**: `src/k8s/`

Watches Kubernetes resources and orchestrates probe deployment.

**Responsibilities**:
- Pod/namespace auto-discovery via `kube-rs`
- CRD-based configuration management
- Label-based filtering
- Node affinity tracking

**Watch Loop**:
```rust
// Pseudo-code
for event in pod_watcher {
    match event {
        Added(pod) => deploy_probes(pod),
        Deleted(pod) => cleanup_probes(pod),
        Modified(pod) => update_probes(pod),
    }
}
```

### 4. Metrics Pipeline

**Location**: `src/metrics/`

Aggregates eBPF events and exports metrics.

**Components**:
- **Aggregator**: Collects events from all eBPF probes
- **Prometheus Exporter**: Exposes metrics at `/metrics`
- **Time-Series Store**: In-memory store for recent data
- **Alert Manager**: Threshold-based alerting (future)

### 5. TUI Dashboard

**Location**: `src/ui/`

Real-time terminal interface built with `ratatui`.

**Views**:
- Cluster overview
- Pod-level metrics
- Network flow graph
- GPU utilization charts

## eBPF Infrastructure

### Probe Types

#### 1. Network Flow Tracing

**eBPF Program Type**: `XDP` or `tc` (Traffic Control)

Captures packet metadata at the network interface level.

```c
// Pseudo eBPF code
SEC("xdp")
int trace_network_flow(struct xdp_md *ctx) {
    // Parse packet headers
    struct ethhdr *eth = data;
    struct iphdr *ip = data + sizeof(*eth);

    // Extract metadata
    struct flow_event event = {
        .src_ip = ip->saddr,
        .dst_ip = ip->daddr,
        .protocol = ip->protocol,
        .timestamp = bpf_ktime_get_ns(),
    };

    // Send to ring buffer
    bpf_ringbuf_output(&events, &event, sizeof(event), 0);
    return XDP_PASS;
}
```

**Collected Metrics**:
- Bytes sent/received
- Packet count
- Connection state
- DNS queries/responses

#### 2. System Call Monitoring

**eBPF Program Type**: `tracepoint` or `kprobe`

Monitors syscall patterns for anomaly detection.

```c
SEC("tracepoint/syscalls/sys_enter_*")
int trace_syscall(struct trace_event_raw_sys_enter *ctx) {
    // Record syscall metadata
    struct syscall_event event = {
        .pid = bpf_get_current_pid_tgid() >> 32,
        .syscall_id = ctx->id,
        .timestamp = bpf_ktime_get_ns(),
    };

    bpf_ringbuf_output(&events, &event, sizeof(event), 0);
    return 0;
}
```

#### 3. GPU Telemetry

**eBPF Program Type**: `uprobe` (user-space probe)

Hooks into CUDA runtime API calls.

**Target Functions**:
- `cudaMalloc` / `cudaFree` - Memory allocation tracking
- `cudaLaunchKernel` - Kernel execution tracing
- `cudaMemcpy` - Data transfer monitoring

```c
SEC("uprobe/cudaMalloc")
int trace_cuda_malloc(struct pt_regs *ctx) {
    // Extract allocation size
    size_t size = PT_REGS_PARM2(ctx);

    struct gpu_event event = {
        .event_type = GPU_ALLOC,
        .size = size,
        .timestamp = bpf_ktime_get_ns(),
    };

    bpf_ringbuf_output(&events, &event, sizeof(event), 0);
    return 0;
}
```

### eBPF Maps

**Event Ring Buffer**:
```rust
#[map]
static EVENTS: RingBuf = RingBuf::with_byte_size(256 * 1024, 0);
```

**Pod Metadata Map**:
```rust
#[map]
static POD_INFO: HashMap<u32, PodMetadata> = HashMap::with_max_entries(10000, 0);
```

## Kubernetes Integration

### Resource Discovery

**CRD Definition** (future):
```yaml
apiVersion: orb8.io/v1alpha1
kind: TraceConfig
metadata:
  name: ml-workload-trace
spec:
  namespace: ml-training
  podSelector:
    matchLabels:
      app: pytorch
  probes:
    - type: network
      enabled: true
    - type: gpu
      enabled: true
      config:
        sampleRate: 1000  # ms
```

### DaemonSet Deployment

orb8 runs as a privileged DaemonSet to access kernel facilities:

```yaml
apiVersion: apps/v1
kind: DaemonSet
metadata:
  name: orb8
spec:
  selector:
    matchLabels:
      name: orb8
  template:
    spec:
      hostNetwork: true
      hostPID: true
      containers:
      - name: orb8
        image: orb8:latest
        securityContext:
          privileged: true
          capabilities:
            add: ["SYS_ADMIN", "NET_ADMIN", "BPF"]
```

## GPU Telemetry

### CUDA Interception

**Approach**: `LD_PRELOAD` + eBPF uprobes

1. Inject orb8 library into CUDA processes
2. Intercept CUDA API calls
3. Emit events to eBPF maps
4. Aggregate in userspace

### Metrics Collected

- **Utilization**: GPU compute % per pod
- **Memory**: Allocated, free, fragmentation
- **Kernel Execution**: Launch count, duration
- **Data Transfers**: Host-to-device, device-to-host bytes
- **Multi-GPU**: Cross-GPU traffic, load balancing

### Leak Detection Algorithm

```rust
fn detect_gpu_memory_leak(events: &[GpuEvent]) -> Option<MemoryLeak> {
    let mut allocations: HashMap<u64, Allocation> = HashMap::new();

    for event in events {
        match event.event_type {
            GpuEventType::Alloc => {
                allocations.insert(event.addr, Allocation {
                    size: event.size,
                    timestamp: event.timestamp,
                });
            }
            GpuEventType::Free => {
                allocations.remove(&event.addr);
            }
        }
    }

    // Detect leaks: allocations older than threshold
    let leaked = allocations.values()
        .filter(|a| a.timestamp < threshold)
        .sum::<u64>();

    if leaked > LEAK_THRESHOLD {
        Some(MemoryLeak { leaked_bytes: leaked })
    } else {
        None
    }
}
```

## Data Flow

### Event Pipeline

```
┌──────────┐     ┌──────────┐     ┌──────────┐     ┌──────────┐
│  eBPF    │────▶│   Ring   │────▶│  Rust    │────▶│  Metrics │
│  Probe   │     │  Buffer  │     │  Handler │     │  Store   │
└──────────┘     └──────────┘     └──────────┘     └──────────┘
                                                           │
                                                           ▼
                                                    ┌──────────┐
                                                    │Prometheus│
                                                    │ /metrics │
                                                    └──────────┘
```

### Latency Targets

| Stage | Target Latency |
|-------|----------------|
| eBPF event capture | <1ms |
| Ring buffer read | <5ms |
| Event aggregation | <10ms |
| Prometheus scrape | <100ms |

## Performance Considerations

### Overhead Budget

- **CPU**: <1% per node (target)
- **Memory**: <100MB per daemon
- **Network**: <1MB/s metrics traffic

### Optimizations

1. **Batching**: Aggregate events before processing
2. **Sampling**: Sample high-frequency events (e.g., network packets)
3. **Filtering**: Filter in eBPF kernel space, not userspace
4. **Zero-Copy**: Use ring buffers to avoid memory copies

### Benchmarking

```bash
cargo bench --bench ebpf_overhead
cargo bench --bench network_throughput
```

## Security Model

### Capabilities Required

- `CAP_BPF`: Load eBPF programs
- `CAP_NET_ADMIN`: Network packet inspection
- `CAP_SYS_ADMIN`: Access to tracepoints

### Threat Model

**Mitigations**:
- Verify eBPF program signatures
- Sandboxed eBPF execution (kernel enforced)
- RBAC for CRD access
- Encrypted metrics transport (TLS)

## Future Architecture

### Multi-Cluster Support (v1.2.0)

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│  Cluster 1  │────▶│   Control   │◀────│  Cluster 2  │
│    orb8     │     │    Plane    │     │    orb8     │
└─────────────┘     └─────────────┘     └─────────────┘
```

### Plugin System (v1.3.0+)

WebAssembly-based plugin system for custom probes:

```rust
trait Probe {
    fn init(&mut self) -> Result<()>;
    fn handle_event(&self, event: &Event) -> Result<()>;
}
```

## References

- [eBPF Documentation](https://ebpf.io/)
- [aya Book](https://aya-rs.dev/)
- [Kubernetes API](https://kubernetes.io/docs/reference/)
- [CUDA Runtime API](https://docs.nvidia.com/cuda/cuda-runtime-api/)
