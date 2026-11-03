#!/bin/bash
# PostToolUse (Write|Edit): once per session, when a sim file changes, remind
# Claude that the replay fixture may need regenerating. Never blocks.
input=$(cat)
if command -v jq >/dev/null 2>&1; then
    path=$(jq -r '.tool_input.file_path // empty' <<<"$input")
    session=$(jq -r '.session_id // "none"' <<<"$input")
else
    path=$(grep -oE '"file_path"[[:space:]]*:[[:space:]]*"[^"]*"' <<<"$input" | head -1 | sed -E 's/.*"([^"]*)"$/\1/')
    session=none
fi
root="${CLAUDE_PROJECT_DIR:-$PWD}"
rel="${path#"$root"/}"
case "$rel" in
    src/game/autopilot.rs | src/game/record/*) exit 0 ;;
    src/game/* | src/bin/verify.rs) ;;
    *) exit 0 ;;
esac
marker="${TMPDIR:-/tmp}/gx-sim-changed-${session//[^A-Za-z0-9_-]/}"
[ -e "$marker" ] && exit 0
: >"$marker" 2>/dev/null
msg="Sim code changed ($rel). If this can alter a run, regenerate the replay fixture: cargo run --features verify --bin verify -- --selftest --write tests/fixtures/selftest.gxr (cargo test says when it is stale). Checklist: .claude/rules/sim.md."
printf '{"hookSpecificOutput":{"hookEventName":"PostToolUse","additionalContext":"%s"}}\n' "$msg"
