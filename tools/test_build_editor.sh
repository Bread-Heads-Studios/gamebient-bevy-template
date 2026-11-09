#!/usr/bin/env bash
# Checks tools/build_editor.sh refuses, by name, when wasm-bindgen is
# absent, instead of failing deep inside cargo. Usage: tools/test_build_editor.sh
set -euo pipefail
cd "$(dirname "$0")/.."
# A PATH with the core tools but no cargo-installed binaries.
out=$(PATH="/usr/bin:/bin" bash tools/build_editor.sh 2>&1 || true)
if printf '%s' "$out" | grep -q "wasm-bindgen-cli missing"; then
    echo "ok: build_editor.sh names the missing tool"
else
    echo "FAIL: expected 'wasm-bindgen-cli missing', got:" >&2
    printf '%s\n' "$out" >&2
    exit 1
fi
# The bundle must come from the wasm-editor profile (fast relinks); a slip
# back to wasm-release would cost two minutes per change in the editor.
if grep -q -- '--profile wasm-editor' tools/build_editor.sh && ! grep -q 'wasm-release' tools/build_editor.sh; then
    echo "ok: build_editor.sh uses the wasm-editor profile"
else
    echo "FAIL: build_editor.sh must build with --profile wasm-editor and not mention wasm-release" >&2
    exit 1
fi
if grep -q '^\[profile.wasm-editor\]' Cargo.toml; then
    echo "ok: Cargo.toml defines profile.wasm-editor"
else
    echo "FAIL: Cargo.toml lacks [profile.wasm-editor]" >&2
    exit 1
fi
