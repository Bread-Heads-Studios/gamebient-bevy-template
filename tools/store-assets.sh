#!/usr/bin/env bash
# Builds the store assets the ColecoVision GX site renders on a game page
# from an existing tools/record.sh capture, and rewrites assets/info.json:
#   assets/screenshots/01..04.png  <- build/record/shots/{04,05,06,07}-*.png at 1280x720
#   assets/trailer.mp4             <- build/record/clips/signature.mp4 (fallback 05-mid-play, 04-early-play)
#   properties.screenshots / trailer_url / version, released_at (only when
#   absent or --released-at given). Developer / Developer URL / Tags are only
#   written with --set-*; missing ones are reported so a human fills them in.
# Metadata contract: the website's docs/publish-api.md.
#
# Usage: tools/store-assets.sh [--released-at YYYY-MM-DD] [--set-developer NAME]
#        [--set-developer-url URL] [--set-tags "a, b"] [--dry-run]
# Must be run from a game repo root (paths below are relative to $PWD, not
# to this script's own location, since rollout-store-assets.sh copies this
# file into each game's tools/ but the game repo is what has assets/build/).
set -euo pipefail

RELEASED_AT=""; DEVELOPER=""; DEVELOPER_URL=""; TAGS=""; DRY=0
while [ $# -gt 0 ]; do
  case "$1" in
    --released-at) RELEASED_AT="$2"; shift 2 ;;
    --set-developer) DEVELOPER="$2"; shift 2 ;;
    --set-developer-url) DEVELOPER_URL="$2"; shift 2 ;;
    --set-tags) TAGS="$2"; shift 2 ;;
    --dry-run) DRY=1; shift ;;
    *) echo "usage: $0 [--released-at YYYY-MM-DD] [--set-developer NAME] [--set-developer-url URL] [--set-tags \"a, b\"] [--dry-run]" >&2; exit 2 ;;
  esac
done
[ -z "$RELEASED_AT" ] || [[ "$RELEASED_AT" =~ ^[0-9]{4}-[0-9]{2}-[0-9]{2}$ ]] || { echo "store-assets: --released-at must be YYYY-MM-DD" >&2; exit 2; }
command -v ffmpeg >/dev/null 2>&1 || { echo "store-assets: ffmpeg not on PATH (brew install ffmpeg)" >&2; exit 1; }
command -v python3 >/dev/null 2>&1 || { echo "store-assets: python3 not on PATH" >&2; exit 1; }
[ -f assets/info.json ] || { echo "store-assets: assets/info.json not found" >&2; exit 1; }

SHOTS=build/record/shots; CLIPS=build/record/clips
MAX_TRAILER_BYTES=8000000; MAX_TRAILER_SECONDS=20

# Pick the four gameplay beats in order; a missing beat is skipped, none is an error.
picked=()
for beat in 04 05 06 07; do
  f=$(find "$SHOTS" -maxdepth 1 -name "$beat-*.png" 2>/dev/null | sort | head -1 || true)
  [ -n "$f" ] && picked+=("$f")
done
[ ${#picked[@]} -gt 0 ] || { echo "store-assets: no gameplay shots in $SHOTS (run tools/record.sh first)" >&2; exit 1; }

trailer_src=""
for c in signature.mp4 05-mid-play.mp4 04-early-play.mp4; do
  [ -f "$CLIPS/$c" ] && { trailer_src="$CLIPS/$c"; break; }
done

if [ $DRY -eq 1 ]; then
  echo "would write ${#picked[@]} screenshots from: ${picked[*]}"
  echo "would write trailer from: ${trailer_src:-<none>}"
  exit 0
fi

mkdir -p assets/screenshots
rm -f assets/screenshots/0[1-6].png
i=0; urls=()
for f in "${picked[@]}"; do
  i=$((i+1)); n=$(printf '%02d' "$i")
  ffmpeg -v error -y -i "$f" -vf "scale=1280:720:flags=lanczos" -frames:v 1 "assets/screenshots/$n.png"
  urls+=("$n.png")
done

trailer_written=0
if [ -n "$trailer_src" ]; then
  ffmpeg -v error -y -i "$trailer_src" -t "$MAX_TRAILER_SECONDS" \
    -vf "scale=1280:720:flags=lanczos" -c:v libx264 -preset slow -crf 23 -pix_fmt yuv420p \
    -c:a aac -b:a 96k -movflags +faststart -f mp4 assets/trailer.mp4.tmp
  size=$(stat -f%z assets/trailer.mp4.tmp 2>/dev/null || stat -c%s assets/trailer.mp4.tmp)
  if [ "$size" -gt "$MAX_TRAILER_BYTES" ]; then
    rm -f assets/trailer.mp4.tmp
    echo "store-assets: trailer would be $size bytes (> $MAX_TRAILER_BYTES); not written" >&2; exit 1
  fi
  mv assets/trailer.mp4.tmp assets/trailer.mp4; trailer_written=1
fi

pkg_version=$(sed -n 's/^version *= *"\(.*\)"/\1/p' Cargo.toml | head -1)
sha=$(git rev-parse --short=7 HEAD 2>/dev/null || echo unknown)
VERSION="${pkg_version:-0.0.0}+$sha"

# Rewrite info.json. python3 keeps key order and 4-space indent to match the file.
SCREENSHOTS="${urls[*]}" TRAILER="$trailer_written" VERSION="$VERSION" RELEASED_AT="$RELEASED_AT" \
DEVELOPER="$DEVELOPER" DEVELOPER_URL="$DEVELOPER_URL" TAGS="$TAGS" python3 - <<'PY'
import json, os
p = "assets/info.json"
d = json.load(open(p))
props = d.setdefault("properties", {})
base = (props.get("game_url") or props.get("demo_url") or "").rstrip("/")
if not base:
    raise SystemExit("store-assets: info.json has no game_url/demo_url to build asset URLs from")
props["screenshots"] = [f"{base}/assets/screenshots/{n}" for n in os.environ["SCREENSHOTS"].split()]
if os.environ["TRAILER"] == "1":
    props["trailer_url"] = f"{base}/assets/trailer.mp4"
props["version"] = os.environ["VERSION"]
if os.environ["RELEASED_AT"]:
    props["released_at"] = os.environ["RELEASED_AT"]
attrs = d.setdefault("attributes", [])
def set_attr(trait, value):
    for a in attrs:
        if a.get("trait_type") == trait:
            a["value"] = value; return
    attrs.append({"trait_type": trait, "value": value})
def get_attr(trait):
    return next((a.get("value") for a in attrs if a.get("trait_type") == trait), None)
def is_unset(v):
    # Absent, or still the template's "PLACEHOLDER: ..." marker (assets/info.json's
    # established convention for a value a human must replace before publishing).
    return not v or (isinstance(v, str) and v.strip().upper().startswith("PLACEHOLDER"))
if os.environ["DEVELOPER"]: set_attr("Developer", os.environ["DEVELOPER"].strip())
if os.environ["DEVELOPER_URL"]: set_attr("Developer URL", os.environ["DEVELOPER_URL"].strip())
if os.environ["TAGS"]:
    seen = []
    for t in os.environ["TAGS"].split(","):
        t = t.strip().lower()
        if t and t not in seen: seen.append(t)
    set_attr("Tags", ", ".join(seen[:8]))
missing = [k for k, v in (("released_at", props.get("released_at")), ("Developer", get_attr("Developer")),
           ("Developer URL", get_attr("Developer URL")), ("Tags", get_attr("Tags"))) if is_unset(v)]
with open(p, "w") as fh:
    json.dump(d, fh, indent=4, ensure_ascii=False)
    fh.write("\n")
print(f"store-assets: wrote {len(props['screenshots'])} screenshots, trailer={'yes' if os.environ['TRAILER']=='1' else 'no'}, version={props['version']}")
if missing:
    print("store-assets: still missing (fill by hand or with --set-*): " + ", ".join(missing))
PY
