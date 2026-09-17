#!/usr/bin/env bash
# Tests tools/rollout-replay.sh against two scratch copies (never the real
# checkouts):
#
#   (a) a copy of this template itself -> running the script must be a
#       no-op (it already has every feature file and wiring edit in place).
#   (b) a copy of games/cannonball-putt at its last PRE-ROLLOUT commit (not
#       its working tree, and no longer `main` either: the pilot has since
#       merged, so `main` is a ported game and would silently turn this into
#       a re-run test) -> the one documented hand edit
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
#   (e) a template copy whose src/game/sim.rs is an OLDER committed version
#       of the template's own sim.rs (what an already-ported game looks like
#       on a second rollout) -> the HAND EDIT must say "stale template copy"
#       and must not tell you to rename the file.
#   (f) the same shape, run with --upgrade: a template copy whose
#       src/game/sim.rs and src/bin/verify.rs are older committed versions
#       and whose src/game/replay/feeder.rs has a local edit -> the two
#       stale verbatim copies must come back byte-identical to HEAD's, the
#       locally modified one must survive untouched with a HAND EDIT naming
#       it, and nothing else in the copy may change.
#   (g) a template copy made to look like a FRESH port in the two ways the
#       wave-1 rollouts exposed, both of which failed SILENTLY:
#         * scoring.rs has lost its `impl LeaderboardScore for GameData`
#           (a `pub score: u32` and no impl, because the trait is never
#           copied by the script) -> the script must write the impl back and
#           the copy must still `cargo check --features verify`;
#         * build_web.sh has the verify.wasm COMMENT (whose prose contains
#           the literal "tools/build_verify.sh") but not the step -> the
#           script must still insert `bash tools/build_verify.sh` and its
#           `cp ... dist/verify.zip`, and print no HAND EDIT about it. This
#           is the bug that shipped games whose verify_url 404s.
#   (h) the --upgrade branch for .github/workflows/ci.yml: a template copy
#       whose Node step is the OLD single-fixture shape (taken from the
#       template's own history) -> --upgrade must replace it with the
#       wasm-gate + informational-native pair; and the same copy with that
#       step RENAMED -> a HAND EDIT, with the file left untouched.
#
# Usage: tools/test_rollout_replay.sh [--no-game]
#
#   --no-game skips part (b), the only part that needs a sibling
#   games/cannonball-putt checkout (and its pre-rollout history). CI runs
#   the suite that way, since the template's own workflow has no access to
#   a private sibling repo; run it WITHOUT the flag locally, where the
#   checkout exists, because (b) is the only end-to-end "a real unported
#   game compiles after the rollout" coverage there is.
set -euo pipefail
TEMPLATE="$(cd "$(dirname "$0")/.." && pwd)"
NO_GAME=0
while [ $# -gt 0 ]; do
  case "$1" in
    --no-game) NO_GAME=1 ;;
    *)
      echo "unknown option: $1 (usage: $0 [--no-game])" >&2
      exit 2
      ;;
  esac
  shift
done
if [ "$NO_GAME" -eq 0 ]; then
  GAMES_ROOT="$(cd "$TEMPLATE/../../games" && pwd)"
  CANNONBALL="$GAMES_ROOT/cannonball-putt"
fi

SCRATCH_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/rollout-replay-test.XXXXXX")"
trap 'rm -rf "$SCRATCH_ROOT"' EXIT

# A stable (not auto-deleted) target dir so repeat runs of this test don't
# recompile the whole Bevy dependency graph from scratch every time. Shared by
# every part that runs cargo ((b) and (g)).
CARGO_TARGET_DIR="${TMPDIR:-/tmp}/rollout-replay-test-cargo-target"
export CARGO_TARGET_DIR

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
# (b) Cannonball Putt copy. Skipped under --no-game (CI): it is the only part
# that needs a sibling games/ checkout.
# ---------------------------------------------------------------------------
if [ "$NO_GAME" -eq 1 ]; then
  echo "SKIP: (b) cannonball-putt rollout (--no-game); parts (a), (c)-(h) still run"
else
[ -d "$CANNONBALL" ] || fail "games/cannonball-putt not found at $CANNONBALL"

# The pilot merged, so `main` is now a ported game: snapshotting it would
# quietly convert this part from "a first rollout onto a virgin game" into a
# re-run, and the renames below would clobber the game's real shot_sim.rs.
# Derive the last commit before the rollout instead — the parent of whichever
# commit first added src/game/replay/mod.rs — and assert it really is
# pre-rollout before using it.
ADDED_REPLAY="$(git -C "$CANNONBALL" rev-list main -- src/game/replay/mod.rs | tail -1)"
[ -n "$ADDED_REPLAY" ] || fail "(b) setup: no commit on cannonball-putt's main adds src/game/replay/mod.rs"
PRE_ROLLOUT="$ADDED_REPLAY^"
for path in src/game/replay/mod.rs src/bin/verify.rs; do
  if git -C "$CANNONBALL" cat-file -e "$PRE_ROLLOUT:$path" 2>/dev/null; then
    fail "(b) setup: $PRE_ROLLOUT still has $path, so it is not a pre-rollout commit"
  fi
done

COPY_B="$SCRATCH_ROOT/cannonball-putt-copy"
snapshot_git_ref_as_baseline "$CANNONBALL" "$PRE_ROLLOUT" "$COPY_B"

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

# --- The verify.zip step, and why this assertion exists ---------------------
#
# The script makes both build_web.sh edits in ONE perl pass. The first
# inserts the comment block asserted just above, whose prose contains the
# literal text "tools/build_verify.sh". The second used to be guarded by
# `if ($c !~ /tools\/build_verify\.sh/)` against the ALREADY-EDITED $c — so
# the comment the same pass had just written satisfied the guard, the real
# step was skipped, and NO HAND EDIT printed. The build and the Vercel deploy
# both succeeded; the only symptom was in production, where
# properties.verify_url 404s and every honest run comes back "No verifier for
# this game build yet". Cannonball Putt escaped it only by being rolled out
# before the comment existed.
#
# So assert on the outcome, on this same fresh-rollout copy: both edits
# present, and the `cp` on the line after the `bash`.
grep -qx "bash tools/build_verify.sh" "$COPY_B/build_web.sh" \
  || fail "(b) build_web.sh never gained the 'bash tools/build_verify.sh' step (the deploy would ship no dist/verify.zip and verify_url would 404)"
grep -qx "cp dist-verify.zip dist/verify.zip" "$COPY_B/build_web.sh" \
  || fail "(b) build_web.sh runs build_verify.sh but never copies dist-verify.zip to dist/verify.zip"
grep -A1 -x "bash tools/build_verify.sh" "$COPY_B/build_web.sh" \
  | grep -qx "cp dist-verify.zip dist/verify.zip" \
  || fail "(b) build_web.sh's 'cp dist-verify.zip dist/verify.zip' does not follow 'bash tools/build_verify.sh'"
pass "(b) build_web.sh gained BOTH the verify.wasm comment and the build_verify.sh + cp verify.zip step"

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
fi  # end of part (b)

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

# ---------------------------------------------------------------------------
# (e) Stale template sim.rs, on a fresh template copy.
#
# Re-running the rollout on a game that was ported months ago leaves it with
# a sim.rs that is the template's -- just an older one. The pre-flight used
# to lump that in with "your game has its own sim.rs called sim.rs" and tell
# you to rename it, which would be actively wrong: the fix is to re-copy the
# current one and re-apply the game's own wiring. Reproduce it by putting an
# earlier committed version of the template's own sim.rs into the copy.
# ---------------------------------------------------------------------------
COPY_E="$SCRATCH_ROOT/template-copy-e"
snapshot_as_git_baseline "$TEMPLATE" "$COPY_E"

OLD_SIM_SHA="$(git -C "$TEMPLATE" log --format=%H -- src/game/sim.rs | sed -n 2p)"
[ -n "$OLD_SIM_SHA" ] || fail "(e) setup: the template has only one commit touching src/game/sim.rs"
git -C "$TEMPLATE" show "$OLD_SIM_SHA:src/game/sim.rs" >"$COPY_E/src/game/sim.rs"

bash "$TEMPLATE/tools/rollout-replay.sh" "$COPY_E" >"$SCRATCH_ROOT/e-output.txt" 2>&1
cat "$SCRATCH_ROOT/e-output.txt"

grep -q "stale template copy" "$SCRATCH_ROOT/e-output.txt" \
  || fail "(e) HAND EDIT output doesn't identify the sim.rs as a stale template copy"
if grep "sim\.rs" "$SCRATCH_ROOT/e-output.txt" | grep -q "rename"; then
  fail "(e) HAND EDIT output still tells you to rename a stale template sim.rs"
fi
pass "(e) an older committed template sim.rs is reported as a stale copy, not as a name clash"

# ---------------------------------------------------------------------------
# (f) --upgrade on a fresh template copy that has fallen behind.
#
# The mode exists for games ported months ago, and it has to tell two cases
# apart that look identical to `cmp`: a file nobody touched in the game that
# the template has since moved on from (refresh it), and a file the game
# edited (leave it alone and say so). Reproduce both in one copy: sim.rs and
# src/bin/verify.rs are restored to the FIRST committed version of each --
# what a game ported at that commit still has -- while feeder.rs gets a
# local edit that matches no template version at all.
# ---------------------------------------------------------------------------
COPY_F="$SCRATCH_ROOT/template-copy-f"
snapshot_as_git_baseline "$TEMPLATE" "$COPY_F"

# `log` is newest-first, so the last line is the commit that added the file.
first_commit_for() {
  git -C "$TEMPLATE" log --format=%H -- "$1" | tail -1
}
OLD_SIM="$(first_commit_for src/game/sim.rs)"
OLD_VERIFY="$(first_commit_for src/bin/verify.rs)"
[ -n "$OLD_SIM" ] || fail "(f) setup: no commit found for src/game/sim.rs"
[ -n "$OLD_VERIFY" ] || fail "(f) setup: no commit found for src/bin/verify.rs"
git -C "$TEMPLATE" show "$OLD_SIM:src/game/sim.rs" >"$COPY_F/src/game/sim.rs"
git -C "$TEMPLATE" show "$OLD_VERIFY:src/bin/verify.rs" >"$COPY_F/src/bin/verify.rs"
if cmp -s "$TEMPLATE/src/game/sim.rs" "$COPY_F/src/game/sim.rs"; then
  fail "(f) setup: the first committed sim.rs is identical to HEAD's; nothing to refresh"
fi
if cmp -s "$TEMPLATE/src/bin/verify.rs" "$COPY_F/src/bin/verify.rs"; then
  fail "(f) setup: the first committed verify.rs is identical to HEAD's; nothing to refresh"
fi

# The locally modified one: a comment no template version ever had.
FEEDER_F="$COPY_F/src/game/replay/feeder.rs"
printf '\n// Local edit this game made; --upgrade must not throw it away.\n' >>"$FEEDER_F"
cp "$FEEDER_F" "$SCRATCH_ROOT/feeder-before.rs"

bash "$TEMPLATE/tools/rollout-replay.sh" --upgrade "$COPY_F" >"$SCRATCH_ROOT/f-output.txt" 2>&1
cat "$SCRATCH_ROOT/f-output.txt"

cmp -s "$TEMPLATE/src/game/sim.rs" "$COPY_F/src/game/sim.rs" \
  || fail "(f) --upgrade did not restore src/game/sim.rs to the template's current version"
cmp -s "$TEMPLATE/src/bin/verify.rs" "$COPY_F/src/bin/verify.rs" \
  || fail "(f) --upgrade did not restore src/bin/verify.rs to the template's current version"
pass "(f) --upgrade overwrote the two stale verbatim copies with HEAD's versions"

cmp -s "$SCRATCH_ROOT/feeder-before.rs" "$FEEDER_F" \
  || fail "(f) --upgrade overwrote a locally modified src/game/replay/feeder.rs"
grep -q "HAND EDIT: src/game/replay/feeder.rs: locally modified" "$SCRATCH_ROOT/f-output.txt" \
  || fail "(f) no 'locally modified' HAND EDIT for the edited feeder.rs"
pass "(f) the locally modified feeder.rs survived untouched and got a HAND EDIT naming it"

# The summary has to name what it rewrote: an upgrade that silently changes
# files is worse than one that refuses to.
grep -q "src/game/sim.rs" "$SCRATCH_ROOT/f-output.txt" \
  || fail "(f) the --upgrade summary doesn't name src/game/sim.rs"
grep -q "src/bin/verify.rs" "$SCRATCH_ROOT/f-output.txt" \
  || fail "(f) the --upgrade summary doesn't name src/bin/verify.rs"
pass "(f) the summary names both refreshed files"

# Nothing else moved: sim.rs and verify.rs are back to the baseline commit's
# content, so the only dirty path left must be the one we edited ourselves.
DIRTY_F="$(cd "$COPY_F" && git status --porcelain -- . ':!Cargo.lock' | awk '{print $2}')"
if [ "$DIRTY_F" != "src/game/replay/feeder.rs" ]; then
  echo "$DIRTY_F" >&2
  fail "(f) --upgrade touched files beyond the stale copies and the edited feeder.rs"
fi
pass "(f) --upgrade made no collateral edits (git status shows only the deliberately edited file)"


# ---------------------------------------------------------------------------
# (g) LeaderboardScore on a FRESH port.
#
# `impl LeaderboardScore for GameData` lives in each game's OWN
# src/game/scoring.rs — it is not one of the files rollout-replay.sh copies —
# while sim.rs, replay/mod.rs, replay/recorder.rs and host.rs (all copied or
# edited by the script) `use` the trait. So a game without one does not
# compile after the rollout. The old pre-flight only spoke up when there was
# no `score` FIELD at all, which is the minority case; the common shape (a
# `pub score: u32` and no impl) got no warning and a red build. Reproduce it
# on a template copy by deleting the impl, and assert the script writes it
# back and the result still compiles.
# ---------------------------------------------------------------------------
COPY_G="$SCRATCH_ROOT/template-copy-g"
snapshot_as_git_baseline "$TEMPLATE" "$COPY_G"

python3 - "$COPY_G" <<'PYEOF'
import sys
path = sys.argv[1] + "/src/game/scoring.rs"
src = open(path).read()
impl = """impl LeaderboardScore for GameData {
    fn leaderboard_score(&self) -> u32 {
        self.score
    }
}
"""
assert src.count(impl) == 1, "part (g) setup: the template's LeaderboardScore impl is not in the shape this test expects"
open(path, "w").write(src.replace(impl, "", 1))
PYEOF

grep -q 'impl LeaderboardScore' "$COPY_G/src/game/scoring.rs" \
  && fail "(g) setup: the impl is still there after deleting it"
grep -qE '^\s*pub score: u32,' "$COPY_G/src/game/scoring.rs" \
  || fail "(g) setup: the copy has no 'pub score: u32' field, so this isn't the case under test"

# --- and, on the same copy, the build_web.sh verify.zip insertion ---------
#
# THE production bug, reproduced without needing a sibling game checkout (so
# it runs under --no-game, i.e. in CI). The script makes two build_web.sh
# edits in one perl pass: a comment block whose prose contains the literal
# text "tools/build_verify.sh", and the real step. The real step used to be
# guarded by `if ($c !~ /tools\/build_verify\.sh/)` against the
# already-edited $c, so the comment the same pass had just inserted satisfied
# the guard: the step was skipped and NO HAND EDIT printed. Build green,
# deploy green, and in production every `verify_url` 404s with "No verifier
# for this game build yet".
#
# The precondition is precisely "the comment is there and the step is not",
# so strip only the step from this copy and leave the comment alone.
python3 - "$COPY_G" <<'PYEOF'
import sys
path = sys.argv[1] + "/build_web.sh"
src = open(path).read()
step = """
# Build the headless replay verifier and publish it alongside the game bundle
# as dist/verify.zip — the site fetches it from properties.verify_url. Run
# after this script's own wasm-bindgen/wasm-opt steps (and after the `find`
# above, which already excludes verify.wasm by name) so there's no ambiguity
# about which .wasm is the game bundle.
bash tools/build_verify.sh
cp dist-verify.zip dist/verify.zip
"""
assert src.count(step) == 1, "part (g) setup: build_web.sh's verify.zip step is not in the shape this test expects"
open(path, "w").write(src.replace(step, "", 1))
PYEOF

grep -q "verify.wasm is excluded by name" "$COPY_G/build_web.sh" \
  || fail "(g) setup: the comment block containing the literal 'tools/build_verify.sh' is gone, so the bug's precondition isn't reproduced"
if grep -qx "bash tools/build_verify.sh" "$COPY_G/build_web.sh"; then
  fail "(g) setup: the verify.zip step is still in build_web.sh after stripping it"
fi

bash "$TEMPLATE/tools/rollout-replay.sh" "$COPY_G" >"$SCRATCH_ROOT/g-output.txt" 2>&1
cat "$SCRATCH_ROOT/g-output.txt"

grep -qx "bash tools/build_verify.sh" "$COPY_G/build_web.sh" \
  || fail "(g) the script did not re-insert 'bash tools/build_verify.sh' into build_web.sh (the deploy would ship no dist/verify.zip and verify_url would 404)"
grep -A1 -x "bash tools/build_verify.sh" "$COPY_G/build_web.sh" \
  | grep -qx "cp dist-verify.zip dist/verify.zip" \
  || fail "(g) 'cp dist-verify.zip dist/verify.zip' does not follow 'bash tools/build_verify.sh' in build_web.sh"
if grep -q "HAND EDIT.*build_web" "$SCRATCH_ROOT/g-output.txt"; then
  fail "(g) the script inserted the verify.zip step but still printed a HAND EDIT about build_web.sh"
fi
pass "(g) build_web.sh's verify.zip step is inserted even though the comment above it already names tools/build_verify.sh"

grep -q 'impl LeaderboardScore for GameData' "$COPY_G/src/game/scoring.rs" \
  || fail "(g) the script did not write 'impl LeaderboardScore for GameData' back into scoring.rs"
grep -q "added 'impl LeaderboardScore for GameData'" "$SCRATCH_ROOT/g-output.txt" \
  || fail "(g) the script wrote the impl but said nothing about it"
if grep -q "HAND EDIT.*LeaderboardScore" "$SCRATCH_ROOT/g-output.txt"; then
  fail "(g) the script both wrote the impl and asked for it by hand"
fi
pass "(g) a missing LeaderboardScore impl over a 'pub score: u32' field is written automatically"

if (cd "$COPY_G" && cargo check --features verify >"$SCRATCH_ROOT/g-check.txt" 2>&1); then
  pass "(g) cargo check --features verify succeeds on the restored impl"
else
  tail -40 "$SCRATCH_ROOT/g-check.txt" >&2
  fail "(g) cargo check --features verify failed after the script restored LeaderboardScore"
fi

# Idempotent: a second run must not append a second impl.
bash "$TEMPLATE/tools/rollout-replay.sh" "$COPY_G" >"$SCRATCH_ROOT/g-output-2.txt" 2>&1
IMPL_COUNT="$(grep -c 'impl LeaderboardScore for GameData' "$COPY_G/src/game/scoring.rs")"
[ "$IMPL_COUNT" = "1" ] \
  || fail "(g) a second run produced $IMPL_COUNT LeaderboardScore impls, not 1"
pass "(g) a second run is a no-op on scoring.rs"

# ---------------------------------------------------------------------------
# (h) --upgrade's .github/workflows/ci.yml branch.
#
# A game ported before the wasm-recorded fixture existed has a single Node
# step reading the NATIVE fixture. --upgrade replaces that step (and the
# comment block above it) with the wasm gate + informational native pair.
# Both halves matter: it must fire on the shape the template itself used to
# have, and it must REFUSE — with a HAND EDIT, and without touching the file
# — on anything else, because rewriting a workflow step it doesn't recognize
# is exactly the guess this script exists not to make.
# ---------------------------------------------------------------------------
OLD_CI_SHA="$(git -C "$TEMPLATE" rev-list HEAD -- .github/workflows/ci.yml \
  | while read -r h; do
      if git -C "$TEMPLATE" show "$h:.github/workflows/ci.yml" 2>/dev/null \
        | grep -q 'Verify the committed fixture under Node'; then
        echo "$h"
        break
      fi
    done)"
[ -n "$OLD_CI_SHA" ] \
  || fail "(h) setup: no commit in the template's history has the single-fixture 'Verify the committed fixture under Node' step"

COPY_H="$SCRATCH_ROOT/template-copy-h"
snapshot_as_git_baseline "$TEMPLATE" "$COPY_H"
git -C "$TEMPLATE" show "$OLD_CI_SHA:.github/workflows/ci.yml" >"$COPY_H/.github/workflows/ci.yml"
grep -q 'selftest-wasm.gxr' "$COPY_H/.github/workflows/ci.yml" \
  && fail "(h) setup: the old ci.yml already mentions selftest-wasm.gxr, so the branch under test can't fire"

bash "$TEMPLATE/tools/rollout-replay.sh" --upgrade "$COPY_H" >"$SCRATCH_ROOT/h-output.txt" 2>&1
cat "$SCRATCH_ROOT/h-output.txt"

grep -q 'Verify the wasm-recorded fixture under Node' "$COPY_H/.github/workflows/ci.yml" \
  || fail "(h) --upgrade did not add the wasm-fixture gate step to ci.yml"
grep -q 'Cross-check the native fixture under Node (informational)' "$COPY_H/.github/workflows/ci.yml" \
  || fail "(h) --upgrade did not add the informational native cross-check step to ci.yml"
if grep -q 'Verify the committed fixture under Node' "$COPY_H/.github/workflows/ci.yml"; then
  fail "(h) --upgrade left the old single-fixture step behind alongside the new pair"
fi
if grep -q "HAND EDIT.*ci.yml" "$SCRATCH_ROOT/h-output.txt"; then
  fail "(h) --upgrade replaced the step but still printed a HAND EDIT about ci.yml"
fi
pass "(h) --upgrade replaces the old single-fixture Node step with the wasm gate + informational pair"

# The refusal half: the same old ci.yml with the step RENAMED.
COPY_H2="$SCRATCH_ROOT/template-copy-h2"
snapshot_as_git_baseline "$TEMPLATE" "$COPY_H2"
git -C "$TEMPLATE" show "$OLD_CI_SHA:.github/workflows/ci.yml" \
  | sed 's/- name: Verify the committed fixture under Node/- name: Replay fixture check (renamed by hand)/' \
  >"$COPY_H2/.github/workflows/ci.yml"
grep -q 'Replay fixture check (renamed by hand)' "$COPY_H2/.github/workflows/ci.yml" \
  || fail "(h) setup: the rename did not take"
cp "$COPY_H2/.github/workflows/ci.yml" "$SCRATCH_ROOT/h2-ci-before.yml"

bash "$TEMPLATE/tools/rollout-replay.sh" --upgrade "$COPY_H2" >"$SCRATCH_ROOT/h2-output.txt" 2>&1
cat "$SCRATCH_ROOT/h2-output.txt"

grep -q "HAND EDIT.*ci.yml" "$SCRATCH_ROOT/h2-output.txt" \
  || fail "(h) an unrecognized Node step shape printed no HAND EDIT about ci.yml"
cmp -s "$SCRATCH_ROOT/h2-ci-before.yml" "$COPY_H2/.github/workflows/ci.yml" \
  || fail "(h) --upgrade edited a ci.yml whose Node step it does not recognize"
pass "(h) an unrecognized Node step gets a HAND EDIT and the workflow file is left untouched"

echo "test_rollout_replay: all checks passed"
