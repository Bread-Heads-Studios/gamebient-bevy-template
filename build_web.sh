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
# verify.wasm is excluded by name: tools/build_verify.sh builds the replay
# verifier into the same directory, and shipping it as the game would deploy a
# module with no window, renderer or assets. Anything else unexpected in there
# is a hard error rather than a coin flip about which .wasm gets deployed.
mkdir -p dist
WASM=$(find target/wasm32-unknown-unknown/wasm-release -maxdepth 1 -name '*.wasm' ! -name 'verify.wasm')
if [ -z "$WASM" ]; then
    echo "ERROR: no game .wasm found in target/wasm32-unknown-unknown/wasm-release/" >&2
    exit 1
fi
if [ "$(printf '%s\n' "$WASM" | wc -l | tr -d ' ')" -ne 1 ]; then
    echo "ERROR: more than one candidate .wasm in target/wasm32-unknown-unknown/wasm-release/:" >&2
    printf '%s\n' "$WASM" >&2
    echo "Remove the stale ones (or 'cargo clean') so the deployed bundle is unambiguous." >&2
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

# Build the headless replay verifier and publish it alongside the game bundle
# as dist/verify.zip — the site fetches it from properties.verify_url. Run
# after this script's own wasm-bindgen/wasm-opt steps (and after the `find`
# above, which already excludes verify.wasm by name) so there's no ambiguity
# about which .wasm is the game bundle.
bash tools/build_verify.sh
cp dist-verify.zip dist/verify.zip

# --- Content-hash the immutable assets ---------------------------------------
# index.html stays unhashed (served no-cache); the wasm and the JS glue get an
# 8-char content hash so vercel.json can mark them immutable and repeat visits
# (store Wi-Fi, kiosks) skip the network entirely. dist/index.html is rewritten
# to the hashed names; the source index.html keeps plain names for local dev.
hash8() {
    if command -v sha256sum &> /dev/null; then sha256sum "$1"; else shasum -a 256 "$1"; fi | cut -c1-8
}
rm -f dist/gamebient-game_bg.????????.wasm dist/gamebient-game_bg.????????.wasm.br dist/gamebient-game_bg.????????.wasm.gz \
      dist/gamebient-game.????????.js dist/gamebient-game.????????.js.br dist/gamebient-game.????????.js.gz \
      dist/gamebient-game_bg.wasm.br dist/gamebient-game_bg.wasm.gz dist/gamebient-game.js.br dist/gamebient-game.js.gz
WASM_HASH=$(hash8 dist/gamebient-game_bg.wasm)
JS_HASH=$(hash8 dist/gamebient-game.js)
WASM_OUT="gamebient-game_bg.${WASM_HASH}.wasm"
JS_OUT="gamebient-game.${JS_HASH}.js"
mv dist/gamebient-game_bg.wasm "dist/${WASM_OUT}"
mv dist/gamebient-game.js "dist/${JS_OUT}"
# The glue's default wasm path is only used when init() is called without a
# precompiled module; keep it correct anyway.
sed -i.bak "s#gamebient-game_bg\.wasm#${WASM_OUT}#g" "dist/${JS_OUT}" && rm -f "dist/${JS_OUT}.bak"
sed -i.bak -e "s#\./gamebient-game_bg\.wasm#./${WASM_OUT}#g" -e "s#\./gamebient-game\.js#./${JS_OUT}#g" dist/index.html && rm -f dist/index.html.bak
echo "Hashed assets: ${WASM_OUT}, ${JS_OUT}"

# Brotli-compress the wasm for production delivery. Uses Node's built-in
# zlib so we don't need a separate brotli binary on the build host.
if command -v node &> /dev/null; then
    echo "Brotli-compressing WASM..."
    node -e "const fs=require('fs'),zlib=require('zlib');const src=fs.readFileSync('dist/${WASM_OUT}');const out=zlib.brotliCompressSync(src,{params:{[zlib.constants.BROTLI_PARAM_QUALITY]:11}});fs.writeFileSync('dist/${WASM_OUT}.br',out);console.log('  '+src.length+' -> '+out.length+' bytes ('+(out.length*100/src.length).toFixed(1)+'%)')"
else
    echo "WARNING: node not found; skipping brotli compression. Production builds should produce gamebient-game_bg.wasm.br." >&2
fi

echo ""
echo "Build complete! Files in dist/"
echo "To test locally: cd dist && python3 -m http.server 8080"
echo "Then open http://localhost:8080"
