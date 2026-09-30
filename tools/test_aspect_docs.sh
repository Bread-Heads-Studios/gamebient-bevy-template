#!/usr/bin/env bash
# Guards the tooling, its comments and the skills against stale 16:9 claims.
# Each block names strings that must be gone and strings that must be there.
# Plain grep on tracked files; needs no media tools.
# TEMPLATE-ONLY: it asserts the template's own assets/info.json and docs, which
# a game rewrites (its own aspect, marquee and bezel URLs). No rollout script
# copies it into a game (tools/test_rollouts.sh checks that).
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

# Vertical clips: plates are placed by the source's aspect.
absent "centered 16:9 band" README.md
absent "full 16:9 frame" "$SKILLS/recording-game-footage/SKILL.md"
absent "16:9 gameplay" tools/vertical-banner.svg
present "{{CTA_PLATE_Y}}" tools/vertical-banner.svg
present "y 240-1680" "$SKILLS/recording-game-footage/SKILL.md"

# Reel: clips are fitted over a blurred fill, never forced to the reel's size.
absent "16:9 from build/record/clips/" "$SKILLS/recording-game-footage/tools/reel.py"
present "force_original_aspect_ratio=decrease" "$SKILLS/recording-game-footage/tools/reel.py"
present "810x1080" "$SKILLS/recording-game-footage/SKILL.md"

# Cover: the screenshot slot follows the game's ratio.
absent "(16:9)" tools/cartridge-cover.svg
absent "16:9 shot" tools/cartridge-cover.svg
present "360 x 480" tools/cartridge-cover.svg
present "Gameplay panel by ratio" "$SKILLS/designing-cartridge-covers/SKILL.md"
present "x 204, y 424, 360 x 480" "$SKILLS/designing-cartridge-covers/SKILL.md"

# Skill: marquee and bezel art.
FRAME_ART="$SKILLS/designing-cartridge-covers/frame-art.md"
present "marquee" "$SKILLS/designing-cartridge-covers/SKILL.md"
present "[frame-art.md](frame-art.md)" "$SKILLS/designing-cartridge-covers/SKILL.md"
present "tools/frame-art.sh" "$FRAME_ART"
present "1080x360" "$FRAME_ART"
present "1920x1920" "$FRAME_ART"
present "central 1080x1080" "$FRAME_ART"
present "x0.35" "$FRAME_ART"
present "x0.20" "$FRAME_ART"
present "x0.6" "$FRAME_ART"
present "full brightness" "$FRAME_ART"
present "Impact" "$FRAME_ART"

# Metadata: aspect, marquee, bezel. The template ships only "aspect": a game with
# no art of its own falls back to the shared art, so the template has no art URLs.
present '"aspect": "4:3"' assets/info.json
absent '"marquee"' assets/info.json
absent '"bezel"' assets/info.json
present "properties.aspect" docs/build-and-release.md
present "properties.marquee" docs/build-and-release.md
present "properties.bezel" docs/build-and-release.md
absent "(1280x720)" docs/build-and-release.md
present '"aspect": "4:3"' "$SKILLS/generating-cartridge-metadata/SKILL.md"
present '`marquee`' "$SKILLS/generating-cartridge-metadata/SKILL.md"
present '`bezel`' "$SKILLS/generating-cartridge-metadata/SKILL.md"

# --- end of checks ---
[ "$fail" -eq 0 ] || exit 1
echo "test_aspect_docs: OK"
