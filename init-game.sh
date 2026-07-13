#!/bin/bash
# One-time setup: turn this template into a named game by replacing placeholder
# tokens, then remove this script. Run once, from the repo root.
#
#   ./init-game.sh "My Game" [OWNER/REPO]
#
# - "My Game"   display name / window title
# - OWNER/REPO  GitHub repo for release hosting (leave blank to fill in later)
#
# Note: uses BSD/macOS `sed -i ''`. On Linux, change to `sed -i` (no '').
set -euo pipefail

NAME="${1:-}"
REPO="${2:-OWNER/REPO}"
if [ -z "$NAME" ]; then
    echo "Usage: $0 \"My Game\" [OWNER/REPO]" >&2
    exit 1
fi

# Idempotency guard: refuse if the template tokens are already gone.
if ! grep -q 'gamebient-game' Cargo.toml 2>/dev/null; then
    echo "ERROR: template tokens not found in Cargo.toml — already initialized?" >&2
    exit 1
fi

# Derive slugs from the display name.
slug=$(echo "$NAME" | tr '[:upper:]' '[:lower:]' | sed -E 's/[^a-z0-9]+/-/g; s/^-+|-+$//g')
snake=$(echo "$slug" | tr '-' '_')
upper=$(echo "$NAME" | tr '[:lower:]' '[:upper:]')

echo "Name:  $NAME"
echo "Slug:  $slug   (crate, artifacts, wasm bundle)"
echo "Snake: $snake  (Rust identifier form)"
echo "Repo:  $REPO"
echo

# Files to rewrite. Prefer tracked files; fall back to a filtered find. Always
# skip binary assets and generated dirs.
if files=$(git ls-files 2>/dev/null) && [ -n "$files" ]; then
    :
else
    files=$(find . -type f \
        -not -path './.git/*' -not -path './target/*' -not -path './dist/*' \
        -not -name '*.png' -not -name '*.tar.gz')
fi

while IFS= read -r f; do
    [ -n "$f" ] || continue
    case "$f" in *.png | *.tar.gz) continue ;; esac
    [ -f "$f" ] || continue
    sed -i '' \
        -e "s/gamebient_game/${snake}/g" \
        -e "s/gamebient-game/${slug}/g" \
        -e "s/GAMEBIENT GAME/${upper}/g" \
        -e "s/Gamebient Game/${NAME}/g" \
        -e "s#OWNER/REPO#${REPO}#g" \
        "$f" 2>/dev/null || true
done <<< "$files"

echo "Done. Next steps:"
echo "  1. cargo build         # refresh Cargo.lock with the new package name"
echo "  2. git add -A && git commit -m 'chore: initialize $NAME from template'"
echo "  3. Create the GitHub repo $REPO and push."
echo "  4. In Vercel: import the repo and set GH_TOKEN (fine-grained PAT, Contents: Read)."
echo "  5. Tag a release (git tag v0.0.1 && git push --tags) to build cartridge artifacts."
echo

# Remove this script.
rm -- "$0"
echo "Removed init-game.sh."
