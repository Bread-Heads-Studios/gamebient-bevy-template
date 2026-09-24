#!/usr/bin/env bash
# Copies tools/store-assets.sh from this template into a game checkout.
# Idempotent. Usage: tools/rollout-store-assets.sh <game-dir>
set -euo pipefail
TEMPLATE="$(cd "$(dirname "$0")/.." && pwd)"
GAME="$(cd "${1:?usage: $0 <game-dir>}" && pwd)"
mkdir -p "$GAME/tools"
if cmp -s "$TEMPLATE/tools/store-assets.sh" "$GAME/tools/store-assets.sh" 2>/dev/null; then
  echo "rollout-store-assets: $GAME already current"
else
  cp "$TEMPLATE/tools/store-assets.sh" "$GAME/tools/store-assets.sh"; chmod +x "$GAME/tools/store-assets.sh"
  echo "rollout-store-assets: copied tools/store-assets.sh into $GAME"
fi
