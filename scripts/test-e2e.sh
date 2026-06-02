#!/usr/bin/env bash
set -e

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

cargo build --release --bin lm-test
"$ROOT/scripts/render.sh"

if target/release/lm-test; then
  echo "PASS"
else
  echo "FAIL"
  exit 1
fi
