# Development Guide

This guide explains how to set up your development environment for orb8 and run tests locally.

## Table of Contents

- [Prerequisites](#prerequisites)
- [Development Environment Setup](#development-environment-setup)
  - [macOS Setup](#macos-setup)
  - [Linux Setup](#linux-setup)
- [Building the Project](#building-the-project)
- [Testing](#testing)
- [Development Workflow](#development-workflow)
- [Troubleshooting](#troubleshooting)

## Prerequisites

orb8 is an eBPF-powered observability toolkit, which means it requires a **Linux kernel** to run eBPF programs. However, development can happen on macOS using a Linux VM.

### Required Tools

**For macOS developers:**
- Homebrew
- Lima (Linux VM manager)
- QEMU (virtualization)

**For Linux developers:**
- Linux kernel 5.8+ (5.15+ recommended)
- BTF (BPF Type Format) enabled in kernel
- CAP_BPF, CAP_NET_ADMIN, CAP_SYS_ADMIN capabilities for loading eBPF programs

**All developers need:**
- Rust stable toolchain
- Rust nightly toolchain with `rust-src` component
- `bpf-linker` - eBPF linker for Rust
- LLVM and Clang (version 14+)
- Git

## Development Environment Setup

### macOS Setup

orb8 uses Lima to create a Linux VM with all necessary tools pre-installed.

#### 1. Install Prerequisites

```bash
# Install Homebrew if not already installed
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"

# Install Lima and QEMU
make install-tools
```

#### 2. Create Development VM

```bash
# This creates and provisions the VM (takes 5-10 minutes on first run)
make dev
```

The VM will be provisioned with:
- Ubuntu 22.04 with kernel 5.15+
- Rust stable + nightly (with rust-src)
- bpf-linker
- cargo-generate
- eBPF tools (LLVM 14, clang, bpftool)
- kubectl, minikube, Docker

#### 3. Verify Installation

```bash
# Enter the VM
make shell

# Inside VM: verify eBPF tools
rustc +nightly --version
bpf-linker --version
bpftool version

# Check kernel version (should be 5.15+)
uname -r

# Build the project
cd $(pwd)
cargo build
```

#### 4. Local Testing on macOS (Limited)

You can build and test **non-eBPF** components locally on macOS:

```bash
# Build all workspace crates (eBPF probes will use build.rs)
cargo build -p orb8-probes    # Compiles eBPF to bytecode
cargo build -p orb8-common     # Shared types
cargo build -p orb8-agent      # User-space agent
cargo build -p orb8-cli        # CLI tool

# Run tests (integration tests requiring Linux will be skipped)
cargo test
```

**Note:** eBPF programs compile to bytecode on macOS but cannot be loaded/executed. Use the Lima VM for full testing.

### Linux Setup

If you're on Linux, you can develop natively without a VM.

#### 1. Install System Dependencies

**Ubuntu/Debian:**
```bash
sudo apt-get update
sudo apt-get install -y \
  build-essential \
  curl \
  git \
  pkg-config \
  libssl-dev \
  llvm-14 \
  llvm-14-dev \
  clang-14 \
  libclang-14-dev \
  libbpf-dev \
  linux-headers-$(uname -r) \
  linux-tools-generic \
  bpftool
```

**Fedora/RHEL:**
```bash
sudo dnf install -y \
  gcc \
  make \
  curl \
  git \
  openssl-devel \
  llvm-devel \
  clang-devel \
  libbpf-devel \
  kernel-headers \
  bpftool
```

#### 2. Install Rust

```bash
# Install rustup
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- \
  -y \
  --default-toolchain stable \
  --profile default

# Source environment
source "$HOME/.cargo/env"

# Install nightly with rust-src
rustup toolchain install nightly --component rust-src

# Install aya tools
cargo install bpf-linker
cargo install cargo-generate
```

#### 3. Verify Installation

```bash
# Check toolchain
rustc --version
rustc +nightly --version
bpf-linker --version

# Check kernel requirements
uname -r  # Should be 5.8+ (5.15+ recommended)

# Verify BTF is enabled
ls /sys/kernel/btf/vmlinux  # Should exist
```

## Building the Project

### Standard Build

```bash
# Build all workspace crates
cargo build

# Build specific crate
cargo build -p orb8-probes
cargo build -p orb8-agent

# Release build
cargo build --release
```

### eBPF Probe Compilation

The `orb8-probes` crate uses a custom `build.rs` that automatically compiles eBPF programs:

```bash
# This compiles Rust → eBPF bytecode (.bpf.o files)
cargo build -p orb8-probes

# eBPF artifacts are in:
# target/bpfel-unknown-none/release/*.bpf.o
```

**How it works:**
1. `build.rs` uses `aya-build` to invoke nightly Rust
2. Compiles with `-Z build-std=core` for `bpfel-unknown-none` target
3. `bpf-linker` links the eBPF bytecode
4. Output is ELF object files that the agent loads into the kernel

### Code Quality

```bash
# Format code
cargo fmt

# Lint (must pass with zero warnings)
cargo clippy --workspace -- -D warnings

# Type check without building
cargo check --workspace
```

## Testing

### Unit Tests

```bash
# Run all unit tests
cargo test --lib

# Test specific crate
cargo test -p orb8-agent --lib
```

### Integration Tests

Integration tests require a Linux environment with root privileges:

```bash
# On Linux or in Lima VM
sudo -E cargo test --test integration_test

# Or use make command (in VM)
make test
```

### eBPF Probe Tests

eBPF probes need to be loaded into the kernel to test properly:

```bash
# Inside Lima VM or on Linux with sudo
sudo cargo test -p orb8-probes
```

## Development Workflow

### Using Lima VM (macOS)

```bash
# Start VM if not running
make dev

# Enter VM shell
make shell

# Inside VM: navigate to project
cd $(pwd)

# Make changes in your editor on macOS
# Files are auto-synced to VM via mount

# Inside VM: build and test
cargo build
cargo test

# Exit VM
exit

# Stop VM (keeps it for later)
make stop

# Delete VM completely
make clean
```

### Quick Commands (macOS)

```bash
# Build, test, and install in one command
make magic

# Build locally on macOS (limited eBPF support)
make magic-local

# Run specific commands in VM without entering shell
make build    # Build in VM
make test     # Test in VM
make fmt      # Format in VM
make clippy   # Lint in VM
```

### Testing Your Changes

After implementing Phase 1.1 (aya-ebpf infrastructure), verify:

```bash
# Ensure eBPF build infrastructure works
cargo build -p orb8-probes

# Should see these warnings (expected, no probes yet):
# warning: target filter `bins` specified, but no targets matched
# Finished `release` profile [optimized]

# Run clippy to ensure no warnings
cargo clippy -p orb8-probes -- -D warnings

# Format check
cargo fmt -p orb8-probes --check
```

## Troubleshooting

### Build Errors

**Error: `rust-src` component not found**
```bash
rustup component add rust-src --toolchain nightly
```

**Error: `bpf-linker` not found**
```bash
cargo install bpf-linker
```

**Error: eBPF verifier error**
- Check that kernel is 5.8+: `uname -r`
- Verify BTF is enabled: `ls /sys/kernel/btf/vmlinux`
- Ensure you're running with sufficient privileges (root or CAP_BPF)

### Lima VM Issues

**VM won't start**
```bash
# Check status
limactl list

# View logs
limactl shell orb8-dev dmesg

# Delete and recreate
make clean
make dev
```

**File sync issues**
```bash
# Lima auto-mounts your home directory
# Ensure you're working in a subdirectory of ~
pwd

# Restart VM
limactl stop orb8-dev
limactl start orb8-dev
```

### Kernel Requirements

**Check if BTF is enabled:**
```bash
# Should exist
ls /sys/kernel/btf/vmlinux

# If missing, rebuild kernel with CONFIG_DEBUG_INFO_BTF=y
# Or use a newer kernel version
```

**Check available capabilities:**
```bash
# List capabilities
capsh --print

# For eBPF, you need:
# CAP_BPF (kernel 5.8+) or CAP_SYS_ADMIN (older kernels)
# CAP_NET_ADMIN (for network hooks)
```

## Next Steps

- Read [ARCHITECTURE.md](ARCHITECTURE.md) for technical design
- Read [ROADMAP.md](ROADMAP.md) for implementation plan
- Check [CLAUDE.md](../CLAUDE.md) for coding guidelines
- See [README.md](../README.md) for project overview

## Resources

- [Aya Documentation](https://aya-rs.dev/book/)
- [eBPF Documentation](https://ebpf.io/)
- [Linux eBPF Reference](https://www.kernel.org/doc/html/latest/bpf/)
- [Lima Documentation](https://lima-vm.io/)
