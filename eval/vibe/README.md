# Vibe-coding arm of the natural-language rules eval

The design spec
(`docs/superpowers/specs/2026-11-01-natural-language-rules-design.md`) proposes
that beginners type rules in plain English, and a model translates them into a
small fixed rule vocabulary. This directory tests the alternative: **no
vocabulary at all**. A coding agent gets the beginner's sentence and edits the
Rust game directly.

Both arms use the same 51 sentences
(`2026-11-01-natural-language-rules-eval.jsonl`) and the same starting game,
so their results can be compared case by case.

## What's here

| Path | What it is |
|---|---|
| `BASE` | The template commit the fixture is built on (pinned, so the fixture never drifts) |
| `fixture/` | Files laid over `BASE` to make **Coin Run**, the eval's test game, as real Rust |
| `setup/*.patch` | Rules that some context cases (g01, g03, g05, g06) start with |
| `hidden/<id>.rs` | One behaviour test per case. The agent never sees these |
| `prompt.md` | What the agent is told. `{request}` is replaced by the sentence |
| `run.py` | The runner |

### Coin Run

Coin Run is the rules arm's fixture scene written as a template game:

- a side-view platformer in game pixels (960 × 720, origin at the centre)
- seven kinds of thing: Player, Platform, Coin, Enemy, Bullet, Goal, Spike
- gravity and solid platforms
- counters `score`, `lives` (3) and `coins`
- **no rules**: touching things does nothing, and there's no jump or
  shooting yet

`fixture/src/game/scenario.rs` is a headless harness for checking behaviour:
place things, hold buttons for N ticks, count and locate things by kind. The
hidden tests are written against it, and the fixture's `AGENTS.md` tells the
agent to keep its public functions working, as an editor would require.

## Running it

You need the native Bevy build dependencies (as in `.github/workflows/ci.yml`:
`libasound2-dev libudev-dev libwayland-dev libxkbcommon-dev pkg-config`), Rust,
and a signed-in Claude Code CLI (`claude`).

```sh
python3 eval/vibe/run.py --fixture-only      # build and warm the cache once (~15 min)
python3 eval/vibe/run.py --cases a01         # one case
python3 eval/vibe/run.py --all               # every case
python3 eval/vibe/run.py --all --repeat 5    # what the spec asks for
python3 eval/vibe/run.py --chain             # 17 requests piled onto one game
```

`--apply PATCH` replaces the agent with a known change. It's a free dry run
for checking the pipeline itself; for example,
`--cases a01 --apply eval/vibe/setup/coin_counts.patch` should come out green.

**Permissions:** the agent runs headless (`claude -p`) with edits accepted and
an allowlist of commands: `cargo`, read-only `git` and `ls`. Everything else
is denied, because nobody is there to approve a prompt. It can't commit, push
or reach outside the case's worktree with a shell command.

`--model` picks the agent's model. Work, the shared cargo target directory and
results go to `~/.cache/gamebient-vibe-eval` (`--work` to change it). Cases
that already have results are skipped unless you pass `--force`.

**Cost:** each case is a full agent session that edits code and runs cargo
several times. My guess, not yet measured: 5–20 minutes and a few dollars of model usage per
case, so `--all --repeat 5` is a few hundred agent sessions. Start with a
handful of cases.

## What each case records

`results/<id>.json`, plus the agent's diff as `results/<id>.diff`:

- **Reply:** `changes` (ended with `READBACK:`), `question`, `cannot` or
  `unclear`, plus the text.
- **Time:** the agent's own time, and a fresh `cargo build` afterwards. Their
  sum is the closest equivalent to the rules arm's "the change shows up in the
  preview".
- **Drift:** files changed, lines added and removed, and whether the replay
  fixture was regenerated.
- **CI gates**, exactly as `.github/workflows/ci.yml` runs them: `fmt`,
  `clippy -D warnings`, and `cargo test --all-targets --all-features`. The
  test suite includes the template's determinism tests (`windowed_shape`,
  `archetype_order`, `selftest`), so a change that breaks replay verification
  fails here.
- **Hidden test:** `pass`, `fail`, `compile_error` (usually the harness API
  was broken) or `none`.

`summary.md` gives the headline numbers and one row per case. In chain mode,
`chain.md` shows after each step how many earlier requests still pass their
hidden tests.

## Grading

The automatic numbers are a first pass. A person still grades:

- **Readbacks**, against the `must` lines in the eval set, as for the rules
  arm.
- **Questions** (e-cases, g04): is it the right question?
- **Cases with no hidden test:** e-cases, b06 ("enemies get faster" has
  nothing moving to speed up), the shake in a12, and the f-cases that aren't
  testable headless (f02, f03, f04, f05, f07).
- **Lenient tests** that say so in their header (b05, f06, d01's "start"): a
  failure there can be a reasonable reading the test didn't anticipate.
  Check the diff.

How to read the f-cases: here they measure **capability**. The rules arm is
expected to refuse them; a vibe-coding agent may simply build them. Count how
many it attempts, and of those, how many work.

## Known limits

- **Native builds only.** The timing is a native incremental build. A web
  preview means a wasm build, which is slower.
- **The agent runs cargo itself**, so agent time already includes several
  builds. Some agents will skip the gates, and the runner catches that.
- **One fixture, one prompt.** Both arms share the eval set's bias: the
  sentences were written by Claude, not by beginners.
- **Chain mode uses 17 compatible requests**, not the 30 I guessed at. Past
  that, requests start contradicting each other in ways a hidden test can't
  judge.
