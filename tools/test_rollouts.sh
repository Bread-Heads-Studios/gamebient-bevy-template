#!/usr/bin/env bash
# Tests the three small rollout scripts on scratch game dirs:
#   (a) rollout-store-assets.sh copies store-assets.sh AND game-size.sh, both
#       executable; a second run reports the game is current; a stale
#       game-size.sh is replaced;
#   (b) rollout-frame-art.sh copies frame-art.sh and the two placeholder SVGs;
#       a second run keeps a game's own marquee.svg byte for byte and still
#       refreshes frame-art.sh;
#   (c) rollout-record.sh copies game-size.sh beside record.sh, executable.
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

# (d) template-only tests are never rolled out: no rollout script names them.
if grep -n "test_aspect_docs" "$HERE"/tools/rollout-*.sh; then
  echo "FAIL d: a rollout script copies or names test_aspect_docs.sh, which asserts the template's own info.json"
  exit 1
fi

echo "test_rollouts: OK"
