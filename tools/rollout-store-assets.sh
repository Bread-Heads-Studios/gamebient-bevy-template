#!/usr/bin/env bash
# Copies tools/store-assets.sh, and tools/game-size.sh which it takes the
# game's size from, out of this template into a game checkout. Idempotent.
# A game with no src/display.rs yet runs it as GAME_SIZE=1280x720 tools/store-assets.sh.
# Usage: tools/rollout-store-assets.sh <game-dir>
set -euo pipefail
TEMPLATE="$(cd "$(dirname "$0")/.." && pwd)"
GAME="$(cd "${1:?usage: $0 <game-dir>}" && pwd)"
mkdir -p "$GAME/tools"
copied=()
for f in store-assets.sh game-size.sh; do
  if ! cmp -s "$TEMPLATE/tools/$f" "$GAME/tools/$f" 2>/dev/null; then
    cp "$TEMPLATE/tools/$f" "$GAME/tools/$f"; chmod +x "$GAME/tools/$f"
    copied+=("tools/$f")
  fi
done
if [ ${#copied[@]} -eq 0 ]; then
  echo "rollout-store-assets: $GAME already current"
else
  echo "rollout-store-assets: copied ${copied[*]} into $GAME"
fi
