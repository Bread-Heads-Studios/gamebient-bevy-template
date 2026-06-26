#!/bin/bash
# Fetch the flat Pi cartridge binary (gamebient-game.tar.gz) from the latest GitHub
# release into dist/assets/, so Vercel can serve it at the same public path that
# assets/info.json's binary_url points to — without the 45 MB blob living in git.
#
# NON-FATAL by design: the playable web build does not depend on this file, so a
# missing token or release must NOT fail the deploy. It always exits 0; on any
# problem it warns and leaves binary_url unresolved (404) until fixed.
#
# Requires a GitHub token with read access to this private repo's releases,
# provided as GH_TOKEN (or GITHUB_TOKEN) in the Vercel project environment.

REPO="OWNER/REPO"
ASSET="gamebient-game.tar.gz"
DEST="dist/assets/$ASSET"

warn() { echo "WARNING: $*" >&2; }

main() {
    local token="${GH_TOKEN:-${GITHUB_TOKEN:-}}"
    if [ -z "$token" ]; then
        warn "GH_TOKEN/GITHUB_TOKEN not set; skipping $ASSET fetch. binary_url will 404 until set."
        return 0
    fi

    if ! command -v node >/dev/null 2>&1; then
        warn "node not found; cannot parse the release API response. Skipping $ASSET fetch."
        return 0
    fi

    # Resolve the asset's API URL from the latest release (private-repo safe).
    local asset_url
    asset_url=$(curl -fsSL \
        -H "Authorization: Bearer $token" \
        -H "Accept: application/vnd.github+json" \
        "https://api.github.com/repos/$REPO/releases/latest" 2>/dev/null \
        | node -e 'let d="";process.stdin.on("data",c=>d+=c).on("end",()=>{try{const r=JSON.parse(d);const a=(r.assets||[]).find(x=>x.name==="gamebient-game.tar.gz");process.stdout.write(a?a.url:"")}catch(e){}})')

    if [ -z "$asset_url" ]; then
        warn "$ASSET not found in the latest release of $REPO. binary_url will 404."
        return 0
    fi

    mkdir -p "$(dirname "$DEST")"
    if curl -fsSL \
        -H "Authorization: Bearer $token" \
        -H "Accept: application/octet-stream" \
        -o "$DEST" "$asset_url"; then
        echo "Fetched $ASSET -> $DEST ($(wc -c < "$DEST" | tr -d ' ') bytes)"
    else
        warn "download of $ASSET failed. binary_url will 404."
        rm -f "$DEST"
    fi
    return 0
}

main
exit 0
