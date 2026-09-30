#!/usr/bin/env bash
# Tests the three small rollout scripts on scratch game dirs:
#   (a) rollout-store-assets.sh copies store-assets.sh AND game-size.sh, both
#       executable; a second run reports the game is current; a stale
#       game-size.sh is replaced;
#   (b) rollout-frame-art.sh copies frame-art.sh and the two placeholder SVGs;
#       a second run keeps a game's own marquee.svg byte for byte and still
#       refreshes frame-art.sh;
#   (c) rollout-record.sh copies game-size.sh beside record.sh, executable;
#   (x) no rollout script copies or names test_aspect_docs.sh;
#   (d) rollout-aspect.sh on a scratch copy of the template set to 1:1 changes
#       only the two size constants and adds tests/display_contract.rs, and a
#       second run reports nothing to do;
#   (e) rollout-aspect.sh on a scratch clone of games/pack-the-ripper (skipped
#       when that checkout is not beside this template);
#   (f) frame-accept.sh --check-copies passes on (d) and (e) and fails after a
#       comment is appended to a copied src/frame/layout.rs;
#   (g) rollout-record.sh inserts `record` after a capture-aware autopilot line;
#   (h) frame-accept.sh --shots passes synthetic captures, fails a bad gap, and
#       says so when --frame off leaves no bars;
#   (i) --check-copies passes on a scratch clone of games/gulper (a 6-letter
#       crate name) after rollout-aspect.sh and cargo fmt --all, and fails on a
#       stray tools/test_*.sh;
#   (j) rollout-aspect.sh asks a flat layout (games/Hunted) for sim-sources.txt
#       and a crate with no src/lib.rs (games/moleman-racing) for a lib target;
#   (k) the camera-query scan in rollout-aspect.sh flags a bare camera query
#       followed by an unrelated With<> query, and not a With<> inside its own.
# (e), (i) and (j) skip when the game checkouts are not beside the template.
set -euo pipefail
HERE="$(cd "$(dirname "$0")/.." && pwd)"

T="$(mktemp -d)"; trap 'rm -rf "$T"' EXIT
same() { cmp -s "$HERE/tools/$2" "$1/tools/$2" || { echo "FAIL $3: $2 differs from the template's"; exit 1; }; }

# (a)
mkdir -p "$T/a"
"$HERE/tools/rollout-store-assets.sh" "$T/a" > "$T/a.out"
[ -x "$T/a/tools/store-assets.sh" ] || { echo "FAIL a: store-assets.sh not copied"; exit 1; }
[ -x "$T/a/tools/game-size.sh" ] || { echo "FAIL a: game-size.sh not copied"; exit 1; }
same "$T/a" store-assets.sh a; same "$T/a" game-size.sh a
"$HERE/tools/rollout-store-assets.sh" "$T/a" > "$T/a2.out"
grep -q "already current" "$T/a2.out" || { echo "FAIL a: second run did not report current"; cat "$T/a2.out"; exit 1; }
echo "# stale" >> "$T/a/tools/game-size.sh"
"$HERE/tools/rollout-store-assets.sh" "$T/a" >/dev/null
same "$T/a" game-size.sh "a (stale copy)"

# (b)
mkdir -p "$T/b"
"$HERE/tools/rollout-frame-art.sh" "$T/b" >/dev/null
[ -x "$T/b/tools/frame-art.sh" ] || { echo "FAIL b: frame-art.sh not copied"; exit 1; }
same "$T/b" frame-art.sh b; same "$T/b" marquee.svg b; same "$T/b" bezel.svg b
printf '<svg xmlns="http://www.w3.org/2000/svg" width="1080" height="360"/>\n' > "$T/b/tools/marquee.svg"
cp "$T/b/tools/marquee.svg" "$T/b.marquee.svg"
echo "# stale" >> "$T/b/tools/frame-art.sh"
"$HERE/tools/rollout-frame-art.sh" "$T/b" >/dev/null
cmp -s "$T/b/tools/marquee.svg" "$T/b.marquee.svg" || { echo "FAIL b: the game's own marquee.svg was overwritten"; exit 1; }
same "$T/b" frame-art.sh "b (stale copy)"

# (c)
mkdir -p "$T/c/src"
printf '[package]\nname = "scratch-game"\nversion = "0.1.0"\n\n[features]\nautopilot = []\n' > "$T/c/Cargo.toml"
"$HERE/tools/rollout-record.sh" "$T/c" > "$T/c.out" 2>&1 || { echo "FAIL c: rollout-record.sh exited non-zero"; cat "$T/c.out"; exit 1; }
[ -x "$T/c/tools/record.sh" ] || { echo "FAIL c: record.sh not copied"; exit 1; }
[ -x "$T/c/tools/game-size.sh" ] || { echo "FAIL c: game-size.sh not copied"; exit 1; }
same "$T/c" game-size.sh c

# (x) template-only tests are never rolled out: no rollout script names them.
if grep -n "test_aspect_docs" "$HERE"/tools/rollout-*.sh; then
  echo "FAIL x: a rollout script copies or names test_aspect_docs.sh, which asserts the template's own info.json"
  exit 1
fi

# Scratch template copy for (d): tracked files as they are on disk now.
export CARGO_NET_OFFLINE=true
commit_all() { git -C "$1" add -A && git -C "$1" -c user.name=t -c user.email=t@t commit -qm "$2"; }
mkdir -p "$T/d"
(cd "$HERE" && git ls-files -z | tar --null -T - -cf -) | tar -xf - -C "$T/d"
# The new scripts may not be tracked yet; a template copy carries them.
cp "$HERE/tools/rollout-aspect.sh" "$HERE/tools/frame-accept.sh" "$HERE/tools/frame_accept.py" "$T/d/tools/"
git -C "$T/d" init -q
commit_all "$T/d" base
"$HERE/tools/rollout-aspect.sh" "$T/d" 1:1 > "$T/d.out" 2>&1 || { echo "FAIL d: rollout-aspect.sh exited non-zero"; cat "$T/d.out"; exit 1; }
changed="$(git -C "$T/d" status --porcelain | sort | tr '\n' ';')"
[ "$changed" = " M src/display.rs;?? tests/display_contract.rs;" ] \
  || { echo "FAIL d: expected only src/display.rs and tests/display_contract.rs to change, got: $changed"; cat "$T/d.out"; exit 1; }
git -C "$T/d" diff -U0 src/display.rs | grep '^[-+][^-+]' | grep -qv 'GAME_WIDTH\|GAME_HEIGHT' \
  && { echo "FAIL d: src/display.rs changed beyond the two constants"; exit 1; }
if ! grep -q 'pub const GAME_WIDTH: u32 = 720;' "$T/d/src/display.rs" || ! grep -q 'pub const GAME_HEIGHT: u32 = 720;' "$T/d/src/display.rs"; then
  echo "FAIL d: 1:1 did not pin 720x720"; exit 1
fi
grep -q 'fn pinned_size_is_square' "$T/d/tests/display_contract.rs" || { echo "FAIL d: display_contract.rs is not the 1:1 one"; exit 1; }
grep -q 'files in place' "$T/d.out" || { echo "FAIL d: no 'files in place' line"; cat "$T/d.out"; exit 1; }
commit_all "$T/d" converted
"$HERE/tools/rollout-aspect.sh" "$T/d" 1:1 > "$T/d2.out" 2>&1 || { echo "FAIL d: second run exited non-zero"; cat "$T/d2.out"; exit 1; }
grep -q 'nothing to do' "$T/d2.out" || { echo "FAIL d: second run did not report nothing to do"; cat "$T/d2.out"; exit 1; }
[ -z "$(git -C "$T/d" status --porcelain)" ] || { echo "FAIL d: second run changed files"; git -C "$T/d" status --short; exit 1; }
# A dirty tree is refused.
echo x >> "$T/d/README.md"
if "$HERE/tools/rollout-aspect.sh" "$T/d" 1:1 > "$T/d3.out" 2>&1; then echo "FAIL d: a dirty tree was not refused"; exit 1; fi
git -C "$T/d" checkout -q README.md

# (e)
PACK_SRC="${PACK_SRC:-$HERE/../../games/pack-the-ripper}"
if [ -d "$PACK_SRC/.git" ]; then
  git clone -q "$PACK_SRC" "$T/e"
  # Stale template-only files a copy step may have left behind.
  cp "$HERE/tools/test_rollouts.sh" "$HERE/tools/rollout-record.sh" "$T/e/tools/"
  commit_all "$T/e" stale
  "$HERE/tools/rollout-aspect.sh" "$T/e" 4:3 > "$T/e.out" 2>&1 || { echo "FAIL e: rollout-aspect.sh exited non-zero"; cat "$T/e.out"; exit 1; }
  has() { grep -q "$1" "$T/e/$2" || { echo "FAIL e: $2 lacks: $1"; cat "$T/e.out"; exit 1; }; }
  has '^capture = \[\]' Cargo.toml
  has '^autopilot = \["capture"\]' Cargo.toml
  has '^harness = \["capture"\]' Cargo.toml
  has '^record = \["autopilot"\]' Cargo.toml
  has 'gamebient-input.*tag = "v0.4.0"' Cargo.toml
  has 'pub const GAME_WIDTH: u32 = 960;' src/display.rs
  has 'pub const GAME_HEIGHT: u32 = 720;' src/display.rs
  has 'HIGHER_SCORE_IS_BETTER: bool = true;' src/game/scoring.rs
  has 'add_message::<Highlight>()' src/game/host.rs
  has 'report_highlight,' src/game/host.rs
  if [ ! -f "$T/e/tests/display_contract.rs" ] || [ ! -f "$T/e/tests/display_shape.rs" ]; then echo "FAIL e: tests not written"; exit 1; fi
  grep -q 'pack_the_ripper::display' "$T/e/tests/display_shape.rs" || { echo "FAIL e: display_shape.rs crate name not rewritten"; exit 1; }
  if [ ! -f "$T/e/.claude/skills/designing-cartridge-covers/SKILL.md" ] || [ ! -f "$T/e/.claude/skills/generating-cartridge-metadata/SKILL.md" ]; then
    echo "FAIL e: skill snapshots missing"; exit 1
  fi
  stray="$(find "$T/e/tools" -maxdepth 1 \( -name 'test_*.sh' -o -name 'rollout-*.sh' \) | tr '\n' ' ')"
  [ -z "$stray" ] || { echo "FAIL e: template-only scripts left in the game: $stray"; exit 1; }
  [ -f "$T/e/tools/test_cut_clips.py" ] || { echo "FAIL e: the game's test_cut_clips.py was removed"; exit 1; }
  grep -q '^HAND EDIT: src/main.rs:' "$T/e.out" || { echo "FAIL e: no src/main.rs HAND EDIT"; cat "$T/e.out"; exit 1; }
  # shellcheck disable=SC2016
  grep -q '^HAND EDIT: src/frame/driver.rs: GameData has no `pub score:`' "$T/e.out" \
    || { echo "FAIL e: no driver.rs helper HAND EDIT (score field is \`value\`)"; cat "$T/e.out"; exit 1; }
  grep -q '^HAND EDIT: confirm HIGHER_SCORE_IS_BETTER = true' "$T/e.out" || { echo "FAIL e: no HIGHER_SCORE_IS_BETTER confirm line"; exit 1; }
else
  echo "test_rollouts: SKIP e (no $PACK_SRC)"
fi

# (f)
"$HERE/tools/frame-accept.sh" --check-copies "$T/d" > "$T/f1.out" 2>&1 || { echo "FAIL f: --check-copies failed on the converted template copy"; cat "$T/f1.out"; exit 1; }
if [ -d "$T/e" ]; then
  "$HERE/tools/frame-accept.sh" --check-copies "$T/e" > "$T/f2.out" 2>&1 || { echo "FAIL f: --check-copies failed on the converted Pack The Ripper"; cat "$T/f2.out"; exit 1; }
fi
echo "// drift" >> "$T/d/src/frame/layout.rs"
if "$HERE/tools/frame-accept.sh" --check-copies "$T/d" > "$T/f3.out" 2>&1; then
  echo "FAIL f: --check-copies passed a modified src/frame/layout.rs"; exit 1
fi
if ! grep -q 'src/frame/layout.rs' "$T/f3.out" || ! grep -q '^+// drift' "$T/f3.out"; then
  echo "FAIL f: the failure does not name the file and hunk"; cat "$T/f3.out"; exit 1
fi
git -C "$T/d" checkout -q src/frame/layout.rs
# A hand edit in driver.rs's test module is allowed; one above it is not.
echo "// hand edit" >> "$T/d/src/frame/driver.rs"
"$HERE/tools/frame-accept.sh" --check-copies "$T/d" > /dev/null 2>&1 || { echo "FAIL f: an edit inside driver.rs's test module was refused"; exit 1; }
git -C "$T/d" checkout -q src/frame/driver.rs
perl -0pi -e 's/^#\[cfg\(test\)\]/\/\/ drift\n#[cfg(test)]/m' "$T/d/src/frame/driver.rs"
if "$HERE/tools/frame-accept.sh" --check-copies "$T/d" > /dev/null 2>&1; then echo "FAIL f: an edit above driver.rs's test module was allowed"; exit 1; fi
git -C "$T/d" checkout -q src/frame/driver.rs

# (g)
mkdir -p "$T/g/src"
printf '[package]\nname = "scratch-game"\nversion = "0.1.0"\n\n[features]\ncapture = []\nautopilot = ["capture"]\n' > "$T/g/Cargo.toml"
"$HERE/tools/rollout-record.sh" "$T/g" > "$T/g.out" 2>&1 || { echo "FAIL g: rollout-record.sh exited non-zero"; cat "$T/g.out"; exit 1; }
grep -q '^record = \["autopilot"\]' "$T/g/Cargo.toml" || { echo "FAIL g: no record feature after a capture-aware autopilot line"; cat "$T/g/Cargo.toml"; exit 1; }
grep -q "HAND EDIT: add 'record = " "$T/g.out" && { echo "FAIL g: rollout-record asked for a hand edit on the record feature"; exit 1; }

# (h)
if python3 -c 'import PIL' 2>/dev/null; then
  python3 - "$T/h" <<'PY'
import sys
from pathlib import Path
from PIL import Image, ImageDraw

out = Path(sys.argv[1])
# 1920x1080, 3:4 game: rect x=561 y=8 w=798 h=1064, gap 8.
def shot(name, bezel, game, gap_lit=False):
    d = out / "ok"
    d.mkdir(parents=True, exist_ok=True)
    im = Image.new("RGB", (1920, 1080), (bezel, bezel, bezel))
    dr = ImageDraw.Draw(im)
    dr.rectangle((553, 0, 1366, 1079), fill=(0, 0, 0))
    dr.rectangle((561, 8, 561 + 797, 8 + 1063), fill=(game, game, game))
    im.save(d / name)
    if gap_lit:
        b = out / "bad"
        b.mkdir(parents=True, exist_ok=True)
        dr.rectangle((553, 0, 560, 1079), fill=(200, 200, 200))
        im.save(b / name)
shot("02-title.png", 60, 40)
shot("05-mid-play.png", 20, 120, True)
shot("08-pause.png", 21, 90)
PY
  "$HERE/tools/frame-accept.sh" --shots "$T/h/ok" --window 1920x1080 --ratio 3:4 --frame on > "$T/h1.out" 2>&1 \
    || { echo "FAIL h: synthetic good captures did not pass"; cat "$T/h1.out"; exit 1; }
  cp "$T/h/ok/02-title.png" "$T/h/ok/08-pause.png" "$T/h/bad/"
  if "$HERE/tools/frame-accept.sh" --shots "$T/h/bad" --window 1920x1080 --ratio 3:4 --frame on > "$T/h2.out" 2>&1; then
    echo "FAIL h: a lit gap passed"; exit 1
  fi
  grep -q 'gap not black' "$T/h2.out" || { echo "FAIL h: the lit gap was not named"; cat "$T/h2.out"; exit 1; }
  mkdir -p "$T/h/full"
  python3 - "$T/h/full" <<'PY'
import sys
from PIL import Image
for n in ("02-title.png", "05-mid-play.png", "08-pause.png"):
    Image.new("RGB", (1440, 1080), (60, 60, 60)).save(f"{sys.argv[1]}/{n}")
PY
  "$HERE/tools/frame-accept.sh" --shots "$T/h/full" --window 1440x1080 --ratio 4:3 --frame off > "$T/h4.out" 2>&1 \
    || { echo "FAIL h: --frame off at the game's own aspect failed"; cat "$T/h4.out"; exit 1; }
  grep -q 'no bars at this window; nothing to check' "$T/h4.out" || { echo "FAIL h: no 'no bars' message"; cat "$T/h4.out"; exit 1; }
  if "$HERE/tools/frame-accept.sh" --shots "$T/h/ok" --window 1920x1080 --ratio 4:3 --frame on > "$T/h3.out" 2>&1; then
    echo "FAIL h: captures passed under the wrong ratio"; exit 1
  fi
else
  echo "test_rollouts: SKIP h (no Pillow)"
fi

# (i)
GULPER_SRC="${GULPER_SRC:-$HERE/../../games/gulper}"
if [ -d "$GULPER_SRC/.git" ]; then
  git clone -q "$GULPER_SRC" "$T/i"
  "$HERE/tools/rollout-aspect.sh" "$T/i" 4:3 > "$T/i.out" 2>&1 || { echo "FAIL i: rollout-aspect.sh exited non-zero"; cat "$T/i.out"; exit 1; }
  (cd "$T/i" && cargo fmt --all)
  "$HERE/tools/frame-accept.sh" --check-copies "$T/i" > "$T/i2.out" 2>&1 || { echo "FAIL i: --check-copies failed on gulper after rollout-aspect.sh and cargo fmt"; cat "$T/i2.out"; exit 1; }
  echo '#!/bin/sh' > "$T/i/tools/test_stray.sh"
  if "$HERE/tools/frame-accept.sh" --check-copies "$T/i" > "$T/i3.out" 2>&1; then echo "FAIL i: a stray tools/test_*.sh was allowed"; exit 1; fi
else
  echo "test_rollouts: SKIP i (no $GULPER_SRC)"
fi

# (j)
HUNTED_SRC="${HUNTED_SRC:-$HERE/../../games/Hunted}"
if [ -d "$HUNTED_SRC/.git" ] && [ ! -f "$HUNTED_SRC/src/game/mod.rs" ]; then
  git clone -q "$HUNTED_SRC" "$T/j1"
  "$HERE/tools/rollout-aspect.sh" "$T/j1" 4:3 > "$T/j1.out" 2>&1 || { echo "FAIL j: rollout-aspect.sh failed on the flat layout"; cat "$T/j1.out"; exit 1; }
  grep -q '^HAND EDIT: sim-sources.txt: list the sim .rs files (one per line)' "$T/j1.out" || { echo "FAIL j: no sim-sources.txt HAND EDIT for a flat layout"; cat "$T/j1.out"; exit 1; }
else
  echo "test_rollouts: SKIP j (flat layout: no $HUNTED_SRC)"
fi
MOLEMAN_SRC="${MOLEMAN_SRC:-$HERE/../../games/moleman-racing}"
if [ -d "$MOLEMAN_SRC/.git" ] && [ ! -f "$MOLEMAN_SRC/src/lib.rs" ]; then
  git clone -q "$MOLEMAN_SRC" "$T/j2"
  "$HERE/tools/rollout-aspect.sh" "$T/j2" 4:3 > "$T/j2.out" 2>&1 || { echo "FAIL j: rollout-aspect.sh failed without src/lib.rs"; cat "$T/j2.out"; exit 1; }
  grep -q '^HAND EDIT: src/lib.rs: this crate needs a lib target' "$T/j2.out" || { echo "FAIL j: no lib target HAND EDIT"; cat "$T/j2.out"; exit 1; }
else
  echo "test_rollouts: SKIP j (no lib.rs-less $MOLEMAN_SRC)"
fi

# (k) on the template copy (d), which is committed and clean again.
printf 'pub fn a(\n    camera_q: Query<(&Camera, &GlobalTransform)>,\n    ui_scale: Res<UiScale>,\n    items: Query<&GlobalTransform, With<Spin>>,\n) {}\npub fn b(\n    c: Query<\n        (&Camera, &GlobalTransform),\n        With<MainCamera>,\n    >,\n    items: Query<&GlobalTransform, With<Spin>>,\n) {}\n' > "$T/d/src/ui/zz.rs"
git -C "$T/d" checkout -q . && git -C "$T/d" add -A && git -C "$T/d" -c user.name=t -c user.email=t@t commit -qm zz
"$HERE/tools/rollout-aspect.sh" "$T/d" 1:1 > "$T/k.out" 2>&1 || { echo "FAIL k: rollout-aspect.sh failed"; cat "$T/k.out"; exit 1; }
grep -q '^HAND EDIT: src/ui/zz.rs:2: camera query' "$T/k.out" || { echo "FAIL k: the bare camera query beside a With<Spin> query was not flagged"; cat "$T/k.out"; exit 1; }
if grep -q '^HAND EDIT: src/ui/zz.rs:\(7\|8\|9\|10\|11\):' "$T/k.out"; then echo "FAIL k: a With<> inside the camera's own query was flagged"; cat "$T/k.out"; exit 1; fi

echo "test_rollouts: OK"
