#!/usr/bin/env bash
# Tests tools/store-assets.sh on scratch game dirs with synthetic footage:
#   (a) a 4:3 game (960x720, captured at 1440x1080): shots 04/05/06/07 +
#       signature.mp4 present -> 4 screenshots and a trailer at 960x720, the
#       trailer under 8 MB, info.json gains screenshots/trailer_url/version and
#       released_at from --released-at; Developer/Tags untouched and reported missing.
#   (b) re-run is idempotent (same output, same info.json).
#   (c) existing released_at is preserved when no flag is given.
#   (d) --set-tags normalizes to lowercase and caps at 8.
#   (e) no shots at all -> non-zero exit, nothing written.
#   (f) a 3:4 game (720x960, captured at 1080x1440) -> 720x960 outputs.
#   (g) a 1:1 game (720x720, captured at 1080x1080) -> 720x720 outputs.
#   (h) stale 16:9 footage in a 4:3 game -> non-zero exit, nothing written.
#   (i) no src/display.rs -> non-zero exit naming it; with GAME_SIZE=1280x720
#       the same footage gives 1280x720 outputs (a game not converted yet).
#   properties.aspect is written in every case: 4:3 (a), 3:4 (f), 1:1 (g), 16:9 (i).
#   (j) assets/marquee.png and assets/bezel.png present at the right sizes
#       -> properties.marquee / properties.bezel point at them; absent (a)
#       -> both keys are removed, so the site uses the shared art.
#   (k) a marquee.png of the wrong size -> non-zero exit, nothing written.
#   (l) a game size that is not a sanctioned ratio -> non-zero exit.
set -euo pipefail
HERE="$(cd "$(dirname "$0")/.." && pwd)"
TOOL="$HERE/tools/store-assets.sh"
command -v ffmpeg >/dev/null || { echo "ffmpeg required"; exit 1; }
command -v ffprobe >/dev/null || { echo "ffprobe required"; exit 1; }
unset GAME_SIZE

mk() {  # mk <dir> [game WxH|none] [capture WxH]: synthetic game with footage
  local d="$1" game="${2:-960x720}" cap="${3:-1440x1080}"
  mkdir -p "$d/assets" "$d/src" "$d/build/record/shots" "$d/build/record/clips"
  cp "$HERE/assets/info.json" "$d/assets/info.json"
  printf '[package]\nname = "scratch-game"\nversion = "0.3.1"\n' > "$d/Cargo.toml"
  if [ "$game" != none ]; then
    printf 'pub const GAME_WIDTH: u32 = %s;\npub const GAME_HEIGHT: u32 = %s;\n' "${game%x*}" "${game#*x}" > "$d/src/display.rs"
  fi
  git -C "$d" init -q && git -C "$d" add -A && git -C "$d" -c user.email=t@t -c user.name=t commit -qm init
  for b in 04-early-play 05-mid-play 06-first-score 07-late-play; do
    ffmpeg -v error -y -f lavfi -i "color=c=blue:s=${cap}:d=0.1" -frames:v 1 "$d/build/record/shots/$b.png"
  done
  ffmpeg -v error -y -f lavfi -i "testsrc=size=${cap}:rate=30:duration=3" -f lavfi -i "sine=frequency=440:duration=3" \
    -c:v libx264 -pix_fmt yuv420p -c:a aac "$d/build/record/clips/signature.mp4"
}
json() { python3 -c "import json,sys; d=json.load(open('$1')); print(eval(sys.argv[1]))" "$2"; }
dims() { ffprobe -v error -select_streams v:0 -show_entries stream=width,height -of csv=p=0 "$1"; }
sized() {  # sized <label> <dir> <w,h>: first screenshot and trailer are that size
  [ "$(dims "$2/assets/screenshots/01.png")" = "$3" ] || { echo "FAIL $1: screenshot is $(dims "$2/assets/screenshots/01.png"), want $3"; exit 1; }
  [ "$(dims "$2/assets/trailer.mp4")" = "$3" ] || { echo "FAIL $1: trailer is $(dims "$2/assets/trailer.mp4"), want $3"; exit 1; }
}

T="$(mktemp -d)"; trap 'rm -rf "$T"' EXIT

# (a)
mk "$T/a"; ( cd "$T/a" && "$TOOL" --released-at 2026-09-01 ) > "$T/a.out"
for n in 01 02 03 04; do [ -f "$T/a/assets/screenshots/$n.png" ] || { echo "FAIL a: screenshot $n missing"; exit 1; }; done
[ -f "$T/a/assets/trailer.mp4" ] || { echo "FAIL a: trailer missing"; exit 1; }
sized a "$T/a" "960,720"
[ "$(stat -f%z "$T/a/assets/trailer.mp4" 2>/dev/null || stat -c%s "$T/a/assets/trailer.mp4")" -lt 8000000 ] || { echo "FAIL a: trailer too big"; exit 1; }
[ "$(json "$T/a/assets/info.json" "len(d['properties']['screenshots'])")" = "4" ] || { echo "FAIL a: screenshots count"; exit 1; }
[ "$(json "$T/a/assets/info.json" "d['properties']['screenshots'][0]")" = "https://gamebient-game.vercel.app/assets/screenshots/01.png" ] || { echo "FAIL a: screenshot url"; exit 1; }
[ "$(json "$T/a/assets/info.json" "d['properties']['trailer_url']")" = "https://gamebient-game.vercel.app/assets/trailer.mp4" ] || { echo "FAIL a: trailer url"; exit 1; }
[ "$(json "$T/a/assets/info.json" "d['properties']['released_at']")" = "2026-09-01" ] || { echo "FAIL a: released_at"; exit 1; }
json "$T/a/assets/info.json" "d['properties']['version']" | grep -Eq '^0\.3\.1\+[0-9a-f]{7}$' || { echo "FAIL a: version"; exit 1; }
if ! { grep -q "Developer" "$T/a.out" && grep -q "Tags" "$T/a.out"; }; then echo "FAIL a: missing-field report"; exit 1; fi
[ "$(json "$T/a/assets/info.json" "d['properties']['aspect']")" = "4:3" ] || { echo "FAIL a: aspect"; exit 1; }
[ "$(json "$T/a/assets/info.json" "'marquee' in d['properties'] or 'bezel' in d['properties']")" = "False" ] || { echo "FAIL a: frame art URLs kept with no PNGs behind them"; exit 1; }

# (b)
cp "$T/a/assets/info.json" "$T/a.first.json"; ( cd "$T/a" && "$TOOL" --released-at 2026-09-01 ) >/dev/null
cmp -s "$T/a/assets/info.json" "$T/a.first.json" || { echo "FAIL b: not idempotent"; exit 1; }

# (c)
( cd "$T/a" && "$TOOL" ) >/dev/null
[ "$(json "$T/a/assets/info.json" "d['properties']['released_at']")" = "2026-09-01" ] || { echo "FAIL c: released_at overwritten"; exit 1; }

# (d)
( cd "$T/a" && "$TOOL" --set-tags "Shooter, arcade ,Boss-Rush, a,b,c,d,e,f,g" ) >/dev/null
[ "$(json "$T/a/assets/info.json" "[x['value'] for x in d['attributes'] if x['trait_type']=='Tags'][0]")" = "shooter, arcade, boss-rush, a, b, c, d, e" ] || { echo "FAIL d: tags"; exit 1; }

# (e)
mk "$T/e"; rm -f "$T/e/build/record/shots/"*
if ( cd "$T/e" && "$TOOL" ) >/dev/null 2>&1; then echo "FAIL e: should exit non-zero"; exit 1; fi
[ ! -d "$T/e/assets/screenshots" ] || { echo "FAIL e: wrote screenshots"; exit 1; }

# (f)
mk "$T/f" 720x960 1080x1440; ( cd "$T/f" && "$TOOL" ) >/dev/null
sized f "$T/f" "720,960"
[ "$(json "$T/f/assets/info.json" "d['properties']['aspect']")" = "3:4" ] || { echo "FAIL f: aspect"; exit 1; }

# (g)
mk "$T/g" 720x720 1080x1080; ( cd "$T/g" && "$TOOL" ) >/dev/null
sized g "$T/g" "720,720"
[ "$(json "$T/g/assets/info.json" "d['properties']['aspect']")" = "1:1" ] || { echo "FAIL g: aspect"; exit 1; }

# (h)
mk "$T/h" 960x720 1920x1080
if ( cd "$T/h" && "$TOOL" ) >/dev/null 2>"$T/h.err"; then echo "FAIL h: stale 16:9 footage should be refused"; exit 1; fi
grep -q "re-run tools/record.sh" "$T/h.err" || { echo "FAIL h: message: $(cat "$T/h.err")"; exit 1; }
[ ! -d "$T/h/assets/screenshots" ] || { echo "FAIL h: wrote screenshots"; exit 1; }
[ ! -f "$T/h/assets/trailer.mp4" ] || { echo "FAIL h: wrote a trailer"; exit 1; }

# (i)
mk "$T/i" none 1920x1080
if ( cd "$T/i" && "$TOOL" ) >/dev/null 2>"$T/i.err"; then echo "FAIL i: no display.rs should be refused"; exit 1; fi
grep -q "src/display.rs" "$T/i.err" || { echo "FAIL i: message: $(cat "$T/i.err")"; exit 1; }
( cd "$T/i" && GAME_SIZE=1280x720 "$TOOL" ) >/dev/null
sized i "$T/i" "1280,720"
[ "$(json "$T/i/assets/info.json" "d['properties']['aspect']")" = "16:9" ] || { echo "FAIL i: aspect"; exit 1; }

# (j)
art() {  # art <dir> <marquee WxH> <bezel WxH>: synthetic frame art
  ffmpeg -v error -y -f lavfi -i "color=c=navy:s=${2}:d=0.1" -frames:v 1 "$1/assets/marquee.png"
  ffmpeg -v error -y -f lavfi -i "color=c=navy:s=${3}:d=0.1" -frames:v 1 "$1/assets/bezel.png"
}
mk "$T/j"; art "$T/j" 1080x360 1920x1920; ( cd "$T/j" && "$TOOL" ) >/dev/null
[ "$(json "$T/j/assets/info.json" "d['properties']['marquee']")" = "https://gamebient-game.vercel.app/assets/marquee.png" ] || { echo "FAIL j: marquee url"; exit 1; }
[ "$(json "$T/j/assets/info.json" "d['properties']['bezel']")" = "https://gamebient-game.vercel.app/assets/bezel.png" ] || { echo "FAIL j: bezel url"; exit 1; }

# (k)
mk "$T/k"; art "$T/k" 1080x400 1920x1920
if ( cd "$T/k" && "$TOOL" ) >/dev/null 2>"$T/k.err"; then echo "FAIL k: a 1080x400 marquee should be refused"; exit 1; fi
grep -q "assets/marquee.png is 1080x400, expected 1080x360" "$T/k.err" || { echo "FAIL k: message: $(cat "$T/k.err")"; exit 1; }
[ ! -d "$T/k/assets/screenshots" ] || { echo "FAIL k: wrote screenshots"; exit 1; }

# (l)
mk "$T/l" 1000x700 1500x1050
if ( cd "$T/l" && "$TOOL" ) >/dev/null 2>"$T/l.err"; then echo "FAIL l: 10:7 should be refused"; exit 1; fi
grep -q "10:7" "$T/l.err" || { echo "FAIL l: message: $(cat "$T/l.err")"; exit 1; }
[ ! -d "$T/l/assets/screenshots" ] || { echo "FAIL l: wrote screenshots"; exit 1; }

echo "test_store_assets: OK"
