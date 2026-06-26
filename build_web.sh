#!/bin/bash
set -euo pipefail

echo "Building Voidrunner for Web (WASM)..."

# Ensure wasm target is installed
rustup target add wasm32-unknown-unknown 2>/dev/null || true

# Build
cargo build --profile wasm-release --target wasm32-unknown-unknown

# Check for wasm-bindgen
if ! command -v wasm-bindgen &> /dev/null; then
    echo "Installing wasm-bindgen-cli..."
    cargo install wasm-bindgen-cli
fi

# Generate JS bindings. Find the compiled .wasm by glob (cargo names it after the
# bin target, which varies with the package name) and force deterministic output
# names with --out-name so index.html / wasm-opt / brotli paths are stable.
mkdir -p dist
WASM=$(find target/wasm32-unknown-unknown/wasm-release -maxdepth 1 -name '*.wasm' | head -1)
if [ -z "$WASM" ]; then
    echo "ERROR: no .wasm found in target/wasm32-unknown-unknown/wasm-release/" >&2
    exit 1
fi
wasm-bindgen \
    --out-dir dist \
    --out-name gamebient-game \
    --target web \
    "$WASM"

# Copy web files
cp index.html dist/

# Copy game assets (Bevy expects assets/ relative to the page)
if [ -d assets ]; then
    rm -rf dist/assets
    cp -r assets dist/assets
fi

# The Pi cartridge binary (assets/gamebient-game.tar.gz, ~45 MB) is no longer stored
# in git; it's fetched from the latest GitHub release so binary_url keeps
# resolving at the same Vercel path. Non-fatal: never breaks the web deploy.
bash fetch-cartridge.sh || true

# Optimize WASM size with wasm-opt (required — fail loudly if missing).
if ! command -v wasm-opt &> /dev/null; then
    echo "ERROR: wasm-opt not found. Install binaryen (e.g. 'npm install -g wasm-opt')." >&2
    exit 1
fi
echo "Optimizing WASM with wasm-opt..."
# Explicitly allow the WASM extensions enabled in .cargo/config.toml
# (+bulk-memory, +nontrapping-fptoint, +sign-ext). Without these flags, older
# binaryen builds (e.g. distro packages) reject the feature-using module with
# "all used features should be allowed" instead of inferring them.
wasm-opt -Oz \
    --enable-bulk-memory \
    --enable-nontrapping-float-to-int \
    --enable-sign-ext \
    dist/gamebient-game_bg.wasm -o dist/gamebient-game_bg.wasm

# Brotli-compress the wasm for production delivery. Uses Node's built-in
# zlib so we don't need a separate brotli binary on the build host.
if command -v node &> /dev/null; then
    echo "Brotli-compressing WASM..."
    node -e "const fs=require('fs'),zlib=require('zlib');const src=fs.readFileSync('dist/gamebient-game_bg.wasm');const out=zlib.brotliCompressSync(src,{params:{[zlib.constants.BROTLI_PARAM_QUALITY]:11}});fs.writeFileSync('dist/gamebient-game_bg.wasm.br',out);console.log('  '+src.length+' -> '+out.length+' bytes ('+(out.length*100/src.length).toFixed(1)+'%)')"
else
    echo "WARNING: node not found; skipping brotli compression. Production builds should produce gamebient-game_bg.wasm.br." >&2
fi

echo ""
echo "Build complete! Files in dist/"
echo "To test locally: cd dist && python3 -m http.server 8080"
echo "Then open http://localhost:8080"
