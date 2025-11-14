#!/usr/bin/env bash
set -euo pipefail

echo "Setting up orb8 development environment..."

# Check if Docker is running
if ! docker info > /dev/null 2>&1; then
    echo "Error: Docker is not running. Please start Docker Desktop or OrbStack."
    exit 1
fi
echo "Docker is running"

# Check if minikube is installed
if ! command -v minikube &> /dev/null; then
    echo "Installing minikube..."
    if [[ "$OSTYPE" == "darwin"* ]]; then
        brew install minikube
    else
        echo "Error: Please install minikube manually: https://minikube.sigs.k8s.io/docs/start/"
        exit 1
    fi
fi
echo "minikube installed"

# Check if kubectl is installed
if ! command -v kubectl &> /dev/null; then
    echo "Installing kubectl..."
    if [[ "$OSTYPE" == "darwin"* ]]; then
        brew install kubectl
    else
        echo "Error: Please install kubectl manually"
        exit 1
    fi
fi
echo "kubectl installed"

# Start minikube if not running
if ! minikube status > /dev/null 2>&1; then
    echo "Starting minikube cluster..."
    minikube start \
        --driver=docker \
        --cpus=2 \
        --memory=6144 \
        --kubernetes-version=v1.28.0 \
        --extra-config=kubelet.authentication-token-webhook=true
    echo "minikube started"
else
    echo "minikube already running"
fi

# Create orb8 namespace
echo "Creating orb8 namespace..."
kubectl create namespace orb8 --dry-run=client -o yaml | kubectl apply -f -
echo "Namespace created"

# Build dev container
echo "Building development container..."
docker-compose build dev
echo "Dev container built"

echo ""
echo "Development environment is ready!"
echo ""
echo "Next steps:"
echo "  1. Run 'make shell' to enter the dev container"
echo "  2. Run 'cargo build' inside the container to build orb8"
echo "  3. Run 'cargo test' to run tests"
echo ""
echo "Minikube dashboard: minikube dashboard"
echo "Minikube IP: $(minikube ip 2>/dev/null || echo 'not yet available')"
