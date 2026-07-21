# Code Conventions

The patterns this template establishes. Follow them as the game grows so it stays
testable and easy for both people and agents to reason about.

## Module layout

One file per system/feature, grouped by responsibility under three plugins:

```
src/
  main.rs        # window + plugin wiring only
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

## Curated Bevy features

`Cargo.toml` uses `default-features = false` with an explicit feature list, and gates
native-only features (gilrs, wayland, x11) to `cfg(not(target_arch = "wasm32"))`.
Every feature you add grows the wasm bundle, so add deliberately and keep the list
documented inline.

## Rendering for low-power targets

`main.rs` sets `RENDER_SCALE = 0.5` (internal framebuffer scale) and pins vsync — the
game runs on Raspberry Pi kiosk hardware. UI is authored against `REFERENCE_HEIGHT`
(720) and scaled by `update_ui_scale`, so hardcoded pixel sizes hold on any display.

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
systems read ONLY the `GameInput` resource (src/game/input.rs); menus and
pause use the kit systems with the same key sets.

| Logical | Keyboard | Gamepad | Virtual pad / cabinet |
|---|---|---|---|
| Move / aim | Arrows + WASD | Left stick + D-pad | D-pad |
| A (primary/jump) | Z (+ Space) | South or North | A |
| B (secondary/fire) | X (+ Shift) | West or East | B |
| Confirm (menus) | Enter / Space / Z | any face button | Start or A |
| Pause | Esc | Start | Pause |
| Quit (while paused) | Enter | East | Start |

The website's virtual controller injects exactly these keys (A→Z, B→X,
D-pad→arrows, Start→Enter, Pause→Esc, Select→Shift) via the keyEvent bridge
in index.html — a game that follows the canon is automatically
mobile-playable. Legacy per-game key aliases are fine but must never be an
action's only binding. Control text shown to players is ASCII only.

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
