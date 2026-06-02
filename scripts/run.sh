#!/usr/bin/env bash
set -e

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

cargo build --release --bin lm-test
RENDER=1 cargo run --release --bin lm-test
