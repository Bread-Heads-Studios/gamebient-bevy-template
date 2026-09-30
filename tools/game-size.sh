#!/usr/bin/env bash
# Prints the game's pinned backbuffer size as <width>x<height>, parsed from
# GAME_WIDTH / GAME_HEIGHT in src/display.rs, the single place a game states
# its size. store-assets.sh, record.sh and make-cartridge.sh take their sizes
# from here.
#
# Usage: tools/game-size.sh [--scale FACTOR] [path/to/display.rs]
#   --scale FACTOR  print the size multiplied by FACTOR: the capture size for
#                   AUTOPILOT_SCALE=FACTOR. Fails unless both sides come out
#                   as even whole numbers, which H.264 yuv420p needs.
# Env:   GAME_SIZE=<width>x<height> overrides the file, for a game that has
#        not been converted yet and has no src/display.rs (GAME_SIZE=1280x720).
# Run from a game repo root unless a path is given: the default path is
# relative to $PWD, as in store-assets.sh.
set -euo pipefail

usage() { echo "usage: $0 [--scale FACTOR] [path/to/display.rs]" >&2; exit 2; }

SCALE=""; FILE="src/display.rs"
while [ $# -gt 0 ]; do
  case "$1" in
    --scale) [ $# -ge 2 ] || usage; SCALE="$2"; shift 2 ;;
    -*) usage ;;
    *) FILE="$1"; shift ;;
  esac
done

constant() {  # constant <NAME>: the integer literal on that const's line, or nothing
  sed -n "s/^[[:space:]]*pub const $1: u32 = \([0-9][0-9]*\);.*\$/\1/p" "$FILE" | head -1
}

if [ -n "${GAME_SIZE:-}" ]; then
  [[ "$GAME_SIZE" =~ ^([1-9][0-9]*)x([1-9][0-9]*)$ ]] \
    || { echo "game-size: GAME_SIZE must be <width>x<height>, got '$GAME_SIZE'" >&2; exit 2; }
  W="${BASH_REMATCH[1]}"; H="${BASH_REMATCH[2]}"
else
  [ -f "$FILE" ] || { echo "game-size: $FILE not found (run from the game repo root; a game not converted yet sets GAME_SIZE=1280x720)" >&2; exit 1; }
  W="$(constant GAME_WIDTH)"; H="$(constant GAME_HEIGHT)"
  if [ -z "$W" ] || [ "$W" -le 0 ]; then echo "game-size: no 'pub const GAME_WIDTH: u32 = <number>;' line in $FILE" >&2; exit 1; fi
  if [ -z "$H" ] || [ "$H" -le 0 ]; then echo "game-size: no 'pub const GAME_HEIGHT: u32 = <number>;' line in $FILE" >&2; exit 1; fi
fi

if [ -z "$SCALE" ]; then
  echo "${W}x${H}"
  exit 0
fi

[[ "$SCALE" =~ ^[0-9]+(\.[0-9]+)?$ ]] || { echo "game-size: --scale must be a positive number, got '$SCALE'" >&2; exit 2; }
awk -v w="$W" -v h="$H" -v s="$SCALE" 'BEGIN {
  cw = w * s; ch = h * s
  rw = int(cw + 0.5); rh = int(ch + 0.5)
  dw = cw - rw; if (dw < 0) dw = -dw
  dh = ch - rh; if (dh < 0) dh = -dh
  if (rw < 2 || rh < 2 || dw > 0.001 || dh > 0.001 || rw % 2 || rh % 2) exit 1
  printf "%dx%d\n", rw, rh
}' || { echo "game-size: scale $SCALE turns ${W}x${H} into a size with a fractional or odd side; use 1, 1.5 or 2" >&2; exit 1; }
