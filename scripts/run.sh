#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

cleanup() {
    echo "Cleaning up..."
    pkill -f "lm-test" 2>/dev/null || true
    pkill -f "LaunchSimulator" 2>/dev/null || true
}

trap cleanup EXIT

echo "Starting Unity Simulator (windowed)..."
"$PROJECT_DIR/unity/LaunchSimulator/launch_simulator.app/Contents/MacOS/LaunchSimulator" -screen-fullscreen 0 -screen-width 800 -screen-height 450 &

echo "Starting Launch Monitor Test Harness..."
exec "$PROJECT_DIR/target/release/lm-test"
