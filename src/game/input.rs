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
    input.confirm_just_pressed =
        keyboard.any_just_pressed([KeyCode::Enter, KeyCode::Space, KeyCode::KeyZ]);
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
        input.secondary_just_pressed |=
            gamepad.just_pressed(GamepadButton::West) || gamepad.just_pressed(GamepadButton::East);
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
