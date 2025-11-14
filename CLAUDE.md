# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

orb8 is an eBPF-powered observability toolkit for Kubernetes with first-class GPU telemetry. Built entirely in **Rust** using the aya framework, it provides low-overhead monitoring of network flows, system calls, and GPU performance optimized for AI/ML workloads.

**Architecture**: Dual-mode platform supporting both cluster-wide monitoring (DaemonSet) and standalone on-demand tracing.

**Current Status**: Phase 0 (Foundation) - Monorepo structure established, development environment ready.

## Monorepo Structure

orb8 is organized as a **Cargo workspace** with multiple crates:

```
orb8/
├── Cargo.toml                    # Workspace root
├── orb8-probes/                  # eBPF probes (Rust, kernel space)
├── orb8-common/                  # Shared types between kernel/user space
├── orb8-agent/                   # Node agent (DaemonSet)
├── orb8-server/                  # Central API server
├── orb8-cli/                     # CLI tool
├── orb8-proto/                   # gRPC protocol definitions
├── tests/                        # Integration tests
├── deploy/                       # Kubernetes manifests
└── docs/
    ├── ARCHITECTURE.md           # Detailed technical design
    └── ROADMAP.md                # Phase-based implementation plan
```

### Workspace Commands

```bash
# Build all crates
cargo build --workspace

# Build specific crate
cargo build -p orb8-agent

# Test all crates
cargo test --workspace

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
cargo test                              # All tests
cargo test --lib                        # Unit tests only
cargo test --test integration_test      # Integration tests
cargo test -p orb8-agent                # Test specific crate

# Run
cargo run -p orb8-cli -- --help
cargo run -p orb8-agent
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

# Verify probe compilation
ls target/bpfel-unknown-none/release/*.o

# Load and test probe (requires Linux)
sudo cargo run -p orb8-agent
```

## Architecture

orb8 consists of **eBPF probes** (kernel space) and **Rust services** (user space).

### eBPF Probes (orb8-probes/)

**Run in**: Kernel space
**Language**: Rust (no_std) using aya-bpf
**Compile to**: `.bpf.o` eBPF bytecode

Probes (named following orbit theme):
- `network_probe.rs` - Network flow tracing (tc hook)
- `syscall_probe.rs` - System call monitoring (tracepoint)
- `gpu_probe.rs` - GPU telemetry (kprobe/uprobe)

**Key concept**: Probes extract **cgroup ID** to identify which container/pod an event belongs to.

### User-Space Components

#### orb8-agent (DaemonSet)

**Purpose**: Runs on every Kubernetes node

**Responsibilities**:
- Load eBPF probes into kernel
- Poll ring buffers for events
- Watch Kubernetes API for pod metadata
- Map cgroup IDs → pods
- Aggregate metrics
- Expose gRPC API (:9090)
- Export Prometheus metrics (:9091)

**Key files**:
- `probe_loader.rs` - Manages eBPF probe lifecycle
- `collector.rs` - Polls ring buffers
- `k8s/watcher.rs` - Watches pods, maps cgroups
- `aggregator.rs` - Time-series aggregation
- `api_server.rs` - gRPC service
- `prom_exporter.rs` - Prometheus /metrics endpoint

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
- `NetworkFlowEvent` - Network packet event
- `SyscallEvent` - System call event
- `GpuEvent` - GPU telemetry event

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

### Container Identification (Critical)

**Problem**: eBPF programs run in kernel and don't know about Kubernetes pods.

**Solution**: cgroup v2 ID mapping

1. **eBPF side**: Extract cgroup ID via `bpf_get_current_cgroup_id()`
2. **User-space side**:
   - Watch Kubernetes API for pods
   - Resolve pod UID + container ID → cgroup inode
   - Maintain map: `cgroup_id → PodMetadata`
3. **Enrichment**: Look up cgroup ID to add namespace, pod name, container name to events

### Data Flow

```
1. [KERNEL] Event occurs (packet, syscall, GPU operation)
2. [KERNEL] eBPF probe extracts cgroup_id + event data
3. [KERNEL] Writes to ring buffer (shared memory)
4. [USER] Agent polls ring buffer
5. [USER] Looks up cgroup_id → pod metadata
6. [USER] Enriches event with K8s context
7. [USER] Aggregates into metrics
8. [USER] Exports to Prometheus or serves via gRPC
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
cargo test --lib                    # Test all library code
cargo test -p orb8-agent --lib      # Test specific crate
```

### Integration Tests

```bash
cargo test --test integration_test  # End-to-end tests
```

**Require**: Linux VM with Kubernetes (minikube)

### eBPF Probe Tests

**Require**: Root privileges and Linux

```bash
# Inside VM
sudo cargo test -p orb8-probes
```

## Important Architectural Constraints

1. **eBPF Linux-Only**: Probes only compile and run on Linux
2. **Kernel Version**: Minimum 5.8, recommended 5.15+
3. **BTF Required**: Kernel must have BTF for CO-RE (Compile Once, Run Everywhere)
4. **Kubernetes Required**: Agent expects K8s API access
5. **GPU Features**: Require NVIDIA DCGM or NVML (planned Phase 7)

## Roadmap Context

Development follows **phase-based approach** without strict timelines:

- **Phase 0**: ✅ Foundation & monorepo (COMPLETE)
- **Phase 1**: eBPF Infrastructure (load probes, ring buffers)
- **Phase 2**: Container Identification (cgroup mapping)
- **Phase 3**: Network Tracing MVP (first public release)
- **Phase 4**: Cluster Mode (DaemonSet + central server)
- **Phase 5**: Metrics & Observability (Prometheus, Grafana)
- **Phase 6**: Syscall Monitoring
- **Phase 7**: GPU Telemetry
- **Phase 8**: Advanced Features (TUI, standalone mode)

**Current**: Implementing Phase 1

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

**Planned Approach**: DCGM Integration (Phase 7)

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
**Fix**: Verify pod watcher is running and cgroup paths are correct

**Issue**: Ring buffer full
**Fix**: Increase buffer size or add sampling

## Contributing

When implementing new features:

1. **Check ROADMAP.md** for phase dependencies
2. **Read ARCHITECTURE.md** for design context
3. **Add tests** (unit + integration)
4. **Update docs** as you go
5. **Run `cargo fmt` and `cargo clippy`** before committing

## Code Style

- Follow Rust best practices (use clippy)
- Prefer async/await over blocking operations
- Add error context with `map_err`
- Document public APIs
- No obvious comments (code should be self-documenting)
