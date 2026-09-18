#!/usr/bin/env bash
# Copies replay verification (deterministic sim, GXR1 replays, the wasm
# verifier: src/game/sim.rs, src/game/replay/, src/bin/verify.rs, build.rs,
# tools/build_verify.sh, tools/verify_fixture.mjs, docs/replay-verification.md,
# tests/archetype_order.rs, tests/windowed_shape.rs)
# from this template into a game checkout and wires it: Cargo.toml lib/bin
# split + features + deps, src/lib.rs + src/main.rs, the `pub mod
# replay`/`sim` + `GamePlugin { headless }` shape in src/game/mod.rs,
# build_web.sh's verify.zip step, the CI/release workflow steps, .gitignore,
# assets/info.json's verify_url, and src/game/host.rs's HostCommand::Seed arm
# + optional GlobalVolume. Idempotent. The script never edits a
# copied file's contents (the three exceptions — src/bin/verify.rs,
# tests/selftest.rs, tests/archetype_order.rs and tests/windowed_shape.rs —
# get only a mechanical
# `gamebient_game::` -> `<snake>::` crate-path substitution; see the comment
# at that copy step).
# game-specific behaviour (porting GamePlugin::build onto sim::SimSet,
# adding this game's own checksum_<game> system after sim::checksum_tick,
# writing the selftest script) is a HAND EDIT for the skill's
# port-checklist, not this script.
#
# --upgrade re-runs the rollout on a game that was ported months ago and has
# since fallen behind the template. Every copied feature file is then
# classified rather than skipped: a copy that is byte-identical to ANY
# committed template version of that file is a stale verbatim copy and is
# overwritten with the current one; a copy that matches no template version
# has been edited in the game and is left untouched with a HAND EDIT naming
# the base to merge from. `src/game/replay/selftest.rs` is narrower: it is
# refreshed while it is still byte-identical to a committed template version
# (nobody replaced the skeleton), and once it is the game's own script it is
# left alone with no HAND EDIT, since there is nothing to act on. --upgrade
# also prints a drift ADVISORY for .github/workflows/ci.yml and release.yml:
# the reconciliation above only knows a fixed set of shapes, so everything
# else the game's workflows have drifted into is diffed against the
# template's (with the naming substitutions applied) and summarised rather
# than rewritten.
#
# Usage: tools/rollout-replay.sh [--upgrade] <game-dir>
set -euo pipefail
TEMPLATE="$(cd "$(dirname "$0")/.." && pwd)"
UPGRADE=0
GAME_ARG=""
while [ $# -gt 0 ]; do
  case "$1" in
    --upgrade) UPGRADE=1 ;;
    -*)
      echo "unknown option: $1 (usage: $0 [--upgrade] <game-dir>)" >&2
      exit 2
      ;;
    *) GAME_ARG="$1" ;;
  esac
  shift
done
GAME="$(cd "${GAME_ARG:?usage: $0 [--upgrade] <game-dir>}" && pwd)"
PKG=$(grep -m1 '^name' "$GAME/Cargo.toml" | sed -E 's/.*"([^"]+)".*/\1/' || true)
SNAKE=${PKG//-/_}
if [ -z "$PKG" ]; then
  echo "HAND EDIT: $GAME/Cargo.toml: no 'name' under [package] (a workspace root?); skipping the Cargo.toml/lib.rs/main.rs wiring — point this script at the member crate's directory instead"
fi

# ---------------------------------------------------------------------------
# Pre-flight HAND EDITs. Informational only — never stop the script.
# ---------------------------------------------------------------------------
#
# "Already ported": the gameplay runs through sim::SimSet, so GamePlugin::build
# has been done. Two of the HAND EDITs below are printed unconditionally on a
# first rollout because the script cannot tell whether the work is done — but
# under --upgrade it can, and re-printing them on every upgrade of every game
# devalues a list whose whole point is that each line needs acting on.
PORTED=0
if [ -f "$GAME/src/game/mod.rs" ] && grep -q 'sim::SimSet' "$GAME/src/game/mod.rs"; then
  PORTED=1
fi
SUPPRESS_PORTED_NOISE=0
if [ "$UPGRADE" -eq 1 ] && [ "$PORTED" -eq 1 ]; then
  SUPPRESS_PORTED_NOISE=1
fi

if [ ! -f "$GAME/src/game/mod.rs" ]; then
  echo "HAND EDIT: src/game/mod.rs: flat layout (no src/game/); wire sim/replay and GamePlugin by hand, see irregular-games.md"
fi
if [ ! -d "$GAME/src/game/record" ]; then
  echo "HAND EDIT: src/game/record/ missing; run tools/rollout-record.sh first"
fi
# An existing sim.rs that isn't the template's current one is two very
# different situations with two opposite fixes, so tell them apart: a game
# whose own sim.rs happens to share the name (rename it), versus a copy of an
# older template sim.rs left behind by an earlier rollout (re-copy it). The
# latter is what a second rollout onto an already-ported game looks like, and
# telling it to rename its sim.rs would be actively wrong.
#
# Under --upgrade this is dead weight: re-copying the stale case is exactly
# what that mode does, and it says so itself (through the same
# refresh-or-hand-edit path as every other copied file), so skip the advice
# rather than print two messages about one file.
if [ "$UPGRADE" -eq 0 ] && [ -e "$GAME/src/game/sim.rs" ] && ! cmp -s "$TEMPLATE/src/game/sim.rs" "$GAME/src/game/sim.rs"; then
  STALE_AT=""
  if git -C "$TEMPLATE" rev-parse --git-dir >/dev/null 2>&1; then
    while read -r h; do
      [ -n "$h" ] || continue
      if git -C "$TEMPLATE" show "$h:src/game/sim.rs" 2>/dev/null | cmp -s - "$GAME/src/game/sim.rs"; then
        STALE_AT="$h"
        break
      fi
    done < <(git -C "$TEMPLATE" log --format=%H -- src/game/sim.rs 2>/dev/null)
  fi
  if [ -n "$STALE_AT" ]; then
    echo "HAND EDIT: src/game/sim.rs: stale template copy (byte-identical to template commit ${STALE_AT:0:7}); re-copy from the template and re-apply your game's checksum/end_run wiring"
  else
    echo "HAND EDIT: src/game/sim.rs: already exists and isn't the template's sim.rs; rename your sim.rs (e.g. shot_sim.rs) and re-run"
  fi
fi
# `LeaderboardScore` is NOT one of the files this script copies: the trait
# and its impl live in each game's own src/game/scoring.rs, while sim.rs,
# replay/mod.rs, replay/recorder.rs and host.rs (all copied or edited here)
# `use` it. So a game without one does not compile after the rollout — and
# the old check only spoke up when there was no `score` field at all, which
# is the minority case. Most of the fleet has `pub score: u32` and no impl,
# got no warning, and simply failed to build.
#
# The `pub score: u32` case is mechanical and identical in every game, so
# automate it with the template's own text rather than asking for it. Any
# other shape (a u64 score, a score on something that isn't GameData, no
# score at all) is a judgement call about polarity and units — golf strokes
# are lower-is-better, a race time needs inverting — so that stays a HAND
# EDIT. Guarded on the impl, so a second run is a no-op.
#
# The "is it already implemented" test spans all of src/ and tolerates a
# qualified path: `impl scoring::LeaderboardScore for GameData` and
# `impl crate::game::scoring::LeaderboardScore for GameData` are both real
# shapes, and an impl does not have to live in scoring.rs. A `grep -q 'impl
# LeaderboardScore'` on scoring.rs alone misses every one of those and
# appends a duplicate, which is E0119 — a worse failure than the one this
# automation exists to prevent.
if [ ! -f "$GAME/src/game/scoring.rs" ]; then
  echo "HAND EDIT: src/game/scoring.rs: missing; replay/mod.rs needs an impl of LeaderboardScore for your GameData"
elif grep -rqE 'impl\b[^{]*\bLeaderboardScore\b[^{]*\bfor\b' "$GAME/src" --include='*.rs'; then
  : # already implemented, anywhere in src/ and under any path spelling
elif grep -rq 'trait LeaderboardScore' "$GAME/src" --include='*.rs' \
  && ! grep -q 'trait LeaderboardScore' "$GAME/src/game/scoring.rs"; then
  # The trait exists but not in the file we would append to, so the impl
  # would need a `use` whose path we would have to guess. Ask instead.
  echo "HAND EDIT: src/game/scoring.rs: the LeaderboardScore trait is declared outside scoring.rs but never implemented for GameData; add 'impl LeaderboardScore for GameData { fn leaderboard_score(&self) -> u32 { .. } }' with the right 'use' by hand"
elif grep -qE '^\s*pub score: u32,' "$GAME/src/game/scoring.rs" \
  && grep -q 'struct GameData' "$GAME/src/game/scoring.rs"; then
  # Decided BEFORE the append opens the file for writing: reading and writing
  # the same file in one redirection group is a trap even when the order
  # happens to work out.
  NEED_TRAIT=0
  grep -q 'trait LeaderboardScore' "$GAME/src/game/scoring.rs" || NEED_TRAIT=1
  IMPL_BLOCK="$(mktemp)"
  {
    printf '\n'
    if [ "$NEED_TRAIT" -eq 1 ]; then
      cat <<'RUST'
/// The single higher-is-better integer the leaderboard ranks. The template's
/// `GameData` has `score`; games without one (golf, racing) compute it here.
pub trait LeaderboardScore {
    fn leaderboard_score(&self) -> u32;
}

RUST
    fi
    cat <<'RUST'
impl LeaderboardScore for GameData {
    fn leaderboard_score(&self) -> u32 {
        self.score
    }
}
RUST
  } >"$IMPL_BLOCK"
  # A file that ends with `#[cfg(test)] mod tests` must get the impl BEFORE
  # that module: appending after it trips clippy's `items_after_test_module`
  # under -D warnings (Attic Excavator's scoring.rs ends that way).
  if grep -q '^#\[cfg(test)\]' "$GAME/src/game/scoring.rs"; then
    IMPL_BLOCK="$IMPL_BLOCK" perl -0pi -e 'BEGIN { local $/; open my $f, "<", $ENV{IMPL_BLOCK} or die; $b = <$f>; close $f; $b =~ s/^\n//; $b .= "\n" } s/^#\[cfg\(test\)\]/$b#[cfg(test)]/m' "$GAME/src/game/scoring.rs"
  else
    cat "$IMPL_BLOCK" >>"$GAME/src/game/scoring.rs"
  fi
  rm -f "$IMPL_BLOCK"
  echo "rollout-replay: src/game/scoring.rs: added 'impl LeaderboardScore for GameData' over the existing 'pub score: u32' field"
else
  echo "HAND EDIT: src/game/scoring.rs: no 'pub score: u32' field on GameData to implement LeaderboardScore from; write 'impl LeaderboardScore for GameData { fn leaderboard_score(&self) -> u32 { .. } }' by hand (higher is better — invert a time, convert strokes to points), or the copied sim/replay/recorder/host code will not compile (it all uses the trait)"
fi
# The fixed timestep must be pinned to `sim::tick_duration()`. Bevy's default
# `Time<Fixed>` is 64 Hz, `sim::tick_duration()` is 60, and `Replay::decode`
# rejects a header whose tick rate is not the sim's with `BadTickRate` — so
# every recording a game makes is unverifiable and the failure surfaces as a
# decode error in the verifier rather than anywhere near the missing line.
# Two-for-two on wave 2 (Grand Theft Auto-Reply, Pack The Ripper), and both
# times as a hand edit found by debugging. The template puts the insert in
# `GamePlugin::build`; some games put it in `main.rs`, which is equally fine,
# so both are searched.
if [ -f "$GAME/src/game/mod.rs" ] || [ -f "$GAME/src/main.rs" ]; then
  if ! grep -qs 'Time::<Fixed>' "$GAME/src/game/mod.rs" "$GAME/src/main.rs"; then
    echo "HAND EDIT: src/game/mod.rs: pin the fixed timestep — .insert_resource(Time::<Fixed>::from_duration(sim::tick_duration())) in GamePlugin::build (or in main.rs). Bevy's default is 64 Hz, the sim is 60, and Replay::decode rejects any other rate as BadTickRate, so without this line every run this game records fails to decode."
  fi
fi

# Comment lines are stripped before the check: the template's own `input.rs`
# mentions `ButtonInput<KeyCode>` in a doc comment explaining what NOT to do,
# and firing on prose teaches people to skim the HAND EDIT list.
if grep -rn "ButtonInput<KeyCode>" "$GAME/src/game" --include="*.rs" 2>/dev/null \
  | grep -v autopilot \
  | sed -E 's/^[^:]*:[0-9]+://' \
  | grep -vE '^[[:space:]]*(//|\*|/\*)' \
  | grep -q .; then
  echo "HAND EDIT: src/game: gameplay reads raw ButtonInput<KeyCode>; route through TickInput instead"
fi
# `drive_autopilot.before(collect_input)` was unambiguous while gameplay read
# the per-frame `GameInput`. It is not once the sim reads `TickInput`:
# gamebient-input registers `accumulate_input.before(collect_input)`, so
# "before collect_input" leaves the bot and the accumulator unordered relative
# to each other, and Bevy's schedule builder picks. A tap written after the
# accumulator has run is folded into `GameInput` for that frame and never
# reaches a fixed tick — so the bot appears to press buttons the replay never
# records. Order it before the accumulator instead.
#
# Searched across ALL of src/, not pinned to the template's own
# `src/game/autopilot.rs`: Pack The Ripper keeps its bot at
# `src/autopilot.rs`, where the path-pinned version of this rewrite did
# nothing and said nothing, and the port shipped with `.before(collect_input)`
# still in place.
while IFS= read -r ap; do
  [ -n "$ap" ] || continue
  ap_rel="${ap#"$GAME/"}"
  if [ "$SUPPRESS_PORTED_NOISE" -eq 0 ] && grep -qE 'data\.[A-Za-z_][A-Za-z0-9_]* = ' "$ap"; then
    echo "HAND EDIT: $ap_rel: writes GameData fields directly (dev-only; keep that path out of the selftest script)"
  fi
  if grep -q 'before(gamebient_input::input::collect_input)' "$ap"; then
    perl -pi -e 's/before\(gamebient_input::input::collect_input\)/before(gamebient_input::input::accumulate_input)/g' "$ap"
    echo "rollout-replay: $ap_rel: reordered drive_autopilot before accumulate_input (collect_input is too late once the sim reads TickInput)"
  fi
done < <(find "$GAME/src" -name 'autopilot.rs' 2>/dev/null | sort)

# The same defect, one level out: anything that WRITES `VirtualInput` and is
# still ordered only against `collect_input` after the rewrite above. A
# screenshot harness, a demo attractor, a second bot — the file does not have
# to be called autopilot.rs, and the symptom is the same (presses the player
# can see and the replay never recorded). Never rewritten automatically,
# because outside an autopilot this script cannot know the system is meant to
# feed the tick path at all.
LATE_VIRTUAL=""
while IFS= read -r f; do
  [ -n "$f" ] || continue
  grep -q 'VirtualInput' "$f" || continue
  grep -q 'before(gamebient_input::input::collect_input)' "$f" || continue
  LATE_VIRTUAL="${LATE_VIRTUAL}${f#"$GAME/"} "
done < <(find "$GAME/src" -name '*.rs' 2>/dev/null | sort)
if [ -n "$LATE_VIRTUAL" ]; then
  echo "HAND EDIT: ${LATE_VIRTUAL% }: writes VirtualInput but is still ordered .before(gamebient_input::input::collect_input); order it .before(gamebient_input::input::accumulate_input) instead. gamebient-input registers accumulate_input.before(collect_input), so ordering against collect_input alone leaves this system and the accumulator unordered: a press written after the accumulator ran is folded into GameInput for that frame and never reaches a fixed tick, so it never reaches the replay."
fi

# Advisory only, and deliberately a heuristic. A presentation system left in
# `Update` that INSERTS a component onto an entity a SimSet system queries
# changes that entity's archetype, and archetype order is the order Bevy
# iterates a `Query` in. The verifier adds no render plugins, so it never
# performs the insert and iterates a different order — a desync that every
# fixture passes (nothing renders during a selftest) and only replays of
# real, rendered runs expose. Grand Theft Auto-Reply hit exactly this:
# `src/assets/projectiles.rs` and `inbox_view.rs` decorate the live
# `Projectile` and `Email` entities that `combat::advance_projectiles` and
# `inbox::tick_emails` iterate.
#
# Scans ALL of src/, `src/game/` included: Dough.io keeps its decorators in
# `src/game/presentation.rs`, so excluding the sim's own directory would
# print nothing for it. That means benign shapes are named too — a system
# that spawns an entity and decorates it in the same breath (this template's
# own `player::spawn_player`), or a decorator already inside `SimSet`. Hence
# "advisory": it says which files to read, never which are wrong. The test
# that answers the question is `tests/archetype_order.rs`.
# Emits one `<path>:<Component>` line per (file, component) pair where that
# file pulls an entity out of a query and inserts/removes/despawns on it,
# naming a component declared and queried under $1/src/game. Two filters do
# the discriminating: the component must be one the GAME declares (so
# `Transform`, `Sprite` and friends, which are everywhere, never appear), and
# the file must not itself spawn it ("decorate the entity I just spawned" is
# benign — both paths do it, so no archetype diverges; decorating an entity
# SOMEONE ELSE spawned is the shape that bites).
archetype_insert_pairs() {
  local root="$1" comps f comp
  [ -d "$root/src/game" ] || return 0
  comps="$(grep -rhE -A4 '#\[derive\([^)]*\bComponent\b' "$root/src/game" --include='*.rs' 2>/dev/null \
    | grep -oE '^(pub )?(struct|enum) [A-Z][A-Za-z0-9_]*' \
    | awk '{print $NF}' | sort -u)"
  while read -r f; do
    [ -n "$f" ] || continue
    grep -q '\.entity(' "$f" || continue
    grep -qE '\.insert\(|\.remove::<|\.despawn\(' "$f" || continue
    for comp in $comps; do
      grep -qE "(Query<[^>]*&(mut )?${comp}\b|With<${comp}>|Without<${comp}>|&(mut )?${comp}\b)" "$f" || continue
      grep -rqE "(Query<[^>]*&(mut )?${comp}\b|With<${comp}>|Without<${comp}>)" "$root/src/game" --include='*.rs' 2>/dev/null || continue
      grep -qE "spawn\(\(?[^)]*\b${comp}\b" "$f" && continue
      printf '%s:%s\n' "${f#"$root/"}" "$comp"
    done
  done < <(find "$root/src" -name '*.rs' 2>/dev/null)
}

# Advisory only, and deliberately a heuristic. A presentation system left in
# `Update` that INSERTS a component onto an entity a SimSet system queries
# changes that entity's archetype, and archetype order is the order Bevy
# iterates a `Query` in. The verifier adds no render plugins, so it never
# performs the insert and iterates a different order — a desync that every
# fixture passes (nothing renders during a selftest) and only replays of
# real, rendered runs expose. Grand Theft Auto-Reply hit exactly this:
# `src/assets/projectiles.rs` and `inbox_view.rs` decorate the live
# `Projectile` and `Email` entities that `combat::advance_projectiles` and
# `inbox::tick_emails` iterate.
#
# Scans ALL of src/, `src/game/` included: Dough.io keeps its decorators in
# `src/game/presentation.rs`, so excluding the sim's own directory would find
# nothing for it. What IS subtracted is whatever the same scan finds in this
# template — every game inherits the same `cleanup_game_entities`,
# pause-overlay and audio-fade boilerplate, and flagging it in every game for
# ever would bury the game-specific hits that matter. (Run against the
# template itself the two sets are equal, so nothing prints.)
if [ -d "$GAME/src/game" ]; then
  TEMPLATE_PAIRS="$(archetype_insert_pairs "$TEMPLATE" | sort -u)"
  GAME_PAIRS="$(archetype_insert_pairs "$GAME" | sort -u)"
  ADVISORY="$(comm -13 <(printf '%s\n' "$TEMPLATE_PAIRS") <(printf '%s\n' "$GAME_PAIRS") | tr '\n' ' ')"
  ADVISORY="${ADVISORY% }"
  if [ -n "$ADVISORY" ]; then
    echo "HAND EDIT (advisory): these files change the archetype of entities src/game/ also queries — ${ADVISORY}. Where the entity is one a SimSet system ITERATES and the insert happens from Update, it reorders that query, and the verifier (no render plugins, so no insert) never sees the same order. Put the presentation on a CHILD entity, move the insert into the sim chain, or sort the sim query by a stable per-entity key. This is a grep: benign hits are expected (a decorator already inside SimSet is fine), so read the files, then extend tests/archetype_order.rs's markers to your real decorators and let that test answer it. See docs/replay-verification.md rule 1."
  fi
fi

# Advisory, and the resource-level sibling of the archetype scan above: a sim
# system may not READ anything `UiPlugin` owns. `ScreenFade` is the one that
# has actually bitten — Sundae Shooter's `fire_scoop`/`swap_queue` refused to
# act while the fade was busy, which is three bugs at once: the fade lives in
# `UiPlugin`, which the verifier never builds, so it blocks nothing there; it
# is ticked from `Update` on the FRAME delta, so how many sim ticks it covers
# depends on the frame rate; and it is busy for the first ~24 ticks of every
# run, because entering `Playing` goes through it. That game's browser-
# recorded run came back claiming 195 points against 200 re-simulated.
#
# `Option<Res<ScreenFade>>` is a compile fix, not a determinism fix: the run
# still forks on whether the resource was there. `sim::end_run`'s
# `Option<ResMut<ScreenFade>>` is the one sanctioned touch (it writes a fade
# request on the tick `sim::RunOver` has already frozen the set), and rule 8's
# `toggle_pause` is the other. Both are excluded by name below, as is
# `src/game/autopilot.rs`, a dev harness the game registers in `Update`.
#
# Reports `<file>:<fn>` pairs, and subtracts whatever the same scan finds in
# this template so a game is never told about boilerplate it inherited. The
# test that ANSWERS this question is `tests/windowed_shape.rs`.
sim_fade_reads() {
  local root="$1" f
  [ -d "$root/src/game" ] || return 0
  while read -r f; do
    [ -n "$f" ] || continue
    case "$f" in */autopilot.rs) continue ;; esac
    awk -v rel="${f#"$root/"}" '
      /^[[:space:]]*#\[cfg\(test\)\]/ { intest = 1 }
      intest { next }
      /^[[:space:]]*(\/\/|\*|\/\*)/ { next }
      # Track the enclosing item so a fade in a `RunEnd`-style SystemParam
      # bundle is attributed to the struct rather than to whatever function
      # happened to be above it.
      /^[[:space:]]*(pub(\([^)]*\))? )?(async )?fn [A-Za-z0-9_]+/ {
        if (match($0, /fn [A-Za-z0-9_]+/)) item = substr($0, RSTART + 3, RLENGTH - 3)
      }
      /^[[:space:]]*(pub(\([^)]*\))? )?(struct|enum) [A-Za-z0-9_]+/ {
        if (match($0, /(struct|enum) [A-Za-z0-9_]+/)) {
          t = substr($0, RSTART, RLENGTH); sub(/^(struct|enum)[ ]+/, "", t); item = t
        }
      }
      # The sanctioned shapes, recognised by what the item DOES rather than
      # by name alone: rule 10 hands the fade to sim::end_run (directly or
      # through a RunEnd SystemParam), which only writes a request on the
      # tick sim::RunOver has already frozen SimSet.
      /end_run\(|RunEnd/ { if (item != "") ok[item] = 1 }
      /Res(Mut)?<[^>]*Fade>/ {
        if (item != "" && item != "end_run" && item != "toggle_pause") cand[item] = 1
      }
      END { for (i in cand) if (!(i in ok)) print rel ":" i }
    ' "$f"
  done < <(find "$root/src/game" -name '*.rs' 2>/dev/null | sort)
}
if [ -d "$GAME/src/game" ]; then
  FADE_ADVISORY="$(comm -13 \
    <(sim_fade_reads "$TEMPLATE" | sort -u) \
    <(sim_fade_reads "$GAME" | sort -u) | tr '\n' ' ')"
  FADE_ADVISORY="${FADE_ADVISORY% }"
  if [ -n "$FADE_ADVISORY" ]; then
    echo "HAND EDIT (advisory): these read a fade resource UiPlugin owns — ${FADE_ADVISORY}. If any of them runs inside sim::SimSet, delete the read: ScreenFade is absent in the verifier, ticks on the frame delta, and is busy for the first ~24 ticks of every run, so the two builds disagree about what the player was allowed to do. sim::end_run's Option<ResMut<ScreenFade>> and rule 8's toggle_pause are the only sanctioned touches (both excluded here). Presentation systems in Update may read it freely — this is a grep, so read the registrations. tests/windowed_shape.rs is the test that answers it."
  fi
fi

# ---------------------------------------------------------------------------
# Copy the feature files verbatim. A file that already exists and isn't
# byte-identical to the template's is left alone (HAND EDIT), never
# overwritten.
# ---------------------------------------------------------------------------
mkdir -p "$GAME/src/game/replay" "$GAME/src/bin" "$GAME/tests/fixtures" "$GAME/tools"

# Files --upgrade actually rewrote, for the summary at the end. A plain
# newline-joined string rather than an array: `"${a[@]}"` on an empty array
# is an unbound-variable error under `set -u` in older bashes.
REFRESHED=""
# Set when --upgrade found no tests/archetype_order.rs and deliberately did
# not create one (see the probe's block below); reported in the summary.
MISSING_PROBE=0
# Set when tests/windowed_shape.rs was written into a game that did not have
# it; reported in the summary, because it is a new test that can legitimately
# go red on an already-green checkout (that being the point of it).
ADDED_SHAPE=0
SHAPE_WHY=""

# The newest template commit whose version of $1 is byte-identical to the
# file $2 — with the `gamebient_game::` -> `$SNAKE::` substitution applied
# first when $3 is "rename", so the two crate-renamed copies compare on equal
# terms. Prints nothing (and still succeeds) when no committed version
# matches, or when the template isn't a git checkout.
template_sha_matching() {
  local rel="$1" file="$2" mode="${3:-verbatim}" h
  git -C "$TEMPLATE" rev-parse --git-dir >/dev/null 2>&1 || return 0
  while read -r h; do
    [ -n "$h" ] || continue
    if [ "$mode" = rename ]; then
      if git -C "$TEMPLATE" show "$h:$rel" 2>/dev/null \
        | sed "s/gamebient_game::/${SNAKE}::/g" | cmp -s - "$file"; then
        printf '%s\n' "$h"
        return 0
      fi
    elif git -C "$TEMPLATE" show "$h:$rel" 2>/dev/null | cmp -s - "$file"; then
      printf '%s\n' "$h"
      return 0
    fi
  done < <(git -C "$TEMPLATE" log --format=%H -- "$rel" 2>/dev/null)
  return 0
}

# The newest template commit whose version of $1 matches SOME version of $1
# in the GAME's own history — i.e. the last point at which this game's copy
# was still a verbatim template copy. That is the base a locally modified
# file should be merged from: diffing it against the template's HEAD is
# exactly the set of template changes the game hasn't got yet.
game_history_base_sha() {
  local rel="$1" mode="${2:-verbatim}" gh tmp sha
  git -C "$GAME" rev-parse --git-dir >/dev/null 2>&1 || return 0
  tmp="$(mktemp)"
  while read -r gh; do
    [ -n "$gh" ] || continue
    git -C "$GAME" show "$gh:$rel" >"$tmp" 2>/dev/null || continue
    sha="$(template_sha_matching "$rel" "$tmp" "$mode")"
    if [ -n "$sha" ]; then
      rm -f "$tmp"
      printf '%s\n' "$sha"
      return 0
    fi
  done < <(git -C "$GAME" log --format=%H -- "$rel" 2>/dev/null)
  rm -f "$tmp"
  return 0
}

# --upgrade's one decision, shared by the verbatim and the crate-renamed
# copies. Reached only for a file that exists and differs from the
# template's current version.
refresh_or_hand_edit() {
  local rel="$1" src="$2" dst="$3" mode="$4" stale base
  stale="$(template_sha_matching "$rel" "$dst" "$mode")"
  if [ -n "$stale" ]; then
    # Byte-identical to a committed template version: nobody edited it in
    # the game, it was just left behind by an older rollout.
    if [ "$mode" = rename ]; then
      sed "s/gamebient_game::/${SNAKE}::/g" "$src" >"$dst"
    else
      cp "$src" "$dst"
    fi
    REFRESHED="${REFRESHED}${rel} (was template ${stale:0:7})"$'\n'
    return 0
  fi
  base="$(game_history_base_sha "$rel" "$mode")"
  if [ -n "$base" ]; then
    echo "HAND EDIT: $rel: locally modified; merge template changes by hand (git -C $TEMPLATE diff $base:$rel HEAD:$rel shows what changed)"
  else
    echo "HAND EDIT: $rel: locally modified; merge template changes by hand (no committed template version matches this game's history of the file, so diff it against $TEMPLATE/$rel yourself)"
  fi
}

copy_or_hand_edit() {
  local rel="$1" src dst
  src="$TEMPLATE/$rel"
  dst="$GAME/$rel"
  if [ -e "$dst" ]; then
    cmp -s "$src" "$dst" && return 0
    if [ "$UPGRADE" -eq 1 ]; then
      refresh_or_hand_edit "$rel" "$src" "$dst" verbatim
    else
      echo "HAND EDIT: $rel: exists and differs from the template's version; move it aside and re-run to pick up the template copy (or re-run with --upgrade)"
    fi
  else
    mkdir -p "$(dirname "$dst")"
    cp "$src" "$dst"
  fi
}

COPY_LIST=(
  src/game/replay/mod.rs
  src/game/replay/recorder.rs
  src/game/replay/feeder.rs
  build.rs
  tools/build_verify.sh
  tools/verify_fixture.mjs
  docs/replay-verification.md
)
if [ "$UPGRADE" -eq 1 ]; then
  # sim.rs joins the classified copies: on an already-ported game an
  # out-of-date sim.rs is the whole point of --upgrade. (On a first rollout
  # it keeps its own pre-flight message and the copy-if-absent below, which
  # must not overwrite a game's own unrelated sim.rs.)
  COPY_LIST=(src/game/sim.rs "${COPY_LIST[@]}")
  # src/game/replay/selftest.rs is NOT in the list: after the port it is the
  # game's own script, so the "locally modified" HAND EDIT would fire on
  # every upgrade of every game and mean nothing. It is handled by its own
  # narrowed rule below instead of being exempted outright.
else
  # sim.rs already has its own pre-flight message (rename-and-re-run) above;
  # only perform the copy here, and only when nothing is in the way.
  [ -e "$GAME/src/game/sim.rs" ] || cp "$TEMPLATE/src/game/sim.rs" "$GAME/src/game/sim.rs"
  COPY_LIST+=(src/game/replay/selftest.rs)
fi

for rel in "${COPY_LIST[@]}"; do
  copy_or_hand_edit "$rel"
done

# The two files the port REPLACES rather than keeps — replay/selftest.rs's
# script and tests/archetype_order.rs's markers are this game's, not the
# template's — get a narrower --upgrade rule than "never touch them". A copy
# that is byte-identical to SOME committed template version is one nobody
# ever replaced (the port skipped it, or the template moved the skeleton
# underneath it), and refreshing that throws nothing away: do it, and name it
# in the summary like any other stale verbatim copy. A copy that matches no
# template version is the game's own: leave it, and say nothing, because
# there is nothing for a human to act on.
if [ "$UPGRADE" -eq 1 ] && [ -n "$PKG" ]; then
  refresh_if_untouched() {
    local rel="$1" mode="$2" dst="$GAME/$1" expected stale
    if [ "$mode" = rename ]; then
      expected="$(sed "s/gamebient_game::/${SNAKE}::/g" "$TEMPLATE/$rel")"
    else
      expected="$(cat "$TEMPLATE/$rel")"
    fi
    if [ ! -e "$dst" ]; then
      mkdir -p "$(dirname "$dst")"
      printf '%s\n' "$expected" >"$dst"
      return 0
    fi
    [ "$(cat "$dst")" = "$expected" ] && return 0
    stale="$(template_sha_matching "$rel" "$dst" "$mode")"
    if [ -n "$stale" ]; then
      printf '%s\n' "$expected" >"$dst"
      REFRESHED="${REFRESHED}${rel} (was template ${stale:0:7})"$'\n'
    fi
  }
  refresh_if_untouched src/game/replay/selftest.rs verbatim
fi

chmod +x "$GAME/tools/build_verify.sh" 2>/dev/null || true

# src/bin/verify.rs and tests/selftest.rs hardcode `use gamebient_game::...`
# (the template's own crate name). Cargo has no self-dependency aliasing
# that would let `gamebient_game::` resolve to this game's lib crate
# (verified empirically: a `path = "."` self-dependency is a cyclic-package
# error under [dependencies], and dev-dependencies aren't linked into plain
# `cargo build`/`cargo check` bin targets either) — so a byte-identical copy
# cannot compile once [lib].name is this game's own <snake>. Copy these two
# files with that one mechanical substitution; nothing else about their
# content changes. On the template itself SNAKE == "gamebient_game", so the
# substitution is a no-op and the file stays byte-identical (still a no-op
# rerun on the template's own checkout).
copy_with_crate_rename() {
  local rel="$1" src dst expected
  src="$TEMPLATE/$rel"
  dst="$GAME/$rel"
  expected="$(sed "s/gamebient_game::/${SNAKE}::/g" "$src")"
  if [ -e "$dst" ]; then
    [ "$(cat "$dst")" = "$expected" ] && return 0
    if [ "$UPGRADE" -eq 1 ]; then
      refresh_or_hand_edit "$rel" "$src" "$dst" rename
    else
      echo "HAND EDIT: $rel: exists and differs from the template's version (with gamebient_game:: -> ${SNAKE}::); move it aside and re-run (or re-run with --upgrade)"
    fi
  else
    mkdir -p "$(dirname "$dst")"
    printf '%s\n' "$expected" >"$dst"
  fi
}
# SNAKE is empty when PKG is (see the HAND EDIT above); an empty-prefix
# substitution would produce invalid Rust ("use ::game::replay::..."), so
# skip these too rather than write garbage.
if [ -n "$PKG" ]; then
  copy_with_crate_rename src/bin/verify.rs
  copy_with_crate_rename tests/selftest.rs

  # ---- tests/archetype_order.rs -------------------------------------------
  #
  # The differential probe for the archetype-order trap. It has its own rule
  # rather than going through copy_with_crate_rename, for one reason: the
  # template's version hard-references the template's own sim entity
  # (`game::player::Player`), which only a handful of games have. A copy of it
  # does not compile anywhere else, so every path has to be deliberate about
  # whether it is putting a red test into a game.
  #
  #   absent, first rollout  -> write it, and say it must be adapted. The port
  #                             is happening now; a red test with a named hand
  #                             edit beats no test at all.
  #   absent, --upgrade      -> do NOT write it. The game compiles today, and
  #                             materialising a file that references a `Player`
  #                             it does not have would turn a green checkout
  #                             red with nothing in the output to explain it.
  #                             Ask for it instead, and name it in the summary.
  #   present, byte-identical to the template's (after the crate rename)
  #                          -> unadapted, whenever it got there. Say so.
  #   present and different  -> adapted. Silent, always: that is the finished
  #                             state, and it is never "exists and differs;
  #                             move it aside and re-run".
  #   present, matching an OLDER template version, --upgrade
  #                          -> a stale unadapted copy: refresh it to current
  #                             (and the identity check below then asks for it
  #                             to be adapted, which it still needs).
  PROBE_REL=tests/archetype_order.rs
  PROBE_DST="$GAME/$PROBE_REL"
  PROBE_EXPECTED="$(sed "s/gamebient_game::/${SNAKE}::/g" "$TEMPLATE/$PROBE_REL")"
  if [ ! -e "$PROBE_DST" ]; then
    if [ "$UPGRADE" -eq 1 ]; then
      MISSING_PROBE=1
      echo "HAND EDIT: $PROBE_REL: missing; copy it from the template and adapt the markers to your sim entities, keeping BOTH Decoration modes (faithful documents the real partitioning; split is the detector — see the port checklist, \"The two order traps\"). Not copied automatically: the template's version references its own \`Player\` entity, so dropping it in would turn this checkout's tests red with no explanation."
    else
      mkdir -p "$(dirname "$PROBE_DST")"
      printf '%s\n' "$PROBE_EXPECTED" >"$PROBE_DST"
    fi
  elif [ "$UPGRADE" -eq 1 ] && [ "$(cat "$PROBE_DST")" != "$PROBE_EXPECTED" ]; then
    PROBE_STALE="$(template_sha_matching "$PROBE_REL" "$PROBE_DST" rename)"
    if [ -n "$PROBE_STALE" ]; then
      printf '%s\n' "$PROBE_EXPECTED" >"$PROBE_DST"
      REFRESHED="${REFRESHED}${PROBE_REL} (was template ${PROBE_STALE:0:7})"$'\n'
    fi
  fi
  # Byte-identity with the template's current version is the ONLY test for
  # "unadapted" — not whether the game has a `Player`, which says nothing
  # about whether the markers were ever replaced. On the template's own
  # checkout ($SNAKE == gamebient_game) the file is its own, by definition.
  if [ "$SNAKE" != gamebient_game ] && [ -e "$PROBE_DST" ] \
    && [ "$(cat "$PROBE_DST")" = "$PROBE_EXPECTED" ]; then
    echo "HAND EDIT: $PROBE_REL: still the template's copy — adapt it before it compiles. It references the template's own \`Player\` sim entity; replace the marker components and decorate_* systems with stand-ins for THIS game's Update decorators, and point trace_order at the queries your order-sensitive sim systems iterate. KEEP BOTH Decoration modes: 'faithful' must reproduce your decorators' exact branching (different entities getting different component sets is what splits the archetype) and documents the real partitioning, while 'split' — which halves every sim archetype on sim::SpawnOrder parity, so point it at EVERY kind of sim entity you have — is the detector. Measured on Dough.io against a real instance of the bug: faithful passed 18/18 by luck, split failed 2/18. It is the only test that catches the archetype-order trap; the \"delete the system and re-run --selftest\" check provably cannot. See the file's doc comment and the port checklist, 'What may stay in Update'."
  fi

  # ---- tests/windowed_shape.rs --------------------------------------------
  #
  # The resource-level sibling of the probe above: record the selftest script
  # in an app carrying `ScreenFade` and the windowed build's `Update`
  # systems, verify it in a bare one. Sundae Shooter's port found a real
  # instance (`fire_scoop`/`swap_queue` gated on the fade; a browser-recorded
  # run claimed 195 against 200 re-simulated), and nothing else in the kit
  # can see that class of bug: the fixtures and `--selftest` all run in bare
  # apps with no fade, and `archetype_order` adds components, not resources.
  #
  # Unlike `archetype_order.rs` this file is copyable as-is — everything it
  # names is template-owned and present in every ported game — so it is
  # written whenever it is absent, on --upgrade as well, and reported in the
  # summary. The two things it needs that a long-ported game may not have
  # are checked first, and asked for rather than guessed at:
  #
  #   * `selftest.rs` must export its `script` (the template's does; a port
  #     that replaced the body may have dropped the `pub`);
  #   * `ui::transition::ScreenFade` must exist and have `boot()`.
  SHAPE_REL=tests/windowed_shape.rs
  SHAPE_DST="$GAME/$SHAPE_REL"
  SHAPE_EXPECTED="$(sed "s/gamebient_game::/${SNAKE}::/g" "$TEMPLATE/$SHAPE_REL")"
  SHAPE_OK=1
  if ! grep -qs 'pub fn script' "$GAME/src/game/replay/selftest.rs"; then
    SHAPE_OK=0
    SHAPE_WHY="src/game/replay/selftest.rs does not export 'pub fn script'"
  elif ! grep -qs 'pub fn boot()' "$GAME/src/ui/transition.rs"; then
    SHAPE_OK=0
    SHAPE_WHY="src/ui/transition.rs has no 'pub fn boot()' on ScreenFade"
  fi
  if [ ! -e "$SHAPE_DST" ]; then
    if [ "$SHAPE_OK" -eq 1 ]; then
      mkdir -p "$(dirname "$SHAPE_DST")"
      printf '%s\n' "$SHAPE_EXPECTED" >"$SHAPE_DST"
      ADDED_SHAPE=1
    else
      echo "HAND EDIT: $SHAPE_REL: not created — $SHAPE_WHY, so the copy would not compile. Fix that (make the selftest script pub; keep ScreenFade::boot()), then copy it from $TEMPLATE/$SHAPE_REL with gamebient_game:: -> ${SNAKE}::. It is the only test that catches a sim system reading presentation state."
    fi
  elif [ "$UPGRADE" -eq 1 ] && [ "$(cat "$SHAPE_DST")" != "$SHAPE_EXPECTED" ]; then
    SHAPE_STALE="$(template_sha_matching "$SHAPE_REL" "$SHAPE_DST" rename)"
    if [ -n "$SHAPE_STALE" ]; then
      printf '%s\n' "$SHAPE_EXPECTED" >"$SHAPE_DST"
      REFRESHED="${REFRESHED}${SHAPE_REL} (was template ${SHAPE_STALE:0:7})"$'\n'
    fi
  fi
  # Unadapted is a usable state here (the shipped file already covers
  # ScreenFade, the trap that has actually bitten), so this is a nudge rather
  # than the probe's "adapt it before it compiles" — and it is suppressed on
  # an upgrade of an already-ported game, where repeating it forever would
  # just crowd out the lines that need acting on.
  if [ "$SUPPRESS_PORTED_NOISE" -eq 0 ] && [ "$SNAKE" != gamebient_game ] \
    && [ -e "$SHAPE_DST" ] && [ "$(cat "$SHAPE_DST")" = "$SHAPE_EXPECTED" ]; then
    echo "HAND EDIT (advisory): $SHAPE_REL: still the template's copy. As shipped it covers ScreenFade only; add this game's own !headless shape to record_windowed — every Update system GamePlugin::build registers behind !self.headless, and any UiPlugin/AssetsPlugin resource a sim system might read. Option<Res<T>> in a sim system is a compile fix, not a determinism fix."
  fi
fi

# ---------------------------------------------------------------------------
# Wiring edits: Cargo.toml, src/lib.rs + src/main.rs, src/game/mod.rs,
# build_web.sh, CI/release workflows, .gitignore, assets/info.json. One perl
# pass so every anchor/guard lives in one place; each edit is guarded so a
# second run is a no-op, and every miss prints a HAND EDIT instead of
# guessing. Skipped entirely when PKG is empty (HAND EDIT already printed
# above) — every one of these edits keys off the package name in some way.
# ---------------------------------------------------------------------------
if [ -n "$PKG" ]; then
perl - "$GAME" "$PKG" "$SNAKE" "$UPGRADE" "$SUPPRESS_PORTED_NOISE" <<'PERL_EOF'
use strict;
use warnings;

my ($game, $pkg, $snake, $upgrade, $suppress_ported_noise) = @ARGV;
my @hand_edits;
sub hand_edit { push @hand_edits, "HAND EDIT: $_[0]"; }

sub slurp {
    my $p = shift;
    return undef unless -e $p;
    open my $fh, '<', $p or die "read $p: $!";
    local $/;
    my $c = <$fh>;
    close $fh;
    return $c;
}
sub spit {
    my ($p, $c) = @_;
    open my $fh, '>', $p or die "write $p: $!";
    print $fh $c;
    close $fh;
}

# ---------- Cargo.toml ----------
{
    my $path = "$game/Cargo.toml";
    my $c = slurp($path);
    if (defined $c) {
        my $orig = $c;

        if ($c !~ /^\[lib\]/m) {
            my $block = "\n[lib]\nname = \"$snake\"\npath = \"src/lib.rs\"\n\n[[bin]]\nname = \"$pkg\"\npath = \"src/main.rs\"\n\n[[bin]]\nname = \"verify\"\npath = \"src/bin/verify.rs\"\nrequired-features = [\"verify\"]\n";
            unless ($c =~ s/(edition = "2024"\n)/$1$block/) {
                hand_edit("Cargo.toml: no 'edition = \"2024\"' line to anchor the [lib]/[[bin]] blocks on; add them by hand");
            }
        }

        # After the [lib] insertion above, so it lands between `edition` and
        # the blank line that starts the [lib] block -- i.e. still inside
        # [package], which is the only place cargo accepts it. Without it
        # `cargo run` is ambiguous the moment the verify bin exists.
        if ($c !~ /^default-run = /m) {
            my $line = "# `cargo run` / `cargo run --features verify` is ambiguous with two [[bin]]\n# targets; name the game explicitly.\ndefault-run = \"$pkg\"\n";
            unless ($c =~ s/(^edition = "2024"\n)/$1$line/m) {
                hand_edit("Cargo.toml: no 'edition = \"2024\"' line to anchor 'default-run = \"$pkg\"' on; add it under [package] by hand or `cargo run` stays ambiguous");
            }
        }

        if ($c !~ /^verify = /m) {
            my $block = "# Replay verifier entry point (src/bin/verify.rs): native CLI and the\n# wasm-bindgen module the site runs under Node. See docs/replay-verification.md.\nverify = [\"dep:wasm-bindgen\"]\n";
            unless ($c =~ s/(^record = \[.*\]\n)/$1$block/m) {
                hand_edit('Cargo.toml: add \'verify = ["dep:wasm-bindgen"]\' under [features], after record');
            }
        }

        if ($c !~ /^rand_xoshiro = /m) {
            my $block = "# Sim RNG: Xoshiro256++ explicitly, never SmallRng (which picks a different\n# algorithm on wasm32 and would break native-vs-wasm replay determinism).\nrand_xoshiro = \"0.7\"\n";
            unless ($c =~ s/(^rand = "0\.9"\n)/$1$block/m) {
                hand_edit('Cargo.toml: add rand_xoshiro = "0.7" after rand = "0.9"');
            }
        }

        # `=0.2.108`, not `0.2.108`: a caret range lets `cargo update` walk the
        # game to 0.2.126 while install.sh and both workflows still install the
        # 0.2.108 CLI, and a CLI/lib skew produces a bundle that fails to load
        # at runtime with nothing in CI to catch it. That is the drift the pilot
        # had to unpick by hand; the `=` form makes cargo refuse it instead.
        if ($c !~ /^wasm-bindgen = /m) {
            my $line = "wasm-bindgen = { version = \"=0.2.108\", optional = true }\n";
            unless ($c =~ s/(^getrandom = \{ version = "0\.3", features = \["wasm_js"\] \}\n)/$1$line/m) {
                hand_edit('Cargo.toml: add wasm-bindgen = { version = "=0.2.108", optional = true } after the wasm32 getrandom line');
            }
        }
        # A game pinned before the `=` form existed, or one whose pin drifted.
        $c =~ s/^wasm-bindgen = \{ version = "0\.2\.\d+", optional = true \}$/wasm-bindgen = { version = "=0.2.108", optional = true }/m;
        if ($c =~ /^wasm-bindgen = /m
            && $c !~ /^wasm-bindgen = \{ version = "=0\.2\.108", optional = true \}$/m) {
            hand_edit('Cargo.toml: wasm-bindgen is not pinned as { version = "=0.2.108", optional = true }; pin it exactly by hand, or cargo update can drift the library away from the 0.2.108 CLI install.sh and both workflows install');
        }

        if ($c =~ /gamebient-input = .*tag = "v0\.2\.\d+"/) {
            $c =~ s/(gamebient-input = .*tag = ")v0\.2\.\d+(")/$1v0.3.0$2/;
        }
        if ($c !~ /gamebient-input = .*tag = "v0\.3\.0"/) {
            hand_edit('Cargo.toml: pin gamebient-input tag = "v0.3.0"');
        }

        spit($path, $c) if $c ne $orig;
    } else {
        hand_edit("Cargo.toml not found");
    }
}

# ---------- src/lib.rs + src/main.rs ----------
{
    my $lib_path = "$game/src/lib.rs";
    my $main_path = "$game/src/main.rs";
    my $main_c = slurp($main_path);
    if (defined $main_c) {
        my $orig_main = $main_c;
        my $has_gameplugin = ($main_c =~ /game::GamePlugin\b/) ? 1 : 0;

        # Keep line endings on each element so re-joining is exact.
        my @lines = split /(?<=\n)/, $main_c;
        my @out;
        my @plain;
        my @cfg; # [ "#[cfg(...)]\n", "modname" ]

        # The `cfg` match must balance nested parens. `[^)]*` did not, so
        # `#[cfg(any(feature = "harness", feature = "autopilot"))] mod bot;`
        # (Pack The Ripper) classified `bot` as a PLAIN mod and left the
        # attribute behind in @out — where it landed immediately above the
        # generated `use <snake>::{...}` and feature-gated the whole import,
        # so the default build stopped compiling. A greedy `.+` anchored on
        # `)]` at end of line is enough for a single-line attribute, which is
        # the only shape rustfmt produces here.
        for my $line (@lines) {
            if ($line =~ /^(?:pub )?mod (\w+);\s*\n?$/) {
                my $name = $1;
                if (@out && $out[-1] =~ /^#\[cfg\(.+\)\]\s*\n?$/) {
                    my $attr = pop @out;
                    push @cfg, [$attr, $name];
                } else {
                    push @plain, $name;
                }
                next;
            }
            push @out, $line;
        }

        if (@plain || @cfg) {
            if (-e $lib_path) {
                # lib.rs already exists but main.rs still has 'mod'
                # declarations to remove: not the idempotent case (a prior
                # successful run would have stripped them together with
                # writing lib.rs), so this lib.rs's pub mod set can't be
                # trusted to cover what main.rs is about to import. Leave
                # both files' mod wiring alone rather than write a `use
                # <snake>::{...}` that might not resolve.
                hand_edit("src/lib.rs: already exists while src/main.rs still has mod declarations to remove; wire 'pub mod' in lib.rs and 'use ${snake}::{...}' in main.rs by hand instead of risking a mismatch");
                @out = @lines;
            } else {
                my $lib_c = "#![allow(clippy::too_many_arguments, clippy::type_complexity)]\n\n";
                $lib_c .= "pub mod $_;\n" for @plain;
                $lib_c .= "$_->[0]pub mod $_->[1];\n" for @cfg;
                spit($lib_path, $lib_c);

                my $use_line = "use $snake" . "::{" . join(", ", @plain) . "};\n";
                my @cfg_use = map { "$_->[0]" . "use $snake" . "::$_->[1];\n" } @cfg;

                my $insert_at = 0;
                for my $i (0 .. $#out) {
                    if ($out[$i] =~ /^use /) { $insert_at = $i; last; }
                }
                # Belt and braces for the same bug: whatever the classifier
                # above did, the unconditional `use` must never land under an
                # attribute. Back up over the run of blank/attribute lines
                # immediately above the insertion point to the TOPMOST
                # attribute in it -- an attribute applies to the next item
                # even with blank lines in between, so anything left there
                # (a `#[cfg]` orphaned by a removed `mod`, or one that
                # genuinely belongs to the first `use`) would swallow the
                # generated import. Inserting above the whole run is always
                # safe: it steals nothing and gates nothing.
                {
                    my $k = $insert_at;
                    my $top = $insert_at;
                    while ($k > 0 && $out[$k - 1] =~ /^\s*(?:\#\[.*)?$/) {
                        $k--;
                        $top = $k if $out[$k] =~ /^\s*\#\[/;
                    }
                    $insert_at = $top;
                }
                splice(@out, $insert_at, 0, $use_line, @cfg_use);
            }
        }

        $main_c = join('', @out);
        if ($has_gameplugin) {
            $main_c =~ s/\bgame::GamePlugin\b(?!::default\(\))/game::GamePlugin::default()/g;
        } else {
            hand_edit("src/main.rs: no literal 'game::GamePlugin' found; wire game::GamePlugin::default() by hand");
        }

        spit($main_path, $main_c) if $main_c ne $orig_main;
    } else {
        hand_edit("no src/main.rs found");
    }
}

# ---------- src/game/mod.rs ----------
{
    my $path = "$game/src/game/mod.rs";
    my $c = slurp($path);
    if (defined $c) {
        my $orig = $c;

        if ($c !~ /^pub mod replay;/m) {
            unless ($c =~ s/^pub mod scoring;\n/pub mod replay;\npub mod scoring;\n/m) {
                hand_edit("src/game/mod.rs: declare 'pub mod replay;' (alphabetically, next to the other pub mod lines)");
            }
        }
        if ($c !~ /^pub mod sim;/m) {
            unless ($c =~ s/^pub mod scoring;\n/pub mod scoring;\npub mod sim;\n/m) {
                hand_edit("src/game/mod.rs: declare 'pub mod sim;' (alphabetically, next to the other pub mod lines)");
            }
        }

        if ($c !~ /pub headless: bool/) {
            my $repl = "#[derive(Default)]\npub struct GamePlugin {\n    pub headless: bool,\n}\n";
            unless ($c =~ s/^pub struct GamePlugin;\s*?\n/$repl/m) {
                hand_edit('src/game/mod.rs: replace \'pub struct GamePlugin;\' with \'#[derive(Default)] pub struct GamePlugin { pub headless: bool }\'');
            }
        }

        # ---- .init_resource::<sim::SpawnCounter>() ----
        #
        # `sim::begin_run` takes `ResMut<SpawnCounter>`, so a game that picks
        # up the current sim.rs without registering the resource fails Bevy's
        # parameter validation on OnEnter(Playing) with the nameless
        # "Parameter ... failed validation: Resource does not exist". That is
        # a real hazard for --upgrade specifically: an already-ported game
        # has the GamePlugin::build HAND EDIT suppressed, so nothing else
        # would mention it. Anchored on `.init_resource::<sim::RunOver>()`,
        # which every ported game has (rule 10 requires it).
        if ($c !~ /init_resource::<sim::SpawnCounter>/) {
            my $anchor = ".init_resource::<sim::RunOver>()";
            my $idx = index($c, $anchor);
            if ($idx >= 0) {
                # Reuse the anchor line's own indentation so the builder
                # chain stays rustfmt-clean (cargo fmt runs at the end anyway).
                my $line_start = rindex($c, "\n", $idx) + 1;
                my $indent = substr($c, $line_start, $idx - $line_start);
                substr($c, $idx + length($anchor), 0) = "\n$indent.init_resource::<sim::SpawnCounter>()";
            } elsif ($upgrade && $suppress_ported_noise) {
                # Only when the GamePlugin::build HAND EDIT below is
                # suppressed. On a first rollout that one already carries
                # the registration list, and two hand edits saying the same
                # thing is how a list stops being read.
                hand_edit("src/game/mod.rs: add .init_resource::<sim::SpawnCounter>() to GamePlugin::build (no .init_resource::<sim::RunOver>() line to hang it off). sim::begin_run takes ResMut<SpawnCounter>; without the registration OnEnter(Playing) dies with Bevy's nameless \"Parameter ... failed validation: Resource does not exist\". SpawnOrder/SpawnCounter is the stable sorting key determinism rule 1 asks order-sensitive sim systems to use.");
            }
        }

        # Always surfaced on a first rollout: the build() body is
        # game-specific free-form code this script cannot safely touch
        # (system ordering, what stays in Update vs moves into sim::SimSet,
        # what this game's own checksum system folds). Under --upgrade on a
        # game whose mod.rs already names sim::SimSet the work is demonstrably
        # done, and repeating it every time devalues the rest of the list.
        unless ($upgrade && $suppress_ported_noise) {
            hand_edit('src/game/mod.rs: port GamePlugin::build to run gameplay through sim::SimSet, gate scene/audio/dev-harness setup on !self.headless, and add a checksum_<game> system after sim::checksum_tick folding your key run state (see docs/replay-verification.md rule 6). Three specifics that cost the pilot hours: (a) the set\'s run condition is THREE clauses -- sim::SimSet.run_if(in_state(GameState::Playing).and(states::not_paused).and(sim::run_not_over)) -- plus .init_resource::<sim::RunOver>() and .init_resource::<sim::SpawnCounter>() (sim::begin_run takes both; a missing one fails Bevy\'s parameter validation with no system name), or the game-over fade adds a frame-rate-dependent tail of ticks the verifier cannot reproduce (rule 10); (b) every system the headless app runs must take resources the headless app actually has -- assets in particular: GameAssets/Assets<Mesh>/Assets<StandardMaterial> are only inserted by the windowed build, so a spawn system needs Option<Res<GameAssets>> and an asset-free branch, or Bevy fails it with \"Parameter ... failed validation\" and no system name; (c) CI\'s Node fixture steps fail until tests/fixtures/selftest.gxr AND tests/fixtures/selftest-wasm.gxr are generated (skill step 4)');
        }

        spit($path, $c) if $c ne $orig;
    } else {
        hand_edit("no src/game/mod.rs found (flat layout?); declare 'pub mod replay'/'pub mod sim' and port GamePlugin by hand");
    }
}

# ---------- src/game/host.rs ----------
#
# Two edits every game written against the template's pre-v0.3.0 host.rs
# needs, and both are hard failures rather than warnings:
#
#   * `mut global_volume: ResMut<GlobalVolume>` — `GlobalVolume` is inserted
#     by Bevy's `AudioPlugin`, which the headless verifier never adds, so the
#     system fails Bevy's parameter validation with a message that names no
#     system. This was the real cause of the pilot's "8 red tests".
#   * `HostCommand::Seed` — added by gamebient-input v0.3.0 (which this
#     script pins). A `match command` with no wildcard arm stops compiling
#     the moment the tag bumps, and without the arm a host-issued seed never
#     reaches `sim::PendingSeed`, so every run would be `origin = Local`.
{
    my $path = "$game/src/game/host.rs";
    my $c = slurp($path);
    if (defined $c) {
        my $orig = $c;

        if ($c !~ /global_volume:\s*Option<ResMut<GlobalVolume>>/) {
            my $comment = "    // `GlobalVolume` is only inserted by `AudioPlugin`, which the headless\n"
                        . "    // build (no window, no audio) never adds — read as optional so a host\n"
                        . "    // `mute` doesn't panic there.\n";
            unless ($c =~ s/^[ \t]*mut global_volume: ResMut<GlobalVolume>,\n/$comment    mut global_volume: Option<ResMut<GlobalVolume>>,\n/m) {
                hand_edit("src/game/host.rs: 'mut global_volume: ResMut<GlobalVolume>,' not found in apply_host_commands; make it Option<ResMut<GlobalVolume>> by hand — the headless verifier has no AudioPlugin and Bevy rejects the system with a nameless \"Parameter ... failed validation\"");
            }
        }
        # The `Mute` arm then has to cope with the Option.
        if ($c !~ /global_volume\.as_deref_mut\(\)/) {
            unless ($c =~ s/^([ \t]*)global_volume\.volume = ([^\n]*);\n/${1}if let Some(volume) = global_volume.as_deref_mut() {\n${1}    volume.volume = ${2};\n${1}}\n/m) {
                hand_edit("src/game/host.rs: the Mute arm's 'global_volume.volume = ...;' line not found; guard it with 'if let Some(volume) = global_volume.as_deref_mut()' by hand");
            }
        }

        if ($c !~ /HostCommand::Seed/) {
            if ($c =~ /^\s*_\s*=>/m) {
                hand_edit("src/game/host.rs: apply_host_commands has a '_ =>' wildcard arm, so it compiles against gamebient-input v0.3.0 — but a host-issued seed is silently swallowed and every run stays origin = Local. Add 'HostCommand::Seed(bytes) => pending.0 = Some(*bytes),' (plus a 'mut pending: ResMut<crate::game::sim::PendingSeed>' param) explicitly");
            } else {
                my $before_seed = $c;
                my $ok = 1;
                # Param: before the closing paren of apply_host_commands's
                # signature, found by scanning forward from the fn to the
                # first line that is exactly ") {".
                my $fn = index($c, "fn apply_host_commands(");
                my $close = $fn >= 0 ? index($c, "\n) {", $fn) : -1;
                if ($close >= 0) {
                    substr($c, $close + 1, 0) = "    mut pending: ResMut<crate::game::sim::PendingSeed>,\n";
                } else {
                    hand_edit("src/game/host.rs: apply_host_commands's signature doesn't end in a line of its own ') {'; add 'mut pending: ResMut<crate::game::sim::PendingSeed>' by hand");
                    $ok = 0;
                }
                # Arm: after the Hello arm, which every template-derived
                # host.rs has as the last arm of the match.
                unless ($c =~ s/^([ \t]*)(HostCommand::Hello \{ \.\. \} => \{\}\n)/${1}${2}${1}HostCommand::Seed(bytes) => pending.0 = Some(*bytes),\n/m) {
                    hand_edit("src/game/host.rs: no 'HostCommand::Hello { .. } => {}' arm to anchor on; add 'HostCommand::Seed(bytes) => pending.0 = Some(*bytes),' to apply_host_commands by hand, or the match won't compile against gamebient-input v0.3.0");
                    $ok = 0;
                }
                # Half an edit is worse than none: it would not compile and
                # the HAND EDIT above would be about the other half. Rewind
                # only the Seed edits -- the GlobalVolume ones above stand.
                $c = $before_seed unless $ok;
            }
        }

        spit($path, $c) if $c ne $orig;
    } else {
        hand_edit("no src/game/host.rs found; wire HostCommand::Seed -> sim::PendingSeed and an optional GlobalVolume by hand");
    }
}

# ---------- build_web.sh ----------
{
    my $path = "$game/build_web.sh";
    my $c = slurp($path);
    my $verify_step_reported = 0;
    if (defined $c) {
        my $orig = $c;

        if ($c !~ /! -name 'verify\.wasm'/) {
            my $old = "WASM=\$(find target/wasm32-unknown-unknown/wasm-release -maxdepth 1 -name '*.wasm' | head -1)\n"
                    . "if [ -z \"\$WASM\" ]; then\n"
                    . "    echo \"ERROR: no .wasm found in target/wasm32-unknown-unknown/wasm-release/\" >&2\n"
                    . "    exit 1\n"
                    . "fi\n";
            my $new = "# verify.wasm is excluded by name: tools/build_verify.sh builds the replay\n"
                    . "# verifier into the same directory, and shipping it as the game would deploy a\n"
                    . "# module with no window, renderer or assets. Anything else unexpected in there\n"
                    . "# is a hard error rather than a coin flip about which .wasm gets deployed.\n"
                    . "WASM=\$(find target/wasm32-unknown-unknown/wasm-release -maxdepth 1 -name '*.wasm' ! -name 'verify.wasm')\n"
                    . "if [ -z \"\$WASM\" ]; then\n"
                    . "    echo \"ERROR: no game .wasm found in target/wasm32-unknown-unknown/wasm-release/\" >&2\n"
                    . "    exit 1\n"
                    . "fi\n"
                    . "if [ \"\$(printf '%s\\n' \"\$WASM\" | wc -l | tr -d ' ')\" -ne 1 ]; then\n"
                    . "    echo \"ERROR: more than one candidate .wasm in target/wasm32-unknown-unknown/wasm-release/:\" >&2\n"
                    . "    printf '%s\\n' \"\$WASM\" >&2\n"
                    . "    echo \"Remove the stale ones (or 'cargo clean') so the deployed bundle is unambiguous.\" >&2\n"
                    . "    exit 1\n"
                    . "fi\n";
            my $idx = index($c, $old);
            if ($idx >= 0) {
                substr($c, $idx, length($old)) = $new;
            } else {
                hand_edit("build_web.sh: the 'find ... *.wasm | head -1' block doesn't match the template's; add the verify.wasm exclusion by hand");
            }
        }

        # Guard on the INSERTED LINE, anchored, not on the bare path. The
        # verify.wasm comment block above contains the literal text
        # "tools/build_verify.sh" in its prose, and it is inserted by this
        # same pass into this same $c — so a guard of /tools\/build_verify\.sh/
        # read its own sibling's comment as proof the work was done, skipped
        # the insertion, and printed no HAND EDIT. The build and the deploy
        # both succeeded; the only symptom was in production, where
        # properties.verify_url 404s and every honest run came back "No
        # verifier for this game build yet". Every game rolled out between
        # the comment landing and this fix is affected.
        if ($c !~ /^bash tools\/build_verify\.sh$/m) {
            my $anchor = "    dist/${pkg}_bg.wasm -o dist/${pkg}_bg.wasm\n";
            my $addition = "\n# Build the headless replay verifier and publish it alongside the game bundle\n"
                          . "# as dist/verify.zip — the site fetches it from properties.verify_url. Run\n"
                          . "# after this script's own wasm-bindgen/wasm-opt steps (and after the `find`\n"
                          . "# above, which already excludes verify.wasm by name) so there's no ambiguity\n"
                          . "# about which .wasm is the game bundle.\n"
                          . "bash tools/build_verify.sh\n"
                          . "cp dist-verify.zip dist/verify.zip\n";
            my $idx = index($c, $anchor);
            if ($idx >= 0) {
                substr($c, $idx + length($anchor), 0) = $addition;
            } else {
                hand_edit("build_web.sh: wasm-opt output line ('dist/${pkg}_bg.wasm -o dist/${pkg}_bg.wasm') not found; add 'bash tools/build_verify.sh' + 'cp dist-verify.zip dist/verify.zip' by hand after wasm-opt, or the deploy ships without dist/verify.zip and verify_url 404s");
                $verify_step_reported = 1;
            }
        }

        # Belt and braces for the silent-skip class of bug above: whatever
        # path we took, build_web.sh must end up publishing verify.zip. Say
        # so loudly if it does not, rather than leaving it to production.
        if (!$verify_step_reported && $c !~ /^bash tools\/build_verify\.sh$/m) {
            hand_edit("build_web.sh: still has no 'bash tools/build_verify.sh' line after this script's edits; add it (and 'cp dist-verify.zip dist/verify.zip' after it) by hand, or the web deploy ships without dist/verify.zip and properties.verify_url 404s for every run");
        }
        if ($c =~ /^bash tools\/build_verify\.sh$/m && $c !~ /^cp dist-verify\.zip dist\/verify\.zip$/m) {
            hand_edit("build_web.sh: runs tools/build_verify.sh but never copies the result; add 'cp dist-verify.zip dist/verify.zip' after it by hand, or dist/ has no verify.zip to deploy");
        }

        spit($path, $c) if $c ne $orig;
    } else {
        hand_edit("no build_web.sh found");
    }
}

# ---------- .github/workflows/ci.yml ----------
{
    my $path = "$game/.github/workflows/ci.yml";
    my $c = slurp($path);
    # The pair of Node steps in their current shape, shared by the
    # first-time insertion below and by --upgrade's replacement of the
    # single-fixture step a pre-wasm-fixture rollout left behind.
    my $node_steps = "      # The gate: selftest-wasm.gxr was recorded by this same wasm\n"
                    . "      # module, so both sides are wasm arithmetic - what actually\n"
                    . "      # ships, and spec-deterministic. A failure here is real.\n"
                    . "      - name: Verify the wasm-recorded fixture under Node\n"
                    . "        run: node tools/verify_fixture.mjs tests/fixtures/selftest-wasm.gxr\n"
                    . "\n"
                    . "      # Informational: selftest.gxr was recorded natively, so this\n"
                    . "      # compares native libm against wasm and can drift by an ulp on\n"
                    . "      # a sim that calls sin/cos/powf. See docs/replay-verification.md.\n"
                    . "      - name: Cross-check the native fixture under Node (informational)\n"
                    . "        continue-on-error: true\n"
                    . "        run: node tools/verify_fixture.mjs tests/fixtures/selftest.gxr\n";
    if (defined $c) {
        my $orig = $c;
        if ($c !~ /verify_fixture\.mjs/) {
            my $anchor = "      - name: Build web bundle\n        run: bash build_web.sh\n";
            my $addition = "\n      - uses: actions/setup-node\@v4\n"
                          . "        with:\n"
                          . "          node-version: 22\n"
                          . "\n"
                          . $node_steps;
            my $idx = index($c, $anchor);
            if ($idx >= 0) {
                substr($c, $idx + length($anchor), 0) = $addition;
            } else {
                hand_edit(".github/workflows/ci.yml: 'Build web bundle' step not found in the template's shape; add the setup-node + verify_fixture.mjs steps by hand");
            }
        } elsif ($c !~ /selftest-wasm\.gxr/) {
            # A game ported before the wasm-recorded fixture existed has a
            # single Node step reading the NATIVE fixture — often with a
            # `continue-on-error: true` and a paragraph of its own
            # explaining the libm drift that forced it. That whole
            # apparatus is what the wasm fixture replaced, so the step and
            # the comment block above it are replaced together with the
            # gate + informational pair. Only under --upgrade: on a plain
            # re-run, rewriting a workflow step is exactly the kind of
            # guess this script refuses to make.
            if ($upgrade) {
                my $step = qr{
                    (?:^[ \t]*\#[^\n]*\n)*                                  # its own comment block
                    ^[ \t]*-\ name:\ Verify\ the\ committed\ fixture\ under\ Node\n
                    (?:^[ \t]*continue-on-error:\ true\n)?
                    ^[ \t]*run:\ node\ tools/verify_fixture\.mjs[^\n]*\n
                }mx;
                unless ($c =~ s/$step/$node_steps/) {
                    hand_edit(".github/workflows/ci.yml: the single-fixture Node step isn't in the shape --upgrade knows how to replace ('- name: Verify the committed fixture under Node'); replace it with the wasm-gate + informational-native pair by hand");
                }
            } else {
                hand_edit(".github/workflows/ci.yml: the Node step still verifies the native fixture only; re-run with --upgrade to replace it with the wasm gate + informational native cross-check");
            }
        }

        # ---- the Test step's feature set ----
        #
        # A template-derived game's Test step is `cargo test --all-targets`
        # with no features, so it never builds `src/bin/verify.rs` (it is
        # `required-features = ["verify"]`) and never runs anything gated on
        # a feature -- while the Clippy step two lines above it DOES pass
        # --all-features, so the two steps compile different crates and CI's
        # green tells you less than it looks. It is also the gate the rollout
        # and the port checklist both document as `cargo test --all-features`.
        # Dough.io, Pack The Ripper and Sundae Shooter each fixed this by
        # hand in their own port; this makes the rollout do it.
        #
        # Applied on a first rollout as well as --upgrade, because the step
        # is the template's own and this only widens it. Idempotent: a step
        # that already says --all-features is left alone.
        if ($c =~ /^([ \t]*)-\ name:\ Test\n([ \t]*)run:\ (cargo\ test[^\n]*)\n/m) {
            my ($ind, $run_ind, $cmd) = ($1, $2, $3);
            if ($cmd !~ /--all-features/) {
                my $old = "$ind- name: Test\n$run_ind" . "run: $cmd\n";
                my $new = "$ind- name: Test\n$run_ind" . "run: cargo test --all-targets --all-features\n";
                my $idx = index($c, $old);
                substr($c, $idx, length($old)) = $new if $idx >= 0;
            }
        } else {
            hand_edit(".github/workflows/ci.yml: no recognisable '- name: Test' step running cargo test; make it 'cargo test --all-targets --all-features' by hand. Without --all-features the Test step never builds src/bin/verify.rs (required-features = [\"verify\"]) and compiles a different crate from the Clippy step next to it.");
        }

        # ---- CARGO_PROFILE_DEV_DEBUG on the check job ----
        #
        # Adding tests/archetype_order.rs and tests/windowed_shape.rs takes a
        # game to five Bevy test binaries, and the debug info across them exhausts the runner's
        # disk. It does not announce itself as an out-of-disk failure: the
        # Test step dies inside rust-lld with
        #   collect2: fatal error: ld terminated with signal 7 [Bus error]
        # and a request to file an llvm-project bug. Measured on Dough.io,
        # where it landed first: --all-targets --all-features is 12 GB of
        # target dir with debug info and 2.0 GB without, with an identical
        # test result. In the workflow rather than Cargo.toml so local
        # debugging keeps its symbols. Idempotent, and applied on a plain
        # rollout as well as --upgrade: it is a pure addition to an `env:`
        # block, not a rewrite of anything the game may have tuned.
        if ($c !~ /CARGO_PROFILE_DEV_DEBUG/) {
            my $anchor = "      RUSTFLAGS: -D warnings\n";
            my $addition = "      # Four Bevy test binaries' debug info exhausts the runner disk; the\n"
                          . "      # failure reads as 'collect2: ld terminated with signal 7' inside\n"
                          . "      # rust-lld, not as out-of-disk. Measured 12 GB -> 2.0 GB of target\n"
                          . "      # dir with an identical test result. Here and not in Cargo.toml so\n"
                          . "      # local debugging keeps its symbols.\n"
                          . "      CARGO_PROFILE_DEV_DEBUG: \"0\"\n";
            my $idx = index($c, $anchor);
            if ($idx >= 0) {
                substr($c, $idx + length($anchor), 0) = $addition;
            } else {
                hand_edit(".github/workflows/ci.yml: no 'RUSTFLAGS: -D warnings' line to hang CARGO_PROFILE_DEV_DEBUG: \"0\" off; add it to the check job's env: by hand, or the Test step will die linking the fourth test binary with 'collect2: ld terminated with signal 7' (runner disk, not a compiler bug)");
            }
        }

        spit($path, $c) if $c ne $orig;
    } else {
        hand_edit("no .github/workflows/ci.yml found");
    }
}

# ---------- .github/workflows/release.yml ----------
{
    my $path = "$game/.github/workflows/release.yml";
    my $c = slurp($path);
    if (defined $c) {
        my $orig = $c;
        my $verify_zip = "$pkg-verify.zip";
        # Captured before the package-step insertion below, which itself
        # contains the literal text "build/$verify_zip" (its own `cp`
        # line) — checking this after insertion would make the upload
        # path's HAND EDIT unreachable whenever the 'Zip web bundle'
        # anchor matched but the path: line had drifted.
        my $had_zip = index($c, "build/$verify_zip") >= 0;

        if (index($c, "Package replay verifier") < 0) {
            my $anchor = "          (cd dist && zip -r ../build/$pkg-web.zip .)\n";
            my $addition = "\n      # build.sh web -> build_web.sh already ran tools/build_verify.sh and\n"
                          . "      # produced dist-verify.zip at the repo root; just place it under its\n"
                          . "      # release asset name.\n"
                          . "      - name: Package replay verifier\n"
                          . "        if: matrix.target == 'web'\n"
                          . "        run: |\n"
                          . "          mkdir -p build\n"
                          . "          cp dist-verify.zip build/$verify_zip\n";
            my $idx = index($c, $anchor);
            if ($idx >= 0) {
                substr($c, $idx + length($anchor), 0) = $addition;
            } else {
                hand_edit(".github/workflows/release.yml: 'Zip web bundle' step not found in the template's shape; add the verify.zip packaging step by hand");
            }
        }

        if (!$had_zip || index($c, "path: |") < 0) {
            my $old_path = "          path: build/$pkg-" . '${{ matrix.target }}.*' . "\n";
            my $new_path = "          path: |\n"
                          . "            build/$pkg-" . '${{ matrix.target }}.*' . "\n"
                          . "            build/$verify_zip\n";
            my $idx = index($c, $old_path);
            if ($idx >= 0) {
                substr($c, $idx, length($old_path)) = $new_path;
            } elsif (!$had_zip) {
                hand_edit(".github/workflows/release.yml: upload artifact 'path:' line not found in the template's shape; add build/$verify_zip to it by hand");
            }
        }

        spit($path, $c) if $c ne $orig;
    } else {
        hand_edit("no .github/workflows/release.yml found");
    }
}

# ---------- .gitignore ----------
{
    my $path = "$game/.gitignore";
    my $c = slurp($path);
    $c = '' unless defined $c;
    my $orig = $c;
    unless ($c =~ /^\/dist-verify$/m) {
        $c .= "\n# Replay verifier module — regenerated by tools/build_verify.sh\n/dist-verify\n/dist-verify.zip\n";
    }
    unless ($c =~ /^\/build\/replays$/m) {
        $c .= "# Local replays written when GX_REPLAY_DIR points here\n/build/replays\n";
    }
    spit($path, $c) if $c ne $orig;
}

# ---------- assets/info.json ----------
{
    my $path = "$game/assets/info.json";
    my $c = slurp($path);
    if (defined $c) {
        my $orig = $c;
        if (index($c, '"verify_url"') < 0) {
            if ($c =~ /"game_url":\s*"([^"]*)"/) {
                my $game_url = $1;
                my $verify_line = "        \"verify_url\": \"$game_url/verify.zip\",\n";
                unless ($c =~ s/("demo_url":\s*"[^"]*",\n)/$1$verify_line/) {
                    hand_edit("assets/info.json: 'demo_url' line not found; add \"verify_url\" by hand");
                }
            } else {
                hand_edit('assets/info.json: no "game_url" found; add "verify_url": "<game_url>/verify.zip" by hand');
            }
        }
        spit($path, $c) if $c ne $orig;
    } else {
        hand_edit("no assets/info.json found");
    }
}

print "$_\n" for @hand_edits;
PERL_EOF
fi

# ---------------------------------------------------------------------------
# Workflow drift advisory (--upgrade only).
#
# The reconciliation above knows a fixed set of shapes: the Node fixture
# steps, the verify.zip packaging step, the upload path, the disk setting.
# Everything else a game's workflows have drifted into is invisible to it —
# a pinned wasm-bindgen-cli the template has since bumped, an apt package
# added by hand, a job that was never updated when the template's was. None
# of that is safe for a script to rewrite, and all of it is worth a human's
# eye on an upgrade, so: diff the game's workflows against the template's
# with the naming substitutions applied and say how far apart they are.
#
# Advisory, like the archetype-insert scan: expect legitimate differences
# (a game with no cutter has no "Cutter tests" step) and read it as a
# reading list, not a verdict.
# ---------------------------------------------------------------------------
if [ "$UPGRADE" -eq 1 ] && [ -n "$PKG" ]; then
  DRIFT_MAX_LINES=40
  for wf in ci release; do
    GAME_WF="$GAME/.github/workflows/$wf.yml"
    TEMPLATE_WF="$TEMPLATE/.github/workflows/$wf.yml"
    if [ ! -e "$GAME_WF" ] || [ ! -e "$TEMPLATE_WF" ]; then
      continue
    fi
    # The template side with `gamebient-game` -> the game's crate name and
    # `gamebient_game` -> its snake_case path, which is every substitution
    # the rollout itself makes to a workflow. What survives is real drift.
    DRIFT="$(diff \
      <(sed -e "s/gamebient-game/${PKG}/g" -e "s/gamebient_game/${SNAKE}/g" "$TEMPLATE_WF") \
      "$GAME_WF" | grep '^[<>]' || true)"
    if [ -z "$DRIFT" ]; then
      continue
    fi
    DRIFT_N="$(printf '%s\n' "$DRIFT" | wc -l | tr -d ' ')"
    printf '%s\n' "$DRIFT" | head -n "$DRIFT_MAX_LINES" | sed 's/^/    /'
    if [ "$DRIFT_N" -gt "$DRIFT_MAX_LINES" ]; then
      echo "    ... ($DRIFT_MAX_LINES of $DRIFT_N lines shown)"
    fi
    echo "HAND EDIT (advisory): .github/workflows/$wf.yml differs from the template in $DRIFT_N lines; review with: diff <(sed -e 's/gamebient-game/${PKG}/g' -e 's/gamebient_game/${SNAKE}/g' $TEMPLATE_WF) $GAME_WF"
  done
fi

# The perl edits above write multi-line blocks with their own spacing (not
# necessarily rustfmt's); format the game so CI stays green, same as
# rollout-record.sh.
if command -v cargo >/dev/null 2>&1; then
  (cd "$GAME" && cargo fmt --all) || echo "HAND EDIT: cargo fmt failed in $GAME; run it before committing"
else
  echo "HAND EDIT: cargo not on PATH; run cargo fmt --all in $GAME before committing"
fi

if [ "$UPGRADE" -eq 1 ]; then
  if [ -n "$REFRESHED" ]; then
    echo "rollout-replay --upgrade: refreshed these stale verbatim copies from the template:"
    printf '%s' "$REFRESHED" | sed 's/^/  - /'
  else
    echo "rollout-replay --upgrade: nothing to refresh — every copied file is already the template's current version or locally modified (see the HAND EDIT lines above)"
  fi
  echo "rollout-replay --upgrade: src/game/replay/selftest.rs and tests/archetype_order.rs are refreshed only while they are still byte-identical to a committed template version; once the port has replaced them they are left alone, silently"
  if [ "$MISSING_PROBE" -eq 1 ]; then
    echo "rollout-replay --upgrade: tests/archetype_order.rs is ABSENT and was not created — copy it from $TEMPLATE/tests/archetype_order.rs and adapt its markers (see the HAND EDIT above). It is the only test that catches the archetype-order trap."
  fi
fi

if [ "$ADDED_SHAPE" -eq 1 ]; then
  echo "rollout-replay: tests/windowed_shape.rs is NEW in this checkout. It records the selftest script with ScreenFade present and verifies it without, so it can fail on a game that was green a minute ago — that is a finding, not a regression: some sim system is reading presentation state. See the file's module comment."
fi

echo "rollout-replay: files in place for $GAME"
echo "next: (cd $GAME && cargo check --features verify)"
