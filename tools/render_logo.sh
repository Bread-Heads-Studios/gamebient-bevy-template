#!/usr/bin/env bash
# Render an SVG logo to a PNG.
# Usage: tools/render_logo.sh <in.svg> <out.png> [height=256]
set -euo pipefail

if [ "$#" -lt 2 ]; then
    echo "usage: $0 <in.svg> <out.png> [height=256]" >&2
    exit 2
fi
if ! command -v rsvg-convert >/dev/null 2>&1; then
    echo "rsvg-convert not found; install librsvg (brew install librsvg, apt install librsvg2-bin)" >&2
    exit 1
fi

rsvg-convert -h "${3:-256}" -o "$2" "$1"
echo "wrote $2 ($(wc -c <"$2" | tr -d ' ') bytes)"
