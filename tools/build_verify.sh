#!/usr/bin/env bash
# Builds the replay verifier as a wasm-bindgen *nodejs* module into
# dist-verify/. Same target, profile and wasm-opt flags as build_web.sh so the
# sim's float behaviour is the shipped game's. Usage: tools/build_verify.sh
set -euo pipefail
cd "$(dirname "$0")/.."
rustup target add wasm32-unknown-unknown 2>/dev/null || true
cargo build --profile wasm-release --target wasm32-unknown-unknown \
    --bin verify --features verify
command -v wasm-bindgen >/dev/null || { echo "wasm-bindgen-cli missing (see install.sh)" >&2; exit 1; }
command -v wasm-opt >/dev/null || { echo "wasm-opt missing (binaryen)" >&2; exit 1; }
rm -rf dist-verify && mkdir -p dist-verify
wasm-bindgen --out-dir dist-verify --out-name verify --target nodejs \
    target/wasm32-unknown-unknown/wasm-release/verify.wasm
wasm-opt -Oz --enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext \
    dist-verify/verify_bg.wasm -o dist-verify/verify_bg.wasm
echo "GX_BUILD_ID=$(strings dist-verify/verify_bg.wasm | grep -m1 -E '^[0-9]+\.[0-9]+\.[0-9]+\+' || true)" > dist-verify/BUILD
ls -la dist-verify

# Zip dist-verify/ so build_web.sh can publish it as dist/verify.zip (what the
# site fetches at properties.verify_url) and release.yml can attach it as
# gamebient-game-verify.zip without re-zipping the directory itself.
rm -f dist-verify.zip
(cd dist-verify && zip -qr ../dist-verify.zip .)
ls -la dist-verify.zip
