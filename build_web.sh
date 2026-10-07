#!/bin/bash
set -euo pipefail

echo "Building Gamebient Game for Web (WASM)..."

# Ensure wasm target is installed
rustup target add wasm32-unknown-unknown 2>/dev/null || true

# Check for wasm-bindgen
if ! command -v wasm-bindgen &> /dev/null; then
    echo "Installing wasm-bindgen-cli..."
    cargo install wasm-bindgen-cli
fi
# wasm-opt is required; fail loudly if missing.
if ! command -v wasm-opt &> /dev/null; then
    echo "ERROR: wasm-opt not found. Install binaryen (e.g. 'npm install -g wasm-opt')." >&2
    exit 1
fi

hash8() {
    if command -v sha256sum &> /dev/null; then sha256sum "$1"; else shasum -a 256 "$1"; fi | cut -c1-8
}

# bundle OUT FEATURES: builds one web bundle into OUT. The demo (`--features
# demo`) goes to dist/, the full game to dist/full/; Vercel middleware gates
# the latter. Both are built from this same commit, so a demo run and a full
# run are identical up to the demo's cut.
bundle() {
    local out="$1" features="$2"
    echo "== bundle: ${out} (features: ${features:-none})"

    # --locked: Cargo.lock is committed, and this same script is the Vercel
    # build command, so a deploy must use the exact dependency versions CI
    # tested (wasm-bindgen in particular, whose CLI install.sh pins separately).
    if [ -n "$features" ]; then
        cargo build --locked --profile wasm-release --target wasm32-unknown-unknown --features "$features"
    else
        cargo build --locked --profile wasm-release --target wasm32-unknown-unknown
    fi

    # Find the compiled .wasm by glob (cargo names it after the bin target) and
    # force deterministic output names with --out-name. verify.wasm is excluded
    # by name: tools/build_verify.sh builds the replay verifier into the same
    # directory. Anything else unexpected in there is a hard error.
    mkdir -p "$out"
    local wasm
    wasm=$(find target/wasm32-unknown-unknown/wasm-release -maxdepth 1 -name '*.wasm' ! -name 'verify.wasm')
    if [ -z "$wasm" ]; then
        echo "ERROR: no game .wasm found in target/wasm32-unknown-unknown/wasm-release/" >&2
        exit 1
    fi
    if [ "$(printf '%s\n' "$wasm" | wc -l | tr -d ' ')" -ne 1 ]; then
        echo "ERROR: more than one candidate .wasm in target/wasm32-unknown-unknown/wasm-release/:" >&2
        printf '%s\n' "$wasm" >&2
        echo "Remove the stale ones (or 'cargo clean') so the deployed bundle is unambiguous." >&2
        exit 1
    fi
    wasm-bindgen --out-dir "$out" --out-name gamebient-game --target web "$wasm"

    cp index.html "$out/"

    echo "Optimizing WASM with wasm-opt..."
    # Explicitly allow the WASM extensions enabled in .cargo/config.toml.
    wasm-opt -Oz --enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext \
        "$out/gamebient-game_bg.wasm" -o "$out/gamebient-game_bg.wasm"

    # Content-hash the immutable assets; index.html stays unhashed (no-cache).
    rm -f "$out"/gamebient-game_bg.????????.wasm "$out"/gamebient-game_bg.????????.wasm.br "$out"/gamebient-game_bg.????????.wasm.gz \
          "$out"/gamebient-game.????????.js "$out"/gamebient-game.????????.js.br "$out"/gamebient-game.????????.js.gz \
          "$out"/gamebient-game_bg.wasm.br "$out"/gamebient-game_bg.wasm.gz "$out"/gamebient-game.js.br "$out"/gamebient-game.js.gz
    local wasm_hash js_hash wasm_out js_out
    wasm_hash=$(hash8 "$out/gamebient-game_bg.wasm")
    js_hash=$(hash8 "$out/gamebient-game.js")
    wasm_out="gamebient-game_bg.${wasm_hash}.wasm"
    js_out="gamebient-game.${js_hash}.js"
    mv "$out/gamebient-game_bg.wasm" "$out/${wasm_out}"
    mv "$out/gamebient-game.js" "$out/${js_out}"
    sed -i.bak "s#gamebient-game_bg\.wasm#${wasm_out}#g" "$out/${js_out}" && rm -f "$out/${js_out}.bak"
    sed -i.bak -e "s#\./gamebient-game_bg\.wasm#./${wasm_out}#g" -e "s#\./gamebient-game\.js#./${js_out}#g" "$out/index.html" && rm -f "$out/index.html.bak"
    echo "Hashed assets: ${wasm_out}, ${js_out}"

    # Brotli-compress the wasm with Node's zlib (no separate brotli binary).
    if command -v node &> /dev/null; then
        echo "Brotli-compressing WASM..."
        node -e "const fs=require('fs'),zlib=require('zlib');const src=fs.readFileSync('${out}/${wasm_out}');const o=zlib.brotliCompressSync(src,{params:{[zlib.constants.BROTLI_PARAM_QUALITY]:11}});fs.writeFileSync('${out}/${wasm_out}.br',o);console.log('  '+src.length+' -> '+o.length+' bytes ('+(o.length*100/src.length).toFixed(1)+'%)')"
    else
        echo "WARNING: node not found; skipping brotli compression." >&2
    fi
}

# Fresh output: the full bundle lives inside dist/, so stale files from a
# previous layout must not survive.
rm -rf dist
bundle dist demo
bundle dist/full ""

# Game assets (Bevy expects assets/ relative to the page); the full bundle gets
# its own copy so nothing resolves across the /full/ boundary.
if [ -d assets ]; then
    rm -rf dist/assets dist/full/assets
    cp -r assets dist/assets
    cp -r assets dist/full/assets
fi

# The Pi cartridge binary (~45 MB) is fetched from the latest GitHub release so
# binary_url keeps resolving at the same Vercel path. Non-fatal.
bash fetch-cartridge.sh || true

# The headless replay verifier, once, from the full sim: dist/verify.zip is
# what properties.verify_url names. Built last so the `find` above never sees
# verify.wasm before it is excluded by name.
bash tools/build_verify.sh
cp dist-verify.zip dist/verify.zip

echo ""
echo "Build complete! Demo in dist/, full game in dist/full/ (gated by middleware.ts)."
echo "To test locally: cd dist && python3 -m http.server 8080  (full: /full/ is open locally; only Vercel runs the middleware)"
