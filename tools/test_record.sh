#!/usr/bin/env bash
# Tests tools/record.sh end to end on scratch games, with a stub `cargo` that
# writes what the real recorder writes (frames/, events.jsonl, manifest.json)
# at a size the test chooses:
#   (a) 4:3 game 960x720   -> tour, shot and clip at 1440x1080, vertical at 1080x1920
#   (b) 3:4 game 720x960   -> 1080x1440
#   (c) 1:1 game 720x720   -> 1080x1080
#   (d) the window was clamped (captured 1080x1300, expected 1080x1440)
#       -> non-zero exit naming both sizes, no tour.mp4
#   (e) AUTOPILOT_SCALE=1.0 -> the game's own size
#   (f) a scale that gives an odd side is refused before anything is recorded
#   (g) a game with no src/display.rs records with GAME_SIZE=1280x720
# The stub also fails unless GX_FRAME=off, so the cabinet frame is never in a capture.
set -euo pipefail
HERE="$(cd "$(dirname "$0")/.." && pwd)"
command -v ffmpeg >/dev/null || { echo "ffmpeg required"; exit 1; }
command -v ffprobe >/dev/null || { echo "ffprobe required"; exit 1; }

T="$(mktemp -d)"; trap 'rm -rf "$T"' EXIT

mkdir -p "$T/bin"
cat > "$T/bin/cargo" <<'STUB'
#!/usr/bin/env bash
# Stub for `cargo run --release --features record`: one second of test pattern.
set -euo pipefail
[ "${GX_FRAME:-}" = "off" ] || { echo "stub cargo: GX_FRAME is not off" >&2; exit 1; }
mkdir -p "$RECORD_DIR/frames"
ffmpeg -nostdin -v error -y -f lavfi -i "testsrc=size=${STUB_SIZE}:rate=60:duration=1" "$RECORD_DIR/frames/%06d.png"
printf '{"frame":30,"t":0.500,"kind":"beat","data":{"name":"05-mid-play"}}\n' > "$RECORD_DIR/events.jsonl"
printf '{"name":"Scratch Game","fps":60,"width":%s,"height":%s,"frames":60,"duration_s":1.000,"beats":[{"name":"05-mid-play","frame":30}],"audio":null}\n' \
  "${STUB_SIZE%x*}" "${STUB_SIZE#*x}" > "$RECORD_DIR/manifest.json"
STUB
chmod +x "$T/bin/cargo"

mk() {  # mk <dir> <game WxH|none>: scratch game holding copies of the tools
  local d="$1"
  mkdir -p "$d/tools" "$d/src" "$d/assets"
  cp "$HERE/tools/record.sh" "$HERE/tools/game-size.sh" "$HERE/tools/cut_clips.py" \
     "$HERE/tools/vertical-banner.svg" "$d/tools/"
  if [ "$2" != none ]; then
    printf 'pub const GAME_WIDTH: u32 = %s;\npub const GAME_HEIGHT: u32 = %s;\n' "${2%x*}" "${2#*x}" > "$d/src/display.rs"
  fi
}
rec() {  # rec <dir> <stub capture WxH> [VAR=value...]: runs record.sh there; output in <dir>.out
  local d="$1" cap="$2"; shift 2
  ( cd "$d" && env -u GAME_SIZE -u RECORD_DIR -u AUTOPILOT_SCALE PATH="$T/bin:$PATH" STUB_SIZE="$cap" "$@" tools/record.sh ) > "$d.out" 2>&1
}
dims() { ffprobe -v error -select_streams v:0 -show_entries stream=width,height -of csv=p=0 "$1"; }
ok() {  # ok <label> <game WxH|none> <capture WxH> [VAR=value...]
  local label="$1" game="$2" cap="$3"; shift 3
  local d="$T/$label" want="${cap%x*},${cap#*x}"
  mk "$d" "$game"
  rec "$d" "$cap" "$@" || { echo "FAIL $label: record.sh exited non-zero"; cat "$d.out"; exit 1; }
  [ "$(dims "$d/build/record/tour.mp4")" = "$want" ] || { echo "FAIL $label: tour is $(dims "$d/build/record/tour.mp4"), want $want"; exit 1; }
  [ "$(dims "$d/build/record/shots/05-mid-play.png")" = "$want" ] || { echo "FAIL $label: shot size"; exit 1; }
  [ "$(dims "$d/build/record/clips/05-mid-play.mp4")" = "$want" ] || { echo "FAIL $label: clip size"; exit 1; }
  grep -q "capture ${cap}" "$d.out" || { echo "FAIL $label: output does not name the capture size"; cat "$d.out"; exit 1; }
  if command -v rsvg-convert >/dev/null; then
    [ "$(dims "$d/build/record/tour-vertical.mp4")" = "1080,1920" ] || { echo "FAIL $label: vertical tour size"; exit 1; }
    [ "$(dims "$d/build/record/clips/vertical/05-mid-play.mp4")" = "1080,1920" ] || { echo "FAIL $label: vertical clip size"; exit 1; }
  fi
}

ok a-4x3 960x720 1440x1080
ok b-3x4 720x960 1080x1440
ok c-1x1 720x720 1080x1080

# (d)
mk "$T/d" 720x960
if rec "$T/d" 1080x1300; then echo "FAIL d: a clamped window should fail"; exit 1; fi
grep -q "captured 1080x1300, expected 1080x1440" "$T/d.out" || { echo "FAIL d: message"; cat "$T/d.out"; exit 1; }
[ ! -f "$T/d/build/record/tour.mp4" ] || { echo "FAIL d: encoded the wrong size anyway"; exit 1; }

ok e-scale1 960x720 960x720 AUTOPILOT_SCALE=1.0

# (f)
mk "$T/f" 720x960
if rec "$T/f" 1080x1440 AUTOPILOT_SCALE=1.0125; then echo "FAIL f: an odd capture size should be refused"; exit 1; fi
grep -q "fractional or odd" "$T/f.out" || { echo "FAIL f: message"; cat "$T/f.out"; exit 1; }
[ ! -d "$T/f/build/record" ] || { echo "FAIL f: recorded anyway"; exit 1; }

ok g-legacy none 1920x1080 GAME_SIZE=1280x720

echo "test_record: OK"
