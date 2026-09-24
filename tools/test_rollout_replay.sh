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
#       arm, and build_web.sh's verify.wasm comment block. tests/
#       archetype_order.rs must arrive crate-renamed and flagged as needing
#       adaptation (this game has no `Player`, so the copy would not compile).
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
#   (i) tests/archetype_order.rs, which references the template's own
#       `Player` and so cannot be dropped into an arbitrary game: --upgrade
#       must NOT create it when absent (HAND EDIT + summary line instead), a
#       first rollout must still write it, and an adapted copy must survive
#       both modes untouched and unmentioned.
#   (h) the --upgrade branch for .github/workflows/ci.yml: a template copy
#       whose Node step is the OLD single-fixture shape (taken from the
#       template's own history) -> --upgrade must replace it with the
#       wasm-gate + informational-native pair, and must add
#       CARGO_PROFILE_DEV_DEBUG: "0" to the check job's env exactly once
#       (without it a fourth Bevy test binary's debug info exhausts the
#       runner disk and CI dies inside rust-lld with "collect2: ld
#       terminated with signal 7"); and the same copy with that step RENAMED
#       -> a HAND EDIT, with nothing but the env line added.
#   (k) sim.rs is a verbatim copy, so --upgrade hands an already-ported game
#       a begin_run that takes ResMut<SpawnCounter>. A template copy with
#       the .init_resource::<sim::SpawnCounter>() line deleted -> --upgrade
#       must put it back (exactly once, and back in the builder chain), or
#       OnEnter(Playing) dies with Bevy's nameless "Resource does not exist".
#   (j) the workflow drift advisory: --upgrade on a copy whose ci.yml has one
#       changed line must name the file, list the differing lines and print
#       a reproducible diff command; on a copy whose workflows match the
#       template's it must be silent.
#   (l) the autopilot ordering rewrite is not pinned to the template's path:
#       a copy whose autopilot lives at src/autopilot.rs (Pack The Ripper's
#       layout, where the path-pinned version did nothing and said nothing)
#       must be rewritten, and so must one at the old src/game/autopilot.rs.
#       A third file that writes VirtualInput and is still ordered before
#       collect_input must draw a HAND EDIT rather than a silent skip.
#   (m) the src/main.rs module-block rewrite must never let a `#[cfg(...)]`
#       from a `mod` line it removes attach itself to the generated import:
#       a main.rs with `#[cfg(any(feature = .., feature = ..))] mod bot;`
#       beside plain mods (the nested parens are what defeated the old
#       classifier) must still `cargo check` in the default build, with the
#       plain imports unconditional and `bot` on its own cfg-gated `use`.
#   (n) .github/workflows/ci.yml's Test step becomes `cargo test
#       --all-targets --all-features`, matching the template's own and the
#       gate the checklist documents: on a first rollout, idempotently, and
#       with a HAND EDIT when the step is not recognisable.
#   (o) the fixed-timestep pin: a copy with no `Time::<Fixed>` insert in
#       src/game/mod.rs or src/main.rs must draw a HAND EDIT (Bevy's default
#       64 Hz is rejected by Replay::decode as BadTickRate), and a copy that
#       keeps it in main.rs instead must stay silent.
#   (p) tests/windowed_shape.rs: written when absent (and named in the
#       summary, because it is a new test that can legitimately go red), but
#       refused with a HAND EDIT when the game's selftest script is not
#       `pub`, or when the game has no assets::AssetsPlugin for the copy to
#       build — either way the copy would not compile.
#       An adapted copy that predates the second-run row is told, and one
#       that has it but predates Bot::Holder is told that row is blind to
#       hold-dependent carried state; both notices are silent once the rows
#       are in.
#   (q) the presentation-resource advisory: a sim file reading a fade
#       resource must be named, while sim::end_run's own callers,
#       toggle_pause and the autopilot must not — and the pause toggler is
#       recognised by its pause_just_pressed edge, so a game that renamed it
#       is still silent.
#   (r) the Cutter tests step: a game that ships tools/test_cut_clips.py but
#       whose ci.yml never runs it gets the step inserted after the Test
#       step (first rollout and --upgrade, idempotent), while a game without
#       the cutter file must not be handed a step that would fail.
#   (s) the transcendental advisory: a sim file calling sin/cos/exp/powf/
#       atan2 must be named (Gulper folded an atan2 and got three checksums
#       for one fixture from three libms), while the audio synth palette
#       every game inherits from the template must not, or the advisory is
#       noise on every game for ever.
#   (u) the run-clock wiring: --upgrade on an already-ported copy whose
#       mod.rs has neither .init_resource::<sim::RunClock>() nor
#       sim::sync_run_clock must put BOTH back (exactly once, in the builder
#       chain and second in the SimSet tuple). The resource is the loud
#       failure (begin_run takes ResMut<RunClock>, so OnEnter(Playing) dies
#       with Bevy's nameless "Resource does not exist"); the system is the
#       SILENT one (RunClock stays 0 for the whole run).
#   (t) the app-clock advisory: a sim file reading Time::elapsed_secs() must
#       be named with a pointer to rule 7 (Dive Rise shipped that in nine sim
#       systems and lost a production run to it), while the template's own
#       benign wall-clock hits — the dev recorder and the replay header stamp
#       — must not be, and an `allow-app-clock` marker must silence it.
#   (v) the sim-Local advisory: a file whose systems SimSet runs must be
#       named when it keeps state in a `Local<_>` (Grand Theft Auto-Reply's
#       cursor auto-repeat did, and a second run in one browser session
#       began with run 1's last held direction), while the dev recorder's
#       dedupe boxes under `src/game/record/` — which no SimSet system runs
#       — must not be, and an `allow-local` marker must silence it.
#   (w) host.rs's Mute arm: the idempotency guard is "is the assignment
#       already guarded", not "did this script write the guard". A copy
#       carrying a hand-written `if let Some(..) = global_volume.as_mut()`
#       must come back byte-identical under --upgrade (wrapping it a second
#       time does not compile — grand-theft-otto#6); an unguarded one must
#       still gain `as_deref_mut()`; an ambiguous one (two assignments, no
#       guard) must draw a HAND EDIT and keep its file.
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

# The probe is a copied file on the template's own checkout too (SNAKE ==
# gamebient_game, so the rename is a no-op); the no-op assertion above covers
# it, but say so explicitly since it is the newest copied file.
cmp -s "$TEMPLATE/tests/archetype_order.rs" "$COPY_A/tests/archetype_order.rs" \
  || fail "(a) tests/archetype_order.rs was modified on the template's own checkout"
if grep -q "HAND EDIT: tests/archetype_order.rs" "$SCRATCH_ROOT/a-output.txt"; then
  fail "(a) the template's own checkout was told to adapt tests/archetype_order.rs"
fi
pass "(a) tests/archetype_order.rs is untouched and unflagged on the template itself"

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
grep -q "init_resource::<sim::SpawnCounter>" "$SCRATCH_ROOT/b-output.txt" \
  || fail "(b) the GamePlugin::build HAND EDIT doesn't name .init_resource::<sim::SpawnCounter>(), which sim::begin_run now requires"
pass "(b) HAND EDIT output names LeaderboardScore, the GamePlugin::build port and the SpawnCounter registration, nothing about sim.rs"

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

# The archetype-order probe is copied in, crate-renamed, and — because the
# template's version queries the template's own `Player` sim entity, which
# this game does not have — announced as a HAND EDIT rather than left to be
# discovered as a compile error at `cargo test` time.
[ -f "$COPY_B/tests/archetype_order.rs" ] \
  || fail "(b) tests/archetype_order.rs was not copied in"
grep -q 'use cannonball_putt::' "$COPY_B/tests/archetype_order.rs" \
  || fail "(b) tests/archetype_order.rs was copied without the gamebient_game:: -> cannonball_putt:: rename"
grep -q "HAND EDIT: tests/archetype_order.rs" "$SCRATCH_ROOT/b-output.txt" \
  || fail "(b) no HAND EDIT telling the porter to adapt tests/archetype_order.rs (the copy does not compile against a game with no Player)"
pass "(b) tests/archetype_order.rs is copied, crate-renamed, and flagged as needing adaptation"

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

# The impl must land BEFORE a trailing `#[cfg(test)]` module, or clippy's
# `items_after_test_module` fails the game's -D warnings CI (Attic Excavator).
IMPL_LINE="$(grep -n 'impl LeaderboardScore for GameData' "$COPY_G/src/game/scoring.rs" | head -1 | cut -d: -f1)"
TEST_LINE="$(grep -n '^#\[cfg(test)\]' "$COPY_G/src/game/scoring.rs" | head -1 | cut -d: -f1)"
[ -n "$TEST_LINE" ] \
  || fail "(g) the template's scoring.rs no longer ends with a #[cfg(test)] module, so this check no longer exercises the insert-before-tests path; give the copy one"
[ "$IMPL_LINE" -lt "$TEST_LINE" ] \
  || fail "(g) the impl was appended after #[cfg(test)] (line $IMPL_LINE vs $TEST_LINE); clippy items_after_test_module would fail the game's CI"
pass "(g) the impl lands before the trailing #[cfg(test)] module"

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

# --- and the same guard against a QUALIFIED impl ---------------------------
#
# `impl scoring::LeaderboardScore for GameData` and
# `impl crate::game::scoring::LeaderboardScore for GameData` are both real
# shapes, and an impl need not live in scoring.rs at all. A guard of
# `grep -q 'impl LeaderboardScore'` on scoring.rs misses every one of those
# and appends a duplicate — E0119, a worse failure than the missing impl this
# automation exists to prevent.
COPY_G2="$SCRATCH_ROOT/template-copy-g2"
snapshot_as_git_baseline "$TEMPLATE" "$COPY_G2"

python3 - "$COPY_G2" <<'PYEOF'
import sys

root = sys.argv[1]
path = root + "/src/game/scoring.rs"
src = open(path).read()
impl = (
    "impl LeaderboardScore for GameData {\n"
    "    fn leaderboard_score(&self) -> u32 {\n"
    "        self.score\n"
    "    }\n"
    "}\n"
)
assert src.count(impl) == 1, "part (g2) setup: the template's impl is not in the shape this test expects"
# Remove it from scoring.rs and re-add it, QUALIFIED, from another module.
open(path, "w").write(src.replace(impl, "", 1))
open(root + "/src/game/leaderboard_impl.rs", "w").write(
    "//! The impl, deliberately spelled with a qualified path and out of\n"
    "//! scoring.rs -- the shape the pre-flight used to miss.\n"
    "\n"
    "use super::scoring::GameData;\n"
    "\n"
    "impl crate::game::scoring::LeaderboardScore for GameData {\n"
    "    fn leaderboard_score(&self) -> u32 {\n"
    "        self.score\n"
    "    }\n"
    "}\n"
)
mod_path = root + "/src/game/mod.rs"
mod_src = open(mod_path).read()
assert "pub mod scoring;" in mod_src
open(mod_path, "w").write(
    mod_src.replace("pub mod scoring;", "pub mod leaderboard_impl;\npub mod scoring;", 1)
)
PYEOF

bash "$TEMPLATE/tools/rollout-replay.sh" "$COPY_G2" >"$SCRATCH_ROOT/g2-output.txt" 2>&1
cat "$SCRATCH_ROOT/g2-output.txt"

G2_IMPLS="$(grep -rh 'LeaderboardScore for GameData' "$COPY_G2/src" --include='*.rs' | grep -c .)"
[ "$G2_IMPLS" = "1" ] \
  || fail "(g) a qualified impl elsewhere in src/ was not recognised: now $G2_IMPLS impls, expected 1 (a duplicate is E0119)"
if grep -q "added 'impl LeaderboardScore for GameData'" "$SCRATCH_ROOT/g2-output.txt"; then
  fail "(g) the script appended an impl over an existing qualified one"
fi
if grep -q "HAND EDIT.*LeaderboardScore" "$SCRATCH_ROOT/g2-output.txt"; then
  fail "(g) the script asked for a LeaderboardScore impl that already exists, qualified"
fi
pass "(g) a qualified 'impl crate::game::scoring::LeaderboardScore' outside scoring.rs is recognised, not duplicated"

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
# The drift advisory (part (j)) is expected here and is excluded: an old
# ci.yml taken from the template's own history differs from HEAD's in far
# more than the Node step, and saying so is the advisory's job. What must
# not appear is a HAND EDIT asking a human to do what --upgrade just did.
if grep "HAND EDIT.*ci.yml" "$SCRATCH_ROOT/h-output.txt" | grep -vq "(advisory)"; then
  grep "HAND EDIT.*ci.yml" "$SCRATCH_ROOT/h-output.txt" >&2
  fail "(h) --upgrade replaced the step but still printed a HAND EDIT about ci.yml"
fi
pass "(h) --upgrade replaces the old single-fixture Node step with the wasm gate + informational pair"

# ...and, on the same copy, the CI disk setting. A game whose ci.yml predates
# tests/archetype_order.rs has four Bevy test binaries' worth of debug info
# and no setting to stop it, and the failure that produces ("collect2: ld
# terminated with signal 7" inside rust-lld) reads as a compiler bug rather
# than as the out-of-disk it is. The rollout adds it, once.
grep -q 'CARGO_PROFILE_DEV_DEBUG: "0"' "$COPY_H/.github/workflows/ci.yml" \
  || fail "(h) the rollout did not add CARGO_PROFILE_DEV_DEBUG to the check job's env"
DEBUG_ENV_N="$(grep -c 'CARGO_PROFILE_DEV_DEBUG' "$COPY_H/.github/workflows/ci.yml")"
[ "$DEBUG_ENV_N" -eq 1 ] \
  || fail "(h) CARGO_PROFILE_DEV_DEBUG appears $DEBUG_ENV_N times in ci.yml; the insertion is not idempotent"
bash "$TEMPLATE/tools/rollout-replay.sh" --upgrade "$COPY_H" >"$SCRATCH_ROOT/h-output-2.txt" 2>&1
DEBUG_ENV_N2="$(grep -c 'CARGO_PROFILE_DEV_DEBUG' "$COPY_H/.github/workflows/ci.yml")"
[ "$DEBUG_ENV_N2" -eq 1 ] \
  || fail "(h) a second --upgrade added CARGO_PROFILE_DEV_DEBUG again ($DEBUG_ENV_N2 occurrences)"
pass "(h) the rollout adds CARGO_PROFILE_DEV_DEBUG to a game's check job exactly once"

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

# A real HAND EDIT, not just the drift advisory (which fires on any old
# workflow and would mask a missing refusal).
grep "HAND EDIT.*ci.yml" "$SCRATCH_ROOT/h2-output.txt" | grep -vq "(advisory)" \
  || fail "(h) an unrecognized Node step printed no HAND EDIT about ci.yml beyond the drift advisory"
# The CARGO_PROFILE_DEV_DEBUG insertion is a pure addition to `env:` and is
# unrelated to the Node step it refuses to touch, so it still lands; the
# refusal is about the step, not about the file.
diff "$SCRATCH_ROOT/h2-ci-before.yml" "$COPY_H2/.github/workflows/ci.yml" \
  >"$SCRATCH_ROOT/h2-ci-diff.txt" || true
if grep -q '^<' "$SCRATCH_ROOT/h2-ci-diff.txt"; then
  cat "$SCRATCH_ROOT/h2-ci-diff.txt" >&2
  fail "(h) --upgrade removed or rewrote lines in a ci.yml whose Node step it does not recognize"
fi
if grep '^>' "$SCRATCH_ROOT/h2-ci-diff.txt" \
  | grep -v 'CARGO_PROFILE_DEV_DEBUG' | grep -vq '^>[[:space:]]*#'; then
  cat "$SCRATCH_ROOT/h2-ci-diff.txt" >&2
  fail "(h) --upgrade added something other than the CARGO_PROFILE_DEV_DEBUG env line to a ci.yml whose Node step it does not recognize"
fi
pass "(h) an unrecognized Node step gets a HAND EDIT and the workflow file is otherwise left untouched"


# ---------------------------------------------------------------------------
# (i) tests/archetype_order.rs is not a file that can be dropped into an
# arbitrary game: it hard-references the template's own
# `game::player::Player`, which only a handful of games have. Three rules,
# one part.
#
#   --upgrade + absent  -> do NOT create it. Writing it into a ported game
#                          without a `Player` turns a green checkout red at
#                          `cargo test` with nothing in the output to explain
#                          where the file came from, and --upgrade's whole
#                          promise is that it does not break a working game.
#                          cannonball-putt (the pilot, and the first
#                          --upgrade target) and grand-theft-auto-reply both
#                          lack `Player`. Ask for it and say so in the
#                          summary instead.
#   first rollout       -> still write it: the port is happening now, and a
#                          named hand edit beats no test at all.
#   adapted             -> untouched and unmentioned, in both modes.
# ---------------------------------------------------------------------------
COPY_I="$SCRATCH_ROOT/template-copy-i"
snapshot_as_git_baseline "$TEMPLATE" "$COPY_I"
rm -f "$COPY_I/tests/archetype_order.rs"
[ -e "$COPY_I/tests/archetype_order.rs" ] && fail "(i) setup: the probe is still there after deleting it"

bash "$TEMPLATE/tools/rollout-replay.sh" --upgrade "$COPY_I" >"$SCRATCH_ROOT/i-output.txt" 2>&1
cat "$SCRATCH_ROOT/i-output.txt"

if [ -e "$COPY_I/tests/archetype_order.rs" ]; then
  fail "(i) --upgrade created tests/archetype_order.rs; on a game without a Player that is an unexplained red test"
fi
grep -q "HAND EDIT: tests/archetype_order.rs: missing" "$SCRATCH_ROOT/i-output.txt" \
  || fail "(i) --upgrade left the probe absent but printed no HAND EDIT naming it"
grep -q "rollout-replay --upgrade: tests/archetype_order.rs is ABSENT" "$SCRATCH_ROOT/i-output.txt" \
  || fail "(i) the --upgrade summary does not name the absent probe"
pass "(i) --upgrade asks for a missing tests/archetype_order.rs instead of materialising a red one"

COPY_I2="$SCRATCH_ROOT/template-copy-i2"
snapshot_as_git_baseline "$TEMPLATE" "$COPY_I2"
rm -f "$COPY_I2/tests/archetype_order.rs"

bash "$TEMPLATE/tools/rollout-replay.sh" "$COPY_I2" >"$SCRATCH_ROOT/i2-output.txt" 2>&1
cat "$SCRATCH_ROOT/i2-output.txt"

[ -f "$COPY_I2/tests/archetype_order.rs" ] \
  || fail "(i) a first rollout did not write tests/archetype_order.rs"
cmp -s "$TEMPLATE/tests/archetype_order.rs" "$COPY_I2/tests/archetype_order.rs" \
  || fail "(i) the first-rollout copy of tests/archetype_order.rs is not the template's"
pass "(i) a first rollout still writes the probe"

COPY_I3="$SCRATCH_ROOT/template-copy-i3"
snapshot_as_git_baseline "$TEMPLATE" "$COPY_I3"
printf '\n// Adapted for this game: markers replaced.\n' >>"$COPY_I3/tests/archetype_order.rs"
cp "$COPY_I3/tests/archetype_order.rs" "$SCRATCH_ROOT/i3-probe-before.rs"

bash "$TEMPLATE/tools/rollout-replay.sh" --upgrade "$COPY_I3" >"$SCRATCH_ROOT/i3-output.txt" 2>&1
cat "$SCRATCH_ROOT/i3-output.txt"

cmp -s "$SCRATCH_ROOT/i3-probe-before.rs" "$COPY_I3/tests/archetype_order.rs" \
  || fail "(i) --upgrade overwrote an adapted tests/archetype_order.rs"
# No HAND EDIT and no "refreshed"/"ABSENT" summary entry about it. The
# standing policy line naming both never-refreshed files is not a nag about
# THIS game and is expected on every --upgrade, so it is excluded here.
if grep "archetype_order.rs" "$SCRATCH_ROOT/i3-output.txt" \
  | grep -vq "are refreshed only while they are still byte-identical"; then
  grep "archetype_order.rs" "$SCRATCH_ROOT/i3-output.txt" >&2
  fail "(i) --upgrade nagged about an already-adapted tests/archetype_order.rs"
fi
pass "(i) an adapted probe survives --upgrade untouched and unmentioned"


# ---------------------------------------------------------------------------
# (j) The workflow drift advisory.
#
# --upgrade reconciles a fixed set of known shapes in ci.yml/release.yml and
# is deliberately blind to everything else. That blindness is the right
# default — rewriting a workflow a game has tuned is the guess this script
# exists not to make — but silence about it is not: a pinned tool the
# template has since bumped, a job that never got updated, an apt package
# added by hand all sit there invisibly. So --upgrade diffs both workflows
# against the template's with the naming substitutions applied and says how
# far apart they are, as an advisory.
#
# Both halves matter: it must fire on a single changed line, and it must be
# silent when the workflows match, or it becomes noise nobody reads.
# ---------------------------------------------------------------------------
COPY_J="$SCRATCH_ROOT/template-copy-j"
snapshot_as_git_baseline "$TEMPLATE" "$COPY_J"
# An inert one-line change the script itself has no opinion about, so what
# the advisory reports is drift and nothing else.
sed -i.bak 's/retention-days: 14/retention-days: 7/' "$COPY_J/.github/workflows/ci.yml"
rm -f "$COPY_J/.github/workflows/ci.yml.bak"
grep -q 'retention-days: 7' "$COPY_J/.github/workflows/ci.yml" \
  || fail "(j) setup: the one-line ci.yml change did not take"

bash "$TEMPLATE/tools/rollout-replay.sh" --upgrade "$COPY_J" >"$SCRATCH_ROOT/j-output.txt" 2>&1
cat "$SCRATCH_ROOT/j-output.txt"

grep -q "HAND EDIT (advisory): .github/workflows/ci.yml differs from the template" \
  "$SCRATCH_ROOT/j-output.txt" \
  || fail "(j) a drifted ci.yml printed no drift advisory"
grep -q "retention-days: 7" "$SCRATCH_ROOT/j-output.txt" \
  || fail "(j) the advisory did not list the line that actually differs"
grep -q "review with: diff" "$SCRATCH_ROOT/j-output.txt" \
  || fail "(j) the advisory did not print a command to reproduce the diff"
# release.yml was left alone, so it must not be named.
if grep -q "HAND EDIT (advisory): .github/workflows/release.yml differs" "$SCRATCH_ROOT/j-output.txt"; then
  fail "(j) an untouched release.yml got a drift advisory"
fi
pass "(j) a one-line workflow drift prints an advisory naming the file, the lines and the diff command"

COPY_J2="$SCRATCH_ROOT/template-copy-j2"
snapshot_as_git_baseline "$TEMPLATE" "$COPY_J2"
bash "$TEMPLATE/tools/rollout-replay.sh" --upgrade "$COPY_J2" >"$SCRATCH_ROOT/j2-output.txt" 2>&1
cat "$SCRATCH_ROOT/j2-output.txt"

if grep -q "differs from the template in" "$SCRATCH_ROOT/j2-output.txt"; then
  grep "differs from the template in" "$SCRATCH_ROOT/j2-output.txt" >&2
  fail "(j) workflows identical to the template's still got a drift advisory"
fi
pass "(j) workflows that match the template are silent"


# ---------------------------------------------------------------------------
# (k) .init_resource::<sim::SpawnCounter>() on an already-ported game.
#
# sim.rs is a verbatim copy, so --upgrade hands a game ported months ago a
# `begin_run` that takes `ResMut<SpawnCounter>` — and an already-ported game
# has the GamePlugin::build HAND EDIT suppressed, so nothing would otherwise
# tell it to register the resource. The failure is Bevy's nameless
# "Parameter ... failed validation: Resource does not exist" on
# OnEnter(Playing): a green compile and a dead game. The script inserts the
# line next to the RunOver registration every ported game already has.
# ---------------------------------------------------------------------------
COPY_K="$SCRATCH_ROOT/template-copy-k"
snapshot_as_git_baseline "$TEMPLATE" "$COPY_K"
grep -v 'init_resource::<sim::SpawnCounter>' "$COPY_K/src/game/mod.rs" >"$SCRATCH_ROOT/k-mod.rs"
cp "$SCRATCH_ROOT/k-mod.rs" "$COPY_K/src/game/mod.rs"
grep -q 'init_resource::<sim::SpawnCounter>' "$COPY_K/src/game/mod.rs" \
  && fail "(k) setup: the SpawnCounter registration is still in mod.rs after deleting it"

bash "$TEMPLATE/tools/rollout-replay.sh" --upgrade "$COPY_K" >"$SCRATCH_ROOT/k-output.txt" 2>&1
cat "$SCRATCH_ROOT/k-output.txt"

grep -q 'init_resource::<sim::SpawnCounter>()' "$COPY_K/src/game/mod.rs" \
  || fail "(k) --upgrade did not add .init_resource::<sim::SpawnCounter>() to GamePlugin::build"
K_N="$(grep -c 'init_resource::<sim::SpawnCounter>' "$COPY_K/src/game/mod.rs")"
[ "$K_N" -eq 1 ] || fail "(k) the SpawnCounter registration appears $K_N times; the insertion is not idempotent"
bash "$TEMPLATE/tools/rollout-replay.sh" --upgrade "$COPY_K" >"$SCRATCH_ROOT/k-output-2.txt" 2>&1
K_N2="$(grep -c 'init_resource::<sim::SpawnCounter>' "$COPY_K/src/game/mod.rs")"
[ "$K_N2" -eq 1 ] || fail "(k) a second --upgrade added the SpawnCounter registration again ($K_N2 occurrences)"
# And the file is otherwise byte-identical to the template's, so the
# insertion landed in the builder chain rather than somewhere that merely
# greps right.
cmp -s "$TEMPLATE/src/game/mod.rs" "$COPY_K/src/game/mod.rs" \
  || fail "(k) mod.rs after the insertion is not byte-identical to the template's"
pass "(k) --upgrade restores a missing .init_resource::<sim::SpawnCounter>(), exactly once"


# ---------------------------------------------------------------------------
# (l) The autopilot ordering rewrite is not pinned to src/game/autopilot.rs.
#
# Pack The Ripper keeps its bot at src/autopilot.rs. The path-pinned version
# of this rewrite skipped it in silence, so the port shipped with
# `drive_autopilot.before(collect_input)` — which leaves the bot and
# gamebient-input's `accumulate_input` unordered, so a tap written after the
# accumulator ran never reaches a fixed tick and never reaches the replay.
# Three cases: the new location, the old one, and a VirtualInput writer that
# is not called autopilot.rs at all (advised, never rewritten).
# ---------------------------------------------------------------------------
revert_autopilot_ordering() {
  perl -pi -e 's/before\(gamebient_input::input::accumulate_input\)/before(gamebient_input::input::collect_input)/g' "$1"
  grep -q 'before(gamebient_input::input::collect_input)' "$1" \
    || fail "(l) setup: $1 still has no collect_input ordering to rewrite"
}

COPY_L="$SCRATCH_ROOT/template-copy-l"
snapshot_as_git_baseline "$TEMPLATE" "$COPY_L"
mkdir -p "$COPY_L/src"
git -C "$COPY_L" mv src/game/autopilot.rs src/autopilot.rs
perl -pi -e 's/^pub mod autopilot;\n//' "$COPY_L/src/game/mod.rs"
revert_autopilot_ordering "$COPY_L/src/autopilot.rs"
# A second VirtualInput writer under a name no path search would guess.
cat >"$COPY_L/src/demo_attractor.rs" <<'DEMO'
use bevy::prelude::*;
use gamebient_input::VirtualInput;

fn drive_demo(mut _virt: ResMut<VirtualInput>) {}

pub struct DemoPlugin;
impl Plugin for DemoPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PreUpdate,
            drive_demo.before(gamebient_input::input::collect_input),
        );
    }
}
DEMO

bash "$TEMPLATE/tools/rollout-replay.sh" "$COPY_L" >"$SCRATCH_ROOT/l-output.txt" 2>&1
cat "$SCRATCH_ROOT/l-output.txt"

grep -q 'before(gamebient_input::input::accumulate_input)' "$COPY_L/src/autopilot.rs" \
  || fail "(l) src/autopilot.rs was not reordered before accumulate_input"
if grep -q 'before(gamebient_input::input::collect_input)' "$COPY_L/src/autopilot.rs"; then
  fail "(l) src/autopilot.rs still orders drive_autopilot before collect_input"
fi
grep -q "rollout-replay: src/autopilot.rs: reordered" "$SCRATCH_ROOT/l-output.txt" \
  || fail "(l) the rewrite of src/autopilot.rs was not reported"
grep -q "HAND EDIT: src/autopilot.rs: writes GameData fields directly" "$SCRATCH_ROOT/l-output.txt" \
  || fail "(l) the GameData-writes HAND EDIT is still pinned to src/game/autopilot.rs"
pass "(l) an autopilot at src/autopilot.rs is found, reordered and reported"

# The advisory for the file the path search cannot find, and it must NOT have
# been rewritten: outside an autopilot the script cannot know the system is
# meant to feed the tick path.
grep -q "HAND EDIT: src/demo_attractor.rs: writes VirtualInput but is still ordered" "$SCRATCH_ROOT/l-output.txt" \
  || fail "(l) no HAND EDIT for the VirtualInput writer still ordered before collect_input"
grep -q 'before(gamebient_input::input::collect_input)' "$COPY_L/src/demo_attractor.rs" \
  || fail "(l) src/demo_attractor.rs was rewritten; only autopilot.rs files are rewritten automatically"
pass "(l) a non-autopilot VirtualInput writer is advised, not silently rewritten"

# The old location still works.
COPY_L2="$SCRATCH_ROOT/template-copy-l2"
snapshot_as_git_baseline "$TEMPLATE" "$COPY_L2"
revert_autopilot_ordering "$COPY_L2/src/game/autopilot.rs"
bash "$TEMPLATE/tools/rollout-replay.sh" "$COPY_L2" >"$SCRATCH_ROOT/l2-output.txt" 2>&1
grep -q 'before(gamebient_input::input::accumulate_input)' "$COPY_L2/src/game/autopilot.rs" \
  || fail "(l) src/game/autopilot.rs (the old location) was not reordered"
grep -q "rollout-replay: src/game/autopilot.rs: reordered" "$SCRATCH_ROOT/l2-output.txt" \
  || fail "(l) the rewrite of src/game/autopilot.rs was not reported"
pass "(l) the template's own src/game/autopilot.rs location still works"

# ---------------------------------------------------------------------------
# (m) src/main.rs's module block: the generated import must never inherit a
# `#[cfg(...)]` from a `mod` line the rewrite removed.
#
# Pack The Ripper's main.rs had `#[cfg(any(feature = "harness", feature =
# "autopilot"))] mod bot;` next to plain `mod assets; mod game; mod ui;`. The
# nested parens defeated the attribute classifier, so `bot` was treated as a
# plain mod, the attribute was left behind, and it landed immediately above
# the generated `use <snake>::{assets, bot, game, ui};` — feature-gating the
# whole import, so the DEFAULT build stopped compiling. cargo check in the
# default feature set is the assertion that matters here.
# ---------------------------------------------------------------------------
COPY_M="$SCRATCH_ROOT/template-copy-m"
snapshot_as_git_baseline "$TEMPLATE" "$COPY_M"
rm "$COPY_M/src/lib.rs"
cat >"$COPY_M/src/bot.rs" <<'BOT'
//! Stand-in for a game's feature-gated helper module.
pub fn planned_taps() -> u32 {
    0
}
BOT
perl -0pi -e 's/^use bevy::prelude::\*;\nuse bevy::window::\{PresentMode, WindowResolution\};\nuse gamebient_game::\{assets, game, ui\};\n/mod assets;\n#[cfg(any(feature = "record", feature = "autopilot"))]\nmod bot;\nmod game;\nmod ui;\n\nuse bevy::prelude::*;\nuse bevy::window::{PresentMode, WindowResolution};\n/' "$COPY_M/src/main.rs"
grep -q '^mod assets;$' "$COPY_M/src/main.rs" \
  || fail "(m) setup: main.rs did not get its pre-port mod block back"
grep -q '^#\[cfg(any(feature = "record", feature = "autopilot"))\]$' "$COPY_M/src/main.rs" \
  || fail "(m) setup: main.rs did not get the nested-paren cfg line"

bash "$TEMPLATE/tools/rollout-replay.sh" "$COPY_M" >"$SCRATCH_ROOT/m-output.txt" 2>&1
cat "$SCRATCH_ROOT/m-output.txt"

# The plain mods import unconditionally...
grep -q '^use gamebient_game::{assets, game, ui};$' "$COPY_M/src/main.rs" \
  || fail "(m) main.rs has no unconditional 'use gamebient_game::{assets, game, ui};'"
# ...with no attribute anywhere above it before the next real item.
perl -0ne 'exit 1 if /#\[cfg\([^\n]*\)\]\s*\n\s*use gamebient_game::\{assets, game, ui\};/' "$COPY_M/src/main.rs" \
  || fail "(m) the unconditional import inherited a #[cfg] from a removed mod line"
# ...and the cfg-gated mod becomes its own cfg-gated use.
perl -0ne 'exit 1 unless /#\[cfg\(any\(feature = "record", feature = "autopilot"\)\)\]\nuse gamebient_game::bot;/' "$COPY_M/src/main.rs" \
  || fail "(m) the cfg-gated mod did not become its own cfg-gated 'use gamebient_game::bot;'"
grep -q '^#\[cfg(any(feature = "record", feature = "autopilot"))\]$' "$COPY_M/src/lib.rs" \
  || fail "(m) lib.rs did not keep the cfg on 'pub mod bot;'"
pass "(m) the generated imports are split: plain mods unconditional, cfg-gated mod on its own use line"

(cd "$COPY_M" && cargo check --quiet) >"$SCRATCH_ROOT/m-check.txt" 2>&1 \
  || {
    tail -40 "$SCRATCH_ROOT/m-check.txt" >&2
    fail "(m) the copy does not cargo check in the default feature set after the rollout"
  }
pass "(m) the rolled-out copy cargo checks in the default build (the cfg-inheritance bug would fail here)"

# ---------------------------------------------------------------------------
# (n) ci.yml's Test step gets --all-features.
#
# A template-derived game's Test step was `cargo test --all-targets` with no
# features, so it never built src/bin/verify.rs (required-features =
# ["verify"]) and compiled a different crate from the Clippy step two lines
# above it, which has always passed --all-features. Dough.io, Pack The Ripper
# and Sundae Shooter each fixed it by hand in their own port.
# ---------------------------------------------------------------------------
COPY_N="$SCRATCH_ROOT/template-copy-n"
snapshot_as_git_baseline "$TEMPLATE" "$COPY_N"
perl -0pi -e 's/^(      - name: Test\n        run: cargo test --all-targets) --all-features\n/$1\n/m' "$COPY_N/.github/workflows/ci.yml"
grep -q '^        run: cargo test --all-targets$' "$COPY_N/.github/workflows/ci.yml" \
  || fail "(n) setup: the Test step was not reverted to the featureless form"

bash "$TEMPLATE/tools/rollout-replay.sh" "$COPY_N" >"$SCRATCH_ROOT/n-output.txt" 2>&1
grep -q '^        run: cargo test --all-targets --all-features$' "$COPY_N/.github/workflows/ci.yml" \
  || fail "(n) a first rollout did not widen the Test step to --all-features"
N_COUNT="$(grep -c 'cargo test --all-targets --all-features' "$COPY_N/.github/workflows/ci.yml")"
[ "$N_COUNT" -eq 1 ] || fail "(n) the Test step appears $N_COUNT times after the rewrite"
bash "$TEMPLATE/tools/rollout-replay.sh" "$COPY_N" >"$SCRATCH_ROOT/n-output-2.txt" 2>&1
cmp -s "$TEMPLATE/.github/workflows/ci.yml" "$COPY_N/.github/workflows/ci.yml" \
  || fail "(n) ci.yml after the rewrite is not byte-identical to the template's (or a second run changed it again)"
pass "(n) the Test step becomes 'cargo test --all-targets --all-features', idempotently"

COPY_N2="$SCRATCH_ROOT/template-copy-n2"
snapshot_as_git_baseline "$TEMPLATE" "$COPY_N2"
perl -pi -e 's/^      - name: Test$/      - name: Run the suite/' "$COPY_N2/.github/workflows/ci.yml"
bash "$TEMPLATE/tools/rollout-replay.sh" "$COPY_N2" >"$SCRATCH_ROOT/n2-output.txt" 2>&1
grep -q "HAND EDIT: .github/workflows/ci.yml: no recognisable '- name: Test' step" "$SCRATCH_ROOT/n2-output.txt" \
  || fail "(n) an unrecognisable Test step drew no HAND EDIT"
grep -q '^        run: cargo test --all-targets --all-features$' "$COPY_N2/.github/workflows/ci.yml" \
  || fail "(n) the renamed step's command was altered instead of being left alone"
pass "(n) an unrecognisable Test step draws a HAND EDIT and is left alone"

# ---------------------------------------------------------------------------
# (o) The fixed-timestep pin.
#
# Bevy's default Time<Fixed> is 64 Hz, sim::tick_duration() is 60, and
# Replay::decode rejects any other rate as BadTickRate — so a game missing
# the insert records runs that no verifier can decode, and the failure
# surfaces nowhere near the missing line. Grand Theft Auto-Reply and Pack The
# Ripper both hit it as an undocumented hand edit.
# ---------------------------------------------------------------------------
COPY_O="$SCRATCH_ROOT/template-copy-o"
snapshot_as_git_baseline "$TEMPLATE" "$COPY_O"
grep -v 'Time::<Fixed>' "$COPY_O/src/game/mod.rs" >"$SCRATCH_ROOT/o-mod.rs"
cp "$SCRATCH_ROOT/o-mod.rs" "$COPY_O/src/game/mod.rs"
bash "$TEMPLATE/tools/rollout-replay.sh" "$COPY_O" >"$SCRATCH_ROOT/o-output.txt" 2>&1
grep -q "HAND EDIT: src/game/mod.rs: pin the fixed timestep" "$SCRATCH_ROOT/o-output.txt" \
  || fail "(o) a missing Time::<Fixed> insert drew no HAND EDIT"
grep -q "BadTickRate" "$SCRATCH_ROOT/o-output.txt" \
  || fail "(o) the HAND EDIT does not name the failure (BadTickRate) it prevents"
pass "(o) a missing fixed-timestep pin draws a HAND EDIT naming BadTickRate"

# A game that pins it in main.rs instead is equally correct and must be silent.
COPY_O2="$SCRATCH_ROOT/template-copy-o2"
snapshot_as_git_baseline "$TEMPLATE" "$COPY_O2"
cp "$SCRATCH_ROOT/o-mod.rs" "$COPY_O2/src/game/mod.rs"
perl -pi -e 's{^fn main\(\) \{$}{// Fixed timestep pinned here instead: Time::<Fixed>::from_duration(game::sim::tick_duration())\nfn main() \{}' "$COPY_O2/src/main.rs"
grep -q 'Time::<Fixed>' "$COPY_O2/src/main.rs" || fail "(o) setup: main.rs did not get the pin"
bash "$TEMPLATE/tools/rollout-replay.sh" "$COPY_O2" >"$SCRATCH_ROOT/o2-output.txt" 2>&1
if grep -q "pin the fixed timestep" "$SCRATCH_ROOT/o2-output.txt"; then
  fail "(o) a game pinning Time::<Fixed> in main.rs was still told to add it"
fi
pass "(o) the pin is accepted in main.rs as well as src/game/mod.rs"

# ---------------------------------------------------------------------------
# (p) tests/windowed_shape.rs.
#
# The resource-level sibling of tests/archetype_order.rs: record the selftest
# script in an app carrying ScreenFade and the windowed Update systems,
# verify it in a bare one. Unlike the archetype probe it is copyable as-is,
# so it is written whenever absent — but only when the two things it names
# are actually there, since a copy that does not compile would turn a green
# checkout red with nothing in the output to explain it.
# ---------------------------------------------------------------------------
COPY_P="$SCRATCH_ROOT/template-copy-p"
snapshot_as_git_baseline "$TEMPLATE" "$COPY_P"
rm "$COPY_P/tests/windowed_shape.rs"
bash "$TEMPLATE/tools/rollout-replay.sh" --upgrade "$COPY_P" >"$SCRATCH_ROOT/p-output.txt" 2>&1
cmp -s "$TEMPLATE/tests/windowed_shape.rs" "$COPY_P/tests/windowed_shape.rs" \
  || fail "(p) tests/windowed_shape.rs was not written back"
grep -q "tests/windowed_shape.rs is NEW in this checkout" "$SCRATCH_ROOT/p-output.txt" \
  || fail "(p) writing a new tests/windowed_shape.rs was not called out in the summary"
pass "(p) an absent tests/windowed_shape.rs is written and announced"

# Every ported game has ADAPTED this file (its own bots, its own counters),
# so --upgrade will never refresh it — which is how a game keeps a copy that
# predates the second-run row without ever being told. The notice is
# self-limiting: it is keyed on the row's own name, so it stops the day the
# row is ported across.
COPY_P1B="$SCRATCH_ROOT/template-copy-p1b"
snapshot_as_git_baseline "$TEMPLATE" "$COPY_P1B"
perl -pi -e 's/^fn a_second_run_in_the_same_app_reproduces_the_first\(\) \{$/fn a_second_run_adapted_away() {/' \
  "$COPY_P1B/tests/windowed_shape.rs"
perl -pi -e 's/a_second_run_in_the_same_app_reproduces_the_first/a_second_run_adapted_away/g' \
  "$COPY_P1B/tests/windowed_shape.rs"
grep -q 'a_second_run_in_the_same_app_reproduces_the_first' "$COPY_P1B/tests/windowed_shape.rs" \
  && fail "(p) setup: the second-run row was not renamed out of the copy"
bash "$TEMPLATE/tools/rollout-replay.sh" --upgrade "$COPY_P1B" >"$SCRATCH_ROOT/p1b-output.txt" 2>&1
grep -q "predates a_second_run_in_the_same_app_reproduces_the_first" "$SCRATCH_ROOT/p1b-output.txt" \
  || fail "(p) an adapted windowed_shape.rs without the second-run row drew no HAND EDIT"
bash "$TEMPLATE/tools/rollout-replay.sh" --upgrade "$COPY_P" >"$SCRATCH_ROOT/p1c-output.txt" 2>&1
if grep -q "predates a_second_run_in_the_same_app_reproduces_the_first" "$SCRATCH_ROOT/p1c-output.txt"; then
  fail "(p) the second-run notice fires on a copy that already has the row"
fi
pass "(p) an adapted copy missing the second-run row is told, once it is there is not"

# And the same again one layer down: a copy that HAS the second-run row but
# predates Bot::Holder has a row that is blind to hold-dependent carried
# state — which is how Grand Theft Auto-Reply's own Local<Repeat> survived
# it with the fix reverted. Keyed on Bot::Holder, so it also stops the day
# the row is ported across. $COPY_P is the unmodified template copy from
# above, which has both rows, so p1c's output doubles as the silence check.
COPY_P1D="$SCRATCH_ROOT/template-copy-p1d"
snapshot_as_git_baseline "$TEMPLATE" "$COPY_P1D"
perl -0pi -e 's/\bBot::Holder\b/Bot::Coaster/g' "$COPY_P1D/tests/windowed_shape.rs"
grep -q 'Bot::Holder' "$COPY_P1D/tests/windowed_shape.rs" \
  && fail "(p) setup: Bot::Holder was not adapted out of the copy"
grep -q 'a_second_run_in_the_same_app_reproduces_the_first' "$COPY_P1D/tests/windowed_shape.rs" \
  || fail "(p) setup: the second-run row went missing too, so this probes the wrong notice"
bash "$TEMPLATE/tools/rollout-replay.sh" --upgrade "$COPY_P1D" >"$SCRATCH_ROOT/p1d-output.txt" 2>&1
grep -q "no Bot::Holder" "$SCRATCH_ROOT/p1d-output.txt" \
  || fail "(p) an adapted windowed_shape.rs with the second-run row but no holder drew no HAND EDIT"
grep -q "held, NOT latched" "$SCRATCH_ROOT/p1d-output.txt" \
  || fail "(p) the holder notice does not carry the held-vs-latched trap"
if grep -q "no Bot::Holder" "$SCRATCH_ROOT/p1c-output.txt"; then
  fail "(p) the holder notice fires on a copy that already has the row"
fi
pass "(p) an adapted copy whose second-run row has no holder bot is told"

COPY_P2="$SCRATCH_ROOT/template-copy-p2"
snapshot_as_git_baseline "$TEMPLATE" "$COPY_P2"
rm "$COPY_P2/tests/windowed_shape.rs"
perl -pi -e 's/^pub fn script\(/fn script(/' "$COPY_P2/src/game/replay/selftest.rs"
bash "$TEMPLATE/tools/rollout-replay.sh" --upgrade "$COPY_P2" >"$SCRATCH_ROOT/p2-output.txt" 2>&1
[ -e "$COPY_P2/tests/windowed_shape.rs" ] \
  && fail "(p) tests/windowed_shape.rs was written into a copy whose selftest script is not pub"
grep -q "HAND EDIT: tests/windowed_shape.rs: not created" "$SCRATCH_ROOT/p2-output.txt" \
  || fail "(p) refusing to write tests/windowed_shape.rs drew no HAND EDIT"
pass "(p) a game whose selftest script is not pub is asked rather than handed a file that cannot compile"

# The copy also builds the game's own AssetsPlugin (Dive Rise's desync came
# in through a component value an AssetsPlugin Update system writes), so a
# game without one must be asked rather than handed a file that cannot
# compile. Hunted is the fleet's one such game.
COPY_P3="$SCRATCH_ROOT/template-copy-p3"
snapshot_as_git_baseline "$TEMPLATE" "$COPY_P3"
rm "$COPY_P3/tests/windowed_shape.rs"
perl -pi -e 's/^pub struct AssetsPlugin;/pub struct ArtPlugin;/' "$COPY_P3/src/assets/mod.rs"
perl -pi -e 's/\bAssetsPlugin\b/ArtPlugin/g' "$COPY_P3/src/main.rs"
if grep -q 'pub struct AssetsPlugin' "$COPY_P3/src/assets/mod.rs"; then
  fail "(p) setup: AssetsPlugin was not renamed in the copy"
fi
bash "$TEMPLATE/tools/rollout-replay.sh" --upgrade "$COPY_P3" >"$SCRATCH_ROOT/p3-output.txt" 2>&1
[ -e "$COPY_P3/tests/windowed_shape.rs" ] \
  && fail "(p) tests/windowed_shape.rs was written into a copy with no assets::AssetsPlugin"
grep -q "HAND EDIT: tests/windowed_shape.rs: not created" "$SCRATCH_ROOT/p3-output.txt" \
  || fail "(p) refusing to write tests/windowed_shape.rs over a missing AssetsPlugin drew no HAND EDIT"
pass "(p) a game with no assets::AssetsPlugin is asked rather than handed a file that cannot compile"

# ---------------------------------------------------------------------------
# (q) The presentation-resource advisory.
#
# Sundae Shooter's fire_scoop/swap_queue refused to act while ScreenFade was
# mid-transition. The fade lives in UiPlugin (which the verifier never
# builds), ticks on the FRAME delta, and is busy for the first ~24 ticks of
# every run — so a browser-recorded run came back claiming 195 points against
# 200 re-simulated. No fixture and no archetype probe can see it.
# ---------------------------------------------------------------------------
COPY_Q="$SCRATCH_ROOT/template-copy-q"
snapshot_as_git_baseline "$TEMPLATE" "$COPY_Q"
perl -0pi -e 's/(pub fn move_player\(\n)/$1    fade: Option<Res<crate::ui::transition::ScreenFade>>,\n/' "$COPY_Q/src/game/player.rs"
grep -q 'fade: Option<Res<crate::ui::transition::ScreenFade>>' "$COPY_Q/src/game/player.rs" \
  || fail "(q) setup: move_player did not get a fade parameter"
bash "$TEMPLATE/tools/rollout-replay.sh" "$COPY_Q" >"$SCRATCH_ROOT/q-output.txt" 2>&1
grep -q "src/game/player.rs:move_player" "$SCRATCH_ROOT/q-output.txt" \
  || fail "(q) a sim system reading ScreenFade was not named by the advisory"
pass "(q) a sim file reading a fade resource is named"

# The sanctioned shapes must stay quiet, or the advisory fires on every
# ported game for ever: sim::end_run's callers, rule 8's toggle_pause, and
# the autopilot (a dev harness the game registers in Update).
if grep -q "read a fade resource UiPlugin owns" "$SCRATCH_ROOT/a-output.txt"; then
  fail "(q) the advisory fires on the template's own checkout (end_run/toggle_pause/autopilot are sanctioned)"
fi
COPY_Q2="$SCRATCH_ROOT/template-copy-q2"
snapshot_as_git_baseline "$TEMPLATE" "$COPY_Q2"
perl -0pi -e 's/(pub fn move_player\(\n)/$1    fade: Option<ResMut<crate::ui::transition::ScreenFade>>,\n/' "$COPY_Q2/src/game/player.rs"
perl -0pi -e 's/(pub fn move_player\([^)]*\) \{\n)/$1    \/\/ hands the fade straight to the latch\n    let _ = \&fade;\n    if false { crate::game::sim::end_run(todo!(), fade, todo!()); }\n/' "$COPY_Q2/src/game/player.rs"
grep -q 'end_run(' "$COPY_Q2/src/game/player.rs" || fail "(q) setup: move_player does not call end_run"
bash "$TEMPLATE/tools/rollout-replay.sh" "$COPY_Q2" >"$SCRATCH_ROOT/q2-output.txt" 2>&1
if grep -q "src/game/player.rs:move_player" "$SCRATCH_ROOT/q2-output.txt"; then
  fail "(q) a fade handed to sim::end_run (rule 10's sanctioned touch) was still flagged"
fi
pass "(q) end_run's callers, toggle_pause and the autopilot are not flagged"

# A game that calls its pause toggler something else is the same system, and
# the advisory must recognise it by the pause edge rather than by the
# template's name — otherwise it fires on that game for ever.
COPY_Q3="$SCRATCH_ROOT/template-copy-q3"
snapshot_as_git_baseline "$TEMPLATE" "$COPY_Q3"
perl -pi -e 's/\bfn toggle_pause\(/fn handle_pause_input(/; s/^                toggle_pause$/                handle_pause_input/' "$COPY_Q3/src/game/mod.rs"
grep -q 'fn handle_pause_input(' "$COPY_Q3/src/game/mod.rs" \
  || fail "(q) setup: toggle_pause was not renamed"
bash "$TEMPLATE/tools/rollout-replay.sh" "$COPY_Q3" >"$SCRATCH_ROOT/q3-output.txt" 2>&1
if grep -q "handle_pause_input" "$SCRATCH_ROOT/q3-output.txt"; then
  fail "(q) a renamed rule-8 pause toggler was flagged; the advisory is matching the name, not the pause_just_pressed edge"
fi
pass "(q) a pause toggler under a different name is still recognised as the rule-8 tolerated read"

# ---------------------------------------------------------------------------
# (r) the Cutter tests step.
#
# tools/test_cut_clips.py ships in every game with the recording harness, and
# the template's own ci.yml runs it — but a game whose ci.yml predates that
# step has the cutter's logic and a green CI that never touches it. Two of
# the six wave-2 upgrades added the step by hand and the rest did not, which
# is exactly the drift the rollout script exists to stop.
# ---------------------------------------------------------------------------
cutter_steps() { grep -c 'name: Cutter tests' "$1" || true; }

COPY_R="$SCRATCH_ROOT/template-copy-r"
snapshot_as_git_baseline "$TEMPLATE" "$COPY_R"
perl -0pi -e 's/\n      - name: Cutter tests\n        working-directory: tools\n        run: python3 -m unittest test_cut_clips -v\n//' \
  "$COPY_R/.github/workflows/ci.yml"
if grep -q 'test_cut_clips' "$COPY_R/.github/workflows/ci.yml"; then
  fail "(r) setup: the Cutter tests step was not removed from the copy"
fi
bash "$TEMPLATE/tools/rollout-replay.sh" "$COPY_R" >"$SCRATCH_ROOT/r-output.txt" 2>&1
[ "$(cutter_steps "$COPY_R/.github/workflows/ci.yml")" = "1" ] \
  || fail "(r) a first rollout did not add the Cutter tests step for a game that ships tools/test_cut_clips.py"
grep -q 'run: python3 -m unittest test_cut_clips -v' "$COPY_R/.github/workflows/ci.yml" \
  || fail "(r) the Cutter tests step was added without its run: line"
cmp -s "$TEMPLATE/.github/workflows/ci.yml" "$COPY_R/.github/workflows/ci.yml" \
  || fail "(r) the rebuilt ci.yml is not byte-identical to the template's (wrong place or wrong indentation)"
pass "(r) a missing Cutter tests step is inserted after the Test step, byte-identical to the template's"

# Idempotent: the step is already there now, so a second run must add nothing.
bash "$TEMPLATE/tools/rollout-replay.sh" "$COPY_R" >"$SCRATCH_ROOT/r2-output.txt" 2>&1
[ "$(cutter_steps "$COPY_R/.github/workflows/ci.yml")" = "1" ] \
  || fail "(r) a second rollout duplicated the Cutter tests step"
bash "$TEMPLATE/tools/rollout-replay.sh" --upgrade "$COPY_R" >"$SCRATCH_ROOT/r3-output.txt" 2>&1
[ "$(cutter_steps "$COPY_R/.github/workflows/ci.yml")" = "1" ] \
  || fail "(r) --upgrade duplicated the Cutter tests step"
pass "(r) the insertion is idempotent under a re-run and under --upgrade"

# A game without the cutter must NOT get a step that would fail: the guard is
# the file, not the workflow.
COPY_R4="$SCRATCH_ROOT/template-copy-r4"
snapshot_as_git_baseline "$TEMPLATE" "$COPY_R4"
perl -0pi -e 's/\n      - name: Cutter tests\n        working-directory: tools\n        run: python3 -m unittest test_cut_clips -v\n//' \
  "$COPY_R4/.github/workflows/ci.yml"
rm -f "$COPY_R4/tools/test_cut_clips.py"
bash "$TEMPLATE/tools/rollout-replay.sh" --upgrade "$COPY_R4" >"$SCRATCH_ROOT/r4-output.txt" 2>&1
[ "$(cutter_steps "$COPY_R4/.github/workflows/ci.yml")" = "0" ] \
  || fail "(r) a game with no tools/test_cut_clips.py was given a Cutter tests step that would fail"
pass "(r) a game without the cutter is left alone"

# ---------------------------------------------------------------------------
# (s) The transcendental advisory.
#
# libm is not required to round sin/cos/exp/powf/atan2 the same way on macOS,
# on the Linux CI runner and in wasm. Gulper folded `head.facing` (an atan2)
# into its checksum and one 5400-tick fixture gave three verifiers three
# answers with identical score and tick count — which turned `cargo test`
# itself red on CI, because the committed NATIVE fixture is re-simulated
# natively. See port-checklist.md rule 5, "The transcendental rule".
# ---------------------------------------------------------------------------
COPY_S="$SCRATCH_ROOT/template-copy-s"
snapshot_as_git_baseline "$TEMPLATE" "$COPY_S"
perl -pi -e 's/let dir = Vec2::new\(input\.move_x, input\.move_y\);/let dir = Vec2::new(input.move_x, (input.move_y * 2.0).sin());/' \
  "$COPY_S/src/game/player.rs"
grep -q '\.sin()' "$COPY_S/src/game/player.rs" \
  || fail "(s) setup: move_player did not get a transcendental call"
bash "$TEMPLATE/tools/rollout-replay.sh" "$COPY_S" >"$SCRATCH_ROOT/s-output.txt" 2>&1
grep -q "src/game/player.rs:.sin()" "$SCRATCH_ROOT/s-output.txt" \
  || fail "(s) a sim file calling sin() was not named by the advisory"
grep -q "The transcendental rule" "$SCRATCH_ROOT/s-output.txt" \
  || fail "(s) the advisory does not point at the rule it is about"
pass "(s) a sim file calling a transcendental is named, with a pointer to the rule"

# The synth palette ships in every game from this template and is
# presentation that folds nothing; naming it would make the advisory noise on
# every game for ever. (a)'s output is the template against itself.
if grep -q "sim code calls libm" "$SCRATCH_ROOT/a-output.txt"; then
  fail "(s) the advisory fires on the template's own checkout (the inherited audio synth must be subtracted)"
fi
grep -q "src/game/audio/synth.rs" "$SCRATCH_ROOT/s-output.txt" \
  && fail "(s) the inherited audio synth palette was named alongside the planted call"
pass "(s) the inherited audio synth palette is not named"

# ---------------------------------------------------------------------------
# (t) The app-clock advisory.
#
# Inside FixedUpdate `Res<Time>` is `Time<Fixed>`, whose `elapsed()` counts
# from APP start and is never reset per run. Dive Rise drove nine sim systems
# off it and a real run came back UNVERIFIED: 11 877 ticks claiming score
# 144, re-simulating to 56 in three independent verifiers, with one extra
# tick of menu time enough to change the whole run. See port-checklist.md
# rule 7.
# ---------------------------------------------------------------------------
COPY_T="$SCRATCH_ROOT/template-copy-t"
snapshot_as_git_baseline "$TEMPLATE" "$COPY_T"
perl -pi -e 's/let dir = Vec2::new\(input\.move_x, input\.move_y\);/let phase = time.elapsed_secs();\n    let dir = Vec2::new(input.move_x + phase, input.move_y);/' \
  "$COPY_T/src/game/player.rs"
grep -q 'time\.elapsed_secs()' "$COPY_T/src/game/player.rs" \
  || fail "(t) setup: move_player did not get an app-lifetime clock read"
bash "$TEMPLATE/tools/rollout-replay.sh" "$COPY_T" >"$SCRATCH_ROOT/t-output.txt" 2>&1
grep -q "src/game/player.rs:.elapsed_secs()" "$SCRATCH_ROOT/t-output.txt" \
  || fail "(t) a sim file reading Time::elapsed_secs() was not named by the advisory"
grep -q "rule 7" "$SCRATCH_ROOT/t-output.txt" \
  || fail "(t) the advisory does not point at the rule it is about"
grep -q "sim::RunClock" "$SCRATCH_ROOT/t-output.txt" \
  || fail "(t) the advisory does not name the replacement"
pass "(t) a sim file reading an app-lifetime clock is named, with the rule and the fix"

# The template's own hits are a dev recorder (`src/game/record/audio.rs`) and
# the wall-clock stamp `replay/recorder.rs` puts on the replay HEADER, which
# no sim system reads. Naming either would make the advisory noise on every
# game for ever. (a)'s output is the template against itself.
if grep -q "sim code reads a clock the replay does not carry" "$SCRATCH_ROOT/a-output.txt"; then
  fail "(t) the advisory fires on the template's own checkout (the inherited wall-clock hits must be subtracted)"
fi
grep -q "src/game/record/audio.rs" "$SCRATCH_ROOT/t-output.txt" \
  && fail "(t) the inherited dev recorder was named alongside the planted read"
grep -q "src/game/replay/recorder.rs" "$SCRATCH_ROOT/t-output.txt" \
  && fail "(t) the replay header's wall-clock stamp was named alongside the planted read"
pass "(t) the template's own benign wall-clock hits are not named"

# A read a port has justified must not nag on every later upgrade.
COPY_T2="$SCRATCH_ROOT/template-copy-t2"
snapshot_as_git_baseline "$TEMPLATE" "$COPY_T2"
perl -pi -e 's/let dir = Vec2::new\(input\.move_x, input\.move_y\);/let phase = time.elapsed_secs(); \/\/ allow-app-clock: dev only\n    let dir = Vec2::new(input.move_x + phase, input.move_y);/' \
  "$COPY_T2/src/game/player.rs"
grep -q 'allow-app-clock' "$COPY_T2/src/game/player.rs" \
  || fail "(t) setup: the marked read was not planted"
bash "$TEMPLATE/tools/rollout-replay.sh" "$COPY_T2" >"$SCRATCH_ROOT/t2-output.txt" 2>&1
if grep -q "sim code reads a clock the replay does not carry" "$SCRATCH_ROOT/t2-output.txt"; then
  fail "(t) an allow-app-clock marked read was still flagged"
fi
pass "(t) an allow-app-clock marker silences the advisory"

# ---------------------------------------------------------------------------
# (v) The sim-Local advisory.
#
# Same bug class as (t) from the other end: state the recording app has and
# the verifier does not, except this one is carried from the LAST RUN rather
# than from before the run. A `Local<T>` belongs to the system instance, so
# it lives as long as the App and no run start can reach it; the verifier
# plays exactly one run per app and the browser does not. Grand Theft
# Auto-Reply kept its cursor auto-repeat in two `Local<Repeat>`s inside a
# SimSet system (grand-theft-auto-reply#8). See port-checklist.md rule 7.
# ---------------------------------------------------------------------------
COPY_V="$SCRATCH_ROOT/template-copy-v"
snapshot_as_git_baseline "$TEMPLATE" "$COPY_V"
perl -pi -e 's/^    mut query: Query<&mut Transform, With<Player>>,$/    mut query: Query<&mut Transform, With<Player>>,\n    mut seen: Local<u32>,/' \
  "$COPY_V/src/game/player.rs"
grep -q 'Local<u32>' "$COPY_V/src/game/player.rs" \
  || fail "(v) setup: move_player did not get a Local"
bash "$TEMPLATE/tools/rollout-replay.sh" "$COPY_V" >"$SCRATCH_ROOT/v-output.txt" 2>&1
grep -q "src/game/player.rs:Local<" "$SCRATCH_ROOT/v-output.txt" \
  || fail "(v) a sim file keeping state in a Local was not named by the advisory"
grep -q "rule 7" "$SCRATCH_ROOT/v-output.txt" \
  || fail "(v) the advisory does not point at the rule it is about"
grep -q "a_second_run_in_the_same_app_reproduces_the_first" "$SCRATCH_ROOT/v-output.txt" \
  || fail "(v) the advisory does not name the probe that answers it"
pass "(v) a sim file keeping state in a Local is named, with the rule and the probe"

# The template's own hits are the dev recorder's dedupe boxes under
# `src/game/record/`, which no SimSet system runs. Naming them would make the
# advisory noise on every game for ever, and would also mean the scope
# derivation had collapsed to "every file". (a)'s output is the template
# against itself.
if grep -q "a sim system's file keeps state in a Local" "$SCRATCH_ROOT/a-output.txt"; then
  fail "(v) the advisory fires on the template's own checkout (record/ is not a sim file)"
fi
grep -q "src/game/record/mod.rs" "$SCRATCH_ROOT/v-output.txt" \
  && fail "(v) the dev recorder's Locals were named alongside the planted one"
pass "(v) the template's own non-sim Locals are not named"

# A Local a port has justified must not nag on every later upgrade.
COPY_V2="$SCRATCH_ROOT/template-copy-v2"
snapshot_as_git_baseline "$TEMPLATE" "$COPY_V2"
perl -pi -e 's/^    mut query: Query<&mut Transform, With<Player>>,$/    mut query: Query<&mut Transform, With<Player>>,\n    mut seen: Local<u32>, \/\/ allow-local: dev only/' \
  "$COPY_V2/src/game/player.rs"
grep -q 'allow-local' "$COPY_V2/src/game/player.rs" \
  || fail "(v) setup: the marked Local was not planted"
bash "$TEMPLATE/tools/rollout-replay.sh" "$COPY_V2" >"$SCRATCH_ROOT/v2-output.txt" 2>&1
if grep -q "a sim system's file keeps state in a Local" "$SCRATCH_ROOT/v2-output.txt"; then
  fail "(v) an allow-local marked Local was still flagged"
fi
pass "(v) an allow-local marker silences the advisory"

# ---------------------------------------------------------------------------
# (u) The run-clock wiring, on --upgrade.
#
# `sim.rs` is a verbatim copy, so an upgrade hands an already-ported game a
# `begin_run` that takes `ResMut<RunClock>` and a `sync_run_clock` its chain
# has never heard of. Missing resource = Bevy's nameless "Parameter ...
# failed validation" on OnEnter(Playing). Missing system = RunClock frozen at
# 0 for the whole run, which is silent. See docs/replay-verification.md rule 7.
# ---------------------------------------------------------------------------
COPY_U="$SCRATCH_ROOT/template-copy-u"
snapshot_as_git_baseline "$TEMPLATE" "$COPY_U"
perl -0pi -e 's/\n\s*\.init_resource::<sim::RunClock>\(\)//' "$COPY_U/src/game/mod.rs"
perl -0pi -e 's/\n\s*\/\/ The sim.s own clock.*?\n\s*sim::sync_run_clock,//s' "$COPY_U/src/game/mod.rs"
grep -q 'sim::RunClock' "$COPY_U/src/game/mod.rs" \
  && fail "(u) setup: the RunClock registration was not removed"
grep -q 'sim::sync_run_clock' "$COPY_U/src/game/mod.rs" \
  && fail "(u) setup: the sync_run_clock system was not removed"
bash "$TEMPLATE/tools/rollout-replay.sh" --upgrade "$COPY_U" >"$SCRATCH_ROOT/u-output.txt" 2>&1
[ "$(grep -c 'init_resource::<sim::RunClock>()' "$COPY_U/src/game/mod.rs")" = "1" ] \
  || fail "(u) --upgrade did not put .init_resource::<sim::RunClock>() back exactly once"
[ "$(grep -c 'sim::sync_run_clock,' "$COPY_U/src/game/mod.rs")" = "1" ] \
  || fail "(u) --upgrade did not chain sim::sync_run_clock exactly once"
grep -A1 'sim::advance_tick,' "$COPY_U/src/game/mod.rs" | grep -q 'sim::sync_run_clock,' \
  || fail "(u) sim::sync_run_clock did not land directly after sim::advance_tick"
(cd "$COPY_U" && cargo fmt --all -- --check >/dev/null 2>&1) \
  || fail "(u) the rewritten builder chain is not rustfmt-clean"
pass "(u) --upgrade restores both halves of the run-clock wiring"

# Idempotent: a second --upgrade must not double either half.
bash "$TEMPLATE/tools/rollout-replay.sh" --upgrade "$COPY_U" >"$SCRATCH_ROOT/u2-output.txt" 2>&1
[ "$(grep -c 'init_resource::<sim::RunClock>()' "$COPY_U/src/game/mod.rs")" = "1" ] \
  || fail "(u) a second --upgrade duplicated the RunClock registration"
[ "$(grep -c 'sim::sync_run_clock,' "$COPY_U/src/game/mod.rs")" = "1" ] \
  || fail "(u) a second --upgrade duplicated sim::sync_run_clock"
pass "(u) the run-clock wiring is idempotent under a re-run"

# ---------------------------------------------------------------------------
# (w) The Mute arm's Option guard: recognise the guard, don't count the
#     script's own handwriting.
#
# `GlobalVolume` is inserted by Bevy's `AudioPlugin`, which the headless
# verifier never adds, so `apply_host_commands` takes it as
# `Option<ResMut<GlobalVolume>>` and the `Mute` arm has to unwrap it. The old
# idempotency guard here asked "does this file contain `as_deref_mut()`" —
# i.e. "did I write this guard" — rather than "is that assignment guarded at
# all". Grand Theft Otto had hand-written
#
#     if let Some(global_volume) = global_volume.as_mut() { .. }
#
# long before this script existed, so `--upgrade` wrapped the inner line a
# SECOND time and emitted `global_volume.as_deref_mut()` inside a binding
# that is already a `&mut GlobalVolume`. That does not compile, and it landed
# on a file every previous run had correctly left alone (grand-theft-otto#6).
# ---------------------------------------------------------------------------
COPY_W="$SCRATCH_ROOT/template-copy-w"
snapshot_as_git_baseline "$TEMPLATE" "$COPY_W"
# Rewrite the template's own guard into Grand Theft Otto's hand-written one.
perl -0pi -e 's/if let Some\(volume\) = global_volume\.as_deref_mut\(\) \{\n(\s*)volume\.volume = /if let Some(global_volume) = global_volume.as_mut() {\n$1global_volume.volume = /' \
  "$COPY_W/src/game/host.rs"
grep -q 'if let Some(global_volume) = global_volume.as_mut()' "$COPY_W/src/game/host.rs" \
  || fail "(w) setup: the hand-written as_mut() guard was not planted"
grep -q 'as_deref_mut' "$COPY_W/src/game/host.rs" \
  && fail "(w) setup: the template's own as_deref_mut guard is still there"
cp "$COPY_W/src/game/host.rs" "$SCRATCH_ROOT/w-host-before.rs"
bash "$TEMPLATE/tools/rollout-replay.sh" --upgrade "$COPY_W" >"$SCRATCH_ROOT/w-output.txt" 2>&1
grep -q 'as_deref_mut' "$COPY_W/src/game/host.rs" \
  && fail "(w) --upgrade wrapped a Mute arm that already had an Option guard (the grand-theft-otto#6 bug: the nested guard does not compile)"
[ "$(grep -c 'global_volume\.volume = ' "$COPY_W/src/game/host.rs")" = "1" ] \
  || fail "(w) the guarded assignment was duplicated or lost"
diff -q "$SCRATCH_ROOT/w-host-before.rs" "$COPY_W/src/game/host.rs" >/dev/null \
  || fail "(w) host.rs changed although its Mute arm was already guarded"
pass "(w) an existing Option guard on global_volume is recognised and left alone"

# ...and the edit the guard exists for still happens on a game that has none.
COPY_W2="$SCRATCH_ROOT/template-copy-w2"
snapshot_as_git_baseline "$TEMPLATE" "$COPY_W2"
perl -0pi -e 's/^([ \t]*)if let Some\(volume\) = global_volume\.as_deref_mut\(\) \{\n[ \t]*volume\.volume = ([^\n]*);\n[ \t]*\}\n/$1global_volume.volume = $2;\n/m' \
  "$COPY_W2/src/game/host.rs"
grep -q 'as_deref_mut' "$COPY_W2/src/game/host.rs" \
  && fail "(w) setup: the unguarded shape was not planted"
grep -q '^\s*global_volume\.volume = ' "$COPY_W2/src/game/host.rs" \
  || fail "(w) setup: the bare assignment was not planted"
bash "$TEMPLATE/tools/rollout-replay.sh" --upgrade "$COPY_W2" >"$SCRATCH_ROOT/w2-output.txt" 2>&1
grep -q 'if let Some(volume) = global_volume.as_deref_mut()' "$COPY_W2/src/game/host.rs" \
  || fail "(w) an unguarded Mute arm was not guarded"
(cd "$COPY_W2" && cargo fmt --all -- --check >/dev/null 2>&1) \
  || fail "(w) the inserted guard is not rustfmt-clean"
pass "(w) an unguarded Mute arm still gets the as_deref_mut guard"

# A shape this script cannot classify draws a HAND EDIT rather than a rewrite.
COPY_W3="$SCRATCH_ROOT/template-copy-w3"
snapshot_as_git_baseline "$TEMPLATE" "$COPY_W3"
perl -0pi -e 's/^([ \t]*)if let Some\(volume\) = global_volume\.as_deref_mut\(\) \{\n[ \t]*volume\.volume = ([^\n]*);\n[ \t]*\}\n/$1global_volume.volume = $2;\n$1global_volume.volume = $2;\n/m' \
  "$COPY_W3/src/game/host.rs"
[ "$(grep -c '^\s*global_volume\.volume = ' "$COPY_W3/src/game/host.rs")" = "2" ] \
  || fail "(w) setup: the two-assignment shape was not planted"
cp "$COPY_W3/src/game/host.rs" "$SCRATCH_ROOT/w3-host-before.rs"
bash "$TEMPLATE/tools/rollout-replay.sh" --upgrade "$COPY_W3" >"$SCRATCH_ROOT/w3-output.txt" 2>&1
grep -q "global_volume.volume" "$SCRATCH_ROOT/w3-output.txt" \
  || fail "(w) an unclassifiable Mute arm drew no HAND EDIT"
diff -q "$SCRATCH_ROOT/w3-host-before.rs" "$COPY_W3/src/game/host.rs" >/dev/null \
  || fail "(w) an unclassifiable Mute arm was rewritten anyway"
pass "(w) an ambiguous Mute arm draws a HAND EDIT and is left untouched"

# ---------------------------------------------------------------------------
# (x) The pre-run-choice advisory.
#
# Beat Bender's title screen wrote song::SelectedSong/SelectedDifficulty and
# driver::setup_match read them on OnEnter(Playing). A replay carries a seed
# and a stream of ticks and no title screen, so the verifier re-simulated
# EVERY run on song 0 at NORMAL: 4 705 recorded ticks against 4 477
# re-simulated. No fixture and no probe can see this — they all start from a
# fresh App and pick the same default the verifier does.
# ---------------------------------------------------------------------------
COPY_X="$SCRATCH_ROOT/template-copy-x"
snapshot_as_git_baseline "$TEMPLATE" "$COPY_X"
cat >>"$COPY_X/src/game/player.rs" <<'RS'

/// Planted by test_rollout_replay.sh part (x): Beat Bender's shape.
#[derive(Resource, Default)]
pub struct SelectedSong(pub usize);

pub fn setup_match_planted(_song: Res<SelectedSong>) {}
RS
cat >>"$COPY_X/src/ui/menu.rs" <<'RS'

/// Planted by test_rollout_replay.sh part (x): the title screen writes it.
pub fn pick_song_planted(mut song: ResMut<crate::game::player::SelectedSong>) {
    song.0 = 1;
}
RS
bash "$TEMPLATE/tools/rollout-replay.sh" "$COPY_X" >"$SCRATCH_ROOT/x-output.txt" 2>&1
grep -q "src/ui/menu.rs:SelectedSong" "$SCRATCH_ROOT/x-output.txt" \
  || fail "(x) a resource written by the menu and read by src/game/ was not named by the advisory"
pass "(x) a pre-run choice written outside src/game/ is named"

# The template's own inherited shape (src/ui/hud.rs writes AudioCapture, which
# is declared under src/game/) must stay quiet, or the advisory fires on every
# game for ever and nobody reads it.
if grep -q "a resource src/game/ reads is written from outside it" "$SCRATCH_ROOT/a-output.txt"; then
  fail "(x) the advisory fires on the template's own checkout (the inherited AudioCapture shape is not subtracted)"
fi
pass "(x) the template's own inherited resource writers are subtracted"

# Sim code owning its own resource is the normal case and must not print, or
# the real hits are buried: every game writes GameData from src/game/.
if grep -qE "src/game/[a-z_/]*\.rs:GameData" "$SCRATCH_ROOT/x-output.txt"; then
  fail "(x) a resource written from INSIDE src/game/ was flagged"
fi
pass "(x) writers inside src/game/ are not flagged"

echo "test_rollout_replay: all checks passed"
