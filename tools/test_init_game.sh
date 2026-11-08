#!/usr/bin/env bash
# Runs init-game.sh on a pristine copy of HEAD and checks the result. Works under
# BSD and GNU sed. Usage: tools/test_init_game.sh
set -euo pipefail
cd "$(dirname "$0")/.."
t=$(mktemp -d)
trap 'rm -rf "$t"' EXIT
git archive HEAD | tar -x -C "$t"
# init-game.sh falls back to find when the copy is not a git repo.
fail=0
(cd "$t" && ./init-game.sh "Test Game" OWNER/REPO >/dev/null)
check() { # check <description> <command...>
    local d="$1"; shift
    if "$@"; then echo "ok: $d"; else echo "FAIL: $d" >&2; fail=1; fi
}
check "Cargo.toml renamed to test-game" grep -q 'name = "test-game"' "$t/Cargo.toml"
check "no *.gxbak files left" test -z "$(find "$t" -name '*.gxbak')"
check "init-game.sh removed itself" test ! -e "$t/init-game.sh"
exit $fail
