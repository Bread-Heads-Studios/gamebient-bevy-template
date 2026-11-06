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
