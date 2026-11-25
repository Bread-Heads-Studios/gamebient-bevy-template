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
#   2. clippy --all-targets with warnings denied, as ci.yml runs it;
#   3. the template's own `tests/app_boots.rs`, copied in with only the
#      crate name changed, compiles and passes against a stand-in game
#      (`gamebient_game`: a state machine, one game-over end card slot,
#      empty plugins): the file a 2D port copies must not need bevy_pbr.
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
mkdir -p "$crate/src" "$crate/tests" "$crate/assets/looks"

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

// A stand-in for the rest of a game: just what tests/app_boots.rs names.
pub mod game {
    use bevy::prelude::*;

    use crate::look::EndCardSlot;
    use states::GameState;

    pub mod states {
        use bevy::prelude::*;

        #[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
        pub enum GameState {
            #[default]
            Menu,
            Playing,
            GameOver,
        }
    }

    // A field, so \`GamePlugin::default()\` is not a unit struct (clippy).
    #[derive(Default)]
    pub struct GamePlugin {
        _windowed: bool,
    }

    impl Plugin for GamePlugin {
        fn build(&self, app: &mut App) {
            app.init_state::<GameState>()
                .add_systems(OnEnter(GameState::GameOver), |mut commands: Commands| {
                    commands.spawn((Node::default(), EndCardSlot));
                });
        }
    }
}

pub mod assets {
    pub struct AssetsPlugin;
    impl bevy::prelude::Plugin for AssetsPlugin {
        fn build(&self, _: &mut bevy::prelude::App) {}
    }
}

pub mod display {
    pub struct DisplayPlugin;
    impl bevy::prelude::Plugin for DisplayPlugin {
        fn build(&self, _: &mut bevy::prelude::App) {}
    }
}

pub mod frame {
    pub struct FramePlugin;
    impl bevy::prelude::Plugin for FramePlugin {
        fn build(&self, _: &mut bevy::prelude::App) {}
    }
}

pub mod ui {
    use bevy::prelude::*;

    use crate::look::EndCardStyle;

    pub mod theme {
        use bevy::color::Color;

        use crate::look::Contract;

        pub static LOOK: Contract = Contract {
            palette: &[
                ("endcard", Color::srgb(0.1, 0.1, 0.1)),
                ("endcardhead", Color::srgb(1.0, 1.0, 1.0)),
                ("endcardbody", Color::srgb(0.9, 0.9, 0.9)),
                ("endcardurl", Color::srgb(0.5, 0.7, 1.0)),
            ],
            strings: &[],
            slots: &[],
            audio: &[],
            fonts: &[],
            glow: &[],
        };
    }

    pub struct UiPlugin;

    impl Plugin for UiPlugin {
        fn build(&self, app: &mut App) {
            app.insert_resource(EndCardStyle {
                display: TextFont::default(),
                body: TextFont::default(),
                display_advance_em: 0.6,
                body_advance_em: 0.6,
                text_width: 400.0,
                headline_size: 28.0,
                body_size: 18.0,
                url_size: 16.0,
            });
        }
    }
}
EOF

# The template's own boot test, as a port copies it: the crate name only.
sed 's/gamebient_game/look_2d_check/g' "$root/tests/app_boots.rs" >"$crate/tests/app_boots.rs"

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
echo "== tests/app_boots.rs, copied unchanged, runs without bevy_pbr"
RUSTFLAGS="${RUSTFLAGS:-} -D warnings" cargo test --test app_boots
echo "ok: src/look/ and tests/app_boots.rs work without bevy_pbr"
