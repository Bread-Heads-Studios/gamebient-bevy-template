#!/usr/bin/env bash
# Tests tools/rollout-replay.sh against two scratch copies (never the real
# checkouts):
#
#   (a) a copy of this template itself -> running the script must be a
#       no-op (it already has every feature file and wiring edit in place).
#   (b) a copy of games/cannonball-putt's `main` (NOT its working tree: the
#       point is a game that has not been ported yet, and the real checkout
#       may be sitting on a rollout branch) -> the one documented hand edit
#       (rename its own test-only src/game/sim.rs) is applied first, then
#       the script must leave `cargo check --features verify` passing after
#       the one still-required per-game hand edit (LeaderboardScore), and its
#       HAND EDIT output must name LeaderboardScore and the GamePlugin::build
#       port, and nothing about sim.rs (it was renamed away before the script
#       ran). It must also have made the edits the pilot had to do by hand:
#       `default-run`, host.rs's optional GlobalVolume and HostCommand::Seed
#       arm, and build_web.sh's verify.wasm comment block.
#   (c) a second copy of the template with release.yml's upload artifact
#       reverted to a pre-rollout shape but its `path:` line already
#       drifted -> the script must print a HAND EDIT naming release.yml's
#       upload path rather than silently leaving it broken.
#   (d) a scratch dir with only a [workspace] Cargo.toml (no [package]
#       name, e.g. a workspace root like games/voidrunner-authoritative)
#       -> the script must still exit 0 and print a HAND EDIT naming
#       Cargo.toml, not silently exit 1 with no output under set -e.
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

# Same, but from a named git ref rather than the working tree: used for the
# game copy, which must be a *pre-rollout* checkout regardless of what branch
# the real repo happens to be on.
snapshot_git_ref_as_baseline() {
  local src="$1" ref="$2" dst="$3"
  mkdir -p "$dst"
  (cd "$src" && git archive "$ref") | tar -C "$dst" -xf -
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
snapshot_git_ref_as_baseline "$CANNONBALL" main "$COPY_B"

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

# --- The edits the script now makes for us ---------------------------------
#
# Each of these cost the pilot real time as a hand edit, and each is a hard
# failure rather than a warning if it is missed:
#
#   default-run            `cargo run` is ambiguous the moment the verify bin
#                          exists, so playing the game stops working.
#   Option<GlobalVolume>   `GlobalVolume` comes from Bevy's AudioPlugin, which
#                          the headless verifier never adds; without the
#                          Option the system fails Bevy's parameter validation
#                          with a message that names no system.
#   HostCommand::Seed      gamebient-input v0.3.0 (pinned by the script) added
#                          the variant; a match with no wildcard arm stops
#                          compiling, and without the arm host seeds never
#                          reach sim::PendingSeed.
#   build_web.sh comment   the four lines explaining why verify.wasm is
#                          excluded from the game-bundle glob.
grep -q '^default-run = "cannonball-putt"$' "$COPY_B/Cargo.toml" \
  || fail "(b) Cargo.toml did not gain default-run"
grep -q 'mut global_volume: Option<ResMut<GlobalVolume>>,' "$COPY_B/src/game/host.rs" \
  || fail "(b) host.rs's global_volume was not made optional"
grep -q 'if let Some(volume) = global_volume.as_deref_mut()' "$COPY_B/src/game/host.rs" \
  || fail "(b) host.rs's Mute arm was not guarded for the optional GlobalVolume"
grep -q 'HostCommand::Seed(bytes) => pending.0 = Some(\*bytes),' "$COPY_B/src/game/host.rs" \
  || fail "(b) host.rs did not gain the HostCommand::Seed arm"
grep -q 'mut pending: ResMut<crate::game::sim::PendingSeed>,' "$COPY_B/src/game/host.rs" \
  || fail "(b) host.rs did not gain the PendingSeed param"
grep -q "verify.wasm is excluded by name" "$COPY_B/build_web.sh" \
  || fail "(b) build_web.sh lost the template's verify.wasm comment block"
pass "(b) default-run, host.rs (optional GlobalVolume + HostCommand::Seed) and build_web.sh's comment block are automated"

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
  pass "(b) cargo check --features verify succeeds after the one documented hand edit"
else
  fail "(b) cargo check --features verify failed"
fi

# `cargo run` must be unambiguous again: with two [[bin]] targets and the
# verify feature on, cargo refuses to pick one without default-run.
DEFAULT_RUN="$(cd "$COPY_B" && cargo metadata --no-deps --format-version 1 \
  | python3 -c 'import json,sys; print(json.load(sys.stdin)["packages"][0].get("default_run"))')"
if [ "$DEFAULT_RUN" = "cannonball-putt" ]; then
  pass "(b) cargo metadata reports default_run = cannonball-putt (cargo run is unambiguous)"
else
  fail "(b) cargo metadata default_run is '$DEFAULT_RUN', expected 'cannonball-putt'"
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

# ---------------------------------------------------------------------------
# (d) Workspace-root Cargo.toml: no [package] name line at all, so PKG ends
# up empty.
#
# Regression test: `PKG=$(grep -m1 '^name' "$GAME/Cargo.toml" | sed ...)`
# under `set -euo pipefail` — with pipefail, that pipeline's exit status is
# grep's (1, no match) even though sed itself succeeds, and under -e an
# assignment command whose right-hand side fails aborts the whole script
# right there, silently (status 1, no output) — before the empty-PKG HAND
# EDIT this script prints ever runs. A [workspace]-only root with no
# [package] name (e.g. games/voidrunner-authoritative) is exactly this
# shape. Wrapped in `if`/`else` rather than run bare like (a)-(c): the
# thing under test here is specifically whether the script's own exit
# status is 0, and this test script itself runs under `set -e`, so it must
# not rely on that being true to keep going.
# ---------------------------------------------------------------------------
COPY_D="$SCRATCH_ROOT/workspace-root"
mkdir -p "$COPY_D/src"
cat >"$COPY_D/Cargo.toml" <<'EOF'
[workspace]
members = ["a"]
EOF

if bash "$TEMPLATE/tools/rollout-replay.sh" "$COPY_D" >"$SCRATCH_ROOT/d-output.txt" 2>&1; then
  cat "$SCRATCH_ROOT/d-output.txt"
  grep -q "HAND EDIT.*Cargo.toml" "$SCRATCH_ROOT/d-output.txt" \
    || fail "(d) HAND EDIT output doesn't name Cargo.toml"
  pass "(d) a workspace-root Cargo.toml (no [package] name) exits 0 and prints a HAND EDIT naming Cargo.toml"
else
  cat "$SCRATCH_ROOT/d-output.txt"
  fail "(d) rollout-replay.sh exited non-zero on a workspace-root Cargo.toml (should exit 0 with a HAND EDIT)"
fi

echo "test_rollout_replay: all checks passed"
