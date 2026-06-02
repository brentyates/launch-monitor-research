#!/usr/bin/env bash
set -e

ROOT="$(cd "$(dirname "$0")/.." && pwd)"

rm -rf "$ROOT/renders"
rm -rf "$ROOT/debug_frames"/*
rm -f "$ROOT/LaunchMonitorSharedMemory"

echo "Cleaned."
