---
paths:
  - "src/game/**"
  - "src/bin/verify.rs"
---

# Sim code

Loaded when you edit game code. The full rules are in `docs/conventions.md`
("Sim determinism", "Presentation only") and `docs/replay-verification.md`;
this is the checklist.

`cargo test` greps for the forbidden names (`thread_rng`, `SmallRng`,
`std::collections::HashMap`, `display::`, ...). These it cannot see, so
check them yourself on every sim change:

- Run state changes only inside `sim::SimSet` (`FixedUpdate`), never in
  `Update`. Sim systems read `TickInput`, not `GameInput`.
- No `Time::elapsed*`, wall clock or frame count in a sim system; use
  `sim::RunClock`. `Time::delta_secs()` is fine inside `FixedUpdate`.
- Run state never lives in a `Local<_>`, a `static` or a plugin-build-time
  cache: put it in a resource or component that `OnEnter(Playing)` resets.
- No sim read of `UiScale`, `Window`, `GameViewport` or `FrameInsets`.
- Fold extra state into `Checksum` when a score could be reached through
  different in-game states.
- Text spawned from `src/game/` takes `Option<Res<UiFonts>>`, never a
  required `Res` (the verifier has no `UiPlugin`).
- `FramePlugin` is added in `main.rs` only, never in `GamePlugin`.

After a change that can alter a run, regenerate the fixture and say so in
the commit:

    cargo run --features verify --bin verify -- --selftest --write tests/fixtures/selftest.gxr

`src/game/autopilot.rs` and `src/game/record/` are dev-only presentation and
do not change the sim.
