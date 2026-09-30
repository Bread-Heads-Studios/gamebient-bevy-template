#!/usr/bin/env bash
# Tests tools/game-size.sh on scratch display.rs files:
#   (a) the three sanctioned sizes and the legacy 16:9 size print as <width>x<height>;
#   (b) indentation and trailing comments on the const lines are accepted, and
#       an explicit path argument is read instead of src/display.rs;
#   (c) a missing file, a missing constant or a non-numeric constant exits
#       non-zero, names what is missing, and prints no size;
#   (d) --scale multiplies both sides and rejects a scale that gives a
#       fractional or odd side;
#   (e) GAME_SIZE overrides the file and is validated;
#   (f) the template's own src/display.rs parses.
set -euo pipefail
HERE="$(cd "$(dirname "$0")/.." && pwd)"
TOOL="$HERE/tools/game-size.sh"
unset GAME_SIZE

mk() {  # mk <dir> <width> <height>: scratch game with a Contract C display.rs
  mkdir -p "$1/src"
  printf 'pub const GAME_WIDTH: u32 = %s;   // per game\npub const GAME_HEIGHT: u32 = %s;  // per game\npub const REFERENCE_SHORT_SIDE: f32 = 720.0;\n' "$2" "$3" > "$1/src/display.rs"
}
want() {  # want <label> <expected> <actual>
  [ "$2" = "$3" ] || { echo "FAIL $1: want '$2', got '$3'"; exit 1; }
}
refuses() {  # refuses <label> <text the message must contain> <command...>
  local label="$1" text="$2"; shift 2
  if "$@" >"$T/out" 2>"$T/err"; then echo "FAIL $label: should exit non-zero"; exit 1; fi
  grep -qF -- "$text" "$T/err" || { echo "FAIL $label: message lacks '$text': $(cat "$T/err")"; exit 1; }
  [ ! -s "$T/out" ] || { echo "FAIL $label: printed a size anyway: $(cat "$T/out")"; exit 1; }
}

T="$(mktemp -d)"; trap 'rm -rf "$T"' EXIT

# (a)
mk "$T/a43" 960 720; mk "$T/a11" 720 720; mk "$T/a34" 720 960; mk "$T/a169" 1280 720
want "a 4:3"  "960x720"  "$(cd "$T/a43" && "$TOOL")"
want "a 1:1"  "720x720"  "$(cd "$T/a11" && "$TOOL")"
want "a 3:4"  "720x960"  "$(cd "$T/a34" && "$TOOL")"
want "a 16:9" "1280x720" "$(cd "$T/a169" && "$TOOL")"

# (b)
printf '    pub const GAME_WIDTH: u32 = 720; // portrait\n\tpub const GAME_HEIGHT: u32 = 960;\n' > "$T/b.rs"
want "b path" "720x960" "$(cd "$T" && "$TOOL" "$T/b.rs")"

# (c)
mkdir -p "$T/c"
refuses "c no file" "src/display.rs not found" bash -c "cd '$T/c' && '$TOOL'"
printf 'pub const GAME_WIDTH: u32 = 960;\n' > "$T/c-w.rs"
refuses "c no height" "GAME_HEIGHT" "$TOOL" "$T/c-w.rs"
printf 'pub const GAME_HEIGHT: u32 = 720;\n' > "$T/c-h.rs"
refuses "c no width" "GAME_WIDTH" "$TOOL" "$T/c-h.rs"
printf 'pub const GAME_WIDTH: u32 = SHORT * 4 / 3;\npub const GAME_HEIGHT: u32 = 720;\n' > "$T/c-expr.rs"
refuses "c expression" "GAME_WIDTH" "$TOOL" "$T/c-expr.rs"

# (d)
want "d 1.5 of 4:3" "1440x1080" "$(cd "$T/a43" && "$TOOL" --scale 1.5)"
want "d 1.5 of 1:1" "1080x1080" "$(cd "$T/a11" && "$TOOL" --scale 1.5)"
want "d 1.5 of 3:4" "1080x1440" "$(cd "$T/a34" && "$TOOL" --scale 1.5)"
want "d 1.0 of 3:4" "720x960"   "$(cd "$T/a34" && "$TOOL" --scale 1.0)"
refuses "d fractional" "fractional or odd" "$TOOL" --scale 1.01 "$T/a34/src/display.rs"
refuses "d odd" "fractional or odd" "$TOOL" --scale 1.0125 "$T/a34/src/display.rs"
refuses "d not a number" "--scale" "$TOOL" --scale big "$T/a34/src/display.rs"
refuses "d no value" "usage" "$TOOL" --scale

# (e)
want "e override" "1280x720" "$(cd "$T/c" && GAME_SIZE=1280x720 "$TOOL")"
want "e override scaled" "1920x1080" "$(cd "$T/c" && GAME_SIZE=1280x720 "$TOOL" --scale 1.5)"
want "e override beats the file" "1280x720" "$(cd "$T/a43" && GAME_SIZE=1280x720 "$TOOL")"
refuses "e bad override" "GAME_SIZE" env GAME_SIZE=wide "$TOOL" "$T/a43/src/display.rs"

# (f)
want "f template" "960x720" "$(cd "$HERE" && "$TOOL")"

echo "test_game_size: OK"
