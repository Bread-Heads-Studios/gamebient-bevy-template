#!/usr/bin/env bash
# Guards the tooling, its comments and the skills against stale 16:9 claims.
# Each block names strings that must be gone and strings that must be there.
# Plain grep on tracked files; needs no media tools.
set -euo pipefail
cd "$(dirname "$0")/.."
SKILLS=.claude/skills
fail=0
absent() {  # absent <fixed string> <file>...: the string must not appear in any of the files
  local s="$1" f; shift
  for f in "$@"; do
    [ -f "$f" ] || { echo "FAIL: $f is missing"; fail=1; continue; }
    if grep -nF -- "$s" "$f"; then echo "FAIL: stale '$s' in $f"; fail=1; fi
  done
}
present() {  # present <fixed string> <file>: the string must appear in the file
  [ -f "$2" ] || { echo "FAIL: $2 is missing"; fail=1; return; }
  grep -qF -- "$1" "$2" || { echo "FAIL: '$1' missing from $2"; fail=1; }
}

# Capture size is 1.5x the game size, never a fixed 1920x1080.
absent "1920x1080" tools/record.sh make-cartridge.sh src/game/autopilot.rs README.md \
  "$SKILLS/designing-cartridge-covers/autopilot.md" "$SKILLS/recording-game-footage/SKILL.md"
absent "1920, 1080" src/game/record/mod.rs
absent "1280, 720" src/game/record/mod.rs
present "game-size.sh" tools/record.sh
present "game-size.sh" make-cartridge.sh
present "GX_FRAME=off" tools/record.sh
present "GX_FRAME=off" make-cartridge.sh
present "GAME_SIZE=1280x720" "$SKILLS/recording-game-footage/SKILL.md"

# --- end of checks ---
[ "$fail" -eq 0 ] || exit 1
echo "test_aspect_docs: OK"
