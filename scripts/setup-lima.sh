#!/usr/bin/env bash
set -euo pipefail

echo "Setting up orb8 development environment with Lima..."

# Check if Homebrew is installed (macOS)
if [[ "$OSTYPE" == "darwin"* ]]; then
    if ! command -v brew &> /dev/null; then
        echo "Error: Homebrew not found. Install from https://brew.sh"
        exit 1
    fi
    echo "Homebrew found"
fi

# Install Lima if not present
if ! command -v limactl &> /dev/null; then
    echo "Installing Lima..."
    if [[ "$OSTYPE" == "darwin"* ]]; then
        brew install lima
    else
        echo "Error: Please install Lima manually: https://lima-vm.io/docs/installation/"
        exit 1
    fi
    echo "Lima installed"
else
    echo "Lima already installed"
fi

# Install QEMU if not present
if ! command -v qemu-system-aarch64 &> /dev/null && ! command -v qemu-system-x86_64 &> /dev/null; then
    echo "Installing QEMU..."
    if [[ "$OSTYPE" == "darwin"* ]]; then
        brew install qemu
    else
        echo "Error: Please install QEMU manually"
        exit 1
    fi
    echo "QEMU installed"
else
    echo "QEMU already installed"
fi

# Check if VM already exists
if limactl list | grep -q "orb8-dev"; then
    echo "orb8-dev VM already exists"

    # Check if it's running
    if limactl list | grep "orb8-dev" | grep -q "Running"; then
        echo "VM is already running"
    else
        echo "Starting existing VM..."
        limactl start orb8-dev
    fi
else
    echo "Creating orb8-dev VM..."
    echo "This will take 5-10 minutes on first run (downloading Ubuntu image and installing tools)"
    echo ""

    # Create VM from config
    limactl create --name=orb8-dev .lima/orb8-dev.yaml

    # Start the VM
    limactl start orb8-dev
fi

echo ""
echo "Development environment ready!"
echo ""
echo "To enter the VM:"
echo "  make shell"
echo "  (or: limactl shell orb8-dev)"
echo ""
echo "Inside the VM, navigate to your project:"
echo "  cd $(pwd)"
echo "  cargo build"
echo "  cargo test"
echo ""
