# orb8 Development Roadmap

**Granular Technical Implementation Plan**

This roadmap breaks down orb8 development into **dependency-based phases** without timeline constraints. Each phase includes technical implementation details, testing strategies, and clear success criteria.

---

## Table of Contents

1. [Roadmap Philosophy](#roadmap-philosophy)
2. [Phase 0: Foundation & Monorepo Setup](#phase-0-foundation--monorepo-setup)
3. [Phase 1: eBPF Infrastructure](#phase-1-ebpf-infrastructure)
4. [Phase 2: Container Identification](#phase-2-container-identification)
5. [Phase 3: Network Tracing MVP](#phase-3-network-tracing-mvp)
6. [Phase 4: Cluster Mode Architecture](#phase-4-cluster-mode-architecture)
7. [Phase 5: Metrics & Observability](#phase-5-metrics--observability)
8. [Phase 6: Syscall Monitoring](#phase-6-syscall-monitoring)
9. [Phase 7: GPU Telemetry (Research & MVP)](#phase-7-gpu-telemetry-research--mvp)
10. [Phase 8: Advanced Features](#phase-8-advanced-features)
11. [Future Enhancements](#future-enhancements)

---

## Roadmap Philosophy

### Principles

1. **One Thing Well**: Each phase delivers a complete, working feature
2. **No Timelines**: Phases complete when done, not by deadlines
3. **Research Spikes**: Explicitly budget time for uncertainty
4. **User Validation**: Get real users after each major phase
5. **Technical Debt**: Document it, don't accumulate it

### Success Criteria Per Phase

Each phase must meet:
- ✅ Feature complete (not prototype)
- ✅ Integration tests passing
- ✅ Documentation written
- ✅ Deployed to test cluster
- ✅ Validated with real workloads

### Phase Dependencies

```
Phase 0 (Foundation)
  ↓
Phase 1 (eBPF Infrastructure) ←─────┐
  ↓                                  │
Phase 2 (Container ID)               │
  ↓                                  │
Phase 3 (Network MVP) ←─ User Validation
  ↓                                  │
Phase 4 (Cluster Mode)               │
  ↓                                  │
Phase 5 (Metrics) ←────── User Validation
  ↓                                  │
Phase 6 (Syscall) ─────────────────┘
  ↓
Phase 7 (GPU) ←──── Research Spike
  ↓
Phase 8 (Advanced)
```

---

## Phase 0: Foundation & Monorepo Setup

**Goal**: Establish Cargo workspace, development environment, and CI/CD

**Status**: 🚧 IN PROGRESS (90% complete)

### Tasks

#### 0.1: Cargo Workspace Structure

- [x] Root `Cargo.toml` with workspace members
- [x] `orb8-probes/` crate skeleton
- [x] `orb8-common/` crate with shared types
- [x] `orb8-agent/` crate skeleton
- [x] `orb8-server/` crate skeleton
- [x] `orb8-cli/` crate skeleton
- [x] `orb8-proto/` crate skeleton

**Deliverable**:
```bash
cargo build --workspace  # All crates compile
```

#### 0.2: Development Environment

- [x] Lima/QEMU VM configuration (`.lima/orb8-dev.yaml`)
- [x] Makefile with `make magic`, `make dev`, `make shell`
- [x] Linux kernel 5.15+ with BTF enabled
- [x] aya dependencies and build toolchain

**Deliverable**:
```bash
make dev    # VM ready with all tools
make shell  # Can enter VM and run cargo build
```

#### 0.3: CI/CD Pipeline

- [x] GitHub Actions workflow for `cargo test`
- [x] GitHub Actions workflow for `cargo clippy`
- [x] GitHub Actions workflow for `cargo fmt --check`
- [ ] Container image builds (agent, server, CLI)
- [ ] Multi-arch builds (amd64, arm64)

**Deliverable**: PR checks pass before merge

#### 0.4: Project Documentation

- [x] README.md with project overview
- [x] ARCHITECTURE.md (detailed technical design)
- [x] ROADMAP.md (this file)
- [x] DEVELOPMENT.md (dev setup guide)
- [ ] CONTRIBUTING.md (contribution guidelines)
- [x] LICENSE (Apache 2.0)

**Deliverable**: New contributors can onboard in <30 minutes

---

## Phase 1: eBPF Infrastructure

**Goal**: Load, attach, and manage eBPF probes written in Rust using aya

**Dependencies**: Phase 0

**Estimated Effort**: 2-3 weeks of focused development

### Tasks

#### 1.1: aya-bpf Setup

**Files**: `orb8-probes/`

- [ ] Create `orb8-probes/Cargo.toml` with aya-bpf dependencies
- [ ] Create `orb8-probes/build.rs` for eBPF compilation
- [ ] Verify eBPF programs compile to `.bpf.o` format
- [ ] Test on kernel 5.15+ with BTF enabled

**Implementation**:
```rust
// orb8-probes/build.rs
use aya_bpf_compiler::*;

fn main() {
    build_ebpf([
        "src/network_probe.rs",
    ])
    .target("bpfel-unknown-none")
    .compile()
    .unwrap();
}
```

**Success Criteria**:
- ✅ `cargo build -p orb8-probes` produces `.bpf.o` files
- ✅ Files are valid eBPF ELF objects (verify with `llvm-objdump`)

#### 1.2: Minimal "Hello World" Probe

**Files**: `orb8-probes/src/network_probe.rs`

- [ ] Create skeleton tc probe that logs "Hello from eBPF"
- [ ] Use aya-log-ebpf for logging
- [ ] Attach to `lo` (loopback) interface for testing
- [ ] Verify logs appear via `/sys/kernel/debug/tracing/trace_pipe`

**Implementation**:
```rust
#![no_std]
#![no_main]

use aya_bpf::{macros::classifier, programs::TcContext, bindings::TC_ACT_OK};
use aya_log_ebpf::info;

#[classifier]
pub fn network_probe(ctx: TcContext) -> i32 {
    info!(&ctx, "Hello from eBPF! packet_len={}", ctx.len());
    TC_ACT_OK
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
```

**Success Criteria**:
- ✅ Probe loads without verifier errors
- ✅ Logs appear when pinging localhost
- ✅ No kernel panics or oops

#### 1.3: Probe Loader (User-Space)

**Files**: `orb8-agent/src/probe_loader.rs`

- [ ] Create `ProbeManager` struct
- [ ] Implement `load_probe()` using aya library
- [ ] Implement `attach_tc()` for network interfaces
- [ ] Implement `unload_all()` for cleanup
- [ ] Implement pre-flight validation (before loading any probes):
  - Check kernel version >= 5.8 (`uname -r`)
  - Verify BTF availability (`/sys/kernel/btf/vmlinux` exists)
  - Validate required capabilities (CAP_BPF, CAP_NET_ADMIN, CAP_SYS_ADMIN)
  - Test compile a trivial probe to verify toolchain
- [ ] Handle errors gracefully (verifier failures, permissions)
- [ ] Graceful degradation strategy:
  - If network probe fails, continue with syscall probe only
  - If all probes fail, exit with clear error and remediation steps
- [ ] Implement diagnostics command: `orb8 diagnose`
  - Check kernel version, BTF, capabilities
  - Attempt to load test probe
  - Report status and suggest fixes

**Implementation**:
```rust
// orb8-agent/src/probe_loader.rs
use aya::{Bpf, programs::{Tc, TcAttachType}};

pub struct ProbeManager {
    bpf: Bpf,
}

impl ProbeManager {
    pub fn load_network_probe() -> Result<Self> {
        let mut bpf = Bpf::load_file("network_probe.bpf.o")?;

        let program: &mut Tc = bpf
            .program_mut("network_probe")
            .unwrap()
            .try_into()?;

        program.load()?;
        program.attach("lo", TcAttachType::Ingress)?;

        Ok(Self { bpf })
    }

    pub fn unload(self) {
        // aya automatically detaches on drop
    }
}
```

**Success Criteria**:
- ✅ Pre-flight checks pass on supported kernels (5.8+)
- ✅ Pre-flight checks fail gracefully on unsupported kernels with helpful errors
- ✅ `orb8-agent` can load probe
- ✅ Probe persists until agent exits
- ✅ Clean unload without leaking eBPF resources
- ✅ `orb8 diagnose` command provides actionable troubleshooting info

#### 1.4: eBPF Maps - Ring Buffer

**Files**: `orb8-probes/src/network_probe.rs`, `orb8-agent/src/collector.rs`

- [ ] Define ring buffer in eBPF probe
- [ ] Write test events from eBPF → ring buffer
- [ ] Poll ring buffer from user-space
- [ ] Deserialize events into Rust structs

**Implementation (eBPF side)**:
```rust
use aya_bpf::{macros::map, maps::RingBuf};

#[repr(C)]
#[derive(Clone, Copy)]
struct TestEvent {
    packet_len: u32,
    timestamp: u64,
}

#[map]
static EVENTS: RingBuf = RingBuf::with_byte_size(256 * 1024, 0);

#[classifier]
pub fn network_probe(ctx: TcContext) -> i32 {
    let event = TestEvent {
        packet_len: ctx.len(),
        timestamp: unsafe { bpf_ktime_get_ns() },
    };

    EVENTS.output(&event, 0);
    TC_ACT_OK
}
```

**Implementation (User-space side)**:
```rust
// orb8-agent/src/collector.rs
use aya::maps::RingBuf;

pub struct EventCollector {
    ring_buf: RingBuf<TestEvent>,
}

impl EventCollector {
    pub fn poll(&mut self) -> Vec<TestEvent> {
        let mut events = Vec::new();
        while let Some(data) = self.ring_buf.next() {
            let event: TestEvent = unsafe {
                std::ptr::read(data.as_ptr() as *const _)
            };
            events.push(event);
        }
        events
    }
}
```

**Success Criteria**:
- ✅ Events flow from kernel → user space
- ✅ No data corruption
- ✅ High-throughput stress test (1M+ events/sec)

#### 1.5: Testing Infrastructure

- [ ] Unit tests for probe loader
- [ ] Integration test: load probe, send traffic, collect events
- [ ] Benchmark: measure overhead of empty probe

**Test**:
```rust
#[test]
fn test_probe_lifecycle() {
    let manager = ProbeManager::load_network_probe().unwrap();

    // Send traffic via loopback
    std::process::Command::new("ping")
        .args(["-c", "10", "127.0.0.1"])
        .output()
        .unwrap();

    // Should have events
    let collector = EventCollector::new(&manager);
    let events = collector.poll();
    assert!(events.len() >= 10);

    // Cleanup
    drop(manager);
}
```

**Phase 1 Deliverables**

✅ eBPF probes compile with aya-bpf
✅ User-space agent loads and attaches probes
✅ Ring buffer communication working
✅ Integration test passes on Linux VM
✅ Documentation: "eBPF Probe Development Guide"

---

## Phase 2: Container Identification

**Goal**: Map eBPF events to Kubernetes pods via cgroup IDs

**Dependencies**: Phase 1

**Estimated Effort**: 1-2 weeks

### Tasks

#### 2.1: cgroup ID Extraction in eBPF

**Files**: `orb8-probes/src/network_probe.rs`

- [ ] Call `bpf_get_current_cgroup_id()` in probe
- [ ] Include cgroup_id in event struct
- [ ] Verify cgroup ID is non-zero and stable

**Implementation**:
```rust
#[repr(C)]
struct NetworkEvent {
    cgroup_id: u64,  // NEW
    packet_len: u32,
    timestamp: u64,
}

#[classifier]
pub fn network_probe(ctx: TcContext) -> i32 {
    let cgroup_id = unsafe {
        aya_bpf::helpers::bpf_get_current_cgroup_id()
    };

    let event = NetworkEvent {
        cgroup_id,
        packet_len: ctx.len(),
        timestamp: unsafe { bpf_ktime_get_ns() },
    };

    EVENTS.output(&event, 0);
    TC_ACT_OK
}
```

**Success Criteria**:
- ✅ cgroup_id field populated
- ✅ Different containers have different cgroup IDs
- ✅ Same container has stable cgroup ID across events

#### 2.2: cgroup Filesystem Resolver

**Files**: `orb8-agent/src/k8s/cgroup.rs`

- [ ] Traverse `/sys/fs/cgroup/kubepods.slice/` hierarchy
- [ ] Map pod UID + container ID → cgroup inode
- [ ] Handle all QoS classes (Guaranteed, Burstable, BestEffort)
- [ ] Handle cgroup v2 vs v1 (prefer v2)
- [ ] Auto-detect container runtime from node (containerd, CRI-O, Docker)
- [ ] Support all runtime-specific cgroup path formats:
  - containerd: `cri-containerd-{id}.scope`
  - CRI-O: `crio-{id}.scope`
  - Docker: `docker-{id}.scope`
- [ ] Handle missing cgroups with retry logic (pod may not be ready yet)
- [ ] Add integration tests with all three container runtimes

**Implementation**:
```rust
// orb8-agent/src/k8s/cgroup.rs
use std::fs;
use std::os::unix::fs::MetadataExt;

pub struct CgroupResolver {
    cgroup_root: String,
}

impl CgroupResolver {
    pub fn get_pod_cgroup_id(
        &self,
        pod_uid: &str,
        container_id: &str,
    ) -> Result<u64> {
        let qos_classes = ["", "burstable-", "besteffort-"];

        for qos in qos_classes {
            let path = format!(
                "{}/kubepods.slice/kubepods-{}pod{}.slice/cri-containerd-{}.scope",
                self.cgroup_root,
                qos,
                pod_uid.replace("-", "_"),
                container_id
            );

            if let Ok(metadata) = fs::metadata(&path) {
                return Ok(metadata.ino());
            }
        }

        Err(Error::CgroupNotFound)
    }
}
```

**Success Criteria**:
- ✅ Resolves cgroup ID for running pods
- ✅ Handles all QoS classes
- ✅ Supports containerd, CRI-O, and Docker runtimes
- ✅ Auto-detects runtime without manual configuration
- ✅ Returns error for non-existent pods

#### 2.3: Kubernetes Pod Watcher

**Files**: `orb8-agent/src/k8s/watcher.rs`

- [ ] Watch all pods using kube-rs
- [ ] Extract pod UID, namespace, name, container IDs
- [ ] Resolve cgroup ID for each container
- [ ] Maintain in-memory map: `cgroup_id → PodMetadata`
- [ ] Handle pod lifecycle (Added, Modified, Deleted)

**Implementation**:
```rust
// orb8-agent/src/k8s/watcher.rs
use kube::{Api, Client, runtime::{watcher, WatchStreamExt}};
use k8s_openapi::api::core::v1::Pod;
use std::collections::HashMap;

pub struct PodWatcher {
    k8s_client: Client,
    cgroup_resolver: CgroupResolver,
    metadata_map: HashMap<u64, PodMetadata>,
}

impl PodWatcher {
    pub async fn watch(&mut self) -> Result<()> {
        let pods: Api<Pod> = Api::all(self.k8s_client.clone());
        let mut stream = watcher(pods, Default::default()).boxed();

        while let Some(event) = stream.try_next().await? {
            match event {
                Event::Applied(pod) => self.handle_pod_added(pod).await?,
                Event::Deleted(pod) => self.handle_pod_deleted(pod).await?,
                _ => {}
            }
        }

        Ok(())
    }

    async fn handle_pod_added(&mut self, pod: Pod) -> Result<()> {
        let pod_uid = pod.metadata.uid.unwrap();
        let namespace = pod.metadata.namespace.unwrap();
        let name = pod.metadata.name.unwrap();

        if let Some(status) = pod.status {
            for container in status.container_statuses.unwrap_or_default() {
                if let Some(container_id) = container.container_id {
                    let id = container_id.split("://").nth(1).unwrap();
                    let cgroup_id = self.cgroup_resolver
                        .get_pod_cgroup_id(&pod_uid, id)?;

                    self.metadata_map.insert(cgroup_id, PodMetadata {
                        namespace: namespace.clone(),
                        pod_name: name.clone(),
                        container_name: container.name.clone(),
                    });

                    info!("Mapped cgroup {} → {}/{}", cgroup_id, namespace, name);
                }
            }
        }

        Ok(())
    }
}
```

**Success Criteria**:
- ✅ Watches all pods in cluster
- ✅ Builds cgroup_id → pod mapping
- ✅ Updates map on pod lifecycle events
- ✅ Handles network failures gracefully

#### 2.3.1: Watch Reliability (Critical for Production)

**Purpose**: Ensure pod watcher recovers from disconnections without losing metadata

- [ ] Implement reconnection logic with exponential backoff
  - Initial retry: 1 second
  - Max backoff: 30 seconds
  - Retry indefinitely (never give up)
- [ ] Full resync on reconnect
  - List all pods via Kubernetes API
  - Rebuild entire `cgroup_id → pod` map
  - Log number of pods resynced
- [ ] Buffer events for unknown cgroups
  - Queue events for unknown cgroup IDs (up to 10 seconds)
  - Retry enrichment when metadata arrives
  - Discard after timeout to prevent memory leak
- [ ] Expose watch health metrics
  - `orb8_k8s_watch_connected` (gauge: 0 or 1)
  - `orb8_k8s_watch_reconnections_total` (counter)
  - `orb8_k8s_last_sync_timestamp` (gauge, Unix timestamp)
  - `orb8_k8s_pods_tracked` (gauge)

**Implementation Pattern**:
```rust
// Spawn watch with automatic reconnection
tokio::spawn(async move {
    let mut backoff = Duration::from_secs(1);

    loop {
        match pod_watcher.watch().await {
            Ok(_) => {
                warn!("Pod watch stream ended, reconnecting...");
                backoff = Duration::from_secs(1); // Reset backoff on clean exit
            }
            Err(e) => {
                error!("Pod watch failed: {}, reconnecting in {:?}", e, backoff);
                tokio::time::sleep(backoff).await;
                backoff = std::cmp::min(backoff * 2, Duration::from_secs(30));
            }
        }

        // Full resync before reconnecting
        if let Err(e) = pod_watcher.resync_all().await {
            error!("Resync failed: {}", e);
        }
    }
});
```

**Success Criteria**:
- ✅ Watch reconnects automatically after network failures
- ✅ All pods resynced within 5 seconds of reconnection
- ✅ No events permanently lost to "unknown" cgroup
- ✅ Metrics expose watch health status

#### 2.4: Event Enrichment

**Files**: `orb8-agent/src/enricher.rs`

- [ ] Look up cgroup_id in metadata map
- [ ] Enrich events with namespace, pod name, container name
- [ ] Handle unknown cgroup IDs gracefully

**Implementation**:
```rust
// orb8-agent/src/enricher.rs
pub struct EventEnricher {
    metadata_map: Arc<RwLock<HashMap<u64, PodMetadata>>>,
}

impl EventEnricher {
    pub fn enrich(&self, event: NetworkEvent) -> EnrichedEvent {
        let metadata_map = self.metadata_map.read().unwrap();

        if let Some(metadata) = metadata_map.get(&event.cgroup_id) {
            EnrichedEvent {
                namespace: metadata.namespace.clone(),
                pod_name: metadata.pod_name.clone(),
                container_name: metadata.container_name.clone(),
                packet_len: event.packet_len,
                timestamp: event.timestamp,
            }
        } else {
            // Unknown cgroup - might be host process
            EnrichedEvent {
                namespace: "unknown".to_string(),
                pod_name: format!("cgroup-{}", event.cgroup_id),
                container_name: "unknown".to_string(),
                packet_len: event.packet_len,
                timestamp: event.timestamp,
            }
        }
    }
}
```

**Success Criteria**:
- ✅ Events enriched with correct pod metadata
- ✅ Unknown cgroups handled without panic
- ✅ Thread-safe access to metadata map

#### 2.5: Integration Test

- [ ] Deploy test pod to Kubernetes
- [ ] Load probe, trigger traffic from test pod
- [ ] Verify events correctly attributed to test pod
- [ ] Verify namespace and pod name are correct

**Test Scenario**:
```bash
# Deploy nginx test pod
kubectl run test-nginx --image=nginx

# Run agent
orb8-agent &

# Generate traffic from pod
kubectl exec test-nginx -- curl localhost

# Verify events
# Expected: events with namespace=default, pod_name=test-nginx
```

**Phase 2 Deliverables**

✅ cgroup ID extraction in eBPF probes
✅ Kubernetes pod watcher
✅ cgroup → pod mapping
✅ Event enrichment with pod metadata
✅ Integration test with real Kubernetes pod
✅ Documentation: "Container Identification Design"

---

## Phase 3: Network Tracing MVP

**Goal**: Production-ready network flow tracing per pod

**Dependencies**: Phase 2

**Estimated Effort**: 2-3 weeks

### Tasks

#### 3.1: Full Network Event Structure

**Files**: `orb8-common/src/events.rs`, `orb8-probes/src/network_probe.rs`

- [ ] Define complete `NetworkFlowEvent` struct
- [ ] Extract src/dst IP, src/dst port, protocol
- [ ] Parse Ethernet, IP, TCP/UDP headers
- [ ] Handle malformed packets gracefully

**Implementation**:
```rust
// orb8-common/src/events.rs
#[repr(C)]
#[derive(Clone, Copy)]
pub struct NetworkFlowEvent {
    pub cgroup_id: u64,
    pub timestamp_ns: u64,
    pub src_ip: u32,      // IPv4 only for MVP
    pub dst_ip: u32,
    pub src_port: u16,
    pub dst_port: u16,
    pub protocol: u8,     // IPPROTO_TCP, IPPROTO_UDP
    pub bytes: u32,
    pub direction: u8,    // 0=ingress, 1=egress
}
```

**Implementation (eBPF)**:
```rust
// orb8-probes/src/network_probe.rs
use aya_bpf::bindings::{ethhdr, iphdr, tcphdr, udphdr};

fn try_network_probe(ctx: TcContext) -> Result<i32, ()> {
    let cgroup_id = unsafe { bpf_get_current_cgroup_id() };

    // Parse Ethernet header
    let eth = unsafe { ptr_at::<ethhdr>(&ctx, 0)? };
    if unsafe { (*eth).h_proto } != ETH_P_IP.to_be() {
        return Ok(TC_ACT_OK); // Not IPv4, skip
    }

    // Parse IP header
    let ip = unsafe { ptr_at::<iphdr>(&ctx, ETH_HLEN)? };
    let protocol = unsafe { (*ip).protocol };

    // Parse transport header
    let (src_port, dst_port) = match protocol {
        IPPROTO_TCP => {
            let tcp = unsafe { ptr_at::<tcphdr>(&ctx, ETH_HLEN + IP_HLEN)? };
            (unsafe { (*tcp).source }.to_be(), unsafe { (*tcp).dest }.to_be())
        },
        IPPROTO_UDP => {
            let udp = unsafe { ptr_at::<udphdr>(&ctx, ETH_HLEN + IP_HLEN)? };
            (unsafe { (*udp).source }.to_be(), unsafe { (*udp).dest }.to_be())
        },
        _ => (0, 0),
    };

    let event = NetworkFlowEvent {
        cgroup_id,
        timestamp_ns: unsafe { bpf_ktime_get_ns() },
        src_ip: unsafe { (*ip).saddr },
        dst_ip: unsafe { (*ip).daddr },
        src_port,
        dst_port,
        protocol,
        bytes: ctx.len(),
        direction: 0, // ingress
    };

    FLOW_EVENTS.output(&event, 0).map_err(|_| ())?;

    Ok(TC_ACT_OK)
}
```

**Success Criteria**:
- ✅ Correctly parses TCP and UDP packets
- ✅ Skips non-IP packets without errors
- ✅ Handles fragmented packets (or documents limitation)

#### 3.2: Multi-Interface Attachment

**Files**: `orb8-agent/src/probe_loader.rs`

- [ ] Discover all network interfaces (except loopback)
- [ ] Attach probe to each interface (ingress + egress)
- [ ] Handle interface hotplug (containers starting/stopping)

**Implementation**:
```rust
// orb8-agent/src/probe_loader.rs
use nix::net::if_::if_nameindex;

impl ProbeManager {
    pub fn attach_to_all_interfaces(&mut self) -> Result<()> {
        let interfaces = if_nameindex()?;

        for iface in interfaces {
            let name = iface.name().to_str().unwrap();

            // Skip loopback
            if name == "lo" {
                continue;
            }

            // Skip non-veth (only monitor container traffic)
            if !name.starts_with("veth") && !name.starts_with("eth") {
                continue;
            }

            self.attach_tc(name, TcAttachType::Ingress)?;
            self.attach_tc(name, TcAttachType::Egress)?;

            info!("Attached to interface {}", name);
        }

        Ok(())
    }
}
```

**Success Criteria**:
- ✅ Attaches to all veth interfaces
- ✅ Captures both ingress and egress traffic
- ✅ Doesn't break on interface churn

#### 3.3: Flow Aggregation

**Files**: `orb8-agent/src/aggregator.rs`

- [ ] Aggregate raw packet events into flows
- [ ] Flow key: (src_ip, dst_ip, src_port, dst_port, protocol)
- [ ] Track bytes sent/received per flow
- [ ] Time-window aggregation (e.g., 10-second buckets)

**Implementation**:
```rust
// orb8-agent/src/aggregator.rs
use std::collections::HashMap;
use std::time::{Duration, Instant};

#[derive(Hash, Eq, PartialEq)]
struct FlowKey {
    namespace: String,
    pod_name: String,
    src_ip: u32,
    dst_ip: u32,
    src_port: u16,
    dst_port: u16,
    protocol: u8,
}

struct FlowStats {
    bytes_sent: u64,
    bytes_received: u64,
    packets_sent: u64,
    packets_received: u64,
    first_seen: Instant,
    last_seen: Instant,
}

pub struct FlowAggregator {
    flows: HashMap<FlowKey, FlowStats>,
    window_duration: Duration,
}

impl FlowAggregator {
    pub fn add_event(&mut self, event: EnrichedNetworkFlow) {
        let key = FlowKey {
            namespace: event.namespace,
            pod_name: event.pod_name,
            src_ip: event.src_ip,
            dst_ip: event.dst_ip,
            src_port: event.src_port,
            dst_port: event.dst_port,
            protocol: event.protocol,
        };

        let stats = self.flows.entry(key).or_insert(FlowStats {
            bytes_sent: 0,
            bytes_received: 0,
            packets_sent: 0,
            packets_received: 0,
            first_seen: Instant::now(),
            last_seen: Instant::now(),
        });

        if event.direction == 1 {  // egress
            stats.bytes_sent += event.bytes as u64;
            stats.packets_sent += 1;
        } else {  // ingress
            stats.bytes_received += event.bytes as u64;
            stats.packets_received += 1;
        }

        stats.last_seen = Instant::now();
    }

    pub fn flush_expired_flows(&mut self) -> Vec<(FlowKey, FlowStats)> {
        let now = Instant::now();
        let mut expired = Vec::new();

        self.flows.retain(|key, stats| {
            if now.duration_since(stats.last_seen) > self.window_duration {
                expired.push((key.clone(), stats.clone()));
                false
            } else {
                true
            }
        });

        expired
    }
}
```

**Success Criteria**:
- ✅ Aggregates packets into flows
- ✅ Correctly separates ingress/egress
- ✅ Expires old flows to prevent memory leak

#### 3.3.1: Network Event Sampling (Critical for High-Traffic Pods)

**Purpose**: Prevent ring buffer overflow at high event rates

**Problem**: At 1M events/sec with 64-byte events, a 1MB ring buffer fills in ~16ms, causing severe event loss.

- [ ] Implement sampling for high-volume flows
  - Sample 1:10 for flows exceeding 10,000 packets/sec
  - Always capture first 10 packets of new flows (for connection establishment)
  - Always capture TCP SYN, FIN, RST packets (critical flow state)
  - Add sampling metadata to events for accurate extrapolation
- [ ] Make ring buffer size configurable
  - Environment variable: `ORB8_RING_BUFFER_SIZE` (default: 1MB, max: 32MB)
  - Per-probe configuration (network vs syscall may need different sizes)
  - Validate size is power of 2 (eBPF requirement)
- [ ] Expose ring buffer health metrics
  - `orb8_ring_buffer_drops_total{probe="network"}` (counter)
  - `orb8_ring_buffer_utilization{probe="network"}` (gauge, 0.0-1.0)
  - `orb8_ring_buffer_size_bytes{probe="network"}` (gauge)
  - `orb8_ring_buffer_events_total{probe="network"}` (counter)
- [ ] Implement backpressure signaling
  - When ring buffer >90% full, signal eBPF probe to increase sampling
  - Adaptive sampling rate based on buffer pressure
  - Log warnings when sustained high pressure detected

**Implementation**:
```rust
// In eBPF probe
if ring_buffer_utilization() > 0.9 {
    // Drop 9 out of 10 events during high pressure
    if bpf_get_prandom_u32() % 10 != 0 {
        return TC_ACT_OK;  // Drop event
    }
}
```

**Success Criteria**:
- ✅ Ring buffer drops <0.1% under normal load (1M events/sec)
- ✅ Sampling preserves TCP state transitions (SYN, FIN, RST)
- ✅ Metrics expose ring buffer health
- ✅ Buffer size configurable without recompilation

#### 3.4: CLI Output Formatting

**Files**: `orb8-cli/src/commands/trace.rs`

- [ ] Display flows in human-readable table
- [ ] Format IP addresses (u32 → dotted decimal)
- [ ] Sort by bytes descending
- [ ] Support JSON output format

**Implementation**:
```rust
// orb8-cli/src/commands/trace.rs
use prettytable::{Table, Row, Cell};

pub async fn handle_trace_network(
    namespace: Option<String>,
    pod: Option<String>,
    duration: Duration,
) -> Result<()> {
    // Collect flows
    let flows = collect_flows(namespace, pod, duration).await?;

    // Display as table
    let mut table = Table::new();
    table.add_row(row![
        "NAMESPACE", "POD", "SRC", "DST", "PROTO", "BYTES", "PACKETS"
    ]);

    for flow in flows {
        table.add_row(row![
            flow.namespace,
            flow.pod_name,
            format_ip(flow.src_ip),
            format_ip(flow.dst_ip),
            format_proto(flow.protocol),
            flow.bytes_sent + flow.bytes_received,
            flow.packets_sent + flow.packets_received,
        ]);
    }

    table.printstd();

    Ok(())
}

fn format_ip(ip: u32) -> String {
    let octets = ip.to_be_bytes();
    format!("{}.{}.{}.{}", octets[0], octets[1], octets[2], octets[3])
}
```

**Success Criteria**:
- ✅ Human-readable output
- ✅ JSON format for scripting
- ✅ Correct IP address formatting

#### 3.5: End-to-End Test

- [ ] Deploy multi-pod test scenario (client → server)
- [ ] Run orb8 network tracing
- [ ] Verify flows captured in both directions
- [ ] Verify byte counts match actual traffic

**Test Scenario**:
```bash
# Deploy client and server pods
kubectl run server --image=nginx
kubectl run client --image=curlimages/curl -- sh -c "while true; do curl http://server; sleep 1; done"

# Run orb8 tracing
orb8 trace network --namespace default --duration 30s

# Expected output:
# NAMESPACE  POD      SRC         DST         PROTO  BYTES   PACKETS
# default    client   10.0.1.5    10.0.1.6    TCP    15KB    50
# default    server   10.0.1.6    10.0.1.5    TCP    150KB   50
```

**Phase 3 Deliverables**

✅ Full network packet parsing (IP, TCP, UDP)
✅ Multi-interface attachment
✅ Flow aggregation
✅ CLI with human-readable output
✅ End-to-end test with real pods
✅ Documentation: "Network Tracing User Guide"
✅ **Public Release**: v0.2.0 - Network Tracing MVP

**User Validation Checkpoint**: Get 10-50 users to try network tracing

---

## Phase 4: Cluster Mode Architecture

**Goal**: DaemonSet deployment with central API server

**Dependencies**: Phase 3

**Estimated Effort**: 2-3 weeks

### Tasks

#### 4.1: gRPC Service Definition

**Files**: `orb8-proto/proto/orb8.proto`

- [ ] Define `OrbitService` with query RPCs
- [ ] Define message types (FlowQuery, FlowResponse, etc.)
- [ ] Generate Rust code with tonic

**Implementation**:
```protobuf
// orb8-proto/proto/orb8.proto
syntax = "proto3";

package orb8;

service OrbitService {
    rpc QueryFlows(FlowQuery) returns (FlowResponse);
    rpc StreamFlows(StreamRequest) returns (stream FlowEvent);
    rpc GetAgentStatus(StatusRequest) returns (StatusResponse);
}

message FlowQuery {
    string namespace = 1;
    optional string pod_name = 2;
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
    uint32 src_port = 5;
    uint32 dst_port = 6;
    string protocol = 7;
    uint64 bytes = 8;
    uint64 packets = 9;
    int64 timestamp_ns = 10;
}
```

**Build Script**:
```rust
// orb8-proto/build.rs
fn main() {
    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .compile(&["proto/orb8.proto"], &["proto"])
        .unwrap();
}
```

**Success Criteria**:
- ✅ Protobuf compiles without errors
- ✅ Rust code generated in `target/`

#### 4.2: Agent gRPC Server

**Files**: `orb8-agent/src/api_server.rs`

- [ ] Implement `OrbitService` trait
- [ ] Query local aggregator for flow data
- [ ] Filter by namespace/pod
- [ ] Handle time-range queries

**Implementation**:
```rust
// orb8-agent/src/api_server.rs
use orb8_proto::orbit_service_server::{OrbitService, OrbitServiceServer};
use orb8_proto::{FlowQuery, FlowResponse, NetworkFlow};
use tonic::{Request, Response, Status};

pub struct AgentApiServer {
    aggregator: Arc<RwLock<FlowAggregator>>,
}

#[tonic::async_trait]
impl OrbitService for AgentApiServer {
    async fn query_flows(
        &self,
        request: Request<FlowQuery>,
    ) -> Result<Response<FlowResponse>, Status> {
        let query = request.into_inner();
        let aggregator = self.aggregator.read().unwrap();

        let flows: Vec<NetworkFlow> = aggregator
            .get_flows()
            .filter(|f| {
                if let Some(ref ns) = query.namespace {
                    if f.namespace != *ns {
                        return false;
                    }
                }
                if let Some(ref pod) = query.pod_name {
                    if f.pod_name != *pod {
                        return false;
                    }
                }
                true
            })
            .map(|f| f.to_proto())
            .collect();

        Ok(Response::new(FlowResponse { flows }))
    }
}

pub async fn serve(aggregator: Arc<RwLock<FlowAggregator>>) -> Result<()> {
    let addr = "0.0.0.0:9090".parse()?;
    let server = AgentApiServer { aggregator };

    Server::builder()
        .add_service(OrbitServiceServer::new(server))
        .serve(addr)
        .await?;

    Ok(())
}
```

**Success Criteria**:
- ✅ gRPC server listens on port 9090
- ✅ Responds to QueryFlows RPC
- ✅ Filters work correctly

#### 4.3: Central API Server

**Files**: `orb8-server/src/main.rs`, `orb8-server/src/api.rs`

- [ ] Discover all agent pods via Kubernetes API
- [ ] Route queries to appropriate node agents
- [ ] Aggregate results from multiple agents
- [ ] Expose external gRPC API on port 8080

**Implementation**:
```rust
// orb8-server/src/api.rs
use kube::{Api, Client};
use k8s_openapi::api::core::v1::Pod;

pub struct CentralApiServer {
    k8s_client: Client,
}

impl CentralApiServer {
    async fn discover_agents(&self) -> Result<Vec<String>> {
        let pods: Api<Pod> = Api::namespaced(self.k8s_client.clone(), "orb8-system");

        let pod_list = pods.list(&Default::default()).await?;

        let agent_addrs: Vec<String> = pod_list
            .items
            .iter()
            .filter(|p| {
                p.metadata.labels.as_ref()
                    .and_then(|l| l.get("app"))
                    .map(|v| v == "orb8-agent")
                    .unwrap_or(false)
            })
            .filter_map(|p| {
                p.status.as_ref()
                    .and_then(|s| s.pod_ip.as_ref())
                    .map(|ip| format!("{}:9090", ip))
            })
            .collect();

        Ok(agent_addrs)
    }
}

#[tonic::async_trait]
impl OrbitService for CentralApiServer {
    async fn query_flows(
        &self,
        request: Request<FlowQuery>,
    ) -> Result<Response<FlowResponse>, Status> {
        let query = request.into_inner();

        // Get all agent addresses
        let agent_addrs = self.discover_agents().await
            .map_err(|e| Status::internal(format!("Agent discovery failed: {}", e)))?;

        // Query all agents in parallel
        let mut handles = Vec::new();
        for addr in agent_addrs {
            let query_clone = query.clone();
            let handle = tokio::spawn(async move {
                let mut client = OrbitServiceClient::connect(format!("http://{}", addr)).await?;
                client.query_flows(query_clone).await
            });
            handles.push(handle);
        }

        // Aggregate results
        let mut all_flows = Vec::new();
        for handle in handles {
            if let Ok(Ok(response)) = handle.await {
                all_flows.extend(response.into_inner().flows);
            }
        }

        Ok(Response::new(FlowResponse { flows: all_flows }))
    }
}
```

**Success Criteria**:
- ✅ Discovers all agent pods
- ✅ Queries agents in parallel
- ✅ Aggregates results correctly

#### 4.4: CLI Cluster Mode

**Files**: `orb8-cli/src/client.rs`

- [ ] Connect to central API server (auto-discover from kubeconfig)
- [ ] Send query via gRPC
- [ ] Display results

**Implementation**:
```rust
// orb8-cli/src/client.rs
pub struct ClusterClient {
    server_addr: String,
}

impl ClusterClient {
    pub fn from_kubeconfig() -> Result<Self> {
        // Discover orb8-server service
        let server_addr = "orb8-server.orb8-system.svc.cluster.local:8080".to_string();
        Ok(Self { server_addr })
    }

    pub async fn query_flows(
        &self,
        namespace: Option<String>,
        pod: Option<String>,
    ) -> Result<Vec<NetworkFlow>> {
        let mut client = OrbitServiceClient::connect(
            format!("http://{}", self.server_addr)
        ).await?;

        let request = FlowQuery {
            namespace,
            pod_name: pod,
            start_time_ns: None,
            end_time_ns: None,
        };

        let response = client.query_flows(request).await?;
        Ok(response.into_inner().flows)
    }
}
```

**Success Criteria**:
- ✅ CLI connects to central server
- ✅ Queries work end-to-end
- ✅ Auto-discovery from kubeconfig

#### 4.5: Kubernetes Manifests

**Files**: `deploy/`

- [ ] Namespace (`orb8-system`)
- [ ] ServiceAccount and RBAC
- [ ] DaemonSet for agents
- [ ] Deployment for central server
- [ ] Service for central server

**DaemonSet**:
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
    spec:
      hostNetwork: true
      hostPID: true
      serviceAccountName: orb8-agent
      containers:
      - name: agent
        image: orb8/agent:latest
        securityContext:
          privileged: true
        ports:
        - containerPort: 9090
          name: grpc
        - containerPort: 9091
          name: metrics
```

**Success Criteria**:
- ✅ `kubectl apply -f deploy/` succeeds
- ✅ Agents running on all nodes
- ✅ Central server accessible from CLI

**Phase 4 Deliverables**

✅ gRPC service definition
✅ Agent gRPC API server
✅ Central API server with agent discovery
✅ CLI cluster mode
✅ Kubernetes deployment manifests
✅ End-to-end cluster mode test
✅ Documentation: "Cluster Mode Deployment Guide"

---

## Phase 5: Metrics & Observability

**Goal**: Prometheus exporter, Grafana dashboards

**Dependencies**: Phase 4

**Estimated Effort**: 1-2 weeks

### Tasks

#### 5.1: Prometheus Exporter

**Files**: `orb8-agent/src/prom_exporter.rs`

- [ ] Expose `/metrics` endpoint on port 9091
- [ ] Export flow metrics as Prometheus gauges/counters
- [ ] Labels: namespace, pod, src_ip, dst_ip, protocol
- [ ] Export ring buffer health metrics:
  - `orb8_ring_buffer_drops_total{probe="network|syscall"}` (counter)
  - `orb8_ring_buffer_utilization{probe="network|syscall"}` (gauge, 0.0-1.0)
  - `orb8_ring_buffer_size_bytes{probe="network|syscall"}` (gauge)
  - `orb8_ring_buffer_events_total{probe="network|syscall"}` (counter)
- [ ] Export Kubernetes watch health metrics:
  - `orb8_k8s_watch_connected` (gauge: 0 or 1)
  - `orb8_k8s_watch_reconnections_total` (counter)
  - `orb8_k8s_last_sync_timestamp` (gauge, Unix timestamp)
  - `orb8_k8s_pods_tracked` (gauge)

**Implementation**:
```rust
// orb8-agent/src/prom_exporter.rs
use prometheus::{Registry, CounterVec, GaugeVec, Encoder, TextEncoder};
use warp::Filter;

pub struct PrometheusExporter {
    registry: Registry,
    flow_bytes: CounterVec,
    flow_packets: CounterVec,
    active_flows: GaugeVec,
}

impl PrometheusExporter {
    pub fn new() -> Self {
        let registry = Registry::new();

        let flow_bytes = CounterVec::new(
            Opts::new("orb8_flow_bytes_total", "Total bytes per flow"),
            &["namespace", "pod", "direction", "protocol"],
        ).unwrap();

        let flow_packets = CounterVec::new(
            Opts::new("orb8_flow_packets_total", "Total packets per flow"),
            &["namespace", "pod", "direction", "protocol"],
        ).unwrap();

        let active_flows = GaugeVec::new(
            Opts::new("orb8_active_flows", "Number of active flows"),
            &["namespace", "pod"],
        ).unwrap();

        registry.register(Box::new(flow_bytes.clone())).unwrap();
        registry.register(Box::new(flow_packets.clone())).unwrap();
        registry.register(Box::new(active_flows.clone())).unwrap();

        Self {
            registry,
            flow_bytes,
            flow_packets,
            active_flows,
        }
    }

    pub fn update_metrics(&self, flows: &[(FlowKey, FlowStats)]) {
        for (key, stats) in flows {
            self.flow_bytes
                .with_label_values(&[
                    &key.namespace,
                    &key.pod_name,
                    "egress",
                    &format_proto(key.protocol),
                ])
                .inc_by(stats.bytes_sent);

            self.flow_packets
                .with_label_values(&[
                    &key.namespace,
                    &key.pod_name,
                    "egress",
                    &format_proto(key.protocol),
                ])
                .inc_by(stats.packets_sent);
        }
    }

    pub async fn serve(&self) {
        let registry = self.registry.clone();

        let metrics_route = warp::path("metrics")
            .map(move || {
                let encoder = TextEncoder::new();
                let metric_families = registry.gather();
                let mut buffer = Vec::new();
                encoder.encode(&metric_families, &mut buffer).unwrap();
                String::from_utf8(buffer).unwrap()
            });

        warp::serve(metrics_route)
            .run(([0, 0, 0, 0], 9091))
            .await;
    }
}
```

**Success Criteria**:
- ✅ `/metrics` endpoint returns Prometheus format
- ✅ Metrics update in real-time
- ✅ Labels correctly populated

#### 5.2: Prometheus ServiceMonitor

**Files**: `deploy/servicemonitor.yaml`

- [ ] ServiceMonitor CRD for Prometheus Operator
- [ ] Scrape agent metrics on port 9091

**Implementation**:
```yaml
# deploy/servicemonitor.yaml
apiVersion: monitoring.coreos.com/v1
kind: ServiceMonitor
metadata:
  name: orb8-agent
  namespace: orb8-system
spec:
  selector:
    matchLabels:
      app: orb8-agent
  endpoints:
  - port: metrics
    interval: 30s
    path: /metrics
```

**Success Criteria**:
- ✅ Prometheus scrapes agents successfully
- ✅ Metrics visible in Prometheus UI

#### 5.3: Grafana Dashboards

**Files**: `deploy/grafana-dashboard.json`

- [ ] Network flow dashboard
- [ ] Top talkers (by bytes)
- [ ] Pod-to-pod communication graph
- [ ] Time-series of bytes/packets

**Dashboard Panels**:
1. **Top Pods by Egress Traffic** (bar chart)
2. **Network Bytes Over Time** (time series)
3. **Active Flows** (gauge)
4. **Protocol Breakdown** (pie chart)
5. **Pod Communication Matrix** (heatmap)

**Success Criteria**:
- ✅ Dashboard imports without errors
- ✅ Real-time data visualization
- ✅ Useful for debugging network issues

**Phase 5 Deliverables**

✅ Prometheus exporter on agents
✅ ServiceMonitor for auto-discovery
✅ Grafana dashboard
✅ Documentation: "Metrics and Monitoring Guide"
✅ **Public Release**: v0.3.0 - Cluster Mode with Metrics

**User Validation Checkpoint**: Get feedback on Prometheus integration

---

## Phase 6: Syscall Monitoring

**Goal**: System call tracing for security anomaly detection

**Dependencies**: Phase 1 (eBPF infra), Phase 2 (container ID)

**Estimated Effort**: 1-2 weeks

### Tasks

#### 6.1: Syscall Probe

**Files**: `orb8-probes/src/syscall_probe.rs`

- [ ] Attach to `tracepoint/raw_syscalls/sys_enter`
- [ ] Capture cgroup_id, PID, syscall ID
- [ ] Sampling: 1:100 for hot syscalls (read/write)

**Implementation**:
```rust
// orb8-probes/src/syscall_probe.rs
#![no_std]
#![no_main]

use aya_bpf::{
    macros::{tracepoint, map},
    maps::RingBuf,
    programs::TracePointContext,
};
use orb8_common::SyscallEvent;

#[map]
static SYSCALL_EVENTS: RingBuf = RingBuf::with_byte_size(512 * 1024, 0);

#[map]
static SAMPLE_RATE: aya_bpf::maps::HashMap<u32, u32> =
    aya_bpf::maps::HashMap::with_max_entries(1, 0);

#[tracepoint(name = "syscall_probe")]
pub fn syscall_probe(ctx: TracePointContext) -> u32 {
    match try_syscall_probe(ctx) {
        Ok(_) => 0,
        Err(_) => 1,
    }
}

fn try_syscall_probe(ctx: TracePointContext) -> Result<(), ()> {
    let syscall_id: i64 = unsafe { ctx.read_at(8)? };

    // Sample hot syscalls
    if is_hot_syscall(syscall_id as u32) {
        // Only trace 1 in 100
        if unsafe { bpf_get_prandom_u32() } % 100 != 0 {
            return Ok(());
        }
    }

    let cgroup_id = unsafe { bpf_get_current_cgroup_id() };
    let pid_tgid = unsafe { bpf_get_current_pid_tgid() };
    let pid = (pid_tgid >> 32) as u32;

    let event = SyscallEvent {
        cgroup_id,
        pid,
        syscall_id: syscall_id as u32,
        timestamp_ns: unsafe { bpf_ktime_get_ns() },
    };

    SYSCALL_EVENTS.output(&event, 0).map_err(|_| ())?;

    Ok(())
}

fn is_hot_syscall(id: u32) -> bool {
    matches!(id, 0 | 1 | 2 | 3)  // read, write, open, close
}
```

**Success Criteria**:
- ✅ Captures syscalls without overwhelming ring buffer
- ✅ Sampling reduces overhead on hot paths
- ✅ Rare syscalls (execve, ptrace) always traced

#### 6.2: Anomaly Detection

**Files**: `orb8-agent/src/syscall_analyzer.rs`

- [ ] Baseline normal syscall patterns per pod
- [ ] Detect anomalies (unusual syscalls, frequency spikes)
- [ ] Alert on suspicious behavior

**Implementation**:
```rust
// orb8-agent/src/syscall_analyzer.rs
use std::collections::HashMap;

pub struct SyscallAnalyzer {
    baselines: HashMap<String, SyscallBaseline>,
}

struct SyscallBaseline {
    syscall_histogram: HashMap<u32, u64>,
    alert_threshold: u64,
}

impl SyscallAnalyzer {
    pub fn analyze(&mut self, pod: &str, event: SyscallEvent) -> Option<Alert> {
        let baseline = self.baselines.entry(pod.to_string())
            .or_insert_with(SyscallBaseline::new);

        baseline.syscall_histogram
            .entry(event.syscall_id)
            .and_modify(|count| *count += 1)
            .or_insert(1);

        // Detect anomalies
        if is_dangerous_syscall(event.syscall_id) {
            return Some(Alert::DangerousSyscall {
                pod: pod.to_string(),
                syscall: syscall_name(event.syscall_id),
            });
        }

        None
    }
}

fn is_dangerous_syscall(id: u32) -> bool {
    matches!(id,
        101 |  // ptrace
        139 |  // syslog
        165 |  // mount
        304    // open_by_handle_at
    )
}
```

**Success Criteria**:
- ✅ Baselines built for normal pods
- ✅ Alerts generated for anomalies
- ✅ Low false positive rate (<5%)

**Phase 6 Deliverables**

✅ Syscall tracing probe
✅ Sampling to reduce overhead
✅ Anomaly detection algorithm
✅ CLI command for syscall tracing
✅ Documentation: "Syscall Monitoring Guide"

---

## Phase 7: GPU Telemetry (Research & MVP)

**Goal**: Per-pod GPU utilization and memory tracking

**Dependencies**: Phase 2 (container ID)

**Estimated Effort**: 3-4 weeks (including research spike)

### Tasks

#### 7.1: Research Spike - GPU Approaches

**Duration**: 1 week

- [ ] **Option A: DCGM Integration**
  - Deploy DCGM exporter in test cluster
  - Validate GPU → pod mapping via device plugin
  - Measure overhead and accuracy
  - **Deliverable**: Feasibility report

- [ ] **Option B: NVML Direct**
  - Test nvml-wrapper Rust crate
  - Poll GPU metrics programmatically
  - Correlate with pod metadata
  - **Deliverable**: Proof of concept

- [ ] **Option C: eBPF Driver Hooks**
  - Research NVIDIA driver ioctl interface
  - Attempt to attach kprobe to driver functions
  - Document stability and breakage risk
  - **Deliverable**: Risk assessment

**Decision Criteria**:
- Production readiness
- Maintenance burden
- Accuracy of per-pod attribution
- Overhead on GPU workloads

**Expected Outcome**: Choose Option A (DCGM) for MVP

#### 7.2: DCGM Sidecar Deployment

**Files**: `deploy/daemonset.yaml` (update)

- [ ] Add DCGM exporter container to agent pod
- [ ] Expose DCGM metrics on localhost:9400
- [ ] Configure DCGM to scrape all GPUs

**DaemonSet Update**:
```yaml
spec:
  template:
    spec:
      containers:
      - name: agent
        image: orb8/agent:latest
        # ... existing config

      - name: dcgm-exporter
        image: nvcr.io/nvidia/k8s/dcgm-exporter:latest
        ports:
        - containerPort: 9400
          name: dcgm-metrics
        securityContext:
          capabilities:
            add: ["SYS_ADMIN"]
```

**Success Criteria**:
- ✅ DCGM exporter runs alongside agent
- ✅ Metrics available at localhost:9400/metrics
- ✅ All GPUs discovered

#### 7.3: GPU Metrics Collector

**Files**: `orb8-agent/src/gpu/dcgm_collector.rs`

- [ ] Scrape DCGM exporter via HTTP
- [ ] Parse Prometheus format
- [ ] Map GPU device ID → pod using device plugin API

**Implementation**:
```rust
// orb8-agent/src/gpu/dcgm_collector.rs
use reqwest::Client;
use prometheus_parse::{Scrape, Sample};

pub struct DcgmCollector {
    http_client: Client,
    dcgm_url: String,
    device_plugin_client: DevicePluginClient,
}

impl DcgmCollector {
    pub async fn collect_metrics(&self) -> Result<Vec<GpuMetric>> {
        // Scrape DCGM exporter
        let response = self.http_client
            .get(&format!("{}/metrics", self.dcgm_url))
            .send()
            .await?;

        let body = response.text().await?;
        let scrape = Scrape::parse(body.lines().map(|s| Ok(s.to_string())))?;

        let mut gpu_metrics = Vec::new();

        for sample in scrape.samples {
            if sample.metric == "DCGM_FI_DEV_GPU_UTIL" {
                let gpu_id = sample.labels.get("gpu").unwrap();
                let utilization = sample.value;

                // Map GPU ID to pod
                if let Some(pod_info) = self.get_pod_for_gpu(gpu_id).await? {
                    gpu_metrics.push(GpuMetric {
                        namespace: pod_info.namespace,
                        pod_name: pod_info.pod_name,
                        gpu_id: gpu_id.clone(),
                        utilization,
                        timestamp: sample.timestamp,
                    });
                }
            }
        }

        Ok(gpu_metrics)
    }

    async fn get_pod_for_gpu(&self, gpu_id: &str) -> Result<Option<PodInfo>> {
        // Query Kubernetes device plugin allocations
        self.device_plugin_client.get_pod_for_device(gpu_id).await
    }
}
```

**Success Criteria**:
- ✅ Scrapes DCGM metrics every 10 seconds
- ✅ Correctly maps GPU → pod
- ✅ Handles pods without GPUs gracefully

#### 7.4: GPU Metrics in Prometheus

**Files**: `orb8-agent/src/prom_exporter.rs` (extend)

- [ ] Export `orb8_gpu_utilization` gauge
- [ ] Export `orb8_gpu_memory_used` gauge
- [ ] Labels: namespace, pod, gpu_id

**Metrics**:
```
orb8_gpu_utilization{namespace="ml-training",pod="pytorch-job",gpu="0"} 95.5
orb8_gpu_memory_used_bytes{namespace="ml-training",pod="pytorch-job",gpu="0"} 15032385536
```

**Success Criteria**:
- ✅ Metrics exported to Prometheus
- ✅ Grafana dashboard shows per-pod GPU usage
- ✅ Accurate correlation with GPU workload pods

#### 7.5: CLI GPU Commands

**Files**: `orb8-cli/src/commands/trace.rs`

- [ ] `orb8 trace gpu --namespace <ns>`
- [ ] Display GPU utilization per pod
- [ ] Display GPU memory usage

**Output**:
```
NAMESPACE     POD             GPU  UTIL%  MEMORY
ml-training   pytorch-job-1   0    95%    14GB / 16GB
ml-training   pytorch-job-2   1    88%    12GB / 16GB
```

**Success Criteria**:
- ✅ CLI displays GPU metrics
- ✅ Real-time updates

**Phase 7 Deliverables**

✅ Research spike complete with decision (DCGM)
✅ DCGM sidecar deployment
✅ GPU → pod mapping
✅ Prometheus GPU metrics
✅ CLI GPU commands
✅ Grafana GPU dashboard
✅ Documentation: "GPU Telemetry Guide"
✅ **Public Release**: v0.4.0 - GPU Telemetry

**User Validation Checkpoint**: Get feedback from ML/AI teams

---

## Phase 8: Advanced Features

**Goal**: Production hardening and advanced capabilities

**Dependencies**: Phases 3-7

**Estimated Effort**: Ongoing

### Tasks

#### 8.1: Standalone Mode

**Files**: `orb8-cli/src/standalone.rs`

- [ ] Implement standalone mode (no DaemonSet required)
- [ ] CLI uses `kubectl exec` to access node
- [ ] Temporarily load probes, collect data, cleanup

**Implementation**:
```rust
// orb8-cli/src/standalone.rs
pub struct StandaloneTracer {
    kube_client: Client,
}

impl StandaloneTracer {
    pub async fn trace_network(
        &self,
        namespace: &str,
        pod: &str,
        duration: Duration,
    ) -> Result<Vec<NetworkFlow>> {
        // 1. Find node running pod
        let node = self.find_node_for_pod(namespace, pod).await?;

        // 2. Create privileged debug pod on that node
        let debug_pod = self.create_debug_pod(&node).await?;

        // 3. Copy probe binary to debug pod
        self.upload_probe(&debug_pod).await?;

        // 4. Run agent in standalone mode
        let output = self.exec_in_pod(
            &debug_pod,
            format!("orb8-agent --standalone --duration={}", duration.as_secs())
        ).await?;

        // 5. Parse output
        let flows = parse_flows(&output)?;

        // 6. Cleanup
        self.delete_debug_pod(&debug_pod).await?;

        Ok(flows)
    }
}
```

**Success Criteria**:
- ✅ Works without DaemonSet installation
- ✅ Cleans up temporary resources
- ✅ Useful for ad-hoc debugging

#### 8.2: TUI Dashboard

**Files**: `orb8-cli/src/commands/dashboard.rs`

- [ ] Real-time TUI using ratatui
- [ ] Display top flows, pods, protocols
- [ ] Interactive filtering and sorting

**Implementation**:
```rust
// orb8-cli/src/commands/dashboard.rs
use ratatui::{
    backend::CrosstermBackend,
    widgets::{Block, Borders, List, ListItem},
    Terminal,
};

pub async fn run_dashboard() -> Result<()> {
    let backend = CrosstermBackend::new(std::io::stdout());
    let mut terminal = Terminal::new(backend)?;

    loop {
        // Fetch latest flows
        let flows = fetch_flows().await?;

        // Render
        terminal.draw(|f| {
            let size = f.size();
            let items: Vec<ListItem> = flows
                .iter()
                .map(|flow| {
                    ListItem::new(format!(
                        "{}/{} → {} {} bytes",
                        flow.namespace, flow.pod_name, flow.dst_ip, flow.bytes
                    ))
                })
                .collect();

            let list = List::new(items)
                .block(Block::default().borders(Borders::ALL).title("Network Flows"));

            f.render_widget(list, size);
        })?;

        // Refresh every 2 seconds
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}
```

**Success Criteria**:
- ✅ Interactive TUI dashboard
- ✅ Real-time updates
- ✅ Keyboard navigation

#### 8.3: Historical Storage

**Files**: `orb8-server/src/storage.rs`

- [ ] Optional TimescaleDB backend
- [ ] Store flow history (configurable retention)
- [ ] Query historical data via CLI

**Success Criteria**:
- ✅ Long-term metric storage
- ✅ Efficient time-range queries
- ✅ Configurable retention policy

#### 8.4: Multi-Cluster Support

**Files**: `orb8-server/src/multi_cluster.rs`

- [ ] Federate multiple clusters
- [ ] Cross-cluster flow correlation
- [ ] Single pane of glass dashboard

**Success Criteria**:
- ✅ Monitor multiple clusters from one CLI
- ✅ Aggregate metrics across clusters

**Phase 8 Deliverables**

✅ Standalone mode for ad-hoc tracing
✅ Interactive TUI dashboard
✅ Historical data storage (optional)
✅ Multi-cluster federation (optional)
✅ **Public Release**: v1.0.0 - Production Ready

---

## Future Enhancements

**Not scheduled, but under consideration**

### DNS Tracing

- Parse DNS queries/responses in network probe
- Track DNS failures per pod
- Detect DNS exfiltration

### IPv6 Support

- Extend network probe to parse IPv6 headers
- Update flow aggregation logic

### eBPF GPU Probes (Research)

- Revisit eBPF hooks into NVIDIA driver
- Kernel-level GPU event tracing
- Requires driver stability analysis

### WebAssembly Plugin System

- Load custom probes as WASM plugins
- Community-contributed probe marketplace

### AI-Powered Insights

- ML model for anomaly detection
- Predictive alerts
- Auto-remediation suggestions

---

## Summary

This roadmap provides a **phase-based, dependency-driven** implementation plan for orb8. Each phase delivers tangible value and can be validated with real users before proceeding.

**Key Principles**:
- ✅ No artificial deadlines
- ✅ Focus on quality over speed
- ✅ User validation at each major milestone
- ✅ Technical debt explicitly managed
- ✅ Research spikes for high-uncertainty areas

**Current Status**: Phase 0 complete (Foundation)

**Next Step**: Phase 1 (eBPF Infrastructure)

---

**Document Version**: 1.0
**Last Updated**: 2025-01-12
**Authors**: orb8 maintainers
