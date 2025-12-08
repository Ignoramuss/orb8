# Platform detection
UNAME_S := $(shell uname -s)

.PHONY: help dev shell test build install uninstall clean stop status install-tools fmt clippy check magic build-local test-local install-local magic-local build-ebpf verify-setup

help:
	@echo "orb8 Development Commands"
	@echo ""
	@echo "Quick Start:"
	@echo "  make magic        - Build, test, and install orb8 (one command)"
	@echo "  make verify-setup - Verify development environment is properly configured"
	@echo ""
	@echo "Development:"
	@echo "  make dev          - Setup development environment (creates Lima VM on macOS)"
	@echo "  make shell        - Enter development VM"
	@echo "  make test         - Run tests"
	@echo "  make build        - Build orb8"
	@echo "  make build-ebpf   - Build only eBPF probes"
	@echo "  make build-agent  - Build orb8-agent"
	@echo "  make run-agent    - Build and run orb8-agent (requires sudo)"
	@echo "  make install      - Install orb8 to ~/.cargo/bin/"
	@echo "  make uninstall    - Remove orb8 from ~/.cargo/bin/"
	@echo ""
	@echo "Code Quality:"
	@echo "  make fmt          - Format code"
	@echo "  make clippy       - Run linter"
	@echo "  make check        - Type check"
	@echo ""
	@echo "Local (no VM):"
	@echo "  make build-local  - Build on current OS"
	@echo "  make test-local   - Test on current OS"
	@echo "  make install-local - Install on current OS"
	@echo "  make magic-local  - Build, test, install locally"
	@echo ""
	@echo "Management:"
	@echo "  make clean        - Delete VM and cleanup"
	@echo "  make stop         - Stop VM (keeps it for later)"
	@echo "  make status       - Check VM status"
	@echo "  make install-tools - Install Lima via Homebrew"
	@echo ""

dev:
	@./scripts/check-prerequisites.sh
	@./scripts/setup-lima.sh
	@echo ""
	@echo "Development environment ready!"
	@echo "Run 'make shell' to enter the VM"

shell:
	@limactl shell orb8-dev

test:
	@echo "Running tests in VM..."
	@limactl shell orb8-dev bash -c "cd $(shell pwd) && cargo test"

build:
	@echo "Building orb8 in VM..."
	@limactl shell orb8-dev bash -c "cd $(shell pwd) && cargo build --release"

install:
	@echo "Installing orb8-cli and orb8-agent to ~/.cargo/bin/..."
	@limactl shell orb8-dev bash -c "cd $(shell pwd) && cargo install --path orb8-cli && cargo install --path orb8-agent"
	@echo "Installation complete. Run 'orb8 --help' to verify."

uninstall:
	@echo "Uninstalling orb8-cli and orb8-agent..."
	@limactl shell orb8-dev bash -c "cargo uninstall orb8-cli orb8-agent"
	@echo "orb8 has been uninstalled"

# Local commands (work directly on current OS, no VM)
build-local:
	@echo "Building orb8 locally..."
	@cargo build --release

test-local:
	@echo "Running tests locally..."
	@cargo test

install-local:
	@echo "Installing orb8-cli and orb8-agent locally to ~/.cargo/bin/..."
	@cargo install --path orb8-cli
	@cargo install --path orb8-agent
	@echo "Installation complete. Run 'orb8 --help' to verify."

magic-local: build-local test-local install-local
	@echo ""
	@echo "Build, test, and install complete!"
	@echo "orb8 is now available in your PATH."
	@echo "Run 'orb8 --help' to get started."

# Magic command (uses VM on macOS, local on Linux)
ifeq ($(UNAME_S),Linux)
magic: magic-local
	@echo "Running on Linux - used native build"
else
magic: dev
	@echo "Building, testing, and installing orb8..."
	@limactl shell orb8-dev bash -c "cd $(shell pwd) && cargo build --release && cargo test && cargo install --path orb8-cli && cargo install --path orb8-agent"
	@echo ""
	@echo "Build, test, and install complete!"
	@echo "Inside the VM, orb8 is now available in your PATH."
	@echo "Run 'make shell' then 'orb8 --help' to get started."
endif

fmt:
	@limactl shell orb8-dev bash -c "cd $(shell pwd) && cargo fmt"

clippy:
	@limactl shell orb8-dev bash -c "cd $(shell pwd) && cargo clippy -- -D warnings"

check:
	@limactl shell orb8-dev bash -c "cd $(shell pwd) && cargo check"

status:
	@limactl list

stop:
	@echo "Stopping VM..."
	@limactl stop orb8-dev || echo "VM already stopped"

clean:
	@echo "Deleting orb8-dev VM..."
	@limactl delete orb8-dev || echo "VM already deleted"
	@echo "Cleanup complete"

install-tools:
	@echo "Installing development tools via Homebrew..."
	@which brew > /dev/null || (echo "Error: Homebrew not found. Install from https://brew.sh" && exit 1)
	@which limactl > /dev/null || brew install lima
	@which qemu-system-aarch64 > /dev/null || brew install qemu
	@echo "Tools installed (Lima + QEMU)"

# Build eBPF probes specifically
build-ebpf:
ifeq ($(UNAME_S),Linux)
	@echo "Building eBPF probes..."
	@cargo build -p orb8-probes
else
	@echo "Building eBPF probes in VM..."
	@limactl shell orb8-dev bash -c "cd $(shell pwd) && cargo build -p orb8-probes"
endif

# Build and run the agent (for testing)
run-agent:
ifeq ($(UNAME_S),Linux)
	@echo "Building and running orb8-agent..."
	@cargo build -p orb8-agent
	@sudo ./target/debug/orb8-agent
else
	@echo "Building and running orb8-agent in VM..."
	@limactl shell orb8-dev bash -c "cd $(shell pwd) && cargo build -p orb8-agent && sudo ./target/debug/orb8-agent"
endif

# Build agent only
build-agent:
ifeq ($(UNAME_S),Linux)
	@echo "Building orb8-agent..."
	@cargo build -p orb8-agent
else
	@echo "Building orb8-agent in VM..."
	@limactl shell orb8-dev bash -c "cd $(shell pwd) && cargo build -p orb8-agent"
endif

# Verify development environment setup
verify-setup:
ifeq ($(UNAME_S),Linux)
	@echo "Verifying Linux development environment..."
	@echo "✓ Platform: Linux (native eBPF support)"
	@rustc --version || (echo "✗ Rust not installed" && exit 1)
	@rustc +nightly --version || (echo "✗ Rust nightly not installed" && exit 1)
	@rustup component list --installed --toolchain nightly | grep -q rust-src || (echo "✗ rust-src not installed (run: rustup component add rust-src --toolchain nightly)" && exit 1)
	@which bpf-linker > /dev/null || (echo "✗ bpf-linker not installed (run: cargo install bpf-linker)" && exit 1)
	@which bpftool > /dev/null || (echo "✗ bpftool not installed" && exit 1)
	@echo "✓ Rust stable: $$(rustc --version)"
	@echo "✓ Rust nightly: $$(rustc +nightly --version)"
	@echo "✓ rust-src component: installed"
	@echo "✓ bpf-linker: $$(bpf-linker --version)"
	@echo "✓ bpftool: $$(bpftool version)"
	@echo "✓ Kernel: $$(uname -r)"
	@test -f /sys/kernel/btf/vmlinux && echo "✓ BTF enabled" || echo "⚠ BTF not found (some eBPF features may not work)"
	@echo ""
	@echo "Environment ready for eBPF development!"
else
	@echo "Verifying macOS development environment..."
	@echo "✓ Platform: macOS (using Lima VM for eBPF)"
	@which limactl > /dev/null || (echo "✗ Lima not installed (run: make install-tools)" && exit 1)
	@echo "✓ Lima installed"
	@limactl list | grep -q orb8-dev && echo "✓ orb8-dev VM exists" || (echo "⚠ orb8-dev VM not created (run: make dev)" && exit 0)
	@limactl list | grep -q "orb8-dev.*Running" && echo "✓ VM is running" || echo "⚠ VM is stopped (run: make dev to start)"
	@rustc --version > /dev/null 2>&1 && echo "✓ Rust (local): $$(rustc --version)" || echo "⚠ Rust not installed locally (optional for macOS)"
	@rustc +nightly --version > /dev/null 2>&1 && echo "✓ Rust nightly (local): installed" || echo "⚠ Rust nightly not installed locally (optional for macOS)"
	@rustup component list --installed --toolchain nightly 2>/dev/null | grep -q rust-src && echo "✓ rust-src component (local): installed" || echo "⚠ rust-src not installed locally (run: rustup component add rust-src --toolchain nightly)"
	@echo ""
	@echo "Environment ready! Use 'make shell' to enter VM for full eBPF development."
endif
