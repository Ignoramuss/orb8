# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

orb8 is an eBPF-powered observability toolkit for Kubernetes with first-class GPU telemetry. Built entirely in **Rust** using the aya framework, it provides low-overhead monitoring of network flows, system calls, and GPU performance optimized for AI/ML workloads.

**Architecture**: Dual-mode platform supporting both cluster-wide monitoring (DaemonSet) and standalone on-demand tracing.

**Current Status**: Phase 4 (Deploy and Run Anywhere) - Network flows working, test infrastructure in place, documentation overhauled.

## Monorepo Structure

orb8 is organized as a **Cargo workspace** with multiple crates:

```
orb8/
├── Cargo.toml                    # Virtual workspace root (no root package)
├── Dockerfile                    # Multi-stage (CI) and local (fast) build targets
├── orb8-probes/                  # eBPF probes (Rust, kernel space)
├── orb8-common/                  # Shared types between kernel/user space
├── orb8-agent/                   # Node agent (DaemonSet)
├── orb8-server/                  # Central API server (stub)
├── orb8-cli/                     # CLI tool
├── orb8-proto/                   # gRPC protocol definitions
├── deploy/                       # K8s manifests (DaemonSet, RBAC, kind config, test pods)
├── scripts/                      # Dev setup, smoke-test.sh, e2e-test.sh
└── docs/
    ├── ARCHITECTURE.md           # Detailed technical design
    └── ROADMAP.md                # Phase-based implementation plan
```

### Workspace Commands

```bash
# Build all crates (excludes orb8-probes on non-Linux)
cargo build

# Build specific crate
cargo build -p orb8-agent

# Test (uses default-members, excludes orb8-probes which is #![no_main])
cargo test

# NEVER use cargo test --workspace (orb8-probes will fail linking)

# Run specific binary
cargo run -p orb8-cli -- --help
```

## Build and Development Commands

### Quick Start

```bash
make magic          # Build, test, install (uses VM on macOS, native on Linux)
make magic-local    # Build, test, install locally without VM
```

### Development Workflow

#### macOS (uses Lima VM for eBPF support)

```bash
make dev            # Setup/start Lima VM (first run: 5-10 min)
make shell          # Enter VM
make status         # Check VM status
make stop           # Stop VM
make clean          # Delete VM completely
```

#### Inside VM or on Linux

```bash
# Build
cargo build                              # Debug build
cargo build --release                    # Release build
cargo build -p orb8-probes              # Build eBPF probes only

# Test
cargo test                              # All tests (default-members)
cargo test --lib                        # Unit tests only
cargo test -p orb8-agent                # Test specific crate

# Run
cargo run -p orb8-cli -- --help
sudo cargo run -p orb8-agent            # Requires root for eBPF
```

### Code Quality

```bash
cargo fmt                               # Format all code
cargo fmt -p orb8-agent                # Format specific crate
cargo clippy --workspace -- -D warnings # Lint (must pass with zero warnings)
cargo check --workspace                 # Type check without building
```

### eBPF Probe Development

eBPF probes are written in Rust using aya-bpf:

```bash
# Build probes (automatically triggered by workspace build)
cargo build -p orb8-probes

# Verify probe compilation (note: no .o extension)
ls target/bpfel-unknown-none/release/orb8_probes

# Load and test probe (requires Linux)
sudo cargo run -p orb8-agent
```

**Critical**: Probes are **embedded in the agent binary** at compile time via `include_bytes_aligned!` in `orb8-agent/build.rs`. The Dockerfile does NOT need to copy probe files separately.

## Architecture

orb8 consists of **eBPF probes** (kernel space) and **Rust services** (user space).

### eBPF Probes (orb8-probes/)

**Run in**: Kernel space
**Language**: Rust (no_std) using aya-bpf
**Compile to**: ELF eBPF relocatable (`target/bpfel-unknown-none/release/orb8_probes`)
**Embedded via**: `include_bytes_aligned!` in agent's build.rs

Probes:
- `network_probe.rs` - Network flow tracing (TC classifier, ingress/egress)

Planned (not yet implemented):
- Syscall monitoring (tracepoint) - Phase 8
- GPU telemetry (kprobe/uprobe) - Phase 9

**Key concept**: TC classifiers run in network/softirq context and cannot access cgroup IDs. Pod identification for network events uses **IP-based enrichment** from the Kubernetes API. Future tracepoint probes (syscall monitoring) will have process context and can use cgroup-based identification.

### User-Space Components

#### orb8-agent (DaemonSet)

**Purpose**: Runs on every Kubernetes node

**Responsibilities**:
- Load eBPF probes into kernel
- Poll ring buffers for events
- Watch Kubernetes API for pod metadata
- Map pod IPs → pod metadata (primary enrichment path)
- Map cgroup IDs → pod metadata (for future tracepoint probes)
- Aggregate flow metrics
- Expose gRPC API (:9090)

**Key files**:
- `probe_loader.rs` - Manages eBPF probe lifecycle, ring buffer polling
- `k8s_watcher.rs` - Watches pods, populates IP and cgroup caches
- `pod_cache.rs` - Dual-indexed cache (by IP and by cgroup ID)
- `aggregator.rs` - Flow aggregation with pre-resolved pod identity
- `grpc_server.rs` - gRPC service (QueryFlows, StreamEvents, GetStatus)
- `net.rs` - IP formatting/parsing, self-traffic filtering
- `cgroup.rs` - Cgroup ID resolution from container IDs

#### orb8-server (Central Control Plane)

**Purpose**: Cluster-wide aggregation and query routing

**Responsibilities**:
- Discover all agent pods
- Route queries to appropriate nodes
- Aggregate results from multiple agents
- Expose external gRPC API (:8080)

#### orb8-cli (User Interface)

**Purpose**: Command-line interface for users

**Modes**:
- **Cluster mode**: Connects to orb8-server via gRPC
- **Standalone mode**: Directly loads probes on target node (no DaemonSet required)

**Commands**:
```bash
# Cluster mode
orb8 --mode=cluster trace network --namespace ml-training
orb8 --mode=cluster trace gpu --pod pytorch-job

# Standalone mode
orb8 --mode=standalone trace network --node worker-1 --duration 30s
```

#### orb8-common (Shared Types)

**Purpose**: Types shared between eBPF (kernel) and user-space

**Key types**:
- `NetworkFlowEvent` - Network packet event (32 bytes, 8-byte aligned)
- `PacketEvent` - Legacy simple packet event (kept for backward compat)

**Important**: Must be `#[repr(C)]` and `no_std` compatible for eBPF.

#### orb8-proto (gRPC Definitions)

**Purpose**: gRPC service and message definitions

**Generates**:
- `OrbitService` server and client
- Protocol buffers for queries and responses

```protobuf
service OrbitService {
    rpc QueryFlows(FlowQuery) returns (FlowResponse);
    rpc StreamFlows(StreamRequest) returns (stream FlowEvent);
}
```

## Key Technical Concepts

### Pod Identification

**Problem**: eBPF programs run in kernel and don't know about Kubernetes pods.

**Two enrichment strategies** depending on probe type:

**1. IP-based enrichment (primary, used by TC classifiers)**:
- TC hooks run in network/softirq context — no process context, `bpf_get_current_cgroup_id()` returns 0
- Agent watches K8s API and maps pod IPs → pod metadata
- TC probe extracts src/dst IPs from packets → agent looks up pod by IP
- Works for all pod traffic (every pod gets a unique IP from the CNI)

**2. Cgroup-based enrichment (future, for tracepoint probes)**:
- Tracepoints run in process context — `bpf_get_current_cgroup_id()` works
- Agent resolves pod UID + container ID → cgroup inode
- Maintains map: `cgroup_id → PodMetadata`
- Will be used for syscall monitoring (Phase 8)

### Network Data Flow

```
1. [KERNEL] Packet arrives/leaves on network interface
2. [KERNEL] TC classifier extracts 5-tuple (IPs, ports, protocol) + timestamp
3. [KERNEL] Sets cgroup_id=0 (unavailable in TC context)
4. [KERNEL] Writes NetworkFlowEvent to ring buffer
5. [USER] Agent polls ring buffer every 100ms
6. [USER] Filters self-traffic (agent's own gRPC connections)
7. [USER] Looks up src_ip and dst_ip in pod cache (IP-based enrichment)
8. [USER] Passes enriched event to aggregator and gRPC broadcast
```

### Communication Channels

- **eBPF ↔ Agent**: Ring buffers and eBPF maps (shared kernel/user memory)
- **Agent ↔ Server**: gRPC over HTTP/2
- **CLI ↔ Server**: gRPC
- **Prometheus ↔ Agent**: HTTP scrape of /metrics endpoint

## Development Environment

### macOS

**Requirement**: Lima/QEMU VM (eBPF requires real Linux kernel)

**Setup**:
```bash
make dev    # Creates Ubuntu VM with Rust + eBPF tools
make shell  # Enter VM
```

**VM Details**:
- Ubuntu 22.04, kernel 5.15+
- Auto-mounted project directory (same path as macOS)
- Rust, aya, minikube pre-installed

### Linux

**Native development** - no VM needed

**Requirements**:
- Kernel 5.8+ with BTF enabled
- Root/CAP_BPF for loading eBPF programs
- aya build dependencies

### eBPF Requirements

- Linux kernel 5.8+ (5.15+ recommended)
- BTF (BPF Type Format) enabled
- CAP_BPF, CAP_NET_ADMIN, CAP_SYS_ADMIN capabilities

## Testing Strategy

### Unit Tests

```bash
cargo test                          # All tests via default-members (excludes orb8-probes)
cargo test -p orb8-agent            # Test specific crate
make test                           # Run cargo test inside Lima VM (from macOS)
```

### Smoke Test

```bash
make smoke-test                     # Probe loading + traffic capture (no k8s)
```

Builds agent, runs with sudo, generates traffic, asserts events are captured and flows appear. No Kubernetes required — all traffic shows as "external/unknown" (expected without pod watcher).

### E2E Test

```bash
make e2e-test                       # Full kind cluster test with pod enrichment
```

Creates a 2-node kind cluster, builds Docker image, deploys DaemonSet with RBAC, generates traffic, and asserts flows contain real pod names (not "external/unknown"). Requires Docker and kind.

### Container Build

```bash
make docker-build                   # Build orb8-agent:test image (uses local binary)
```

Uses the `local` Dockerfile target which copies the pre-built binary. For CI, use the default target which does a full multi-stage build.

## Important Architectural Constraints

1. **eBPF Linux-Only**: Probes only compile and run on Linux
2. **Kernel Version**: Minimum 5.8, recommended 5.15+
3. **BTF Required**: Kernel must have BTF for CO-RE (Compile Once, Run Everywhere)
4. **Kubernetes Required**: Agent expects K8s API access
5. **GPU Features**: Require NVIDIA DCGM or NVML (planned Phase 9)

## Known Network Limitations

- **Same-node pod traffic**: TC probes attach to eth0. Pod-to-pod traffic on the same node stays on veth pairs and never reaches eth0, making it invisible. Would require attaching probes to bridge interfaces (cni0) or per-pod veths.
- **hostNetwork pods**: Share the node's IP. Multiple hostNetwork pods and host processes (kubelet, sshd) are all attributed to whichever hostNetwork pod was last cached for that IP.
- **Service ClusterIP**: kube-proxy applies DNAT before packets reach the TC hook, so flows show the backend pod IP, not the Service address. Enrichment works correctly, but the original Service target is lost.

## Roadmap Context

Development follows **phase-based approach** without strict timelines:

- **Phase 0**: ✅ Foundation & monorepo (COMPLETE)
- **Phase 1**: ✅ eBPF Infrastructure (COMPLETE)
- **Phase 2**: ✅ Container Identification (COMPLETE)
- **Phase 3**: ✅ Network Tracing MVP (COMPLETE)
- **Phase 3.5**: ✅ Structural Cleanup (COMPLETE)
- **Phase 4**: 🔧 DaemonSet deployment (kustomize, CI image builds, env config)
- **Phase 5**: Prometheus metrics
- **Phase 6**: Event pipeline & JSON output
- **Phase 7**: Cluster mode (orb8-server)
- **Phase 8**: Syscall monitoring (validates cgroup enrichment)
- **Phase 9**: GPU telemetry
- **Phase 10**: TUI dashboard, standalone mode, DNS tracing

**Current**: Phase 4

See `docs/ROADMAP.md` for granular implementation details.

## Code Organization Principles

### eBPF Probes (orb8-probes/)

```rust
#![no_std]  // Required for eBPF
#![no_main]

use aya_bpf::{macros::classifier, programs::TcContext};

#[classifier]
pub fn network_probe(ctx: TcContext) -> i32 {
    // Probe logic
    TC_ACT_OK
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}  // eBPF cannot panic
}
```

### Shared Types (orb8-common/)

```rust
#[repr(C)]  // Required for kernel/user sharing
#[derive(Clone, Copy)]
pub struct NetworkFlowEvent {
    pub cgroup_id: u64,
    pub timestamp_ns: u64,
    pub src_ip: u32,
    // ...
}
```

### User-Space Services

```rust
#[tokio::main]  // Async runtime
async fn main() -> Result<()> {
    // Load eBPF probes
    // Start K8s watcher
    // Poll events
    // Serve gRPC/HTTP
}
```

## GPU Telemetry Architecture

**Planned Approach**: DCGM Integration (Phase 9)

**Why not eBPF?**
- NVIDIA driver is closed-source
- No stable ABI for kprobes
- LD_PRELOAD approach doesn't work in Kubernetes
- **Solution**: Scrape DCGM metrics, correlate with pod metadata via device plugin

**Future**: eBPF driver hooks as research spike (high risk)

## Debugging

### eBPF Probe Debugging

```bash
# View eBPF logs
sudo cat /sys/kernel/debug/tracing/trace_pipe

# List loaded eBPF programs
sudo bpftool prog list

# Inspect eBPF maps
sudo bpftool map list
sudo bpftool map dump id <ID>
```

### Agent Debugging

```bash
# Enable debug logging
RUST_LOG=debug cargo run -p orb8-agent

# Check gRPC connectivity
grpcurl -plaintext localhost:9090 list
```

### Common Issues

**Issue**: eBPF verifier error
**Fix**: Check probe code for loops, out-of-bounds access, or unsafe operations

**Issue**: Events missing pod metadata
**Fix**: Verify pod watcher is running and IP-based cache is populated

**Issue**: Ring buffer full
**Fix**: Increase buffer size or add sampling

## Contributing

When implementing new features:

1. **Check ROADMAP.md** for phase dependencies
2. **Read ARCHITECTURE.md** for design context
3. **Add tests** (unit + integration)
4. **Update docs** as you go
5. **Run `cargo fmt` and `cargo clippy`** before committing

## Validation & Testing

### Container Build

Probes are embedded at compile time via `orb8-agent/build.rs` using `aya_build::build_ebpf()` — the Dockerfile only copies the agent binary.

### Agent Startup Verification

When validating agent changes, check logs for these messages in order:

```
INFO  orb8_agent::probe_loader] Running pre-flight checks...
INFO  orb8_agent::probe_loader] Pre-flight checks passed
INFO  orb8_agent::probe_loader] Loading network probe...
INFO  orb8_agent::probe_loader] Attached ingress probe to eth0
INFO  orb8_agent::probe_loader] Attached egress probe to eth0
INFO  orb8_agent::k8s_watcher] Pod watcher initial sync complete. Tracking N pods (by IP)
```

Traffic events appear as:
```
[namespace/pod_name] src_ip:src_port -> dst_ip:dst_port PROTO direction len=N
```

### DaemonSet Security Model

The agent runs with `privileged: false` and specific capabilities:
- `BPF` - Load eBPF programs
- `NET_ADMIN` - Attach TC classifiers
- `SYS_ADMIN` - Access tracepoints
- `PERFMON` - Performance monitoring
- `SYS_RESOURCE` - Increase rlimits

Required volume mounts: `/sys` (ro), `/sys/kernel/debug` (rw), `/sys/fs/cgroup` (ro)

### Common Validation Failures

| Symptom | Cause | Fix |
|---------|-------|-----|
| `Failed to load probes` | Missing BTF or old kernel | Check `/sys/kernel/btf/vmlinux` exists, kernel >= 5.8 |
| `Permission denied` on probe load | Missing capabilities | Run with sudo or verify caps |
| No traffic events | Probes not attached | Check logs for `Attached.*probe to` |
| Events missing pod names | K8s watcher not synced | Check `Pod watcher initial sync complete` in logs |
| Events show `external/unknown` | Traffic from non-pod IPs | Expected for host-level traffic (SSH, node processes) |

### Lima VM Notes

- VM must be running for `make test`, `make build`, etc. on macOS
- DNS issues after VM sleep: `limactl shell orb8-dev -- sudo systemctl restart systemd-resolved`
- Project mounted at same path inside VM
- `cargo clean` can hang on virtiofs mounts — use `rm -rf target` instead

## Code Style

- Follow Rust best practices (use clippy)
- Prefer async/await over blocking operations
- Add error context with `map_err`
- Document public APIs
- No obvious comments (code should be self-documenting)
