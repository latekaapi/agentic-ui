#!/bin/zsh
# Parity check for one gallery entry against its design reference PNG.
#
#   scripts/parity.sh <entry-id> <reference-stem> [--no-build]
#   e.g. scripts/parity.sh foundations/colour 01-color
#
# Renders the entry with `aui-gallery --screenshot`, writes
# target/parity/<stem>.png and target/parity/<stem>-diff.png, and prints the
# ImageMagick RMSE distance (0 = identical) plus the count of differing pixels.
set -euo pipefail
export PATH="/opt/homebrew/bin:$HOME/.cargo/bin:$PATH"
cd "$(dirname "$0")/.."
entry="$1"; stem="$2"; shift 2
ref="design/reference/cards/$stem.png"
[[ -f "$ref" ]] || ref="design/reference/screens/$stem.png"
[[ -f "$ref" ]] || { echo "no reference for $stem" >&2; exit 2; }
if [[ "${1:-}" != "--no-build" ]]; then cargo build -q -p aui-gallery; fi
mkdir -p target/parity
out="target/parity/$stem.png"
( ./target/debug/aui-gallery --screenshot "$entry" "$out" & P=$!; sleep 12; kill $P 2>/dev/null || true; wait $P 2>/dev/null || true )
[[ -f "$out" ]] || { echo "render failed" >&2; exit 1; }
rmse=$(compare -metric RMSE "$ref" "$out" "target/parity/$stem-diff.png" 2>&1 || true)
ae=$(compare -metric AE -fuzz 6% "$ref" "$out" null: 2>&1 || true)
echo "$stem: RMSE $rmse · differing pixels (6% fuzz) $ae · diff at target/parity/$stem-diff.png"
