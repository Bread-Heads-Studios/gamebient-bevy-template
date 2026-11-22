#!/usr/bin/env bash
# Proves `src/look/` compiles, unchanged and warning-free, in a 2D game
# that has no `bevy_pbr`: the template itself keeps pbr (its art is 3D),
# so CI cannot prove that by building the template.
#
# Writes a scratch crate to target/look-2d/crate whose lib is only
# `#[path = ".../src/look/mod.rs"] pub mod look;`, whose build.rs is the
# 2D port recipe (`cargo::rustc-cfg=gx_look_no_pbr` + `embed_looks()`),
# and whose Bevy feature set is the fleet's smallest 2D one
# (Gravestone_Gauntlet's: no bevy_pbr, no bevy_light). It declares no
# `editor` feature, as a game without the tunables layer must not. Then:
#
#   1. `cargo tree` must not find bevy_pbr in the graph at all;
#   2. clippy --all-targets with warnings denied, as ci.yml runs it.
#
# The crate reuses this repo's Cargo.lock (same versions, no new
# downloads) and builds in target/look-2d/target, apart from target/.
#
# Usage: tools/test_look_2d.sh   (GX_LOOK_2D_DIR overrides the scratch dir)
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
dir="${GX_LOOK_2D_DIR:-$root/target/look-2d}"
crate="$dir/crate"
rm -rf "$crate"
mkdir -p "$crate/src" "$crate/assets/looks"

# The game's own gamebient-input pin, so the crate sees the same HostCommand.
input_dep=$(grep -m1 '^gamebient-input = ' "$root/Cargo.toml")

cat >"$crate/Cargo.toml" <<EOF
[package]
name = "look-2d-check"
version = "0.0.0"
edition = "2024"
publish = false

# Not part of any workspace above it.
[workspace]

[dependencies]
bevy = { version = "0.18", default-features = false, features = [
    "async_executor",
    "bevy_asset",
    "bevy_audio",
    "bevy_camera",
    "bevy_color",
    "bevy_core_pipeline",
    "bevy_image",
    "bevy_log",
    "bevy_mesh",
    "bevy_render",
    "bevy_shader",
    "bevy_sprite",
    "bevy_sprite_render",
    "bevy_state",
    "bevy_text",
    "bevy_ui",
    "bevy_ui_render",
    "bevy_window",
    "bevy_winit",
    "default_font",
    "png",
    "std",
    "vorbis",
    "wav",
    "webgl2",
] }
$input_dep
serde = { version = "1", features = ["derive"] }
serde_json = "1"
blake3 = "1"

# winit needs a windowing backend on Linux (CI), as every game's own
# Linux target table gives it.
[target.'cfg(target_os = "linux")'.dependencies]
bevy = { version = "0.18", default-features = false, features = ["x11"] }

[build-dependencies]
blake3 = "1"
EOF

cat >"$crate/build.rs" <<EOF
#[path = "$root/src/look/embed.rs"]
mod look_embed;

fn main() {
    // The 2D port recipe's one extra line: no bevy_pbr in this game.
    println!("cargo::rustc-cfg=gx_look_no_pbr");
    look_embed::embed_looks();
}
EOF

cat >"$crate/src/lib.rs" <<EOF
//! src/look/ as a 2D game without bevy_pbr gets it (tools/test_look_2d.sh).
// The same crate-level allow every game's lib.rs carries (Bevy queries,
// and the generated looks_embedded.rs tables).
#![allow(clippy::too_many_arguments, clippy::type_complexity)]
#[path = "$root/src/look/mod.rs"]
pub mod look;
EOF

cp "$root/Cargo.lock" "$crate/Cargo.lock"

cd "$crate"
export CARGO_TARGET_DIR="$dir/target"

echo "== bevy_pbr must not be in the 2D graph"
# Every package in the graph, one per line ("name vX.Y.Z"). An empty list
# means cargo tree failed, which must not read as "no bevy_pbr".
packages=$(cargo tree -e normal --prefix none)
if ! grep -q '^bevy_sprite_render v' <<<"$packages"; then
  echo "NOT RUN: cargo tree listed no bevy_sprite_render; cannot tell" >&2
  exit 1
fi
if grep -q '^bevy_pbr v' <<<"$packages"; then
  echo "FAIL: bevy_pbr is in the dependency graph; the check would prove nothing" >&2
  cargo tree -e normal -i bevy_pbr >&2
  exit 1
fi
echo "ok: no bevy_pbr"

echo "== clippy, warnings denied"
RUSTFLAGS="${RUSTFLAGS:-} -D warnings" cargo clippy --all-targets -- -D warnings
echo "ok: src/look/ compiles without bevy_pbr"
