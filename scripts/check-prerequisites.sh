#!/usr/bin/env bash
set -euo pipefail

echo "Checking prerequisites..."

# Check Homebrew (macOS only)
if [[ "$OSTYPE" == "darwin"* ]]; then
    if ! command -v brew &> /dev/null; then
        echo "Warning: Homebrew not found. Install from https://brew.sh"
        echo "Lima will be installed via Homebrew during setup."
        exit 1
    fi
fi

# Check if Lima is installed (will be installed by setup script if missing)
if ! command -v limactl &> /dev/null; then
    echo "Lima not found. Will be installed during 'make dev'"
fi

echo "All prerequisites checked"
