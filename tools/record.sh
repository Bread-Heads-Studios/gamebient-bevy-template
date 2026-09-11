#!/usr/bin/env bash
# Records the autopilot tour offline into build/record/:
#   tour.mp4 (60 fps, AAC audio when the game played anything, normalized to
#   -16 LUFS), clips/<beat>.mp4 (with audio), shots/<beat>.png, events.jsonl,
#   manifest.json, chapters.md, banner.png, clips/vertical/<beat>.mp4 and
#   tour-vertical.mp4 (1080x1920).  See src/game/record.rs and
#   tools/cut_clips.py.
#
# Usage: tools/record.sh [--keep-frames]
# Env:   RECORD_DIR (default build/record), AUTOPILOT_SCALE (default 1.5 = 1920x1080)
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
# AutopilotPlugin mutes GlobalVolume by default (like the playtest harness);
# the recorder needs the game's actual mixed audio, so ask for sound.
export AUTOPILOT_SOUND=1

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

echo "record.sh: recording the autopilot tour into $RECORD_DIR (scale $AUTOPILOT_SCALE)"
cargo run --release --features record

frames=$(find "$RECORD_DIR/frames" -name '*.png' | wc -l | tr -d ' ')
[ "$frames" -gt 0 ] || { echo "record.sh: no frames captured" >&2; exit 1; }

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
