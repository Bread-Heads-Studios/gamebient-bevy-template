#!/usr/bin/env bash
# Renders the art the cabinets draw around the game:
#   tools/marquee.svg -> assets/marquee.png  (1080x360,  the title treatment)
#   tools/bezel.svg   -> assets/bezel.png    (1920x1920, quiet fill behind the game)
# with rsvg-convert, the rasteriser the cover uses (brew install librsvg),
# then checks the renders before they reach assets/:
#   - exact sizes;
#   - the bezel's central 1080x1080, which the game covers, holds no detail;
#   - the bezel is low-contrast.
# The frame dims both at runtime (bezel 0.35 idle, 0.20 in play, saturation
# 0.6; marquee 1.00 idle, 0.45 in play), so the art is drawn at full
# brightness. See frame-art.md in the `designing-cartridge-covers` skill.
#
# Usage: tools/frame-art.sh
# rsvg loads <image> files only from beside the SVG: keep them in tools/.
set -euo pipefail
cd "$(dirname "$0")/.."

[ $# -eq 0 ] || { echo "usage: $0" >&2; exit 2; }
command -v rsvg-convert >/dev/null 2>&1 || { echo "frame-art: rsvg-convert not on PATH (brew install librsvg)" >&2; exit 1; }
command -v ffmpeg >/dev/null 2>&1 || { echo "frame-art: ffmpeg not on PATH (brew install ffmpeg)" >&2; exit 1; }
command -v ffprobe >/dev/null 2>&1 || { echo "frame-art: ffprobe not on PATH (brew install ffmpeg)" >&2; exit 1; }
for f in tools/marquee.svg tools/bezel.svg; do
  [ -f "$f" ] || { echo "frame-art: $f not found (tools/rollout-frame-art.sh in the template copies the placeholders)" >&2; exit 1; }
done

# Luma levels, max minus min, as ffmpeg's signalstats reports them.
CENTRE_MAX_RANGE=8
BEZEL_MAX_RANGE=96
WORK=build/frame-art

dims() {  # dims <png>: prints <width>x<height>
  ffprobe -v error -select_streams v:0 -show_entries stream=width,height -of csv=p=0:s=x "$1"
}
luma_range() {  # luma_range <png> <crop w:h:x:y>: luma max minus min inside the crop
  ffmpeg -nostdin -v error -i "$1" -vf "crop=$2,signalstats,metadata=print:file=-" -frames:v 1 -f null - \
    | awk -F= '/YMIN/ {lo = $2} /YMAX/ {hi = $2} END {print hi - lo}'
}

mkdir -p "$WORK"
rsvg-convert -w 1080 -h 360 tools/marquee.svg -o "$WORK/marquee.png"
rsvg-convert -w 1920 -h 1920 tools/bezel.svg -o "$WORK/bezel.png"

d="$(dims "$WORK/marquee.png")"
[ "$d" = "1080x360" ] || { echo "frame-art: marquee rendered at $d, expected 1080x360" >&2; exit 1; }
d="$(dims "$WORK/bezel.png")"
[ "$d" = "1920x1920" ] || { echo "frame-art: bezel rendered at $d, expected 1920x1920" >&2; exit 1; }

r="$(luma_range "$WORK/bezel.png" 1080:1080:420:420)"
[ "$r" -le "$CENTRE_MAX_RANGE" ] || { echo "frame-art: the bezel has detail inside the central 1080x1080 (luma range $r, limit $CENTRE_MAX_RANGE). The game covers that region: keep it one flat colour." >&2; exit 1; }
r="$(luma_range "$WORK/bezel.png" 1920:1920:0:0)"
[ "$r" -le "$BEZEL_MAX_RANGE" ] || { echo "frame-art: the bezel has too much contrast (luma range $r, limit $BEZEL_MAX_RANGE). Bring its darkest and brightest colours closer together." >&2; exit 1; }

mkdir -p assets
cp "$WORK/marquee.png" assets/marquee.png
cp "$WORK/bezel.png" assets/bezel.png
echo "frame-art: wrote assets/marquee.png (1080x360) and assets/bezel.png (1920x1920)"
