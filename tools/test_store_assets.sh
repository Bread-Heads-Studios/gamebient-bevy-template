#!/usr/bin/env bash
# Tests tools/store-assets.sh on a scratch game dir with synthetic footage:
#   (a) shots 04/05/06/07 + signature.mp4 present -> 4 screenshots at 1280x720,
#       a trailer under 8 MB, info.json gains screenshots/trailer_url/version and
#       released_at from --released-at; Developer/Tags untouched and reported missing.
#   (b) re-run is idempotent (same output, same info.json).
#   (c) existing released_at is preserved when no flag is given.
#   (d) --set-tags normalizes to lowercase and caps at 8.
#   (e) no shots at all -> non-zero exit, nothing written.
set -euo pipefail
HERE="$(cd "$(dirname "$0")/.." && pwd)"
TOOL="$HERE/tools/store-assets.sh"
command -v ffmpeg >/dev/null || { echo "ffmpeg required"; exit 1; }

mk() {  # mk <dir>: synthetic game with footage
  local d="$1"; mkdir -p "$d/assets" "$d/build/record/shots" "$d/build/record/clips"
  cp "$HERE/assets/info.json" "$d/assets/info.json"
  printf '[package]\nname = "scratch-game"\nversion = "0.3.1"\n' > "$d/Cargo.toml"
  git -C "$d" init -q && git -C "$d" add -A && git -C "$d" -c user.email=t@t -c user.name=t commit -qm init
  for b in 04-early-play 05-mid-play 06-first-score 07-late-play; do
    ffmpeg -v error -y -f lavfi -i "color=c=blue:s=1920x1080:d=0.1" -frames:v 1 "$d/build/record/shots/$b.png"
  done
  ffmpeg -v error -y -f lavfi -i "testsrc=size=1920x1080:rate=30:duration=3" -f lavfi -i "sine=frequency=440:duration=3" \
    -c:v libx264 -pix_fmt yuv420p -c:a aac "$d/build/record/clips/signature.mp4"
}
json() { python3 -c "import json,sys; d=json.load(open('$1')); print(eval(sys.argv[1]))" "$2"; }

T="$(mktemp -d)"; trap 'rm -rf "$T"' EXIT

# (a)
mk "$T/a"; ( cd "$T/a" && "$TOOL" --released-at 2026-09-01 ) > "$T/a.out"
for n in 01 02 03 04; do [ -f "$T/a/assets/screenshots/$n.png" ] || { echo "FAIL a: screenshot $n missing"; exit 1; }; done
dims=$(ffprobe -v error -select_streams v:0 -show_entries stream=width,height -of csv=p=0 "$T/a/assets/screenshots/01.png")
[ "$dims" = "1280,720" ] || { echo "FAIL a: dims $dims"; exit 1; }
[ -f "$T/a/assets/trailer.mp4" ] || { echo "FAIL a: trailer missing"; exit 1; }
[ "$(stat -f%z "$T/a/assets/trailer.mp4" 2>/dev/null || stat -c%s "$T/a/assets/trailer.mp4")" -lt 8000000 ] || { echo "FAIL a: trailer too big"; exit 1; }
[ "$(json "$T/a/assets/info.json" "len(d['properties']['screenshots'])")" = "4" ] || { echo "FAIL a: screenshots count"; exit 1; }
[ "$(json "$T/a/assets/info.json" "d['properties']['screenshots'][0]")" = "https://gamebient-game.vercel.app/assets/screenshots/01.png" ] || { echo "FAIL a: screenshot url"; exit 1; }
[ "$(json "$T/a/assets/info.json" "d['properties']['trailer_url']")" = "https://gamebient-game.vercel.app/assets/trailer.mp4" ] || { echo "FAIL a: trailer url"; exit 1; }
[ "$(json "$T/a/assets/info.json" "d['properties']['released_at']")" = "2026-09-01" ] || { echo "FAIL a: released_at"; exit 1; }
json "$T/a/assets/info.json" "d['properties']['version']" | grep -Eq '^0\.3\.1\+[0-9a-f]{7}$' || { echo "FAIL a: version"; exit 1; }
if ! { grep -q "Developer" "$T/a.out" && grep -q "Tags" "$T/a.out"; }; then echo "FAIL a: missing-field report"; exit 1; fi

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

echo "test_store_assets: OK"
