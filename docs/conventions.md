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
  `FrameCamera` (`crate::frame::FrameCamera`). A camera that sets its own
  viewport carries `FrameCamera` and places itself inside `GameViewport`.
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
  `GameViewport`, `FrameInsets`); it does not look for
  `UiScale` or `Window`, it skips lines that start with `//`, and it skips
  any line containing `allow-display`. A sim read of `UiScale` or `Window`
  is caught at run time instead: `tests/display_shape.rs` and
  `tests/windowed_shape.rs` record a run under a letterboxed display and
  require it to equal the bare run. The scanned directory is the constant
  `SIM_SOURCE_DIR` in that test. A game with a flat `src/` layout has no
  directory that holds only its sim files (`src/` holds `display.rs`), so it
  adds a `sim-sources.txt` at the crate root instead of editing the verbatim
  `src/display.rs`: one `.rs` path per line, relative to the crate root, `#`
  comments and blank lines allowed. When that file exists the scan reads the
  files it lists and ignores `SIM_SOURCE_DIR`.
- **The `allow-display` marker.** The grep walks `src/game/` only. Code in
  `src/ui/`, `src/assets/` and `src/main.rs` may call into `display` freely
  and needs no marker. Presentation code that lives under `src/game/` (a
  follow camera, camera framing, a dev harness) and needs a display value
  must carry the marker as a trailing comment **on the same line** as the
  name, in the form `// allow-display: <reason>`. The needles are
  `display::`, `GameViewport` and `FrameInsets`, so mark the
  `use` line and later bare uses of a constant need nothing:

  ```rust
  use crate::display::{GAME_HEIGHT, GAME_WIDTH}; // allow-display: camera framing in Update, never read in SimSet
  ```

  A system that takes `Res<GameViewport>` carries the marker on that
  parameter's line as well. The marker records a reviewed exception for
  presentation code; it is never the fix for a `SimSet` system.
- **Capture builds stay windowed.** On Linux the window is fullscreen
  unless the build has the `capture` cargo feature (`autopilot` implies it,
  and `record` implies `autopilot`), so screenshots are the game's own size.
  `game_window` in `src/display.rs` and the frame in `src/frame/mod.rs` read
  only `capture`; never edit them for this. A game with any other capture
  feature (Attic Excavator's `harness`, a `showcase`) makes it imply
  `capture` in its `Cargo.toml`:

  ```toml
  capture = []
  harness = ["capture"]
  ```
- **Checking a TV's shape on a dev machine.** `GX_WINDOW_SIZE=450x800 cargo
  run` opens a portrait window; `800x450` a landscape one. Native only.
- **The cabinet frame** sets `display::FrameInsets { reserve_top, gap }` to
  keep the marquee band and the gap around the game clear. Both are zero
  unless a frame is drawn.
- **Camera rules for a new shape.** Pick the one that fits the game, put it
  in a pure function with a unit test in the game's camera file, and call it
  from an `Update` system, never from `SimSet`. Name the constants so the
  choice can be flipped in review.
  - **C1, fixed field.** The whole playfield stays visible: fit the field's
    extents plus the game's margin at `GAME_WIDTH / GAME_HEIGHT`. Test that
    every field corner projects inside |NDC| 0.95 and at least one lies
    beyond 0.80, so the fit is not loose.
  - **C2, open-world follow camera.** Keep the visible area: scale the view
    by `sqrt(old_aspect / new_aspect)` (1.1547 for 16:9 to 4:3, 1.3333 to
    1:1, 1.5396 to 3:4). Test that the visible area is within 1% of the
    old one.
  - **C3, first-person or chase 3D.** Keep the horizontal FOV, so the
    vertical FOV grows as the screen gets less wide: 57.80 degrees at 4:3,
    up from 45 degrees at 16:9. Test that the horizontal FOV at the new
    aspect is within 0.1 degrees of the old one.
  - **C4, read-ahead axis.** Keep the extent along the axis the player reads
    ahead on (a belt runs horizontally, lanes run vertically) equal to its
    old value within 1%. The other axis takes what the new shape gives.

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

## Cabinet frame

On a cabinet the game is letterboxed, and the rest of the panel is filled by
the frame (`src/frame/`, `FramePlugin`): dim bezel art behind the game, a
black gap around it, and on portrait displays a marquee band across the top
(height = width / 3). Landscape cabinets have a physical marquee, so none is
drawn there. The website draws the same frame around the game on `/cabinet`;
the two implementations share their numbers and test vectors, so change them
together (`src/lib/cabinetFrame/` in the website repo).

- **Native only, windowed only.** `FramePlugin` is added in `main.rs` and
  nowhere else. Never add it to `GamePlugin`: the headless verifier builds
  `GamePlugin` alone. On wasm the plugin does nothing.
- **Art.** `assets/marquee.png` (1080x360) and `assets/bezel.png`
  (1920x1920, square, centre-cropped). Both optional; without them the
  shared art embedded in the binary is used. The shared marquee carries the
  ColecoVision GX logo, and the game's name is drawn in a box to the logo's
  right (uppercase, one line, sized to fit; the formula is `title_box` in
  `src/frame/layout.rs`). A game's own `marquee.png` gets no title drawn over
  it. `tools/frame-art.sh` renders them from `tools/marquee.svg`
  and `tools/bezel.svg`.
- **Art geometry.** The frame writes the score line in rows 288 to 360 of
  the marquee, so lettering stays above row 288. The bezel's central
  1080x1080 is one flat colour; texture goes in the four arms.
- **`src/frame/art/colecovision-gx-logo.png` is a source asset.** It is the
  logo used to regenerate `fallback-marquee.png` and is not read at build
  or run time; leave it in place.
- **Brightness is the frame's job, not the artist's.** Draw the art at full
  strength. The frame multiplies the bezel by 0.35 (0.20 during play) and
  its saturation by 0.6, and the marquee by 1.0 (0.45 during play), fading
  over one second. The bezel never exceeds 0.35.
- **Highlight.** For a boss, a level clear or a big combo, write one message:

  ```rust
  fn boss_defeated(mut highlights: MessageWriter<frame::Highlight>) {
      highlights.write(frame::Highlight::rgb(0xff, 0xcc, 0x00));
  }
  ```

  The native marquee pulses in that colour and web hosts receive
  `HostEvent::Highlight`. The frame allows one pulse per ten seconds and
  drops the rest, so there is no need to ration calls. It is safe to write
  from a sim system: it is write-only and exists in headless builds.
- **Score to beat.** Native builds keep the best `leaderboard_score()` in
  `$GX_DATA_DIR/best-score`, else
  `$XDG_DATA_HOME/gamebient/<package>/best-score`, else
  `$HOME/.local/share/gamebient/<package>/best-score`. Delete the file to
  reset it.
- **Checking the dimming.** Check it by relation, not by a fixed ratio.
  On a landscape capture the bezel's mean brightness at idle (menu, attract)
  is at most 0.35 x 255, in play it is below idle, and paused equals play
  within 10%. On a portrait capture the marquee's mean in play is below idle
  and paused equals play within 10%. `tools/frame-accept.sh --shots <dir>
  --window WxH --ratio <r>` runs these checks on `02-title.png`,
  `05-mid-play.png` and `08-pause.png`, and also that the gap around the game
  is black and the bezel beyond it is not. `tools/frame-accept.sh
  --check-copies <game>` proves the game's verbatim copies still match this
  template.
- **Switches.** `GX_FRAME=off` (also `0`, `false`, `no`) disables the frame and gives the game the
  whole window. Under a `capture` build (`autopilot`, `record`, or a game's own capture feature) the frame is off
  unless `GX_FRAME=on`, so captures never include it.
  `GX_WINDOW_SIZE=1080x1920` (see "Screen shape") opens a window of that
  size for checking a cabinet layout on a desk.
- **Camera queries exclude `FrameCamera`.** The frame adds a second
  `Camera2d`. A game system whose camera query could match it (with no other
  marker narrowing it) matches two cameras: `.single()` and `Single<..>`
  stop matching, and a loop over `With<Camera2d>` (a screen shake) would move
  the bezel during play. Such a query says
  `Without<crate::frame::FrameCamera>`:

  ```rust
  // before
  fn shake(mut cam: Single<&mut Transform, With<Camera2d>>) { .. }
  fn labels(cams: Query<(&Camera, &GlobalTransform)>) { .. }
  // after
  use crate::frame::FrameCamera;
  fn shake(mut cam: Single<&mut Transform, (With<Camera2d>, Without<FrameCamera>)>) { .. }
  fn labels(cams: Query<(&Camera, &GlobalTransform), Without<FrameCamera>>) { .. }
  ```

  The path is `crate::frame::FrameCamera`, available on every target. Naming
  it is not a read of display state, so no `allow-display` marker is needed,
  even under `src/game/`.

  The test `frame::tests::camera_queries_exclude_the_frame_camera` scans
  `src/` (except `src/frame/` and `src/display.rs`) and fails naming the file
  and line. The rule is `camera_query_flagged` in `src/frame/mod.rs`. A line
  is flagged when it is a `Query<`, `Single<` or `Populated<` (on the line
  or the three before) that has `With<Camera2d>` or `With<Camera>`, or that
  has `&Camera` / `&mut Camera` (followed by `,`, `)` or `>`) with no
  `With<` naming another type on the line or the two after. So
  `With<Camera3d>` alone never flags, and neither does `&Camera` behind a
  game's own marker (`With<MainCamera>`). It passes when `FrameCamera`
  appears on the line or the two after, or the line carries
  `// frame-camera-ok: <reason>` for a query that must see every camera.
  The five real fleet cases to fix when rolling the frame out: beat-bender
  `src/ui/fighters.rs` (two queries), Gravestone Gauntlet
  `src/game/effects.rs` and `src/ui/how_to_play.rs`, pizza-pinball
  `src/ui/how_to_play.rs`.
- **The game camera shares the window with the frame camera.** The frame
  camera is `Msaa::Off`. A game `Camera2d` left on Bevy's default 4x MSAA
  aborts the window on the first frame with `GX_FRAME=on` (wgpu rejects a
  1-sample depth attachment against a 4-sample colour attachment); 3D game
  cameras have not shown this. Give every game `Camera2d` that can share the
  window (menu screens included) `Msaa::Off`, and run one `GX_FRAME=on`
  capture before declaring a port done.
- **What the frame reads from the game.** Frame files are copied into games
  unchanged, so these names must exist in every game:
  `crate::game::scoring::{GameData, LeaderboardScore, HIGHER_SCORE_IS_BETTER}`
  (`GameData` implements `Default` and `LeaderboardScore`) and
  `crate::game::states::{GameState, Paused}` (`GameState` has `Playing` and
  `GameOver`; `Paused` is a tuple struct over `bool`). A game whose run type
  has another name aliases it in `scoring.rs` instead of editing the frame,
  for example `pub use super::run::RunStats as GameData;`. The driver's tests
  build and change the score through two helpers, `data_with_score` and
  `add_score` in `src/frame/driver.rs`; a game whose type has no public
  numeric `score` field adapts those two functions only. A game whose scored
  ending is a state other than `GameOver` maps it to `FramePhase::GameOver` in
  its copy of `phase_for`, and the frame then finishes the run on entering it
  (Hunted's `Victory` is the one case).
- **Flat layouts.** A game with no `src/game/` (its sim files sit in
  `src/`) declares the copied modules in `src/lib.rs` and puts
  `pub use crate::{replay, scoring, sim, states};` in `src/game.rs`, so
  `crate::game::scoring` and `crate::game::states` resolve in the copied
  frame files unchanged. `src/fit.rs` stands in for `src/ui/fit.rs`, and the
  display scan reads `sim-sources.txt` (see "Presentation only").
- **A new `GameState` variant** is treated as attract by the frame unless
  you add it to `frame::driver::phase_for`.

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
