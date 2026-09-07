#!/usr/bin/env bash
# Regenerates assets/cartridge.png (the 768x1024 box cover served by
# info.json). Captures real gameplay via the autopilot, then composes the
# cover from tools/cartridge-cover.svg with rsvg-convert (brew install librsvg).
#
#   ./make-cartridge.sh                 # full run: capture + compose
#   ./make-cartridge.sh --skip-capture  # reuse existing shots
#   ./make-cartridge.sh --beat 07-late-play
#   ./make-cartridge.sh --out /tmp/preview.png
#
# The SVG in tools/ is a placeholder layout. Design the real cover in your
# game's own voice before shipping — see the `designing-cartridge-covers` skill.
set -euo pipefail
cd "$(dirname "$0")"

SHOT_DIR="build/cartridge/shots"
WORK_DIR="build/cartridge"
BEAT="05-mid-play"
OUT="assets/cartridge.png"
CAPTURE=1

while [[ $# -gt 0 ]]; do
  case "$1" in
    --skip-capture) CAPTURE=0; shift ;;
    --beat) BEAT="$2"; shift 2 ;;
    --out) OUT="$2"; shift 2 ;;
    *) echo "unknown flag: $1" >&2; exit 2 ;;
  esac
done

command -v rsvg-convert >/dev/null || {
  echo "rsvg-convert not found — brew install librsvg" >&2; exit 1;
}

if [[ $CAPTURE -eq 1 ]]; then
  echo "==> capturing gameplay (autopilot tour, ~60s at 1920x1080)"
  mkdir -p "$SHOT_DIR"
  AUTOPILOT_DIR="$PWD/$SHOT_DIR" AUTOPILOT_SCALE=1.5 cargo run --features autopilot
fi

SHOT="$SHOT_DIR/$BEAT.png"
if [[ ! -f "$SHOT" ]]; then
  echo "no such beat: $SHOT" >&2
  echo "available beats:" >&2
  ls "$SHOT_DIR" 2>/dev/null | sed 's/\.png$//; s/^/  /' >&2
  exit 1
fi

echo "==> composing cover from $SHOT"
mkdir -p "$WORK_DIR"
cp "$SHOT" "$WORK_DIR/shot.png"
cp tools/cartridge-cover.svg "$WORK_DIR/cover.svg"
rsvg-convert -w 768 -h 1024 "$WORK_DIR/cover.svg" -o "$OUT"
echo "==> wrote $OUT ($(file -b "$OUT" | cut -d, -f2 | xargs))"
