#!/usr/bin/env bash
# Tests tools/frame-art.sh on scratch game dirs holding copies of the tool and
# the two templates:
#   (a) the templates render: assets/marquee.png is 1080x360, assets/bezel.png
#       is 1920x1920, and the bezel's central 1080x1080 is flat;
#   (b) a bezel with a shape inside the central 1080x1080 is refused and
#       nothing is written to assets/;
#   (c) a bezel with a high-contrast shape in an arm is refused;
#   (d) a missing template is refused by name;
#   (e) a run that fails leaves an earlier good render in assets/ untouched.
set -euo pipefail
HERE="$(cd "$(dirname "$0")/.." && pwd)"
command -v rsvg-convert >/dev/null || { echo "rsvg-convert required"; exit 1; }
command -v ffmpeg >/dev/null || { echo "ffmpeg required"; exit 1; }
command -v ffprobe >/dev/null || { echo "ffprobe required"; exit 1; }

T="$(mktemp -d)"; trap 'rm -rf "$T"' EXIT

mk() {  # mk <dir>: scratch game with the tool and both templates
  mkdir -p "$1/tools"
  cp "$HERE/tools/frame-art.sh" "$HERE/tools/marquee.svg" "$HERE/tools/bezel.svg" "$1/tools/"
}
dims() { ffprobe -v error -select_streams v:0 -show_entries stream=width,height -of csv=p=0 "$1"; }
centre_range() {  # centre_range <png>: luma max minus min inside the central 1080x1080
  ffmpeg -nostdin -v error -i "$1" -vf "crop=1080:1080:420:420,signalstats,metadata=print:file=-" -frames:v 1 -f null - \
    | awk -F= '/YMIN/ {lo = $2} /YMAX/ {hi = $2} END {print hi - lo}'
}
refused() {  # refused <label> <dir> <text the message must contain>
  if ( cd "$2" && tools/frame-art.sh ) >"$T/out" 2>"$T/err"; then echo "FAIL $1: should exit non-zero"; exit 1; fi
  grep -qF -- "$3" "$T/err" || { echo "FAIL $1: message lacks '$3': $(cat "$T/err")"; exit 1; }
}

# (a)
mk "$T/a"; ( cd "$T/a" && tools/frame-art.sh ) >/dev/null
[ "$(dims "$T/a/assets/marquee.png")" = "1080,360" ] || { echo "FAIL a: marquee is $(dims "$T/a/assets/marquee.png")"; exit 1; }
[ "$(dims "$T/a/assets/bezel.png")" = "1920,1920" ] || { echo "FAIL a: bezel is $(dims "$T/a/assets/bezel.png")"; exit 1; }
[ "$(centre_range "$T/a/assets/bezel.png")" -le 8 ] || { echo "FAIL a: bezel centre is not flat"; exit 1; }

# (b)
mk "$T/b"
perl -0pi -e 's|</svg>|<rect x="900" y="900" width="120" height="120" fill="#ffffff"/>\n</svg>|' "$T/b/tools/bezel.svg"
refused b "$T/b" "central 1080x1080"
[ ! -e "$T/b/assets/bezel.png" ] || { echo "FAIL b: wrote the bezel anyway"; exit 1; }
[ ! -e "$T/b/assets/marquee.png" ] || { echo "FAIL b: wrote the marquee anyway"; exit 1; }

# (c)
mk "$T/c"
perl -0pi -e 's|</svg>|<rect x="100" y="900" width="120" height="120" fill="#ffffff"/>\n</svg>|' "$T/c/tools/bezel.svg"
refused c "$T/c" "contrast"

# (d)
mk "$T/d"; rm "$T/d/tools/marquee.svg"
refused d "$T/d" "tools/marquee.svg"

# (e)
cp "$T/a/assets/bezel.png" "$T/a.bezel.png"
perl -0pi -e 's|</svg>|<rect x="900" y="900" width="120" height="120" fill="#ffffff"/>\n</svg>|' "$T/a/tools/bezel.svg"
refused e "$T/a" "central 1080x1080"
cmp -s "$T/a/assets/bezel.png" "$T/a.bezel.png" || { echo "FAIL e: a failed run replaced the good bezel"; exit 1; }

echo "test_frame_art: OK"
