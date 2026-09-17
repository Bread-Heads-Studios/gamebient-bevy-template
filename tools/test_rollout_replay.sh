#!/usr/bin/env bash
# Tests tools/rollout-replay.sh against two scratch copies (never the real
# checkouts):
#
#   (a) a copy of this template itself -> running the script must be a
#       no-op (it already has every feature file and wiring edit in place).
#   (b) a copy of games/cannonball-putt -> the one documented hand edit
#       (rename its own test-only src/game/sim.rs) is applied first, then
#       the script must leave `cargo check --features verify` passing after
#       the still-required per-game hand edits are applied (see "known gaps"
#       below), and its HAND EDIT output must name LeaderboardScore and the
#       GamePlugin::build port, and nothing about sim.rs (it was renamed
#       away before the script ran).
#
# Usage: tools/test_rollout_replay.sh
set -euo pipefail
TEMPLATE="$(cd "$(dirname "$0")/.." && pwd)"
GAMES_ROOT="$(cd "$TEMPLATE/../../games" && pwd)"
CANNONBALL="$GAMES_ROOT/cannonball-putt"

SCRATCH_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/rollout-replay-test.XXXXXX")"
trap 'rm -rf "$SCRATCH_ROOT"' EXIT

pass() { echo "PASS: $1"; }
fail() {
  echo "FAIL: $1" >&2
  exit 1
}

# Snapshots $1's working tree (tracked + untracked-but-not-gitignored files,
# so uncommitted work like a not-yet-committed rollout-replay.sh is
# included) into $2, then turns $2 into its own git repo with that snapshot
# as a committed baseline, so `git status --porcelain` afterward shows
# exactly what a later step changed.
snapshot_as_git_baseline() {
  local src="$1" dst="$2"
  mkdir -p "$dst"
  (cd "$src" && git ls-files -z --cached --others --exclude-standard) \
    | tar -C "$src" --null -T - -cf - \
    | tar -C "$dst" -xf -
  (
    cd "$dst"
    git init -q
    git config user.email test@example.com
    git config user.name "rollout-replay test"
    git add -A
    git commit -q -m baseline
  )
}

# ---------------------------------------------------------------------------
# (a) Template copy: the script must be a no-op.
# ---------------------------------------------------------------------------
COPY_A="$SCRATCH_ROOT/template-copy"
snapshot_as_git_baseline "$TEMPLATE" "$COPY_A"

bash "$TEMPLATE/tools/rollout-replay.sh" "$COPY_A" >"$SCRATCH_ROOT/a-output.txt" 2>&1
cat "$SCRATCH_ROOT/a-output.txt"

DIRTY_A="$(cd "$COPY_A" && git status --porcelain -- . ':!Cargo.lock')"
if [ -n "$DIRTY_A" ]; then
  echo "$DIRTY_A" >&2
  fail "(a) template copy: rollout-replay.sh was not a no-op"
fi
pass "(a) template copy: rollout-replay.sh is a no-op (git status clean except Cargo.lock)"

# ---------------------------------------------------------------------------
# (b) Cannonball Putt copy.
# ---------------------------------------------------------------------------
[ -d "$CANNONBALL" ] || fail "games/cannonball-putt not found at $CANNONBALL"

COPY_B="$SCRATCH_ROOT/cannonball-putt-copy"
snapshot_as_git_baseline "$CANNONBALL" "$COPY_B"

# The one documented hand edit: cannonball-putt already has its own
# test-only src/game/sim.rs (the shot-physics simulator used by its harness
# and autopilot, cfg-gated on test/harness/autopilot). Rename it out of the
# way before running the script, exactly as rollout-replay.sh's pre-flight
# check tells a real game to do, and fix its mod declaration + the two
# in-crate `sim::` call sites so the renamed copy still compiles under the
# features that use it.
mv "$COPY_B/src/game/sim.rs" "$COPY_B/src/game/shot_sim.rs"
perl -pi -e 's/^pub mod sim;$/pub mod shot_sim;/' "$COPY_B/src/game/mod.rs"
perl -pi -e 's/\bsim::/shot_sim::/g' "$COPY_B/src/harness.rs" "$COPY_B/src/game/autopilot.rs"

bash "$TEMPLATE/tools/rollout-replay.sh" "$COPY_B" >"$SCRATCH_ROOT/b-output.txt" 2>&1
cat "$SCRATCH_ROOT/b-output.txt"

grep -q "LeaderboardScore" "$SCRATCH_ROOT/b-output.txt" \
  || fail "(b) HAND EDIT output doesn't mention LeaderboardScore"
grep -q "GamePlugin::build" "$SCRATCH_ROOT/b-output.txt" \
  || fail "(b) HAND EDIT output doesn't mention the GamePlugin::build port"
if grep -q "sim\.rs" "$SCRATCH_ROOT/b-output.txt"; then
  fail "(b) HAND EDIT output mentions sim.rs, but it was renamed away before the script ran"
fi
pass "(b) HAND EDIT output names LeaderboardScore and the GamePlugin::build port, nothing about sim.rs"

# --- Known gaps beyond the documented LeaderboardScore hand edit -----------
#
# The freshly-copied src/game/sim.rs also won't compile as-is against a game
# shaped like Cannonball Putt, for two reasons neither pre-flight check nor
# HAND EDIT message currently names:
#
#   1. checksum_tick() folds `data.lives` — cannonball-putt's GameData is
#      golf state (hole_index/strokes/results/best_total) with no `lives`.
#   2. checksum_tick() queries `With<super::player::Player>` — cannonball-
#      putt has no `player` module or `Player` component at all (it moves a
#      `Ball`, not a generic Player entity).
#
# The design doc's determinism-port bullet 6 says checksum_tick must fold
# "the game's own key state (ball position and hole index for golf)" — i.e.
# this is real per-game porting work the skill's port-checklist (Task 3+)
# is meant to cover, not something rollout-replay.sh can safely automate
# (same reasoning as the GamePlugin::build HAND EDIT). Apply the minimal
# golf-shaped version of that port here, the same way LeaderboardScore is
# added below, so this test proves the script's mechanical output is
# actually buildable once the *documented and discovered* per-game hand
# edits are done — see task-2-report.md's Concerns section.
perl -0pi -e '
  s/player: Query<&Transform, With<super::player::Player>>/ball: Query<&Transform, With<super::ball::Ball>>/;
  s/sum\.fold\(u64::from\(data\.lives\)\);/sum.fold(u64::from(data.hole_index as u64));/;
  s/if let Ok\(tf\) = player\.single\(\) \{/if let Ok(tf) = ball.single() {/;
' "$COPY_B/src/game/sim.rs"

# The mandated gamebient-input v0.2.0 -> v0.3.0 bump (required by the plan's
# Global Constraints) added HostCommand::Seed. cannonball-putt's own
# pre-existing host.rs matches HostCommand without a wildcard arm, so this
# non-exhaustive match is a hard compile error the moment the tag bumps —
# true for any game's host.rs written against v0.2.0, not just this one.
# rollout-replay.sh's pre-flight now HAND EDITs it (see the "HostCommand::
# Seed" check); wire it here the same way the template does, so
# host-supplied seeds actually reach the sim rather than merely compiling.
perl -0pi -e '
  s/(mut sinks: Query<&mut AudioSink>,\n)(\) \{)/$1    mut pending: ResMut<crate::game::sim::PendingSeed>,\n$2/;
  s/(HostCommand::Hello \{ \.\. \} => \{\}\n)/$1            HostCommand::Seed(bytes) => pending.0 = Some(*bytes),\n/;
' "$COPY_B/src/game/host.rs"

# The documented hand edit: GameData has no `score` field, so LeaderboardScore
# must be implemented for it (the trait itself doesn't exist yet either —
# only the template's own scoring.rs has it; a real port would copy/adapt
# both). This mirrors what a human does per the pre-flight HAND EDIT.
cat >>"$COPY_B/src/game/scoring.rs" <<'RUST'

/// The single higher-is-better integer the leaderboard ranks. golf has no
/// `score` field, so this reduces `GameData`'s own state (0 is a minimal
/// stand-in; a real port would use total_strokes()/total_vs_par()).
pub trait LeaderboardScore {
    fn leaderboard_score(&self) -> u32;
}

impl LeaderboardScore for GameData {
    fn leaderboard_score(&self) -> u32 {
        0
    }
}
RUST

# A stable (not auto-deleted) target dir so repeat runs of this test don't
# recompile the whole Bevy dependency graph from scratch every time.
CARGO_TARGET_DIR="${TMPDIR:-/tmp}/rollout-replay-test-cargo-target"
export CARGO_TARGET_DIR
if (cd "$COPY_B" && cargo check --features verify); then
  pass "(b) cargo check --features verify succeeds after the documented + discovered hand edits"
else
  fail "(b) cargo check --features verify failed"
fi

echo "test_rollout_replay: all checks passed"
