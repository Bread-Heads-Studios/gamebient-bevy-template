#!/usr/bin/env bash
# Acceptance checks for the cabinet frame. Runs from the template, never
# copied into a game. See tools/frame_accept.py for what each mode checks.
#
#   tools/frame-accept.sh --check-copies <game-dir>
#   tools/frame-accept.sh --shots <dir> --window WxH --ratio <4:3|1:1|3:4> [--frame on|off]
#
# --shots reads 02-title.png, 05-mid-play.png and 08-pause.png from <dir>.
# Needs python3 with Pillow for --shots.
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
exec python3 "$HERE/frame_accept.py" "$@"
