#!/usr/bin/env bash
set -euo pipefail

# E2E test: build Docker image, deploy to kind, verify pod enrichment.
# Must be run on Linux with Docker, kind, and kubectl available.

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
CLI_BIN="$PROJECT_DIR/target/release/orb8"
CLUSTER_NAME="orb8-test"
IMAGE_NAME="orb8-agent:test"
FORWARD_PID=""
PASSED=0
FAILED=0

cleanup() {
    if [[ -n "$FORWARD_PID" ]]; then
        kill "$FORWARD_PID" 2>/dev/null || true
    fi
    log "Cleaning up..."
    kubectl delete -f "$PROJECT_DIR/deploy/daemonset.yaml" --ignore-not-found 2>/dev/null || true
    kubectl delete pod traffic-gen --ignore-not-found 2>/dev/null || true
}
trap cleanup EXIT

log() { echo "==> $*"; }
pass() { log "PASS: $*"; PASSED=$((PASSED + 1)); }
fail() { log "FAIL: $*"; FAILED=$((FAILED + 1)); }

# --- Pre-flight ---
log "Running pre-flight checks..."

for cmd in docker kind kubectl; do
    if ! command -v "$cmd" &>/dev/null; then
        echo "Error: $cmd not found"
        exit 1
    fi
done

# --- Build agent + CLI locally (faster than Docker multi-stage) ---
log "Building agent and CLI (release)..."
cd "$PROJECT_DIR"
cargo build --release -p orb8-agent -p orb8-cli 2>&1 | tail -5

# --- Build Docker image using pre-built binary ---
log "Building Docker image ($IMAGE_NAME)..."
docker build --target=local -t "$IMAGE_NAME" "$PROJECT_DIR" 2>&1 | tail -5

# --- Create kind cluster (always fresh) ---
if kind get clusters 2>/dev/null | grep -q "^${CLUSTER_NAME}$"; then
    log "Deleting existing kind cluster..."
    kind delete cluster --name "$CLUSTER_NAME"
fi
log "Creating kind cluster: $CLUSTER_NAME..."
kind create cluster --name "$CLUSTER_NAME" --config "$PROJECT_DIR/deploy/kind-config.yaml"

# --- Load image into kind ---
log "Loading image into kind..."
kind load docker-image "$IMAGE_NAME" --name "$CLUSTER_NAME"

# --- Deploy DaemonSet ---
log "Deploying orb8-agent DaemonSet..."
kubectl apply -f "$PROJECT_DIR/deploy/daemonset.yaml"

# --- Wait for pods to be ready ---
log "Waiting for agent pods to be ready..."
for i in $(seq 1 120); do
    READY=$(kubectl get ds orb8-agent -o jsonpath='{.status.numberReady}' 2>/dev/null || echo "0")
    DESIRED=$(kubectl get ds orb8-agent -o jsonpath='{.status.desiredNumberScheduled}' 2>/dev/null || echo "0")
    if [[ "$READY" -gt 0 ]] && [[ "$READY" -eq "$DESIRED" ]]; then
        break
    fi
    if [[ $((i % 10)) -eq 0 ]]; then
        log "  Waiting... ($READY/$DESIRED ready, ${i}s elapsed)"
    fi
    sleep 1
done

if [[ "$READY" -eq 0 ]] || [[ "$READY" -ne "$DESIRED" ]]; then
    echo "Error: DaemonSet not ready after 120s ($READY/$DESIRED)"
    kubectl describe ds orb8-agent
    kubectl logs -l app=orb8-agent --tail=50
    exit 1
fi
log "DaemonSet ready: $READY/$DESIRED pods"

# --- Check agent logs for probe attachment ---
log "Checking agent logs..."
AGENT_POD=$(kubectl get pods -l app=orb8-agent -o jsonpath='{.items[0].metadata.name}')
AGENT_LOGS=$(kubectl logs "$AGENT_POD" 2>&1)

if echo "$AGENT_LOGS" | grep -q "Attached.*probe to"; then
    pass "Probes attached to network interfaces"
else
    fail "No probe attachment found in agent logs"
    echo "$AGENT_LOGS"
fi

if echo "$AGENT_LOGS" | grep -q "Pod watcher initial sync complete"; then
    pass "Pod watcher synced"
else
    fail "Pod watcher did not sync"
fi

# --- Port-forward to agent ---
log "Setting up port-forward..."
kubectl port-forward "$AGENT_POD" 19090:9090 &>/dev/null &
FORWARD_PID=$!
sleep 2

# --- Wait for CLI connectivity ---
for i in $(seq 1 10); do
    if "$CLI_BIN" --agent "localhost:19090" status >/dev/null 2>&1; then
        break
    fi
    sleep 1
done

# --- Generate inter-pod traffic ---
log "Generating pod-to-pod traffic..."
kubectl run traffic-gen --image=curlimages/curl:latest --restart=Never \
    --command -- sh -c "for i in \$(seq 1 10); do curl -s -o /dev/null http://kubernetes.default.svc.cluster.local/healthz 2>/dev/null || true; sleep 0.5; done" \
    2>/dev/null || true

# Wait for traffic generation + ring buffer poll
sleep 8

# --- Verify status ---
log "Querying agent status..."
STATUS_OUTPUT=$("$CLI_BIN" --agent "localhost:19090" status 2>&1) || true
echo "$STATUS_OUTPUT"

EVENTS_PROCESSED=$(echo "$STATUS_OUTPUT" | grep -i "events processed" | grep -oE '[0-9]+' | tail -1)
if [[ -n "$EVENTS_PROCESSED" ]] && [[ "$EVENTS_PROCESSED" -gt 0 ]]; then
    pass "Events captured: $EVENTS_PROCESSED"
else
    fail "No events captured"
fi

PODS_TRACKED=$(echo "$STATUS_OUTPUT" | grep -i "pods tracked" | grep -oE '[0-9]+' | tail -1)
if [[ -n "$PODS_TRACKED" ]] && [[ "$PODS_TRACKED" -gt 0 ]]; then
    pass "Pods tracked: $PODS_TRACKED"
else
    fail "No pods tracked (pod watcher may not have synced)"
fi

# --- Verify flows have pod enrichment ---
log "Querying flows..."
FLOWS_OUTPUT=$("$CLI_BIN" --agent "localhost:19090" flows --limit 20 2>&1) || true
echo "$FLOWS_OUTPUT"

if echo "$FLOWS_OUTPUT" | grep -qE '(TCP|UDP|ICMP)'; then
    pass "Flows contain protocol information"
else
    fail "No protocol information in flows"
fi

# Check for real pod names (not just external/unknown)
if echo "$FLOWS_OUTPUT" | grep -vE '(NAMESPACE|external|^$|-----)' | grep -q '/'; then
    pass "Flows contain pod-enriched entries"
else
    fail "No pod-enriched flows found (all external/unknown)"
fi

# --- Summary ---
echo ""
echo "========================================"
echo "  E2E Test Results"
echo "========================================"
echo "  Passed: $PASSED"
echo "  Failed: $FAILED"
echo "========================================"

if [[ "$FAILED" -gt 0 ]]; then
    echo ""
    echo "--- Agent logs ---"
    kubectl logs "$AGENT_POD" --tail=100
    echo "--- End logs ---"
    exit 1
fi

exit 0
