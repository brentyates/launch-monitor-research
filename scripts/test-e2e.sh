#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
SHARED_MEM="$PROJECT_DIR/LaunchMonitorSharedMemory"
UNITY_STARTUP_TIMEOUT=30

cleanup() {
    echo "Cleaning up..."
    pkill -f "LaunchSimulator" 2>/dev/null || true
}

trap cleanup EXIT

echo "=== E2E Test Suite ==="
echo ""

rm -f "$SHARED_MEM"

echo "Building release..."
cargo build --manifest-path "$PROJECT_DIR/Cargo.toml" --release --bin lm-test --quiet

echo "Starting Unity Simulator (windowed)..."
"$PROJECT_DIR/unity/LaunchSimulator/launch_simulator.app/Contents/MacOS/LaunchSimulator" -screen-fullscreen 0 -screen-width 800 -screen-height 450 -popupwindow &

echo "Waiting for Unity to create shared memory (max ${UNITY_STARTUP_TIMEOUT}s)..."
elapsed=0
while [ ! -f "$SHARED_MEM" ]; do
    if [ $elapsed -ge $UNITY_STARTUP_TIMEOUT ]; then
        echo "ERROR: Unity failed to create shared memory within ${UNITY_STARTUP_TIMEOUT}s"
        exit 1
    fi
    elapsed=$((elapsed + 1))
    printf "."
    sleep 1
done
echo ""
echo "Unity ready! (took ${elapsed}s)"

echo "Waiting for Unity to fully initialize..."
sleep 2

echo ""
echo "Running E2E test..."
"$PROJECT_DIR/target/release/lm-test"
TEST_EXIT=$?

echo ""
if [ $TEST_EXIT -eq 0 ]; then
    echo "=== E2E TEST PASSED ==="
else
    echo "=== E2E TEST FAILED ==="
fi

exit $TEST_EXIT
