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
