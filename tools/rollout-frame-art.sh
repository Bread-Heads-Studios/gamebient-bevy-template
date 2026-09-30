#!/usr/bin/env bash
# Copies the cabinet frame-art tooling from this template into a game checkout:
# tools/frame-art.sh always (it is tooling), tools/marquee.svg and
# tools/bezel.svg only when the game has none (once designed they are the
# game's own art and are never overwritten). Idempotent.
# Usage: tools/rollout-frame-art.sh <game-dir>
set -euo pipefail
TEMPLATE="$(cd "$(dirname "$0")/.." && pwd)"
GAME="$(cd "${1:?usage: $0 <game-dir>}" && pwd)"
mkdir -p "$GAME/tools"
if cmp -s "$TEMPLATE/tools/frame-art.sh" "$GAME/tools/frame-art.sh" 2>/dev/null; then
  echo "rollout-frame-art: tools/frame-art.sh already current"
else
  cp "$TEMPLATE/tools/frame-art.sh" "$GAME/tools/frame-art.sh"; chmod +x "$GAME/tools/frame-art.sh"
  echo "rollout-frame-art: copied tools/frame-art.sh into $GAME"
fi
for f in marquee.svg bezel.svg; do
  if [ -e "$GAME/tools/$f" ]; then
    echo "rollout-frame-art: kept the game's own tools/$f"
  else
    cp "$TEMPLATE/tools/$f" "$GAME/tools/$f"
    echo "rollout-frame-art: copied the placeholder tools/$f into $GAME (design it: designing-cartridge-covers, frame-art.md)"
  fi
done
