#!/bin/bash
# Vercel install step. Lives in a script (not inline in vercel.json) because
# Vercel's schema caps `installCommand` at 256 characters.
set -euo pipefail

curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | \
    sh -s -- -y --default-toolchain stable
export PATH="$HOME/.cargo/bin:$PATH"
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli@0.2.108 --locked
npm install -g wasm-opt
