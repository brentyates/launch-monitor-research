#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

CASE="${1:-driver}"
shift || true

python3 "$SCRIPT_DIR/visualize.py" --case "$CASE" "$@"

if command -v open >/dev/null 2>&1; then
    SUFFIX=""
    for a in "$@"; do [ "$a" = "--debug" ] && SUFFIX="_debug"; done
    open "$PROJECT_DIR/viz/$CASE/contact_sheet$SUFFIX.png" 2>/dev/null || true
fi
