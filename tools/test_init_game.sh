#!/usr/bin/env bash
# Runs init-game.sh on a pristine copy of HEAD and checks the result, in both
# file-list modes: A) no .git (the find fallback), B) a git repo (the
# `git ls-files` branch a clone uses). Works under BSD and GNU sed.
# Usage: tools/test_init_game.sh
set -euo pipefail
cd "$(dirname "$0")/.."
fail=0
check() { # check <description> <command...>
    local d="$1"; shift
    if "$@"; then echo "ok: $d"; else echo "FAIL: $d" >&2; fail=1; fi
}
no_tokens() { # no template token left outside .git
    [ -z "$(grep -rIlE 'gamebient-game|gamebient_game|GAMEBIENT GAME|Gamebient Game|OWNER/REPO' "$1" --exclude-dir=.git)" ]
}
run_case() { # run_case <label> <git|nogit>
    local label="$1" mode="$2" t
    t=$(mktemp -d)
    git archive HEAD | tar -x -C "$t"
    if [ "$mode" = git ]; then
        (cd "$t" && git init -q && git add -A)
    fi
    (cd "$t" && ./init-game.sh "Test Game" acme/test-repo >/dev/null)
    check "$label: Cargo.toml renamed to test-game" grep -q 'name = "test-game"' "$t/Cargo.toml"
    check "$label: no *.gxbak files left" test -z "$(find "$t" -name '*.gxbak')"
    check "$label: no template token remains" no_tokens "$t"
    check "$label: init-game.sh removed itself" test ! -e "$t/init-game.sh"
    rm -rf "$t"
}
run_case "find fallback" nogit
run_case "git ls-files" git
exit $fail
