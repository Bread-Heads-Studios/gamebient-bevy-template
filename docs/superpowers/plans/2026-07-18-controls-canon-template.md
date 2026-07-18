# Controls Canon — Template Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Land the canonical `GameInput` resource, stick/D-pad movement, canon-aligned menu inputs, and the Controls conventions section in `gamebient-bevy-template` — the reference the other repos copy.

**Architecture:** New `src/game/input.rs` with a `GameInput` resource populated by one `collect_input` system (PreUpdate, unconditional) from keyboard + gamepads per the canon; pure helpers unit-tested. `player::move_player` converts to read the resource. Kit menu systems gain "any face button confirms".

**Tech Stack:** Rust 2024 / Bevy 0.18.

**Spec:** `docs/superpowers/specs/2026-07-18-controls-canon-design.md` (read it first — the canon table drives everything).
**Work in:** `/Users/kelliott/Gamebient/colecovisiongx/gamebient-bevy-template` on existing branch `feat/controls-pass` (stacked on `feat/presentation-polish`; the spec commit is already on it).

**Template facts:** `src/game/player.rs::move_player` reads WASD/arrows raw; `src/ui/menu.rs::menu_input`, `src/ui/how_to_play.rs::how_to_play_input` accept Enter/Space + gamepad South only; pause lives in `src/game/mod.rs` (Esc/Start toggle, Enter/East quit) — pause bindings are already canon, leave them. `GamePlugin` is in `src/game/mod.rs`.

---

### Task 1: `GameInput` resource + collect system (TDD)

**Files:**
- Create: `src/game/input.rs`
- Modify: `src/game/mod.rs` (module decl + resource init + system registration)

- [ ] **Step 1: Create `src/game/input.rs`:**

```rust
use bevy::prelude::*;

/// Analog stick deadzone: values with |v| below this read as 0.
pub const STICK_DEADZONE: f32 = 0.2;

/// Frame-coherent input snapshot, populated once per frame by
/// [`collect_input`] from keyboard and all connected gamepads using the
/// Gamebient canon bindings (see docs/conventions.md "Controls"):
///
/// - Move: arrows + WASD, left stick, D-pad
/// - A (primary): Z (+ Space) · gamepad South or North (DragonRise pairing)
/// - B (secondary): X (+ Shift) · gamepad West or East
/// - Confirm: Enter, Space, Z · any gamepad face button
/// - Pause: Escape · gamepad Start
///
/// Gameplay systems read THIS resource, never raw input, so bindings live in
/// exactly one place per game.
#[derive(Resource, Default)]
pub struct GameInput {
    pub move_x: f32,
    pub move_y: f32,
    pub primary_just_pressed: bool,
    pub primary_held: bool,
    pub primary_just_released: bool,
    pub secondary_just_pressed: bool,
    pub secondary_held: bool,
    pub confirm_just_pressed: bool,
    pub pause_just_pressed: bool,
}

/// Applies the stick deadzone: zero inside the threshold, unchanged outside.
pub fn apply_deadzone(v: f32) -> f32 {
    if v.abs() < STICK_DEADZONE { 0.0 } else { v }
}

/// Merges a digital direction sum with an analog axis, clamped to [-1, 1].
pub fn merge_axis(digital: f32, analog: f32) -> f32 {
    (digital + apply_deadzone(analog)).clamp(-1.0, 1.0)
}

/// Populates [`GameInput`] from keyboard + all gamepads. Runs in `PreUpdate`
/// unconditionally so no state transition can leave stale just-* flags.
pub fn collect_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    mut input: ResMut<GameInput>,
) {
    *input = GameInput::default();

    // --- Movement: keyboard digital ---
    let mut dx = 0.0;
    let mut dy = 0.0;
    if keyboard.any_pressed([KeyCode::KeyA, KeyCode::ArrowLeft]) {
        dx -= 1.0;
    }
    if keyboard.any_pressed([KeyCode::KeyD, KeyCode::ArrowRight]) {
        dx += 1.0;
    }
    if keyboard.any_pressed([KeyCode::KeyS, KeyCode::ArrowDown]) {
        dy -= 1.0;
    }
    if keyboard.any_pressed([KeyCode::KeyW, KeyCode::ArrowUp]) {
        dy += 1.0;
    }

    // --- A / B / confirm / pause: keyboard ---
    let a_keys = [KeyCode::KeyZ, KeyCode::Space];
    let b_keys = [KeyCode::KeyX, KeyCode::ShiftLeft, KeyCode::ShiftRight];
    input.primary_just_pressed = keyboard.any_just_pressed(a_keys);
    input.primary_held = keyboard.any_pressed(a_keys);
    input.primary_just_released =
        keyboard.any_just_released(a_keys) && !keyboard.any_pressed(a_keys);
    input.secondary_just_pressed = keyboard.any_just_pressed(b_keys);
    input.secondary_held = keyboard.any_pressed(b_keys);
    input.confirm_just_pressed = keyboard.any_just_pressed([
        KeyCode::Enter,
        KeyCode::Space,
        KeyCode::KeyZ,
    ]);
    input.pause_just_pressed = keyboard.just_pressed(KeyCode::Escape);

    // --- Gamepads ---
    for gamepad in &gamepads {
        let stick = gamepad.left_stick();
        let dpad_x = dpad_axis(
            gamepad.pressed(GamepadButton::DPadLeft),
            gamepad.pressed(GamepadButton::DPadRight),
        );
        let dpad_y = dpad_axis(
            gamepad.pressed(GamepadButton::DPadDown),
            gamepad.pressed(GamepadButton::DPadUp),
        );
        input.move_x = merge_axis(dx + dpad_x, stick.x);
        input.move_y = merge_axis(dy + dpad_y, stick.y);

        // A = South or North (DragonRise pairing)
        let a_now = gamepad.pressed(GamepadButton::South) || gamepad.pressed(GamepadButton::North);
        let a_just = gamepad.just_pressed(GamepadButton::South)
            || gamepad.just_pressed(GamepadButton::North);
        let a_released = (gamepad.just_released(GamepadButton::South)
            || gamepad.just_released(GamepadButton::North))
            && !a_now;
        input.primary_just_pressed |= a_just;
        input.primary_held |= a_now;
        input.primary_just_released |= a_released;

        // B = West or East
        let b_now = gamepad.pressed(GamepadButton::West) || gamepad.pressed(GamepadButton::East);
        input.secondary_just_pressed |= gamepad.just_pressed(GamepadButton::West)
            || gamepad.just_pressed(GamepadButton::East);
        input.secondary_held |= b_now;

        // Confirm = any face button; Pause = Start
        input.confirm_just_pressed |= a_just
            || gamepad.just_pressed(GamepadButton::West)
            || gamepad.just_pressed(GamepadButton::East);
        input.pause_just_pressed |= gamepad.just_pressed(GamepadButton::Start);
    }

    // No gamepad connected: movement still comes from the keyboard sums.
    if gamepads.is_empty() {
        input.move_x = dx.clamp(-1.0, 1.0);
        input.move_y = dy.clamp(-1.0, 1.0);
    }
}

/// -1/0/+1 from a digital negative/positive button pair.
fn dpad_axis(neg: bool, pos: bool) -> f32 {
    (pos as i32 - neg as i32) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deadzone_zeroes_small_values_and_passes_large() {
        assert_eq!(apply_deadzone(0.1), 0.0);
        assert_eq!(apply_deadzone(-0.19), 0.0);
        assert_eq!(apply_deadzone(0.5), 0.5);
        assert_eq!(apply_deadzone(-1.0), -1.0);
    }

    #[test]
    fn merge_axis_clamps_combined_sources() {
        assert_eq!(merge_axis(1.0, 1.0), 1.0);
        assert_eq!(merge_axis(-1.0, -0.6), -1.0);
        assert_eq!(merge_axis(0.0, 0.1), 0.0); // deadzone applies to analog only
        assert_eq!(merge_axis(1.0, -0.5), 0.5);
    }

    #[test]
    fn dpad_axis_maps_pairs() {
        assert_eq!(dpad_axis(false, false), 0.0);
        assert_eq!(dpad_axis(true, false), -1.0);
        assert_eq!(dpad_axis(false, true), 1.0);
        assert_eq!(dpad_axis(true, true), 0.0);
    }
}
```

Implementation notes: with multiple pads, last pad wins for movement —
acceptable; keyboard `dx`/`dy` is always included via `merge_axis(dx + dpad_x, ...)`.
`Gamepad::left_stick()` returns `Vec2` in Bevy 0.18; if it doesn't resolve,
read the axes via `gamepad.get(GamepadAxis::LeftStickX).unwrap_or(0.0)`.

- [ ] **Step 2: Wire into `src/game/mod.rs`:** add `pub mod input;`, `.init_resource::<input::GameInput>()`, and `.add_systems(PreUpdate, input::collect_input)`.

- [ ] **Step 3:** `cargo test input` — 3 new tests pass. Full `cargo test` — all pass.

- [ ] **Step 4: Commit:** `git add -A && git commit -m "feat: canonical GameInput resource with canon bindings"`

### Task 2: Convert movement + align menu confirm

- [ ] **Step 1:** `src/game/player.rs::move_player` — replace the keyboard reads with:

```rust
pub fn move_player(
    time: Res<Time>,
    input: Res<crate::game::input::GameInput>,
    mut query: Query<&mut Transform, With<Player>>,
) {
    let Ok(mut tf) = query.single_mut() else {
        return;
    };
    let dir = Vec2::new(input.move_x, input.move_y);
    let delta = dir.normalize_or_zero() * PLAYER_SPEED * time.delta_secs();
    tf.translation.x += delta.x;
    tf.translation.y += delta.y;
}
```

- [ ] **Step 2:** Menu confirm = any face button: in `src/ui/menu.rs::menu_input` and `src/ui/how_to_play.rs::how_to_play_input`, extend the gamepad check from `South` only to `South || North || West || East` (keep the keyboard sets as-is; keep fade gating). Add `KeyCode::KeyZ` to both keyboard confirm sets (canon: Enter/Space/Z).

- [ ] **Step 3:** Title controls line in `src/ui/menu.rs`: change to `"Arrows / WASD: Move  |  Z: A  X: B  |  Esc: Pause"`.

- [ ] **Step 4:** `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --all -- --check`. Commit: `feat: GameInput-driven movement, any-face-button confirm`

### Task 3: Conventions "Controls" section

- [ ] **Step 1:** Append to `docs/conventions.md`:

```markdown
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
```

- [ ] **Step 2:** Commit: `docs: controls canon conventions`

### Task 4: Gates + handoff

- [ ] `cargo test` / clippy `-D warnings` / fmt — clean. Do NOT push/PR (controller verifies then ships).
