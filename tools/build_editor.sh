#!/usr/bin/env bash
# Builds the editor bundle: the game with `--features editor` as a web
# bundle into dist-editor/, with the wasm-editor profile (Cargo.toml: fast to
# rebuild, larger) and without wasm-opt, brotli, the demo split or the
# cartridge fetch. The editor serves this directory itself; speed over size.
# Usage: tools/build_editor.sh
set -euo pipefail
cd "$(dirname "$0")/.."
rustup target add wasm32-unknown-unknown 2>/dev/null || true
command -v wasm-bindgen >/dev/null || { echo "wasm-bindgen-cli missing (see install.sh)" >&2; exit 1; }
cargo build --locked --profile wasm-editor --target wasm32-unknown-unknown --features editor
wasm=$(find target/wasm32-unknown-unknown/wasm-editor -maxdepth 1 -name '*.wasm' ! -name 'verify.wasm' ! -name 'look-check.wasm')
if [ "$(printf '%s\n' "$wasm" | wc -l | tr -d ' ')" -ne 1 ] || [ -z "$wasm" ]; then
    echo "ERROR: expected exactly one game .wasm in target/wasm32-unknown-unknown/wasm-editor (cargo clean?):" >&2
    printf '%s\n' "$wasm" >&2
    exit 1
fi
rm -rf dist-editor && mkdir -p dist-editor
wasm-bindgen --out-dir dist-editor --out-name gamebient-game --target web "$wasm"
cp index.html dist-editor/
if [ -d assets ]; then cp -r assets dist-editor/assets; fi
echo "dist-editor/ ready: serve it from a localhost origin (the editor does)."
