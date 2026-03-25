#!/usr/bin/env bash
set -euo pipefail

# E2E test: build Docker image, deploy to kind, verify pod enrichment
# across all supported network modes.
#
# Tests:
#   1. hostNetwork pods     — agent's own traffic (shares node IP)
#   2. Regular pods         — cross-node pod-to-pod by IP
#   3. Service ClusterIP    — cross-node via Service (DNAT to pod IP)
#
# Not tested (known limitation):
#   - Same-node pod-to-pod traffic (stays on veth, never hits eth0)

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
    kubectl delete -f "$PROJECT_DIR/deploy/e2e-test-pods.yaml" --ignore-not-found 2>/dev/null || true
    kubectl delete -f "$PROJECT_DIR/deploy/daemonset.yaml" --ignore-not-found 2>/dev/null || true
}
trap cleanup EXIT

log() { echo "==> $*"; }
pass() { log "PASS: $*"; PASSED=$((PASSED + 1)); }
fail() { log "FAIL: $*"; FAILED=$((FAILED + 1)); }

wait_for_pod() {
    local pod_name="$1"
    local timeout="${2:-120}"
    for i in $(seq 1 "$timeout"); do
        local phase
        phase=$(kubectl get pod "$pod_name" -o jsonpath='{.status.phase}' 2>/dev/null || echo "")
        if [[ "$phase" == "Running" ]]; then
            return 0
        fi
        if [[ $((i % 10)) -eq 0 ]]; then
            log "  Waiting for $pod_name... (${phase:-Pending}, ${i}s)"
        fi
        sleep 1
    done
    echo "Error: pod $pod_name not Running after ${timeout}s"
    kubectl describe pod "$pod_name"
    return 1
}

# --- Pre-flight ---
log "Running pre-flight checks..."

for cmd in docker kind kubectl; do
    if ! command -v "$cmd" &>/dev/null; then
        echo "Error: $cmd not found"
        exit 1
    fi
done

# --- Build agent + CLI locally ---
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

# =====================================================================
# PHASE 1: Deploy agent DaemonSet
# =====================================================================

log "Deploying orb8-agent DaemonSet..."
kubectl apply -f "$PROJECT_DIR/deploy/daemonset.yaml"

log "Waiting for agent pods to be ready..."
for i in $(seq 1 120); do
    READY=$(kubectl get ds orb8-agent -o jsonpath='{.status.numberReady}' 2>/dev/null || echo "0")
    DESIRED=$(kubectl get ds orb8-agent -o jsonpath='{.status.desiredNumberScheduled}' 2>/dev/null || echo "0")
    if [[ "$READY" -gt 0 ]] && [[ "$READY" -eq "$DESIRED" ]]; then
        break
    fi
    if [[ $((i % 10)) -eq 0 ]]; then
        log "  Waiting... ($READY/$DESIRED ready, ${i}s)"
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

# Find the worker agent pod (hostNetwork, so port-forward uses node port)
WORKER_AGENT=$(kubectl get pods -l app=orb8-agent --field-selector spec.nodeName=orb8-test-worker -o jsonpath='{.items[0].metadata.name}')
AGENT_LOGS=$(kubectl logs "$WORKER_AGENT" 2>&1)

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

# --- Port-forward to worker agent ---
log "Setting up port-forward to worker agent..."
kubectl port-forward "$WORKER_AGENT" 19090:9090 &>/dev/null &
FORWARD_PID=$!
sleep 2

for i in $(seq 1 10); do
    if "$CLI_BIN" --agent "localhost:19090" status >/dev/null 2>&1; then
        break
    fi
    sleep 1
done

# =====================================================================
# PHASE 2: Deploy test pods for regular + service traffic
# =====================================================================

log "Deploying test pods (echo-server on control-plane, traffic-gen on worker)..."
kubectl apply -f "$PROJECT_DIR/deploy/e2e-test-pods.yaml"

wait_for_pod "echo-server" 120
wait_for_pod "traffic-gen" 120

ECHO_IP=$(kubectl get pod echo-server -o jsonpath='{.status.podIP}')
log "echo-server pod IP: $ECHO_IP"

# Wait for pod watcher to pick up the new pods
sleep 5

# =====================================================================
# TEST 1: hostNetwork pod traffic (agent's own K8s API calls)
# =====================================================================
log ""
log "--- TEST 1: hostNetwork pod traffic ---"

STATUS_OUTPUT=$("$CLI_BIN" --agent "localhost:19090" status 2>&1) || true
echo "$STATUS_OUTPUT"

EVENTS_PROCESSED=$(echo "$STATUS_OUTPUT" | grep -i "events processed" | grep -oE '[0-9]+' | tail -1)
if [[ -n "$EVENTS_PROCESSED" ]] && [[ "$EVENTS_PROCESSED" -gt 0 ]]; then
    pass "[hostNetwork] Events captured: $EVENTS_PROCESSED"
else
    fail "[hostNetwork] No events captured"
fi

PODS_TRACKED=$(echo "$STATUS_OUTPUT" | grep -i "pods tracked" | grep -oE '[0-9]+' | tail -1)
if [[ -n "$PODS_TRACKED" ]] && [[ "$PODS_TRACKED" -gt 0 ]]; then
    pass "[hostNetwork] Pods tracked: $PODS_TRACKED"
else
    fail "[hostNetwork] No pods tracked"
fi

# =====================================================================
# TEST 2: Regular pod traffic (cross-node, by pod IP)
# =====================================================================
log ""
log "--- TEST 2: Regular pod traffic (cross-node by pod IP) ---"

log "Generating cross-node traffic: traffic-gen → echo-server ($ECHO_IP)..."
for i in $(seq 1 5); do
    kubectl exec traffic-gen -- curl -s -o /dev/null --max-time 5 "http://${ECHO_IP}/" 2>/dev/null || true
done

sleep 2

# Use pod filter to isolate traffic-gen flows (avoids being drowned out by bulk traffic)
FLOWS_OUTPUT=$("$CLI_BIN" --agent "localhost:19090" flows --pod traffic-gen --limit 50 2>&1) || true
echo "$FLOWS_OUTPUT"

# traffic-gen is on the worker node, so its IP should be enriched by the worker agent
if echo "$FLOWS_OUTPUT" | grep -q "traffic-gen"; then
    pass "[Regular pod] traffic-gen appears in flows"
else
    fail "[Regular pod] traffic-gen not found in flows"
fi

# The destination is echo-server's pod IP — check the worker agent sees it
if echo "$FLOWS_OUTPUT" | grep -q "$ECHO_IP"; then
    pass "[Regular pod] echo-server pod IP ($ECHO_IP) visible as destination"
else
    fail "[Regular pod] echo-server pod IP ($ECHO_IP) not found in traffic-gen flows"
fi

# =====================================================================
# TEST 3: Service ClusterIP traffic (cross-node via DNAT)
# =====================================================================
log ""
log "--- TEST 3: Service ClusterIP traffic (cross-node via DNAT) ---"

SVC_IP=$(kubectl get svc echo-svc -o jsonpath='{.spec.clusterIP}')
log "echo-svc ClusterIP: $SVC_IP"

log "Generating Service traffic: traffic-gen → echo-svc ($SVC_IP)..."
for i in $(seq 1 5); do
    kubectl exec traffic-gen -- curl -s -o /dev/null --max-time 5 "http://echo-svc.default.svc.cluster.local/" 2>/dev/null || true
done

sleep 2

FLOWS_OUTPUT=$("$CLI_BIN" --agent "localhost:19090" flows --pod traffic-gen --limit 50 2>&1) || true
echo "$FLOWS_OUTPUT"

# After DNAT, the destination IP is the echo-server's pod IP (not the ClusterIP).
# kube-proxy rewrites dst before the packet hits eth0.
# Verify the Service ClusterIP does NOT appear (confirming DNAT happened before TC)
if echo "$FLOWS_OUTPUT" | grep -q "$SVC_IP"; then
    fail "[Service] ClusterIP ($SVC_IP) visible in flows (DNAT not applied before TC hook)"
else
    pass "[Service] ClusterIP ($SVC_IP) correctly absent (DNAT applied before TC hook)"
fi

# The DNAT'd traffic should show echo-server's real pod IP as destination
if echo "$FLOWS_OUTPUT" | grep -q "$ECHO_IP"; then
    pass "[Service] Traffic resolved to echo-server pod IP after DNAT"
else
    fail "[Service] echo-server pod IP ($ECHO_IP) not found after Service DNAT"
fi

# =====================================================================
# SUMMARY
# =====================================================================
echo ""
echo "========================================"
echo "  E2E Test Results"
echo "========================================"
echo "  Passed: $PASSED"
echo "  Failed: $FAILED"
echo "========================================"
echo ""
echo "  Network modes tested:"
echo "    hostNetwork pods      - agent's own traffic"
echo "    Regular pods          - cross-node pod-to-pod by IP"
echo "    Service ClusterIP     - cross-node via DNAT"
echo ""
echo "  Known limitations (not tested):"
echo "    Same-node pod traffic - stays on veth, invisible on eth0"
echo "========================================"

if [[ "$FAILED" -gt 0 ]]; then
    echo ""
    echo "--- Worker agent logs ---"
    kubectl logs "$WORKER_AGENT" --tail=100
    echo "--- End logs ---"
    exit 1
fi

exit 0
