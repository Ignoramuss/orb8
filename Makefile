# Platform detection
UNAME_S := $(shell uname -s)

.PHONY: help dev shell test build install uninstall clean stop status install-tools fmt clippy check magic build-local test-local install-local magic-local

help:
	@echo "orb8 Development Commands"
	@echo ""
	@echo "Quick Start:"
	@echo "  make magic        - Build, test, and install orb8 (one command)"
	@echo ""
	@echo "Development:"
	@echo "  make dev          - Setup development environment (creates Lima VM on macOS)"
	@echo "  make shell        - Enter development VM"
	@echo "  make test         - Run tests"
	@echo "  make build        - Build orb8"
	@echo "  make install      - Install orb8 to ~/.cargo/bin/"
	@echo "  make uninstall    - Remove orb8 from ~/.cargo/bin/"
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
	@echo "Installing orb8 to ~/.cargo/bin/..."
	@limactl shell orb8-dev bash -c "cd $(shell pwd) && cargo install --path ."
	@echo "Installation complete. Run 'orb8 --help' to verify."

uninstall:
	@echo "Uninstalling orb8..."
	@limactl shell orb8-dev bash -c "cargo uninstall orb8"
	@echo "orb8 has been uninstalled"

# Local commands (work directly on current OS, no VM)
build-local:
	@echo "Building orb8 locally..."
	@cargo build --release

test-local:
	@echo "Running tests locally..."
	@cargo test

install-local:
	@echo "Installing orb8 locally to ~/.cargo/bin/..."
	@cargo install --path .
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
	@limactl shell orb8-dev bash -c "cd $(shell pwd) && cargo build --release && cargo test && cargo install --path ."
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
