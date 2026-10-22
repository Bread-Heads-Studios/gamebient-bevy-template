#!/bin/bash
# Install step for vercel.json's installCommand (also run by `vercel build` in
# .github/workflows/deploy.yml). Lives in a script (not inline in vercel.json)
# because Vercel's schema caps `installCommand` at 256 characters. Each tool is
# installed only when missing, so a runner that already set them up skips it.
set -euo pipefail

if ! command -v rustup &> /dev/null; then
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | \
        sh -s -- -y --default-toolchain stable
fi
export PATH="$HOME/.cargo/bin:$PATH"
rustup target add wasm32-unknown-unknown
if [ "$(wasm-bindgen --version 2>/dev/null)" != "wasm-bindgen 0.2.108" ]; then
    cargo install wasm-bindgen-cli@0.2.108 --locked
fi
command -v wasm-opt &> /dev/null || npm install -g wasm-opt
