#!/usr/bin/env bash
# Tests the cover template's gameplay slot:
#   (a) the <image> that shows shot.png has the game's ratio (tools/game-size.sh),
#       to within 1%, so the shot is neither letterboxed nor squashed;
#   (b) the template renders at 768x1024 with a shot of the capture size beside it.
set -euo pipefail
HERE="$(cd "$(dirname "$0")/.." && pwd)"
SVG="$HERE/tools/cartridge-cover.svg"
command -v rsvg-convert >/dev/null || { echo "rsvg-convert required"; exit 1; }
command -v ffmpeg >/dev/null || { echo "ffmpeg required"; exit 1; }
command -v ffprobe >/dev/null || { echo "ffprobe required"; exit 1; }

GAME="$(cd "$HERE" && tools/game-size.sh)"
CAPTURE="$(cd "$HERE" && tools/game-size.sh --scale 1.5)"

# (a)
slot="$(python3 - "$SVG" <<'PY'
import sys
import xml.etree.ElementTree as ET

SVG_NS = "{http://www.w3.org/2000/svg}"
XLINK_HREF = "{http://www.w3.org/1999/xlink}href"
for el in ET.parse(sys.argv[1]).iter(SVG_NS + "image"):
    if (el.get(XLINK_HREF) or el.get("href")) == "shot.png":
        print(f"{el.get('width')}x{el.get('height')}")
        break
PY
)"
[ -n "$slot" ] || { echo "FAIL a: no <image> with href shot.png in $SVG"; exit 1; }
python3 -c '
import sys
sw, sh = (float(v) for v in sys.argv[1].split("x"))
gw, gh = (float(v) for v in sys.argv[2].split("x"))
sys.exit(0 if abs(sw / sh - gw / gh) <= 0.01 * (gw / gh) else 1)
' "$slot" "$GAME" || { echo "FAIL a: shot.png slot is $slot, not the game's $GAME ratio"; exit 1; }

# (b)
T="$(mktemp -d)"; trap 'rm -rf "$T"' EXIT
cp "$SVG" "$T/cover.svg"
ffmpeg -v error -y -f lavfi -i "testsrc=size=${CAPTURE}:rate=1:duration=1" -frames:v 1 "$T/shot.png"
rsvg-convert -w 768 -h 1024 "$T/cover.svg" -o "$T/cover.png"
dims="$(ffprobe -v error -select_streams v:0 -show_entries stream=width,height -of csv=p=0 "$T/cover.png")"
[ "$dims" = "768,1024" ] || { echo "FAIL b: cover rendered at $dims"; exit 1; }

echo "test_cover_slot: OK"
