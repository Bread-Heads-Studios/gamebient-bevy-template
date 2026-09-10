# Games whose layout differs from the template

`rollout-record.sh` assumes `src/game/{mod,autopilot,states,scoring}.rs` and
`src/game/audio/mod.rs::SfxEvent`. These games need hand edits after the
script runs; `cargo check --features record` names the exact lines.

| Game | Difference | What to do |
|---|---|---|
| `Hunted` | No `src/game/` at all; `autopilot.rs`, `audio.rs`, `GameState`, `Paused` live directly under `src/`, all declared/registered from `main.rs` (not a `GamePlugin`); no score resource, a run timer instead. `main()`'s `app` is a plain `App` local (`let mut app = App::new();`), not `&mut App` | The script detects the flat layout (no `src/game/mod.rs`) before copying and drops the recorder folder module straight into `src/record/` (`mod.rs`, `audio.rs`, `mixer.rs`) — no pre-creating `src/game/`, no manual `git mv`/`rmdir`. It still prints "no `src/game/mod.rs`" HAND EDIT lines for the module declaration and plugin block, since those only get auto-wired into `src/game/mod.rs`. Declare `#[cfg(feature = "record")] mod record;` next to `mod autopilot;` in `main.rs`, and add the plugin block next to `app.add_plugins(autopilot::AutopilotPlugin);` using `crate::GameState`, `audio::SfxEvent`, `Paused` — since `app` here is a value, not a reference, every `record::log_*` call needs `&mut app`, not `app` (this differs from every other game, where the block lives inside `GamePlugin::build(&self, app: &mut App)` and `app` is already a reference). The script's autopilot-`shot()` edit needs no change: `autopilot.rs` is a crate-root module (`mod autopilot;` in `main.rs`), so `super::record::RecordBeat` already resolves to `crate::record::RecordBeat` once `record` is also declared at the crate root. `SfxEvent` in `src/audio.rs` derived only `Message`; the script found it and added `Debug` correctly. |
| `pack-the-ripper` | `autopilot.rs` is a crate-root module declared and registered in `main.rs` (not under `src/game/`, and not inside `GamePlugin`), while `record.rs` stays under `src/game/` per the script. `GameData` fields are `value`/`best`/`streak`/`trust`, not `score` | The script's `src/game/mod.rs` regex for `pub mod record;`/the plugin block never matches (it looks for an `AutopilotPlugin` registration inside `game/mod.rs`, which doesn't exist here), so it always prints both HAND EDIT lines — add `#[cfg(feature = "record")] pub mod record;` to `src/game/mod.rs` by hand. Wire the plugin block in `main.rs` instead (next to `app.add_plugins(autopilot::AutopilotPlugin);`), with `app` already `&mut app` via `&mut app` (main's `app` is a local value here too — same gotcha as Hunted) and paths `game::record::…`, `game::states::GameState`, `game::audio::SfxEvent`, `game::scoring::GameData`; log `"score"` from `.value`, `"streak"` from `.streak`, `"trust"` from `.trust`, and `"pause"` from `game::states::Paused`. The script's autopilot-`shot()` edit wires `super::record::RecordBeat`, which is **wrong** here — `src/autopilot.rs` is a crate-root module but `record` lives under `game::record`, so `super::` (= crate root) doesn't reach it; change it by hand to `crate::game::record::RecordBeat`. `SfxEvent` (`src/game/audio/mod.rs`) already derived `Debug`. |
| `voidrunner` | No `SfxEvent` enum (`src/assets/audio.rs` holds `SfxAssets` only); score arrives via `scoring::ScoreEvent { points }`, applied to `scoring::GameData { score, high_score, lives, drift, max_drift, level, next_extra_life }` | Template-shaped otherwise (`GamePlugin::build` in `src/game/mod.rs`, `app` already `&mut App`). Drop the script's `log_messages::<audio::SfxEvent>` line (doesn't exist) and replace with `record::log_messages::<scoring::ScoreEvent>(app)` after adding `#[derive(Debug)]` to `ScoreEvent` (it only derived `Message`). Keep `log_value::<GameData>(app, "score", |d| i64::from(d.score))` — that field does exist — and add a twin `"lives"` line from `d.lives`. `states::Paused` matches the template; left as-is. |
| `Gravestone_Gauntlet` | `SfxEvent` lives in `src/assets/audio.rs` (script found and correctly added `Debug`); score is `scoring::ScoreBoard { score, combo, combo_timer, high_score, current_wave }`, not `GameData`; there is **no** `Paused` resource — `states.rs` has `GameState` (top-level `States`) and `PlayState` (a `SubStates` sourced on `GameState::Playing`, with variants `Active`/`Paused`/`BossIntro`) | Fix the score line to three `log_value::<scoring::ScoreBoard>` calls: `"score"` (`.score`), `"combo"` (`.combo`), `"wave"` (`.current_wave`). Drop the `Paused` line entirely (no such resource). Do **not** call `log_state::<PlayState>(app)`: `PlayState` derives `SubStates`, and Bevy only inserts the `State<PlayState>` resource while `GameState::Playing` is active — `log_state`'s system takes `Res<State<S>>` unconditionally, so it would panic the instant the game left `Playing` (menu, game over). Log only `GameState`, and leave a code comment explaining why `PlayState` is skipped. Fix the `SfxEvent` path to `crate::assets::audio::SfxEvent` (the script's default `audio::SfxEvent` doesn't resolve from `src/game/mod.rs`, since that crate's audio lives under `src/assets/`, not `src/game/audio/`). |
| `BeerPong` (Table Titans) | Two-player: `scoring::MatchData { current_player, cups_remaining_p1, cups_remaining_p2, winner, last_throw_scored, streak_p1, streak_p2, game_over_requested }`, no single score. `Paused` and `SfxEvent` (`src/game/audio/mod.rs`, already `Debug`) match the template | Replace the script's single `score` line with four `log_value::<scoring::MatchData>` calls: `"cups_p1"` (`.cups_remaining_p1`), `"cups_p2"` (`.cups_remaining_p2`), `"streak_p1"` (`.streak_p1`), `"streak_p2"` (`.streak_p2`). Everything else (`GameState`, `SfxEvent`, `Paused`) is unchanged from what the script wired. |

Every other published game (attic-excavator, cannonball-putt, dive-rise,
dough-io, grand-theft-auto-reply, grand-theft-otto, gulper, ladder-legend,
pizza-pinball, sundae-shooter, tire-stack) matches the template layout; the
script needs no follow-up beyond `cargo check`.

## General gotchas found rolling out the five irregular games

- **`app` is not always `&mut App`.** In every template-shaped game the
  `record::log_*` calls sit inside `impl Plugin for GamePlugin { fn build(&self,
  app: &mut App) }`, so `app` is already a reference and the script's generated
  calls (`record::log_state::<GameState>(app)`, etc.) compile as-is. In games
  that wire everything from a bare `fn main() { let mut app = App::new(); ... }`
  (Hunted, and pack-the-ripper's `main.rs`-side wiring), `app` is a local
  value — every `record::log_*(app, ...)` call needs `&mut app` instead, or
  `cargo check` fails with "expected `&mut App`, found `App`".
- **`super::record::RecordBeat` in `shot()` only works when `autopilot.rs` and
  `record.rs` are siblings under the same parent module.** The script always
  emits `super::record::RecordBeat`. That's correct when both files sit under
  `src/game/` (Gravestone_Gauntlet, BeerPong, voidrunner) or when both are
  crate-root modules (Hunted, once `record` is also moved to `src/record.rs`).
  It's wrong when they're *not* siblings — pack-the-ripper has
  `src/autopilot.rs` (crate root) but keeps `record.rs` under `src/game/`, so
  `super::record` doesn't resolve and needs to be
  `crate::game::record::RecordBeat` by hand.
- **Flat-layout games get `src/record/` straight from the script.** For a game
  with no `src/game/mod.rs` (Hunted), `rollout-record.sh` detects the flat
  layout before copying anything and drops the recorder folder module at
  `src/record/` directly — there is no intermediate `src/game/record/` to
  move and no `src/record.rs` file to delete. Only the module declaration and
  the `RecordPlugin` block in `main.rs` still need a hand edit.
- **A `SubStates` type is not safe to pass to `log_state`.** `log_state`'s
  system takes `Res<State<S>>` with no run condition; Bevy only inserts that
  resource for a `SubStates` while its source state matches, so logging one
  will panic outside that state. Only pass top-level `States` enums (or a
  `SubStates` you've confirmed is always active) to `log_state`.
