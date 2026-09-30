#!/usr/bin/env bash
# Converts a game checkout to one of the three sanctioned screen shapes
# (4:3 = 960x720, 1:1 = 720x720, 3:4 = 720x960) and puts the cabinet frame
# in place: the verbatim copies from this template, the pinned size, the
# display contract test, the recorder/store/frame-art tooling, the Cargo
# features, HIGHER_SCORE_IS_BETTER and the Highlight relay. What a script
# cannot do safely it prints as `HAND EDIT: <path>: <what>` and leaves alone.
# Idempotent: on a game that is already converted a second run changes
# nothing and says so.
#
# Usage: tools/rollout-aspect.sh <game-dir> <4:3|1:1|3:4>
# Prove the result with tools/frame-accept.sh --check-copies <game-dir>.
set -euo pipefail
TEMPLATE="$(cd "$(dirname "$0")/.." && pwd)"
GAME="$(cd "${1:?usage: $0 <game-dir> <4:3|1:1|3:4>}" && pwd)"
RATIO="${2:?usage: $0 <game-dir> <4:3|1:1|3:4>}"

# ratio -> size, pinned-policy name, label, UI width, and the letterbox
# vectors the display contract pins (window -> x, y), (w, h). The gap is
# (8 * short + 540) / 1080 and the marquee is a third of the width.
case "$RATIO" in
  4:3)
    GW=960; GH=720; POLICY=PINNED_4X3; TITLE=four_by_three; UIW=960
    UI_A='960.0, 720.0'; UI_B='1440.0, 1080.0'
    V_TV_ON='((251, 8), (1418, 1064))'; V_TV_OFF='((240, 0), (1440, 1080))'
    V_PORT='((8, 741), (1064, 798))'
    V_DESK='((167, 5), (946, 710))'; V_DESK_P='((4, 370), (532, 399))' ;;
  1:1)
    GW=720; GH=720; POLICY=PINNED_1X1; TITLE=square; UIW=720
    UI_A='720.0, 720.0'; UI_B='1080.0, 1080.0'
    V_TV_ON='((428, 8), (1064, 1064))'; V_TV_OFF='((420, 0), (1080, 1080))'
    V_PORT='((8, 608), (1064, 1064))'
    V_DESK='((285, 5), (710, 710))'; V_DESK_P='((4, 304), (532, 532))' ;;
  3:4)
    GW=720; GH=960; POLICY=PINNED_3X4; TITLE=three_by_four; UIW=720
    UI_A='720.0, 960.0'; UI_B='1080.0, 1440.0'
    V_TV_ON='((561, 8), (798, 1064))'; V_TV_OFF='((555, 0), (810, 1080))'
    V_PORT='((8, 431), (1064, 1418))'
    V_DESK='((374, 5), (532, 710))'; V_DESK_P='((4, 215), (532, 709))' ;;
  *) echo "rollout-aspect: ratio must be 4:3, 1:1 or 3:4, got '$RATIO'" >&2; exit 2 ;;
esac

# 1. Refuse a dirty tree and a game with no layout this script knows.
if [ -n "$(git -C "$GAME" status --porcelain)" ]; then
  echo "rollout-aspect: $GAME has uncommitted changes; commit or stash them first" >&2
  exit 1
fi
FLAT=false
if [ -f "$GAME/src/game/mod.rs" ]; then
  :
elif [ -f "$GAME/src/sim.rs" ]; then
  FLAT=true
else
  echo "rollout-aspect: $GAME has neither src/game/mod.rs nor a flat src/sim.rs" >&2
  exit 1
fi
if [ "$FLAT" = true ]; then
  FIT_DEST=src/fit.rs; SCORING=src/scoring.rs; HOST=src/host.rs
else
  FIT_DEST=src/ui/fit.rs; SCORING=src/game/scoring.rs; HOST=src/game/host.rs
fi
# A template copy (it carries this script) keeps the template's own tests.
IS_TEMPLATE=false
[ -f "$GAME/tools/rollout-aspect.sh" ] && IS_TEMPLATE=true

# The crate name tests import: [lib] name, else the package name.
LIB="$(perl -ne '
  if (/^\s*\[/) { $s = $_; next }
  if (/^\s*name\s*=\s*"([^"]+)"/) {
    my $v = $1;
    if ($s =~ /^\s*\[lib\]/) { $lib = $v }
    elsif ($s =~ /^\s*\[package\]/) { $pkg = $v }
  }
  END { $n = $lib || $pkg; $n =~ s/-/_/g; print $n }' "$GAME/Cargo.toml")"
[ -n "$LIB" ] || { echo "rollout-aspect: no crate name in $GAME/Cargo.toml" >&2; exit 1; }

# 2. Verbatim copies, then the pinned size and the crate name.
cp "$TEMPLATE/src/display.rs" "$GAME/src/display.rs"
perl -pi -e "s/^pub const GAME_WIDTH: u32 = \\d+;/pub const GAME_WIDTH: u32 = $GW;/; s/^pub const GAME_HEIGHT: u32 = \\d+;/pub const GAME_HEIGHT: u32 = $GH;/" "$GAME/src/display.rs"

# src/frame/: every file and art/. A game's driver.rs keeps its own test
# module (the phase_for list and the two score helpers are game-specific),
# so a re-run does not undo the hand edits there.
DRIVER_TAIL=""
if [ -f "$GAME/src/frame/driver.rs" ]; then
  DRIVER_TAIL="$(mktemp)"
  perl -ne 'print if $on ||= /^#\[cfg\(test\)\]/' "$GAME/src/frame/driver.rs" > "$DRIVER_TAIL"
  [ -s "$DRIVER_TAIL" ] || { rm -f "$DRIVER_TAIL"; DRIVER_TAIL=""; }
fi
rm -rf "$GAME/src/frame"
cp -R "$TEMPLATE/src/frame" "$GAME/src/frame"
if [ -n "$DRIVER_TAIL" ]; then
  perl -ne 'last if /^#\[cfg\(test\)\]/; print' "$TEMPLATE/src/frame/driver.rs" > "$GAME/src/frame/driver.rs"
  cat "$DRIVER_TAIL" >> "$GAME/src/frame/driver.rs"
  rm -f "$DRIVER_TAIL"
fi

mkdir -p "$GAME/tests" "$GAME/$(dirname "$FIT_DEST")"
cp "$TEMPLATE/src/ui/fit.rs" "$GAME/$FIT_DEST"
perl -pe "s/\\bgamebient_game::/${LIB}::/g" "$TEMPLATE/tests/display_shape.rs" > "$GAME/tests/display_shape.rs"
echo "rollout-aspect: pinned ${GW}x${GH} ($RATIO) in src/display.rs"

# 3. tests/display_contract.rs: this game's numbers, the pilots' five tests.
RATIO_TITLE="$TITLE" LIB="$LIB" GW="$GW" GH="$GH" POLICY="$POLICY" RATIO="$RATIO" UIW="$UIW" \
UI_A="$UI_A" UI_B="$UI_B" V_TV_ON="$V_TV_ON" V_TV_OFF="$V_TV_OFF" V_PORT="$V_PORT" \
V_DESK="$V_DESK" V_DESK_P="$V_DESK_P" \
perl -pe 's/@(\w+)@/exists $ENV{$1} ? $ENV{$1} : $&/ge' > "$GAME/tests/display_contract.rs" <<'EOF'
//! This game's pinned size, and the rectangles the pilot acceptance checks
//! read off the screen. Pure: no `App`, no window. The generic behaviour of
//! `letterbox` is tested in `src/display.rs`; these are this game's numbers.

use @LIB@::display::{GAME_HEIGHT, GAME_WIDTH, REFERENCE_SHORT_SIDE, letterbox, ui_scale_for};
use @LIB@::ui::fit::ui_width;
use bevy::math::UVec2;
use gamebient_input::CanvasPolicy;

const GAME: UVec2 = UVec2::new(GAME_WIDTH, GAME_HEIGHT);

fn boxed(window: (u32, u32), top: u32, gap: u32) -> ((u32, u32), (u32, u32)) {
    let v = letterbox(UVec2::new(window.0, window.1), GAME, top, gap);
    ((v.position.x, v.position.y), (v.size.x, v.size.y))
}

#[test]
fn pinned_size_is_@RATIO_TITLE@() {
    assert_eq!((GAME_WIDTH, GAME_HEIGHT), (@GW@, @GH@));
    assert_eq!(REFERENCE_SHORT_SIDE, 720.0);
    let policy = CanvasPolicy::Pinned {
        width: GAME_WIDTH,
        height: GAME_HEIGHT,
    };
    assert_eq!(policy, CanvasPolicy::@POLICY@);
    assert_eq!(policy.aspect_label().as_deref(), Some("@RATIO@"));
}

#[test]
fn ui_is_authored_@UIW@_wide() {
    assert_eq!(ui_scale_for(@UI_A@), 1.0);
    assert_eq!(ui_scale_for(@UI_B@), 1.5);
    assert_eq!(ui_width(), @UIW@.0);
}

#[test]
fn on_a_landscape_1080p_tv_inside_the_frame() {
    // No marquee on a landscape display; the frame's gap is 8 px.
    assert_eq!(boxed((1920, 1080), 0, 8), @V_TV_ON@);
    // GX_FRAME=off: no gap, plain bars.
    assert_eq!(boxed((1920, 1080), 0, 0), @V_TV_OFF@);
}

#[test]
fn on_a_portrait_1080p_tv_under_the_marquee() {
    // Marquee band 1080 / 3 = 360 rows; gap 8 px.
    assert_eq!(boxed((1080, 1920), 360, 8), @V_PORT@);
}

#[test]
fn on_the_desk_sized_stand_ins() {
    // 1280x720: gap (8 * 720 + 540) / 1080 = 5.
    assert_eq!(boxed((1280, 720), 0, 5), @V_DESK@);
    // 540x960: marquee 540 / 3 = 180 rows, gap (8 * 540 + 540) / 1080 = 4.
    assert_eq!(boxed((540, 960), 180, 4), @V_DESK_P@);
}
EOF
echo "rollout-aspect: wrote tests/display_contract.rs ($RATIO vectors)"

# 4. Recorder, store assets, frame-art tooling, skill snapshots.
"$TEMPLATE/tools/rollout-record.sh" "$GAME"
"$TEMPLATE/tools/rollout-store-assets.sh" "$GAME"
"$TEMPLATE/tools/rollout-frame-art.sh" "$GAME"
for skill in designing-cartridge-covers generating-cartridge-metadata; do
  mkdir -p "$GAME/.claude/skills"
  rm -rf "${GAME:?}/.claude/skills/$skill"
  cp -R "$TEMPLATE/.claude/skills/$skill" "$GAME/.claude/skills/$skill"
done

# 5. Cargo.toml: capture, features implying it, gamebient-input v0.4.0.
CARGO="$GAME/Cargo.toml"
BEFORE_CARGO="$(cat "$CARGO")"
BEFORE_TAG="$(grep -E '^gamebient-input' "$CARGO" || true)"
if ! grep -q '^capture = ' "$CARGO"; then
  perl -0pi -e 's/^((?:# [^\n]*\n)*)(autopilot|harness|showcase) = /# Marks a build that captures the window (footage, cover shots): the Linux\n# window stays the game'"'"'s own size and the cabinet frame is off unless asked\n# for. src\/display.rs and src\/frame\/mod.rs read only this feature, so a game\n# with another capture feature (`harness`, `showcase`) lists "capture" in it\n# instead of editing those files. Dev-only; never enabled in shipping builds.\ncapture = []\n$1$2 = /m' "$CARGO"
fi
grep -q '^capture = ' "$CARGO" || echo "HAND EDIT: Cargo.toml: add 'capture = []' under [features]"
for f in autopilot harness showcase; do
  perl -pi -e "s/^$f = \\[\\][ \\t]*\$/$f = [\"capture\"]/" "$CARGO"
  if grep -qE "^$f = " "$CARGO" && ! grep -E "^$f = " "$CARGO" | grep -q '"capture"\|"autopilot"'; then
    echo "HAND EDIT: Cargo.toml: $f must imply capture (\"capture\" in its list)"
  fi
done
perl -pi -e 's/(gamebient-input\b[^\n]*tag = ")v0\.[0-3]\.\d+(")/${1}v0.4.0$2/' "$CARGO"
grep -q 'gamebient-input.*tag = "v0.4.0"' "$CARGO" || echo 'HAND EDIT: Cargo.toml: gamebient-input must be pinned by tag = "v0.4.0"'
if [ "$BEFORE_CARGO" != "$(cat "$CARGO")" ]; then
  echo "rollout-aspect: Cargo.toml: capture = [], autopilot/harness imply capture, gamebient-input v0.4.0"
  if [ "$BEFORE_TAG" != "$(grep -E '^gamebient-input' "$CARGO" || true)" ]; then
    (cd "$GAME" && cargo update -p gamebient-input) \
      || echo "HAND EDIT: Cargo.lock: 'cargo update -p gamebient-input' failed in $GAME; run it"
  fi
fi

# 6. HIGHER_SCORE_IS_BETTER lives in the game's scoring module.
if [ -f "$GAME/$SCORING" ]; then
  HIGHER=true
  grep -qE '"score_order"[[:space:]]*:[[:space:]]*"asc"' "$GAME/assets/info.json" 2>/dev/null && HIGHER=false
  if ! grep -q 'HIGHER_SCORE_IS_BETTER' "$GAME/$SCORING"; then
    HIGHER="$HIGHER" perl -0pi -e '
      my $v = $ENV{HIGHER};
      my $doc = "/// Score order of this game, read by the cabinet frame (`src/frame/`). A game\n"
        . "/// whose score is better when lower (a time, a stroke count) sets this to\n"
        . "/// `false`, and must also set `score_order` in `assets/info.json` to match.\n"
        . "/// The frame then shows no score to beat, never announces a new high score\n"
        . "/// and never writes the best-score file. Lives here, not in `src/frame/`,\n"
        . "/// because frame files are copied into games unchanged and this file is the\n"
        . "/// game\x27s own.\n"
        . "pub const HIGHER_SCORE_IS_BETTER: bool = $v;\n\n";
      if (s/^((?:(?:\/\/\/|#\[)[^\n]*\n)*)(pub trait LeaderboardScore)/$doc$1$2/m) {}
      elsif (s/^(#\[cfg\(test\)\])/$doc$1/m) {}
      else { $_ .= "\n" . $doc; s/\n\n\z/\n/ }
    ' "$GAME/$SCORING"
    if grep -q 'HIGHER_SCORE_IS_BETTER' "$GAME/$SCORING"; then
      echo "rollout-aspect: inserted HIGHER_SCORE_IS_BETTER = $HIGHER in $SCORING"
    else
      echo "HAND EDIT: $SCORING: add pub const HIGHER_SCORE_IS_BETTER: bool = $HIGHER;"
    fi
  fi
  if grep -qE '"score_order"[[:space:]]*:[[:space:]]*"asc"' "$GAME/assets/info.json" 2>/dev/null; then
    echo "HAND EDIT: confirm HIGHER_SCORE_IS_BETTER = false (info.json has score_order asc)"
  else
    echo "HAND EDIT: confirm HIGHER_SCORE_IS_BETTER = true (info.json has no score_order)"
  fi
else
  echo "HAND EDIT: $SCORING: missing; the frame needs game::scoring::{GameData, LeaderboardScore, HIGHER_SCORE_IS_BETTER}"
fi

# 7. The Highlight relay in host.rs.
if [ -f "$GAME/$HOST" ]; then
  if ! grep -rqE 'add_message::<([A-Za-z_:]*::)?Highlight>' "$GAME/src" --exclude-dir=frame; then
    perl -0pi -e '
      s/^(use gamebient_input::\{[^\n]*\};\n)/$1\nuse crate::frame::Highlight;/m;
      s/^([ \t]*)(app\.init_resource::<Muted>\(\))\n/$1$2\n$1    \/\/ Registered here, in the half every build shares, so a sim\n$1    \/\/ system can write a highlight under the headless verifier.\n$1    .add_message::<Highlight>()\n/m;
      s/^([ \t]*)(report_paused\.run_if\(resource_changed::<Paused>\),\n)/$1$2$1report_highlight,\n/m;
      my $fn = "/// Relays the game\x27s highlight beats to web hosts. The host applies its own\n"
        . "/// rate limit; the native frame reads `Highlight` directly.\n"
        . "fn report_highlight(mut highlights: MessageReader<Highlight>, mut out: MessageWriter<HostEvent>) {\n"
        . "    for highlight in highlights.read() {\n"
        . "        out.write(HostEvent::Highlight {\n"
        . "            color: highlight.hex(),\n"
        . "        });\n"
        . "    }\n"
        . "}\n\n";
      s/^(#\[cfg\(test\)\])/$fn$1/m or $_ .= "\n" . substr($fn, 0, -1);
    ' "$GAME/$HOST"
    if grep -q 'add_message::<Highlight>' "$GAME/$HOST" && grep -q 'report_highlight,' "$GAME/$HOST" \
       && grep -q '^use crate::frame::Highlight;' "$GAME/$HOST"; then
      echo "rollout-aspect: registered and relayed frame::Highlight in $HOST"
    else
      echo "HAND EDIT: $HOST: register Highlight (.add_message::<Highlight>() in HostBridgePlugin), add report_highlight to Update and its relay fn (see the template's host.rs)"
    fi
  fi
else
  echo "HAND EDIT: $HOST: missing; relay frame::Highlight to HostEvent::Highlight as the template's host.rs does"
fi

# 8. A game does not carry the template's own tests or rollout scripts.
if [ "$IS_TEMPLATE" = true ]; then
  echo "note: $GAME carries tools/rollout-aspect.sh (a template copy); its own tests stay"
else
  for f in "$GAME"/tools/test_*.sh "$GAME"/tools/rollout-*.sh; do
    if [ -e "$f" ]; then
      rm -f "$f"
      echo "rollout-aspect: removed tools/$(basename "$f")"
    fi
  done
fi

# Format what the edits above touched, so CI's cargo fmt --check passes.
if command -v cargo >/dev/null 2>&1; then
  (cd "$GAME" && cargo fmt --all) || echo "HAND EDIT: cargo fmt failed in $GAME; run it before committing"
fi

# 9. Everything else is a hand edit; print, do not edit.
MAIN="$GAME/src/main.rs"
if [ -f "$MAIN" ]; then
  need=()
  grep -q 'display::game_window' "$MAIN" || need+=("window from display::game_window")
  grep -q 'DisplayPlugin' "$MAIN" || need+=("display::DisplayPlugin first in the plugin order")
  grep -q 'FramePlugin' "$MAIN" || need+=("frame::FramePlugin last (never in GamePlugin)")
  grep -qE 'REFERENCE_HEIGHT|update_ui_scale' "$MAIN" && need+=("delete REFERENCE_HEIGHT and update_ui_scale")
  if [ ${#need[@]} -gt 0 ]; then
    joined="$(printf '%s; ' "${need[@]}")"
    echo "HAND EDIT: src/main.rs: ${joined%; }"
  fi
else
  echo "HAND EDIT: src/main.rs: missing"
fi
LIBRS="$GAME/src/lib.rs"
if [ -f "$LIBRS" ]; then
  missing=()
  grep -qE '^pub mod display;' "$LIBRS" || missing+=("\`pub mod display;\`")
  grep -qE '^pub mod frame;' "$LIBRS" || missing+=("\`pub mod frame;\`")
  if [ "$FLAT" = true ]; then
    grep -qE '^pub mod fit;' "$LIBRS" || missing+=("\`pub mod fit;\` (for src/fit.rs)")
  elif ! grep -qE '^\s*pub mod fit;' "$GAME/src/ui/mod.rs" 2>/dev/null; then
    missing+=("\`pub mod fit;\` in src/ui/mod.rs")
  fi
  if [ ${#missing[@]} -gt 0 ]; then
    joined="$(printf '%s, ' "${missing[@]}")"
    echo "HAND EDIT: src/lib.rs: declare ${joined%, }"
  fi
fi
if [ "$FLAT" = true ]; then
  echo "HAND EDIT: src/game.rs: re-export the modules so the copied files resolve unchanged: pub use crate::{replay, scoring, sim, states};"
fi

# Every line the frame's camera_query_flagged test would flag. A grep port
# of the rule in src/frame/mod.rs; keep the two in step.
# shellcheck disable=SC2016
find "$GAME/src" -name '*.rs' -not -path "$GAME/src/frame/*" -not -path "$GAME/src/display.rs" -print0 \
  | sort -z \
  | xargs -0 perl -e '
    for my $file (@ARGV) {
      open(my $fh, "<", $file) or next;
      my @l = map { chomp; $_ } <$fh>;
      close $fh;
      for my $i (0 .. $#l) {
        my $line = $l[$i];
        next if $line =~ /^\s*\/\// || $line =~ /frame-camera-ok:/;
        my $end = $i + 2 > $#l ? $#l : $i + 2;
        my @near = @l[$i .. $end];
        my $direct = $line =~ /With<Camera2d>/ || $line =~ /With<Camera>/;
        my $by_ref = $line =~ /&(?:mut )?Camera(?=[,)>])/;
        my $other = 0;
        for my $n (@near) {
          while ($n =~ /With<(?!Camera>|Camera2d>)/g) { $other = 1 }
        }
        next unless $direct || ($by_ref && !$other);
        my $start = $i - 3 < 0 ? 0 : $i - 3;
        my $in_query = grep { /Query<|Single<|Populated</ } @l[$start .. $i];
        my $excluded = grep { /FrameCamera/ } @near;
        next unless $in_query && !$excluded;
        (my $t = $line) =~ s/^\s+//;
        print "HAND EDIT: $file:", $i + 1, ": camera query must name Without<crate::frame::FrameCamera> (or // frame-camera-ok: <reason>): $t\n";
      }
    }' \
  | sed "s#$GAME/##" || true

# frame/driver.rs reads the score through two test helpers.
if [ -f "$GAME/$SCORING" ]; then
  if ! perl -0ne 'exit(/pub struct GameData \{[^}]*\bpub score:\s*(?:u|i)(?:8|16|32|64|size)\b/s ? 0 : 1)' "$GAME/$SCORING"; then
    echo "HAND EDIT: src/frame/driver.rs: GameData has no \`pub score:\`; adapt data_with_score and add_score (tests only)"
  fi
  # The phase_for test names every state of the template.
  STATES="$GAME/$(dirname "$SCORING")/states.rs"
  if [ -f "$STATES" ]; then
    variants="$(perl -0ne 'if (/enum GameState \{(.*?)\n\}/s) { for (split /\n/, $1) { print "$1\n" if /^\s*([A-Z]\w*)\s*(?:,|\{|\(|=)/ } }' "$STATES")"
    for s in StudioLogo Menu HowToPlay Playing GameOver; do
      if ! printf '%s\n' "$variants" | grep -qx "$s"; then
        echo "HAND EDIT: src/frame/driver.rs: game has no GameState::$s; drop its phase_for assertion in every_state_maps_to_a_phase (tests only)"
      fi
    done
  else
    echo "HAND EDIT: src/frame/driver.rs: no $(basename "$STATES"); match the phase_for test to the game's GameState"
  fi
fi

# Hard-coded 16:9 left over in the game's own code.
leftovers="$(grep -rnE '1280|16\.0 / 9\.0|REFERENCE_HEIGHT' "$GAME/src" \
  --include='*.rs' --exclude-dir=frame --exclude=display.rs --exclude=fit.rs 2>/dev/null \
  | sed "s#$GAME/##" || true)"
if [ -n "$leftovers" ]; then
  total="$(printf '%s\n' "$leftovers" | wc -l | tr -d ' ')"
  printf '%s\n' "$leftovers" | head -20 | sed 's/^\([^:]*:[0-9]*\):[[:space:]]*/HAND EDIT: \1: 16:9 assumption? /'
  [ "$total" -le 20 ] || echo "HAND EDIT: ...and $((total - 20)) more 1280 / 16:9 / REFERENCE_HEIGHT lines under src/"
fi

if [ -z "$(git -C "$GAME" status --porcelain)" ]; then
  echo "rollout-aspect: nothing to do for $GAME (already converted)"
else
  echo "rollout-aspect: files in place for $GAME"
  echo "next: (cd $GAME && cargo clippy --all-targets --all-features -- -D warnings && cargo test --all-targets --all-features)"
fi
