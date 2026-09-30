#!/usr/bin/env bash
# Records the autopilot tour offline into build/record/:
#   tour.mp4 (60 fps, AAC audio when the game played anything, normalized to
#   -16 LUFS), clips/<beat>.mp4 (with audio), shots/<beat>.png, events.jsonl,
#   manifest.json, chapters.md, banner.png, clips/vertical/<beat>.mp4 and
#   tour-vertical.mp4 (1080x1920).  See src/game/record/ and
#   tools/cut_clips.py.
# Frames are captured at the game's size (tools/game-size.sh) times
# AUTOPILOT_SCALE: 1440x1080 for a 4:3 game, 1080x1080 for 1:1, 1080x1440 for
# 3:4 at the default 1.5. The cabinet frame is switched off (GX_FRAME=off).
#
# Usage: tools/record.sh [--keep-frames]
# Env:   RECORD_DIR (default build/record), AUTOPILOT_SCALE (default 1.5),
#        GAME_SIZE (only for a game with no src/display.rs: GAME_SIZE=1280x720)
set -euo pipefail
cd "$(dirname "$0")/.."

KEEP=0
for arg in "$@"; do
  case "$arg" in
    --keep-frames) KEEP=1 ;;
    *) echo "usage: $0 [--keep-frames]" >&2; exit 2 ;;
  esac
done

command -v ffmpeg >/dev/null 2>&1 || { echo "record.sh: ffmpeg not on PATH (brew install ffmpeg)" >&2; exit 1; }
command -v python3 >/dev/null 2>&1 || { echo "record.sh: python3 not on PATH" >&2; exit 1; }

export RECORD_DIR="${RECORD_DIR:-build/record}"
export AUTOPILOT_DIR="$RECORD_DIR/autopilot"
export AUTOPILOT_SCALE="${AUTOPILOT_SCALE:-1.5}"
# Some games' autopilots (e.g. grand-theft-otto) mute GlobalVolume unless
# AUTOPILOT_SOUND is set; the template's does not. The recorder needs the
# game's actual mixed audio, so ask for sound.
export AUTOPILOT_SOUND=1
# Footage shows the game alone: no marquee, bezel or gap around it.
export GX_FRAME=off

# What the game renders and what a capture must therefore measure. Computed
# before anything is wiped, so a bad scale or a missing display.rs stops here.
GAME="$(tools/game-size.sh)"
CAPTURE_SIZE="$(tools/game-size.sh --scale "$AUTOPILOT_SCALE")"

# Refuse to wipe anything outside the repo -- a stray or malicious
# RECORD_DIR (e.g. an absolute path, or one full of "..") must not turn
# this into `rm -rf` of something else on disk. Resolved lexically in
# python3 (already a hard dependency above) rather than `cd`/`dirname`, so
# this works even on a first run where RECORD_DIR's parent doesn't exist
# yet, never creates anything before the check passes, and (comparing
# against a single os.getcwd() call for both sides) can't be fooled by
# bash's $PWD and python's cwd disagreeing over a symlinked path (e.g.
# macOS's /tmp -> /private/tmp).
python3 -c '
import os, sys
root = os.getcwd()
target = os.path.normpath(os.path.join(root, sys.argv[1]))
sys.exit(0 if target.startswith(root + os.sep) else 1)
' "$RECORD_DIR" || { echo "record.sh: RECORD_DIR must be inside the repo" >&2; exit 2; }

rm -rf "$RECORD_DIR"
mkdir -p "$RECORD_DIR"
[ -w "$RECORD_DIR" ] || { echo "record.sh: $RECORD_DIR is not writable" >&2; exit 1; }

echo "record.sh: recording the autopilot tour into $RECORD_DIR (game $GAME, capture $CAPTURE_SIZE, scale $AUTOPILOT_SCALE)"
cargo run --release --features record

frames=$(find "$RECORD_DIR/frames" -name '*.png' | wc -l | tr -d ' ')
[ "$frames" -gt 0 ] || { echo "record.sh: no frames captured" >&2; exit 1; }

# The recorder writes the size it actually captured. If the OS clamped the
# window (a 3:4 capture is 1440 rows tall) or the game opened fullscreen, that
# is not the size asked for; stop rather than encode and cut the wrong picture.
[ -f "$RECORD_DIR/manifest.json" ] || { echo "record.sh: no manifest.json (the run did not exit cleanly)" >&2; exit 1; }
got=$(python3 -c 'import json, sys; m = json.load(open(sys.argv[1])); print("%dx%d" % (m["width"], m["height"]))' "$RECORD_DIR/manifest.json")
[ "$got" = "$CAPTURE_SIZE" ] || { echo "record.sh: captured $got, expected $CAPTURE_SIZE ($GAME at scale $AUTOPILOT_SCALE). The window did not get its size: the display is too small for it, or the game opened fullscreen. Lower AUTOPILOT_SCALE or record on a larger display." >&2; exit 1; }

AUDIO="$RECORD_DIR/audio.wav"
if [ -f "$AUDIO" ]; then
  echo "record.sh: encoding $frames frames + audio"
  ffmpeg -y -loglevel error -framerate 60 -pattern_type glob -i "$RECORD_DIR/frames/*.png" -i "$AUDIO" \
    -c:v libx264 -crf 18 -pix_fmt yuv420p \
    -af loudnorm=I=-16:TP=-1.5:LRA=11 -ar 48000 -c:a aac -b:a 192k -shortest "$RECORD_DIR/tour.mp4"
else
  echo "record.sh: encoding $frames frames (no audio.wav: nothing played)"
  ffmpeg -y -loglevel error -framerate 60 -pattern_type glob -i "$RECORD_DIR/frames/*.png" \
    -c:v libx264 -crf 18 -pix_fmt yuv420p "$RECORD_DIR/tour.mp4"
fi

python3 tools/cut_clips.py "$RECORD_DIR"

[ "$KEEP" = 1 ] || rm -rf "$RECORD_DIR/frames"
echo "record.sh: done -> $RECORD_DIR/tour.mp4 ($frames frames)"
