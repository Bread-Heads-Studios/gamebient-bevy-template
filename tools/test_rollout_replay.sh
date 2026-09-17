#!/usr/bin/env bash
# Tests tools/rollout-replay.sh against two scratch copies (never the real
# checkouts):
#
#   (a) a copy of this template itself -> running the script must be a
#       no-op (it already has every feature file and wiring edit in place).
#   (b) a copy of games/cannonball-putt -> the one documented hand edit
#       (rename its own test-only src/game/sim.rs) is applied first, then
#       the script must leave `cargo check --features verify` passing after
#       the still-required per-game hand edits are applied (LeaderboardScore,
#       plus the HostCommand::Seed match arm — see below), and its HAND EDIT
#       output must name LeaderboardScore and the GamePlugin::build port, and
#       nothing about sim.rs (it was renamed away before the script ran).
#   (c) a second copy of the template with release.yml's upload artifact
#       reverted to a pre-rollout shape but its `path:` line already
#       drifted -> the script must print a HAND EDIT naming release.yml's
#       upload path rather than silently leaving it broken.
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

# --- A gap beyond the documented LeaderboardScore hand edit -----------------
#
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

# ---------------------------------------------------------------------------
# (c) release.yml upload-path HAND EDIT, on a fresh template copy.
#
# Regression test for a bug where the upload `path:` drift check read
# `$c` (the release.yml content) *after* the "Package replay verifier"
# step had already been spliced in — and that step's own `cp dist-
# verify.zip build/<pkg>-verify.zip` line contains the literal substring
# the check was looking for, so on a game whose "Zip web bundle" anchor
# matched but whose `path:` line had drifted from the template's shape,
# the upload silently never gained the verify zip and no HAND EDIT
# printed. Reproduce that shape here: revert a fresh copy's release.yml
# to not yet have the "Package replay verifier" step (so this run inserts
# it fresh, exactly like a first-time rollout), but with the upload
# `path:` line already drifted into something the script's exact-match
# anchor won't recognize.
# ---------------------------------------------------------------------------
COPY_C="$SCRATCH_ROOT/template-copy-c"
snapshot_as_git_baseline "$TEMPLATE" "$COPY_C"

perl - "$COPY_C" <<'PERL_EOF'
use strict;
use warnings;
my $path = "$ARGV[0]/.github/workflows/release.yml";
open my $fh, '<', $path or die "read $path: $!";
local $/;
my $c = <$fh>;
close $fh;

my $package_step = "\n      # build.sh web -> build_web.sh already ran tools/build_verify.sh and\n"
                  . "      # produced dist-verify.zip at the repo root; just place it under its\n"
                  . "      # release asset name.\n"
                  . "      - name: Package replay verifier\n"
                  . "        if: matrix.target == 'web'\n"
                  . "        run: |\n"
                  . "          mkdir -p build\n"
                  . "          cp dist-verify.zip build/gamebient-game-verify.zip\n";
my $idx = index($c, $package_step);
die "part (c) setup: 'Package replay verifier' step not found in the shape this test expects\n" if $idx < 0;
substr($c, $idx, length($package_step)) = "";

my $old_path_block = "          path: |\n"
                    . "            build/gamebient-game-" . '${{ matrix.target }}.*' . "\n"
                    . "            build/gamebient-game-verify.zip\n";
my $idx2 = index($c, $old_path_block);
die "part (c) setup: multi-line upload path block not found in the shape this test expects\n" if $idx2 < 0;
substr($c, $idx2, length($old_path_block)) = "          path: build/gamebient-game-artifacts/\n";

open my $ofh, '>', $path or die "write $path: $!";
print $ofh $c;
close $ofh;
PERL_EOF

bash "$TEMPLATE/tools/rollout-replay.sh" "$COPY_C" >"$SCRATCH_ROOT/c-output.txt" 2>&1
cat "$SCRATCH_ROOT/c-output.txt"

grep -q "release.yml" "$SCRATCH_ROOT/c-output.txt" \
  || fail "(c) HAND EDIT output doesn't mention release.yml"
grep "release.yml" "$SCRATCH_ROOT/c-output.txt" | grep -q "path" \
  || fail "(c) HAND EDIT output mentions release.yml but not the upload 'path'"
pass "(c) a drifted release.yml upload path prints a HAND EDIT instead of silently staying broken"

echo "test_rollout_replay: all checks passed"
