#!/usr/bin/env bash
# Copies the record feature from this template into a game checkout and wires
# it: feature flag, `pub mod record`, the RecordPlugin block in GamePlugin,
# the RecordBeat branch in the autopilot's shot(), Debug on SfxEvent.
# Idempotent. Games whose layout differs from the template (autopilot under
# src/, no GameData, SfxEvent elsewhere) print what still needs a hand edit.
#
# Usage: tools/rollout-record.sh <game-dir>
set -euo pipefail
TEMPLATE="$(cd "$(dirname "$0")/.." && pwd)"
GAME="$(cd "${1:?usage: $0 <game-dir>}" && pwd)"

cp "$TEMPLATE/src/game/record.rs" "$GAME/src/game/record.rs"
mkdir -p "$GAME/tools"
cp "$TEMPLATE/tools/record.sh" "$TEMPLATE/tools/cut_clips.py" "$TEMPLATE/tools/test_cut_clips.py" "$GAME/tools/"
chmod +x "$GAME/tools/record.sh"

# Cargo feature.
if ! grep -q '^record = ' "$GAME/Cargo.toml"; then
  perl -0pi -e 's/^autopilot = \[\]\n/autopilot = []\n# Offline footage recorder layered on the autopilot tour (src\/game\/record.rs,\n# tools\/record.sh). Dev-only; never enabled in shipping builds.\nrecord = ["autopilot"]\n/m' "$GAME/Cargo.toml"
fi
grep -q '^record = ' "$GAME/Cargo.toml" || echo "HAND EDIT: add 'record = [\"autopilot\"]' under [features] in Cargo.toml"

# Module declaration + plugin block.
MOD="$GAME/src/game/mod.rs"
if [ -f "$MOD" ]; then
  if ! grep -qE '^\s*pub mod record;' "$MOD"; then
    perl -0pi -e 's/(#\[cfg\(feature = "autopilot"\)\]\npub mod autopilot;\n)/$1#[cfg(feature = "record")]\npub mod record;\n/' "$MOD"
  fi
  if ! grep -q 'record::RecordPlugin' "$MOD"; then
    perl -0pi -e 's/^([ \t]*)(app\.add_plugins\(autopilot::AutopilotPlugin\);\n)/$1$2$1#[cfg(feature = "record")]\n$1\{\n$1    app.add_plugins(record::RecordPlugin);\n$1    record::log_state::<GameState>(app);\n$1    record::log_messages::<audio::SfxEvent>(app);\n$1    record::log_value::<scoring::GameData>(app, "score", |d| i64::from(d.score));\n$1    record::log_value::<states::Paused>(app, "pause", |p| i64::from(p.0));\n$1\}\n/m' "$MOD"
  fi
  grep -qE '^\s*pub mod record;' "$MOD" || echo "HAND EDIT: declare 'pub mod record' in $MOD"
  grep -q 'record::RecordPlugin' "$MOD" || echo "HAND EDIT: add the RecordPlugin block next to AutopilotPlugin in $MOD"
else
  echo "HAND EDIT: no src/game/mod.rs; declare 'mod record' and add the RecordPlugin block where AutopilotPlugin is registered"
fi

# Autopilot shot().
AUTO="$GAME/src/game/autopilot.rs"
[ -f "$AUTO" ] || AUTO="$GAME/src/autopilot.rs"
if [ -f "$AUTO" ] && ! grep -q 'RecordBeat' "$AUTO"; then
  perl -0pi -e 's/^use bevy::render::view::screenshot::/#[cfg(not(feature = "record"))]\nuse bevy::render::view::screenshot::/m' "$AUTO"
  perl -0pi -e 's/fn shot\(commands: &mut Commands, name: &str\) \{\n    let path = format!\("\{\}\/\{name\}\.png", shot_dir\(\)\);\n    info!\("autopilot: screenshot \{path\}"\);\n    commands\n        \.spawn\(Screenshot::primary_window\(\)\)\n        \.observe\(save_to_disk\(path\)\);\n\}/fn shot(commands: &mut Commands, name: &str) {\n    #[cfg(feature = "record")]\n    {\n        info!("autopilot: beat {name}");\n        commands.write_message(super::record::RecordBeat(name.to_string()));\n    }\n    #[cfg(not(feature = "record"))]\n    {\n        let path = format!("{}\/{name}.png", shot_dir());\n        info!("autopilot: screenshot {path}");\n        commands\n            .spawn(Screenshot::primary_window())\n            .observe(save_to_disk(path));\n    }\n}/' "$AUTO"
fi
grep -q 'RecordBeat' "$AUTO" 2>/dev/null || echo "HAND EDIT: make shot() write RecordBeat under cfg(feature = \"record\") in ${AUTO:-the autopilot file}"

# SfxEvent must be Debug for log_messages.
SFX="$(grep -rl 'pub enum SfxEvent' "$GAME/src" | head -1 || true)"
if [ -n "$SFX" ]; then
  perl -0pi -e 's/#\[derive\(([^)]*)\)\]\npub enum SfxEvent/my $d=$1; $d =~ \/\bDebug\b\/ ? "#[derive($d)]\npub enum SfxEvent" : "#[derive($d, Debug)]\npub enum SfxEvent"/e' "$SFX"
else
  echo "HAND EDIT: no 'pub enum SfxEvent' found; drop or retarget the log_messages line"
fi

echo "rollout-record: files in place for $GAME"
echo "next: (cd $GAME && cargo check --features record && cargo clippy --all-targets --all-features -- -D warnings)"
