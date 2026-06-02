#!/usr/bin/env bash
set -e

BLENDER="${BLENDER:-/Applications/Blender.app/Contents/MacOS/Blender}"
SCRIPT="$(cd "$(dirname "$0")/.." && pwd)/blender/render_shot.py"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"

render() {
  local case="$1" speed="$2" vla="$3" hla="$4" spin="$5" axis="$6"
  echo "Rendering $case..."
  "$BLENDER" --background --factory-startup --python "$SCRIPT" -- \
    --case "$case" --speed "$speed" --vla "$vla" --hla "$hla" \
    --spin "$spin" --axis "$axis" \
    --out "$ROOT/renders/$case" \
    --width 512 --height 384 --fps 240 --frames 14 --samples 64
}

render driver 165 10.5 -2.5 2700 -5
render 7-iron 120 16 1.2 7000 2
render wedge 85 28 -0.8 9500 0

echo "Render complete."
