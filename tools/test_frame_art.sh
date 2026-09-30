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
#   (f) a marquee whose viewBox is 1080x400 is refused, naming 1080x360, and an
#       earlier good marquee.png is left untouched (the render would stretch it).
#   (g) a bezel whose viewBox is 1920x1080 is refused, naming 1920x1920, and an
#       earlier good bezel.png is left untouched.
#   (h) an ffmpeg that succeeds but prints no YMIN/YMAX is refused, not passed.
#   (i) a marquee.svg or a bezel.svg that still says PLACEHOLDER is refused,
#       naming the file, before anything is rendered or written to assets/.
# The shipped templates say PLACEHOLDER (they are for a game author to fill
# in), so `mk` copies them with the word replaced; `mk_raw` copies them as they
# are.
set -euo pipefail
HERE="$(cd "$(dirname "$0")/.." && pwd)"
command -v rsvg-convert >/dev/null || { echo "rsvg-convert required"; exit 1; }
command -v ffmpeg >/dev/null || { echo "ffmpeg required"; exit 1; }
command -v ffprobe >/dev/null || { echo "ffprobe required"; exit 1; }

T="$(mktemp -d)"; trap 'rm -rf "$T"' EXIT

mk_raw() {  # mk_raw <dir>: scratch game with the tool and both templates as shipped
  mkdir -p "$1/tools"
  cp "$HERE/tools/frame-art.sh" "$HERE/tools/marquee.svg" "$HERE/tools/bezel.svg" "$1/tools/"
}
mk() {  # mk <dir>: like mk_raw, with the templates de-placeholdered so they render
  mk_raw "$1"
  sed -i.bak 's/PLACEHOLDER/DRAFT/g' "$1/tools/marquee.svg" "$1/tools/bezel.svg"
  rm -f "$1/tools/marquee.svg.bak" "$1/tools/bezel.svg.bak"
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

# (f)
mk "$T/f"; ( cd "$T/f" && tools/frame-art.sh ) >/dev/null
cp "$T/f/assets/marquee.png" "$T/f.marquee.png"
sed -i.bak 's|viewBox="0 0 1080 360"|viewBox="0 0 1080 400"|' "$T/f/tools/marquee.svg"
refused f "$T/f" "1080x360"
refused f "$T/f" "tools/marquee.svg"
cmp -s "$T/f/assets/marquee.png" "$T/f.marquee.png" || { echo "FAIL f: a refused run replaced the good marquee"; exit 1; }

# (g)
mk "$T/g"; ( cd "$T/g" && tools/frame-art.sh ) >/dev/null
cp "$T/g/assets/bezel.png" "$T/g.bezel.png"
sed -i.bak 's|viewBox="0 0 1920 1920"|viewBox="0 0 1920 1080"|' "$T/g/tools/bezel.svg"
refused g "$T/g" "1920x1920"
cmp -s "$T/g/assets/bezel.png" "$T/g.bezel.png" || { echo "FAIL g: a refused run replaced the good bezel"; exit 1; }

# (h)
mk "$T/h"
mkdir -p "$T/h.bin"; printf '#!/bin/sh\nexit 0\n' > "$T/h.bin/ffmpeg"; chmod +x "$T/h.bin/ffmpeg"
if ( cd "$T/h" && PATH="$T/h.bin:$PATH" tools/frame-art.sh ) >"$T/out" 2>"$T/err"; then echo "FAIL h: empty ffmpeg output should be refused"; exit 1; fi
grep -qF -- "YMIN/YMAX" "$T/err" || { echo "FAIL h: message: $(cat "$T/err")"; exit 1; }
[ ! -e "$T/h/assets/bezel.png" ] || { echo "FAIL h: wrote the bezel anyway"; exit 1; }

# (i)
for which in marquee bezel; do
  mk_raw "$T/i.$which"
  sed -i.bak 's/PLACEHOLDER//g' "$T/i.$which/tools/marquee.svg" "$T/i.$which/tools/bezel.svg"
  grep -q PLACEHOLDER "$HERE/tools/$which.svg" || { echo "FAIL i: shipped $which.svg has no PLACEHOLDER to test with"; exit 1; }
  cp "$HERE/tools/$which.svg" "$T/i.$which/tools/$which.svg"
  refused "i ($which)" "$T/i.$which" "tools/$which.svg"
  grep -qF PLACEHOLDER "$T/err" || { echo "FAIL i ($which): message does not say PLACEHOLDER: $(cat "$T/err")"; exit 1; }
  [ ! -e "$T/i.$which/assets" ] || { echo "FAIL i ($which): wrote assets/ anyway"; exit 1; }
  [ ! -e "$T/i.$which/build" ] || { echo "FAIL i ($which): rendered before refusing"; exit 1; }
done

echo "test_frame_art: OK"
