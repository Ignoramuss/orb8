# Development Guide

This guide will help you set up your development environment for orb8.

## Quick Start (One Command)

The fastest way to start developing orb8:

```bash
make magic
```

This command will:
- Set up the development environment (creates VM on macOS, skips on Linux)
- Build orb8
- Run all tests
- Install orb8 to your PATH

First run on macOS takes 5-10 minutes (downloading Ubuntu image and installing tools).
Subsequent runs take ~2 minutes (build + test + install).

**On Linux:** Runs natively without a VM.
**On macOS:** Uses Lima/QEMU VM for Linux environment.

After `make magic` completes:

**macOS:**
```bash
make shell        # Enter the VM
orb8 --help       # orb8 is installed and ready
orb8 trace network --namespace default
```

**Linux:**
```bash
orb8 --help       # orb8 is installed and ready
sudo orb8 trace network --namespace default  # eBPF requires root
```

### Common Commands

**Quick workflow:**
```bash
make magic        # Build, test, install (adapts to your platform)
make magic-local  # Build, test, install locally (no VM)
```

**Development:**
```bash
make dev          # Create/start VM (macOS only)
make shell        # Enter VM
make test         # Run tests
make build        # Build release binary
make install      # Install orb8 to ~/.cargo/bin/
make uninstall    # Remove orb8 from ~/.cargo/bin/
```

**Local (no VM):**
```bash
make build-local  # Build on current OS
make test-local   # Test on current OS
make install-local # Install on current OS
```

**Code quality:**
```bash
make check        # Run cargo check
make fmt          # Format code
make clippy       # Lint code
```

**VM management:**
```bash
make status       # Check VM status
make stop         # Stop VM (keeps it for later)
make clean        # Delete VM completely
```

---

## Why Lima/QEMU?

orb8 uses **Lima** (Linux Machines) which provides:

1. **Real Linux Environment**: Full Linux kernel, not just containers
2. **eBPF Support**: Direct access to kernel for eBPF programs
3. **No Permission Issues**: Proper user setup, no Docker volume problems
4. **Professional Setup**: How infrastructure teams actually do development
5. **Works on ARM Macs**: Native performance on M1/M2/M3

### Architecture

```
Your macOS
  └─ Lima (wraps QEMU)
       └─ Ubuntu 22.04 VM
            ├─ Full Linux kernel (eBPF works!)
            ├─ Rust toolchain
            ├─ Your code (mounted from macOS)
            └─ minikube (for Kubernetes testing)
```

---

## Prerequisites

### Required

- **macOS** (Lima works best on macOS with M-series or Intel)
- **Homebrew**: Install from [brew.sh](https://brew.sh)
  ```bash
  /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
  ```

### Automatically Installed

The setup script (`make dev`) automatically installs:
- Lima
- QEMU (via Lima)
- Ubuntu VM with all tools

---

## Manual Setup (if needed)

If you want to install things manually:

```bash
# 1. Install Lima
brew install lima

# 2. Create VM
limactl create --name=orb8-dev .lima/orb8-dev.yaml

# 3. Start VM
limactl start orb8-dev

# 4. Enter VM
limactl shell orb8-dev
```

---

## Development Workflow

### Typical Session

```bash
# Start your day
make dev                    # Starts VM (instant if already created)
make shell                  # Enter Linux environment

# Inside VM
cd /path/to/orb8
cargo build                 # Build
cargo test                  # Test
cargo run -- trace network  # Run

# Make code changes on macOS (in your editor)
# Code is automatically synced to VM

# Test changes
cargo test

# Exit VM
exit

# End your day
make stop                   # Stop VM to save resources
```

### Working with Kubernetes

```bash
make shell

# Inside VM
minikube start              # Start local Kubernetes
kubectl get nodes           # Verify cluster

# Deploy test workload
kubectl run test-pod --image=nginx
kubectl get pods

# Run orb8 against cluster
cargo run -- info --namespace default
```

---

## Project Structure

```
orb8/
├── src/
│   ├── main.rs           # CLI entry point
│   ├── lib.rs            # Library root
│   ├── error.rs          # Custom error types
│   ├── cli/              # CLI module
│   ├── ebpf/             # eBPF probe management
│   ├── k8s/              # Kubernetes integration
│   ├── metrics/          # Metrics collection & export
│   └── ui/               # TUI dashboard
├── tests/
│   └── integration_test.rs
├── .lima/
│   └── orb8-dev.yaml     # VM configuration
├── scripts/
│   ├── setup-lima.sh     # VM setup script
│   └── check-prerequisites.sh
├── docs/
│   └── ARCHITECTURE.md
├── Cargo.toml
├── Makefile
└── README.md
```

---

## Troubleshooting

### VM Won't Start

```bash
# Check VM status
make status

# View VM logs
limactl start orb8-dev --debug

# Delete and recreate
make clean
make dev
```

### Can't Find Project Directory in VM

Your project is mounted at the same path as on macOS:
```bash
# If your project is at:
# /Users/mayankdighe/workspace/orb8

# In the VM, it's also at:
# /Users/mayankdighe/workspace/orb8

cd /Users/mayankdighe/workspace/orb8
```

### Rust Not Found in VM

```bash
# Inside VM, source Rust environment
source $HOME/.cargo/env
rustc --version
```

### Build Errors

```bash
# Clean and rebuild
cargo clean
cargo build
```

### Out of Disk Space

```bash
# VM disk is 20GB. To check:
make shell
df -h

# To increase, edit .lima/orb8-dev.yaml and recreate:
# disk: "100GiB"
make clean
make dev
```

---

## Advanced Usage

### SSH into VM Directly

```bash
# Lima provides SSH access
lima orb8-dev

# Or use limactl
limactl shell orb8-dev
```

### Run Commands in VM from macOS

```bash
# Without entering VM
limactl shell orb8-dev bash -c "cd ~/workspace/orb8 && cargo build"

# Or use make targets
make build
make test
```

### Customize VM Resources

Edit `.lima/orb8-dev.yaml`:

```yaml
cpus: 8              # More CPUs
memory: "16GiB"      # More RAM
disk: "100GiB"       # More disk
```

Then recreate:
```bash
make clean
make dev
```

---

## Testing

### Unit Tests

```bash
make shell
cargo test --lib
```

### Integration Tests

```bash
make shell
cargo test --test integration_test
```

### All Tests

```bash
make test
```

---

## Code Quality

```bash
# Format code
make fmt

# Lint code
make clippy

# Check compilation
make check
```

---

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for detailed contribution guidelines.

### Before Submitting a PR

```bash
make fmt           # Format code
make clippy        # Fix all warnings
make test          # Ensure tests pass
```

---

## Resources

- **Lima**: https://lima-vm.io/
- **QEMU**: https://www.qemu.org/
- **Rust Book**: https://doc.rust-lang.org/book/
- **eBPF Documentation**: https://ebpf.io/
- **aya Book**: https://aya-rs.dev/book/
- **kube-rs**: https://kube.rs/

---

## Getting Help

- Open an [issue](https://github.com/Ignoramuss/orb8/issues)
- Check existing issues and discussions
- Read [ARCHITECTURE.md](docs/ARCHITECTURE.md) for design details
