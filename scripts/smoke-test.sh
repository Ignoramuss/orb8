#!/usr/bin/env bash
set -euo pipefail

# Smoke test: verify eBPF probes load, attach, and capture traffic.
# Runs the agent directly (no Kubernetes required).
# Must be run on Linux with root access and BTF-enabled kernel.

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
AGENT_BIN="$PROJECT_DIR/target/release/orb8-agent"
CLI_BIN="$PROJECT_DIR/target/release/orb8"
AGENT_PID=""
AGENT_LOG=$(mktemp /tmp/orb8-smoke-XXXXXX.log)
GRPC_PORT=9090
PASSED=0
FAILED=0

cleanup() {
    if [[ -n "$AGENT_PID" ]]; then
        # Kill the agent process tree (sudo + agent child)
        sudo pkill -9 -f "orb8-agent" 2>/dev/null || true
        wait "$AGENT_PID" 2>/dev/null || true
    fi
    rm -f "$AGENT_LOG"
}
trap cleanup EXIT

log() { echo "==> $*"; }
pass() { log "PASS: $*"; PASSED=$((PASSED + 1)); }
fail() { log "FAIL: $*"; FAILED=$((FAILED + 1)); }

# --- Pre-flight checks ---
log "Running pre-flight checks..."

if [[ "$(uname -s)" != "Linux" ]]; then
    echo "Error: smoke test requires Linux (eBPF). Use 'make smoke-test' to run in Lima VM."
    exit 1
fi

if [[ ! -f /sys/kernel/btf/vmlinux ]]; then
    echo "Error: BTF not available. Kernel must have CONFIG_DEBUG_INFO_BTF=y."
    exit 1
fi

if [[ $EUID -ne 0 ]] && ! sudo -n true 2>/dev/null; then
    echo "Error: root access required for eBPF probe loading."
    exit 1
fi

# --- Build ---
log "Building agent and CLI (release)..."
cd "$PROJECT_DIR"
cargo build --release -p orb8-agent -p orb8-cli 2>&1 | tail -3

if [[ ! -x "$AGENT_BIN" ]]; then
    echo "Error: agent binary not found at $AGENT_BIN"
    exit 1
fi

if [[ ! -x "$CLI_BIN" ]]; then
    echo "Error: CLI binary not found at $CLI_BIN"
    exit 1
fi

# --- Start agent ---
log "Starting orb8-agent (no Kubernetes)..."
sudo RUST_LOG=info "$AGENT_BIN" > "$AGENT_LOG" 2>&1 &
AGENT_PID=$!

# Wait for gRPC server to be ready
log "Waiting for gRPC server on port $GRPC_PORT..."
for i in $(seq 1 30); do
    if "$CLI_BIN" --agent "localhost:$GRPC_PORT" status >/dev/null 2>&1; then
        break
    fi
    if ! kill -0 "$AGENT_PID" 2>/dev/null; then
        echo "Error: agent exited prematurely. Logs:"
        cat "$AGENT_LOG"
        exit 1
    fi
    sleep 1
done

if ! "$CLI_BIN" --agent "localhost:$GRPC_PORT" status >/dev/null 2>&1; then
    echo "Error: agent did not become ready within 30s. Logs:"
    cat "$AGENT_LOG"
    exit 1
fi
log "Agent is ready."

# --- Check probe attachment in logs ---
log "Verifying probe attachment..."
if grep -q "Attached.*probe to" "$AGENT_LOG"; then
    pass "Probes attached to network interfaces"
else
    fail "No probe attachment found in logs"
    echo "--- Agent logs ---"
    cat "$AGENT_LOG"
    echo "--- End logs ---"
fi

# --- Generate traffic ---
log "Generating test traffic..."
# Use local-only traffic to avoid depending on external network access.
# The agent's gRPC queries themselves generate TCP traffic that gets captured,
# but we also generate additional traffic for good measure.
ping -c 3 -W 1 127.0.0.1 > /dev/null 2>&1 || true
# Try external but don't block on it (2s timeout)
timeout 2 curl -s -o /dev/null http://1.1.1.1 2>/dev/null || true

# Let the ring buffer poll cycle pick up events (agent polls every 100ms)
sleep 1

# --- Verify status ---
log "Querying agent status..."
STATUS_OUTPUT=$("$CLI_BIN" --agent "localhost:$GRPC_PORT" status 2>&1) || true
echo "$STATUS_OUTPUT"

EVENTS_PROCESSED=$(echo "$STATUS_OUTPUT" | grep -i "events processed" | grep -oE '[0-9]+' | tail -1)
if [[ -n "$EVENTS_PROCESSED" ]] && [[ "$EVENTS_PROCESSED" -gt 0 ]]; then
    pass "Events captured: $EVENTS_PROCESSED"
else
    fail "No events captured (events_processed = ${EVENTS_PROCESSED:-0})"
fi

ACTIVE_FLOWS=$(echo "$STATUS_OUTPUT" | grep -i "active flows" | grep -oE '[0-9]+' | tail -1)
if [[ -n "$ACTIVE_FLOWS" ]] && [[ "$ACTIVE_FLOWS" -gt 0 ]]; then
    pass "Flows aggregated: $ACTIVE_FLOWS"
else
    fail "No flows aggregated (active_flows = ${ACTIVE_FLOWS:-0})"
fi

# --- Verify flows ---
log "Querying flows..."
FLOWS_OUTPUT=$("$CLI_BIN" --agent "localhost:$GRPC_PORT" flows --limit 10 2>&1) || true
echo "$FLOWS_OUTPUT"

if echo "$FLOWS_OUTPUT" | grep -qE '(TCP|UDP|ICMP)'; then
    pass "Flows contain protocol information"
else
    fail "No protocol information in flows output"
fi

if echo "$FLOWS_OUTPUT" | grep -qE '[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+'; then
    pass "Flows contain IP addresses"
else
    fail "No IP addresses in flows output"
fi

# --- Verify drop counter ---
EVENTS_DROPPED=$(echo "$STATUS_OUTPUT" | grep -i "events dropped" | grep -oE '[0-9]+' | tail -1)
if [[ -n "$EVENTS_DROPPED" ]] && [[ "$EVENTS_DROPPED" -eq 0 ]]; then
    pass "No events dropped"
else
    log "NOTE: $EVENTS_DROPPED events dropped (ring buffer may be small for high traffic)"
fi

# --- Summary ---
echo ""
echo "========================================"
echo "  Smoke Test Results"
echo "========================================"
echo "  Passed: $PASSED"
echo "  Failed: $FAILED"
echo "========================================"

if [[ "$FAILED" -gt 0 ]]; then
    echo ""
    echo "--- Agent logs ---"
    cat "$AGENT_LOG"
    echo "--- End logs ---"
    exit 1
fi

exit 0
