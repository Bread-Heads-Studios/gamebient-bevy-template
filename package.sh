#!/bin/bash
# Bundle a compiled Linux target into build/gamebient-game-<target>.tar.gz.
#   ./package.sh pi    (expects build.sh pi to have run first)
#   ./package.sh x86   (expects build.sh x86 to have run first)
# The web bundle is produced directly by build_web.sh into dist/.
set -euo pipefail

TARGET="${1:-}"
case "$TARGET" in
    pi)  RUST_TARGET="aarch64-unknown-linux-gnu"; USE_X11=1 ;;
    x86) RUST_TARGET="x86_64-unknown-linux-gnu";  USE_X11=0 ;;
    *)   echo "Usage: $0 <pi|x86>" >&2; exit 1 ;;
esac

BIN="target/$RUST_TARGET/release/gamebient-game"
if [ ! -f "$BIN" ]; then
    echo "ERROR: binary not found at $BIN — run ./build.sh $TARGET first." >&2
    exit 1
fi

STAGE="build/gamebient-game-$TARGET"
rm -rf "$STAGE"
mkdir -p "$STAGE"

cp "$BIN" "$STAGE/gamebient-game"
cp -r assets "$STAGE/assets"

# Generate the launcher. The Pi kiosk needs the x11 winit backend; the x86
# desktop build lets winit auto-select wayland/x11.
{
    echo '#!/bin/bash'
    echo 'cd "$(dirname "$0")"'
    if [ "$USE_X11" = 1 ]; then
        echo 'export WINIT_UNIX_BACKEND=x11'
    fi
    echo './gamebient-game "$@"'
} > "$STAGE/run.sh"
chmod +x "$STAGE/run.sh"

tar -czf "build/gamebient-game-$TARGET.tar.gz" -C build "gamebient-game-$TARGET"
rm -rf "$STAGE"

echo "Packaged build/gamebient-game-$TARGET.tar.gz"
