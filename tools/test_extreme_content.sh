#!/usr/bin/env bash
# Proves the content kit's promise: a data save to content/ never breaks
# `cargo test`. Copies the repo to a scratch folder, saves every tunable at
# its max (then, in a second run, at its min) and edits content/level.json
# (one Thing moved, one removed), then runs the full test suite there.
#
# The tunables file comes from the ignored unit test
# `write_extreme_tunables` (src/game/content/saved.rs), which reads the
# game's own tunable index, so the script never goes stale when a game adds
# or changes a tunable. The copy builds in its own target dir,
# target/extreme-content (the first run compiles every dependency once).
# It must not share target/ with this repo: cargo gives both copies the
# same artifact names, so the copy's build.rs output (absolute
# include_bytes! paths into the scratch folder) would be reused here.
#
# Usage: tools/test_extreme_content.sh [max|min ...]   (default: max min)
# GX_EXTREME_TARGET overrides the copy's target dir.
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
ends=("$@")
[ ${#ends[@]} -gt 0 ] || ends=(max min)

work=$(mktemp -d "${TMPDIR:-/tmp}/gx-extreme.XXXXXX")
trap 'rm -rf "$work"' EXIT
copy_target="${GX_EXTREME_TARGET:-$root/target/extreme-content}"

# Tracked and untracked-but-not-ignored files: never target/ or dist/.
copy="$work/game"
mkdir -p "$copy"
(cd "$root" && git ls-files -z --cached --others --exclude-standard |
  while IFS= read -r -d '' f; do [ -e "$f" ] && printf '%s\0' "$f"; done |
  tar --null -T - -cf -) | tar -C "$copy" -xf -

# One Thing moved (the first, 8 px right) and one removed (the last Thing
# of a kind the level holds more than once, so no one-of-a-kind Thing such
# as the player or the goal disappears).
python3 - "$copy/content/level.json" <<'EOF'
import collections, json, sys
path = sys.argv[1]
with open(path) as f:
    level = json.load(f)
things = level["things"]
counts = collections.Counter(t["kind"] for t in things)
dupes = [i for i, t in enumerate(things) if counts[t["kind"]] > 1]
if not dupes:
    sys.exit("level.json has no kind placed twice; nothing safe to remove")
removed = things.pop(dupes[-1])
things[0]["at"][0] += 8
with open(path, "w") as f:
    f.write(json.dumps(level, indent=2) + "\n")
print(f"level.json: moved {things[0]['id']}, removed {removed['id']}")
EOF

for end in "${ends[@]}"; do
  echo "== every tunable at its $end"
  (cd "$root" && GX_EXTREME="$end" GX_EXTREME_OUT="$copy/content/tunables.json" \
    cargo test --lib --all-features -- --ignored --exact \
    game::content::saved::tests::write_extreme_tunables --quiet)
  (cd "$copy" && CARGO_TARGET_DIR="$copy_target" cargo test --all-targets --all-features)
done
echo "extreme content: cargo test passed for: ${ends[*]}"
