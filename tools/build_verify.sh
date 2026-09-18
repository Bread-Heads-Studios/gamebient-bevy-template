#!/usr/bin/env bash
# Builds the replay verifier as a wasm-bindgen *nodejs* module into
# dist-verify/. Same target, profile and wasm-opt flags as build_web.sh so the
# sim's float behaviour is the shipped game's. Usage: tools/build_verify.sh
set -euo pipefail
cd "$(dirname "$0")/.."
rustup target add wasm32-unknown-unknown 2>/dev/null || true
# --locked for the same reason build_web.sh uses it: the verifier module and
# the game bundle must be built from one dependency set, and the wasm-bindgen
# library must match the pinned CLI below.
cargo build --locked --profile wasm-release --target wasm32-unknown-unknown \
    --bin verify --features verify
command -v wasm-bindgen >/dev/null || { echo "wasm-bindgen-cli missing (see install.sh)" >&2; exit 1; }
command -v wasm-opt >/dev/null || { echo "wasm-opt missing (binaryen)" >&2; exit 1; }
# Needed at the very end, for dist-verify.zip (what build_web.sh publishes as
# dist/verify.zip and release.yml attaches). Checked here, with the other two
# tool guards, so a missing zip fails with a name rather than with bash's
# "zip: command not found" after the wasm-bindgen/wasm-opt work is done.
command -v zip >/dev/null || { echo "zip missing (needed for dist-verify.zip)" >&2; exit 1; }
rm -rf dist-verify && mkdir -p dist-verify
wasm-bindgen --out-dir dist-verify --out-name verify --target nodejs \
    target/wasm32-unknown-unknown/wasm-release/verify.wasm
wasm-opt -Oz --enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext \
    dist-verify/verify_bg.wasm -o dist-verify/verify_bg.wasm
# Compute GX_BUILD_ID exactly the way build.rs bakes it into the replay
# header (rustc-env at compile time): <CARGO_PKG_VERSION>+<short sha>. This
# can't be scraped back out of the optimized wasm reliably (wasm-opt -Oz can
# drop or mangle the embedded string), so it's recomputed here instead. It
# only agrees with what's actually embedded in verify_bg.wasm as long as
# both are produced from the same checkout state — build.rs only re-runs on
# `.git/HEAD`/refs changes, so don't build the verifier from a dirty tree or
# a different commit than the one that produced the game binary being served.
VERSION=$(grep -m1 '^version' Cargo.toml | sed -E 's/.*"([^"]+)".*/\1/')
SHA=$(git rev-parse --short=7 HEAD 2>/dev/null || true)
if [ -z "$SHA" ]; then
    # ${VERCEL_GIT_COMMIT_SHA:-} guards the substring expansion below against
    # `set -u` when the var is unset entirely (e.g. no git checkout AND no
    # Vercel env — a hard failure either way, but nounset must not pre-empt
    # the clear error message with an "unbound variable" trace instead).
    SHA="${VERCEL_GIT_COMMIT_SHA:-}"
    SHA="${SHA:0:7}"
fi
if [ -z "$SHA" ]; then
    echo "ERROR: cannot determine a unique build id; a git checkout or VERCEL_GIT_COMMIT_SHA is required" >&2
    exit 1
fi
echo "GX_BUILD_ID=${VERSION}+${SHA}" > dist-verify/BUILD
grep -qE '^GX_BUILD_ID=[0-9A-Za-z.+_-]+\+[0-9a-f]{7}$' dist-verify/BUILD \
    || { echo "BUILD id malformed" >&2; exit 1; }
echo "dist-verify/BUILD: $(cat dist-verify/BUILD)"
ls -la dist-verify

# Zip dist-verify/ so build_web.sh can publish it as dist/verify.zip (what the
# site fetches at properties.verify_url) and release.yml can attach it as
# `<package>-verify.zip` — the release asset named after this crate — without
# re-zipping the directory itself. This file is copied verbatim into every
# game, so the asset name is deliberately not spelled out here: it is
# whatever that game's Cargo.toml `name` is, and release.yml is the file
# that has it.
rm -f dist-verify.zip
(cd dist-verify && zip -qr ../dist-verify.zip .)
ls -la dist-verify.zip
