#!/usr/bin/env bash
# Builds the editor bundle: the game with `--features editor` as a web
# bundle into dist-editor/, without wasm-opt, brotli, the demo split or the
# cartridge fetch (the editor serves this directory from a localhost origin;
# speed over size). Usage: tools/build_editor.sh
set -euo pipefail
cd "$(dirname "$0")/.."
rustup target add wasm32-unknown-unknown 2>/dev/null || true
command -v wasm-bindgen >/dev/null || { echo "wasm-bindgen-cli missing (see install.sh)" >&2; exit 1; }
cargo build --locked --profile wasm-release --target wasm32-unknown-unknown --features editor
wasm=$(find target/wasm32-unknown-unknown/wasm-release -maxdepth 1 -name '*.wasm' ! -name 'verify.wasm')
if [ "$(printf '%s\n' "$wasm" | wc -l | tr -d ' ')" -ne 1 ] || [ -z "$wasm" ]; then
    echo "ERROR: expected exactly one game .wasm in target/wasm32-unknown-unknown/wasm-release (cargo clean?):" >&2
    printf '%s\n' "$wasm" >&2
    exit 1
fi
rm -rf dist-editor && mkdir -p dist-editor
wasm-bindgen --out-dir dist-editor --out-name gamebient-game --target web "$wasm"
cp index.html dist-editor/
if [ -d assets ]; then cp -r assets dist-editor/assets; fi
echo "dist-editor/ ready: serve it from a localhost origin (the editor does)."
