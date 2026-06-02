#!/bin/bash
# Chunked dataset generation. Restarts Blender every BATCH shots to dodge the
# Metal/Cycles long-session shader-cache crash. Tolerant: a crashed batch is
# skipped, the loop continues (completed shots are already flushed to labels).
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
B="${BLENDER:-/Applications/Blender.app/Contents/MacOS/Blender}"
NAME="${1:-train}"
TOTAL="${2:-3000}"
SB="${3:-1000}"
FPS_MIN="${FPS_MIN:-500}"
FPS_MAX="${FPS_MAX:-1000}"
SAMPLES="${SAMPLES:-8}"
ENGINE="${ENGINE:-eevee}"
BATCH=20
OUT="$ROOT/ml/data/$NAME"
rm -rf "$OUT"; mkdir -p "$OUT"

s=$SB
end=$((SB + TOTAL))
while [ $s -lt $end ]; do
  n=$BATCH
  if [ $((s + BATCH)) -gt $end ]; then n=$((end - s)); fi
  "$B" --background --factory-startup --python "$ROOT/blender/gen_dataset.py" -- \
    --out "$OUT" --shots $n --seed-base $s \
    --fps-min $FPS_MIN --fps-max $FPS_MAX --focal-min 16 --focal-max 28 \
    --rpm-min 1500 --rpm-max 11000 --samples $SAMPLES --engine $ENGINE >> "$OUT/gen.log" 2>&1 || true
  echo "seed $s (+$n) -> total $(wc -l < "$OUT/labels.jsonl" 2>/dev/null || echo 0) shots"
  s=$((s + BATCH))
done
echo "GEN DONE: $(wc -l < "$OUT/labels.jsonl") shots"
