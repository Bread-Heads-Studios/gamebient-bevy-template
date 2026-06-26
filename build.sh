#!/bin/bash
# Build Voidrunner for a single target.
#   ./build.sh pi    -> aarch64-unknown-linux-gnu (Raspberry Pi), via cross
#   ./build.sh x86   -> x86_64-unknown-linux-gnu, native on Linux else via cross
#   ./build.sh web   -> wasm32 bundle (delegates to build_web.sh / Vercel path)
set -euo pipefail

TARGET="${1:-}"

require_cross() {
    if ! command -v cross >/dev/null 2>&1; then
        echo "ERROR: 'cross' not found. Install it with: cargo install cross --locked" >&2
        echo "       (cross also requires Docker to be running.)" >&2
        exit 1
    fi
}

case "$TARGET" in
    web)
        exec bash build_web.sh
        ;;
    pi)
        require_cross
        echo "Building Voidrunner for Raspberry Pi (aarch64-unknown-linux-gnu)..."
        cross build --release --target aarch64-unknown-linux-gnu
        ;;
    x86)
        if [ "$(uname -s)" = "Linux" ]; then
            echo "Building Voidrunner for x86_64-unknown-linux-gnu (native)..."
            rustup target add x86_64-unknown-linux-gnu 2>/dev/null || true
            cargo build --release --target x86_64-unknown-linux-gnu
        else
            require_cross
            echo "Building Voidrunner for x86_64-unknown-linux-gnu (via cross)..."
            cross build --release --target x86_64-unknown-linux-gnu
        fi
        ;;
    *)
        echo "Usage: $0 <pi|x86|web>" >&2
        exit 1
        ;;
esac

echo "Build complete for target: $TARGET"
