# Code Conventions

The patterns this template establishes. Follow them as the game grows so it stays
testable and easy for both people and agents to reason about.

## Module layout

One file per system/feature, grouped by responsibility under three plugins:

```
src/
  main.rs        # window + plugin wiring only
  display.rs     # DisplayPlugin: game size, native letterbox, UI scale
  game/          # GamePlugin: state machine, resources, gameplay systems
  assets/        # AssetsPlugin: load/create meshes, materials, audio
  ui/            # UiPlugin: menus + HUD
```

Keep files focused. When one grows past a single clear responsibility, split it —
small files are easier to hold in context and edit reliably.

## State machine

`GameState` (`src/game/states.rs`) drives flow: `Menu → Playing → GameOver`.

- Per-frame gameplay: `.add_systems(Update, my_system.run_if(in_state(GameState::Playing)))`.
- Screen/run setup and teardown: `OnEnter(state)` / `OnExit(state)`.
- Transitions: read input, call `next.set(GameState::…)` (see `ui::menu::menu_input`).

## The `GameEntity` cleanup pattern

Anything spawned for a single run (player, HUD, enemies, …) gets the `GameEntity`
marker component. `cleanup_game_entities` despawns all of them on `OnExit(Playing)`,
so leaving a run never leaks entities. Spawn run-scoped entities with `GameEntity`;
spawn persistent ones (camera, lights) without it.

## Extract pure logic for tests

Bevy systems are awkward to unit-test (they need a `World`). So keep game *rules* in
plain methods/functions and test those directly. Example from `src/game/scoring.rs`:

```rust
impl GameData {
    pub fn add_score(&mut self, points: u32) {
        self.score += points;
        if self.score > self.high_score {
            self.high_score = self.score;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn add_score_accumulates() {
        let mut d = GameData::default();
        d.add_score(50);
        d.add_score(100);
        assert_eq!(d.score, 150);
    }
}
```

The system (`handle_score_events`) stays a thin wrapper that calls the tested method.
Apply this to difficulty scaling, collision math, scoring thresholds, etc.

## Procedural-first content

The skeleton creates meshes/materials from Bevy primitives (`Cuboid`, etc.) — no
external model files. Prefer procedural geometry; when you do add binary assets
(audio, textures), load them through `AssetsPlugin` and remember they ship inside
the cartridge tarball and the web bundle.

## Sim determinism

Run state must be reproducible from a replay (seed + per-tick input) so the
site can re-simulate a run headlessly and confirm its score. See
[docs/replay-verification.md](replay-verification.md) for the full contract;
the rules for game code:

1. **`SimSet` only.** Anything that mutates run state runs inside
   `sim::SimSet` (`FixedUpdate`, 60 Hz, chained) — never in `Update`.
2. **`TickInput`, not `GameInput`, in sim systems.** `GameInput` stays a
   per-frame resource for menus/UI.
3. **`GameRng` only.** All randomness in sim code draws from `GameRng`
   (`Xoshiro256PlusPlus`) — nothing else.
4. **No `std::collections::HashMap` in sim state.** Use
   `bevy::platform::collections::HashMap` or `BTreeMap` instead.
5. **Fold extra state into `Checksum`** via `Checksum::fold(&mut self, u64)`
   whenever a score could be reached through different in-game states.
6. **A sim system may read only what the replay carries**: `TickInput`,
   `RunSeed`/`GameRng`, its own run state, and the tick-derived clock
   (`sim::RunClock`). Never `Time::elapsed*` (inside `FixedUpdate` that is
   `Time<Fixed>`'s **app**-lifetime elapsed, so a run forks on how long the
   player sat in the menu), never a wall clock, never a frame count.
   `Time::delta_secs()` is fine — inside `FixedUpdate` it is the fixed
   timestep. And **run state may not outlive the run**: it lives in a
   resource or component `OnEnter(Playing)` resets, never in a `Local<_>`
   (which belongs to the system instance, so no run start can reach it), a
   `static`, or a plugin-build-time cache. The verifier is always a fresh
   app that plays exactly one run; the browser is not.

The greppable ones are enforced by the forbidden-names test in
`src/game/sim.rs` (`cargo test`); the rest need review.

## Curated Bevy features

`Cargo.toml` uses `default-features = false` with an explicit feature list, and gates
native-only features (gilrs, wayland, x11) to `cfg(not(target_arch = "wasm32"))`.
Every feature you add grows the wasm bundle, so add deliberately and keep the list
documented inline.

## Screen shape and rendering for low-power targets

`src/display.rs` is the single source for the game's shape. Set `GAME_WIDTH`
and `GAME_HEIGHT` there to one sanctioned size; nothing else in the game
states a resolution.

| Ratio | `GAME_WIDTH` x `GAME_HEIGHT` | Megapixels |
|---|---|---|
| 4:3 (template default) | 960 x 720 | 0.69 |
| 1:1 | 720 x 720 | 0.52 |
| 3:4 | 720 x 960 | 0.69 |

The short side is always 720, and `cargo test` fails on any other size.

- **The window is pinned.** `display::game_window` builds it through
  `CanvasPolicy::Pinned`, so `fit_canvas_to_parent` is `false` and the web
  backbuffer is exactly `GAME_WIDTH` x `GAME_HEIGHT` on every display (the Pi
  fill-rate budget). The gamebient-input glue sizes the canvas per
  `devicePixelRatio` and letterboxes it in the page. There is no render-scale
  constant: `with_scale_factor_override` never changes the pixel count.
- **Native builds letterbox inside the window.** On Linux the window is
  borderless fullscreen; on macOS and Windows it opens at the game's size.
  `DisplayPlugin` computes `GameViewport`, the largest rect of the game's
  ratio that fits the window, and sets it as `Camera.viewport` on every
  window camera. The bars show `ClearColor`. On web no camera viewport is
  set: the canvas is already the game's size.
- **UI is authored against a short side of 720.** `DisplayPlugin` sets
  `UiScale` to `ui_scale_for(w, h)` = min(w, h) / 720 of the viewport's
  logical size, and Bevy lays UI out inside the camera viewport, so
  hardcoded `Val::Px` and `font_size` values hold on any display. The view
  is 960 UI pixels wide at 4:3 and 720 at 1:1 and 3:4: size a line of text
  with `ui::fit::fit_font_size` when it could be wider than that.
- **A camera of your own.** Any extra camera that renders to the window is
  letterboxed too. One that must not be (the cabinet frame) carries
  `display::FrameCamera`. A camera that sets its own viewport carries
  `FrameCamera` and places itself inside `GameViewport`.
- **Projecting world positions into UI.** `Camera::world_to_viewport`
  returns window pixels that include the letterbox offset; UI nodes are
  relative to the viewport. Subtract `camera.logical_viewport_rect().min`
  and divide by `UiScale`. `display::label_origin(item_px, view_min,
  ui_scale, label_width)` does exactly that; every label, popup or key
  pinned to a world position calls it rather than carrying a hand copy.
- **Presentation only.** No sim system may read `GameViewport`,
  `FrameInsets`, `UiScale` or `Window`: the replay verifier has no window.
  Two checks guard this. `display::tests::game_code_does_not_read_display_state`
  scans `src/game/` for the display module's names only (`display::`,
  `GameViewport`, `FrameInsets`, `FrameCamera`); it does not look for
  `UiScale` or `Window`, it skips lines that start with `//`, and it skips
  any line containing `allow-display`. A sim read of `UiScale` or `Window`
  is caught at run time instead: `tests/display_shape.rs` and
  `tests/windowed_shape.rs` record a run under a letterboxed display and
  require it to equal the bare run. The scanned directory is the constant
  `SIM_SOURCE_DIR` in that test; a game with a flat `src/` layout points it
  at the directory holding its sim files, never at `src/` itself.
- **The `allow-display` marker.** The grep walks `src/game/` only. Code in
  `src/ui/`, `src/assets/` and `src/main.rs` may call into `display` freely
  and needs no marker. Presentation code that lives under `src/game/` (a
  follow camera, camera framing, a dev harness) and needs a display value
  must carry the marker as a trailing comment **on the same line** as the
  name, in the form `// allow-display: <reason>`. The needles are
  `display::`, `GameViewport`, `FrameInsets` and `FrameCamera`, so mark the
  `use` line and later bare uses of a constant need nothing:

  ```rust
  use crate::display::{GAME_HEIGHT, GAME_WIDTH}; // allow-display: camera framing in Update, never read in SimSet
  ```

  A system that takes `Res<GameViewport>` carries the marker on that
  parameter's line as well. The marker records a reviewed exception for
  presentation code; it is never the fix for a `SimSet` system.
- **Capture builds stay windowed.** On Linux the window is fullscreen
  unless the build has the `autopilot` cargo feature (`record` implies it),
  so screenshots are the game's own size. A game with any other capture
  feature (Attic Excavator's `harness`) must add it to that check. The line
  to edit is in `game_window` in `src/display.rs`:

  ```rust
  mode: window_mode_for(cfg!(target_os = "linux"), cfg!(feature = "autopilot")),
  ```

  becomes

  ```rust
  mode: window_mode_for(
      cfg!(target_os = "linux"),
      cfg!(any(feature = "autopilot", feature = "harness")),
  ),
  ```
- **Checking a TV's shape on a dev machine.** `GX_WINDOW_SIZE=450x800 cargo
  run` opens a portrait window; `800x450` a landscape one. Native only.
- **The cabinet frame** sets `display::FrameInsets { reserve_top, gap }` to
  keep the marquee band and the gap around the game clear. Both are zero
  unless a frame is drawn.

Vsync is pinned (`PresentMode::AutoVsync`).

## Boot flow & presentation kit

The template ships the Gamebient presentation kit: `StudioLogo` (Bread Heads
splash, auto-advance ~2.8 s, any button skips) → `Menu` → `HowToPlay` (once
per session) → `Playing` → `GameOver` → `Menu`. Every transition goes through
`ui::transition::ScreenFade` — never set `NextState<GameState>` directly;
call `fade.request(target)` and gate input handlers on `fade.is_idle()`.

Per-game work when building on the template:
- **Title:** keep the text title, or switch to full-bleed artwork (see the
  commented example in `src/ui/menu.rs`).
- **How to play:** replace the placeholder item in
  `src/ui/how_to_play.rs::spawn_how_to_play` with your game's real entities —
  spawn from the same meshes/materials gameplay uses, one `Spin` +
  `HowToPlayScreen` entity per item, one `ItemLabel` per entity. Labels track
  automatically. `HowToPlay` must only be entered through the fade (see the
  comment on `position_labels`).
- **Pause:** gate every gameplay `Update` system on `states::not_paused`.
- **Web boot:** `index.html` starts the engine on the "Click to Start"
  gesture; the wasm is prefetched behind the progress bar. Don't move
  `init()` back before the unlock.

## Controls (the Gamebient canon)

Every gameplay action must be reachable on all three surfaces. Gameplay
systems read ONLY the `GameInput` resource, which comes from the
`gamebient-input` crate (re-exported at src/game/input.rs); menus and
pause use the kit systems with the same key sets. The crate also ships the
web glue: the `gx:` host protocol, the legacy `keyEvent` bridge, a Gamepad
API poller, the Presentation receiver and a DOM touch overlay, so none of
that lives in index.html any more.

| Logical | Keyboard | Gamepad | Virtual pad / cabinet |
|---|---|---|---|
| Move / aim | Arrows + WASD | Left stick + D-pad | D-pad |
| A (primary/jump) | Z (+ Space) | South or North | A |
| B (secondary/fire) | X (+ Shift) | West or East | B |
| Confirm (menus) | Enter / Space / Z | any face button | Start or A |
| Pause | Esc | Start | Pause |
| Quit (while paused) | Enter | East | Start |

The website's virtual controller injects exactly these keys (A→Z, B→X,
D-pad→arrows, Start→Enter, Pause→Esc, Select→Shift) via the keyEvent bridge,
or the same buttons as a `gx:input` bitmask (see gamebient-input's
docs/host-protocol.md) — a game that follows the canon is automatically
mobile-playable. Legacy per-game key aliases are fine but must never be an
action's only binding. Control text shown to players is ASCII only.

## Host protocol (embedding)

Games built on this template speak the ColecoVision GX host protocol through
`gamebient-input` (docs/host-protocol.md in that repo). The crate posts
`ready` and every `GameState` transition; `src/game/host.rs` adds what a host
acts on: `started` on entering `Playing`, `gameover` plus the final `score`
on entering `GameOver`, `score` whenever `GameData.score` changes, and
`paused` whenever `Paused` changes. It also honours host commands: pause and
resume (only while `Playing`; the pause overlay follows the `Paused` resource,
so a host pause looks like a player pause) and mute/unmute (`GlobalVolume`
plus live sinks). Report anything else with `HostEvent::Custom`. Hosts treat
all of it as untrusted.

## Audio

The kit is asset-free by default: SFX are synthesized at startup
(`src/game/audio/synth.rs` — pure `f(t)` generators rendered to in-memory
WAV). To add a sound: add an `SfxEvent` variant, bake its source in
`setup_sfx`, emit the event from gameplay. Never play audio directly from
gameplay systems — always go through the bus (keeps mixing/despawn policy in
one place).

Music is table-driven: fill a slot in `MUSIC` (src/game/audio/mod.rs) with a
path under `assets/audio/music/` and the crossfade director handles the rest.
`None` slots load nothing. A `GameState` variant absent from the table also
resolves to silence — when you add a state, add its row. Authored-asset
precedent: voidrunner / Gravestone_Gauntlet; procedural precedent: Hunted.

Web autoplay is already handled by the boot flow's AudioContext unlock.
