# Presentation Kit — Template Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Land the canonical presentation kit (studio logo boot, fade transitions, pulsing title, once-per-session How to Play, pause with quit-to-title, reworked web boot) in `gamebient-bevy-template`, which the four game repos then copy.

**Architecture:** Verbatim ports of the proven Voidrunner modules (`transition.rs`, `studio_logo.rs` — extract with `git show` from the reference commit) plus template-adapted modules (`how_to_play.rs` with a placeholder showcase item, reworked `menu.rs`, new pause in `game/mod.rs`). Two new `GameState` variants; all flow transitions route through `ScreenFade`.

**Tech Stack:** Rust 2024 / Bevy 0.18 (same curated features as Voidrunner), plain JS in `index.html`.

**Reference implementation:** `../voidrunner` at commit `cab6559` (branch `feat/presentation-polish`, PR #8, runtime-verified). Extract reference files with `git -C ../voidrunner show cab6559:<path>`.
**Spec:** `docs/superpowers/specs/2026-07-10-presentation-kit-rollout-design.md`
**Work in:** `/Users/kelliott/Gamebient/colecovisiongx/gamebient-bevy-template` on a new branch `feat/presentation-polish` (create from `main` in Task 1).

**Template facts (verified):**
- `GameState` in `src/game/states.rs`: `Menu` (default) / `Playing` / `GameOver`. No pause anywhere.
- Camera: plain `Camera3d` spawned once at Startup in `setup_scene` (`src/game/mod.rs`) — **no marker component**; light is also global (not state-scoped), so How-to-Play needs no extra light.
- `src/ui/mod.rs` wires Menu/GameOver via `menu::spawn_menu`/`spawn_game_over`/`despawn_menu` (shared `MenuRoot` marker) and a globally-running `menu_input` that sends both `Menu → Playing` and `GameOver → Playing`.
- `src/ui/menu.rs` is text-only ("GAMEBIENT GAME\nPress ENTER to start").
- `index.html` is Voidrunner's **pre-rework** boot with `gamebient-game` name tokens (`await init(...)` at line ~228 runs before the `#audio-unlock` overlay shows).
- `update_ui_scale`/`REFERENCE_HEIGHT=720` in `src/main.rs`; no `ClearColor`.
- Remote: `git@github.com:Bread-Heads-Studios/gamebient-bevy-template.git`.
- `init-game.sh` rewrites the name tokens (`gamebient-game`, `Gamebient Game`, etc.); do not introduce new game-name strings — studio strings ("BREAD HEADS STUDIOS") are constant across games and are NOT tokens.

---

### Task 1: Branch + studio logo asset

**Files:**
- Create: `assets/breadheads_logo.png`

- [ ] **Step 1: Create the branch**

```bash
cd /Users/kelliott/Gamebient/colecovisiongx/gamebient-bevy-template
git checkout main && git checkout -b feat/presentation-polish
```

- [ ] **Step 2: Copy the logo from the reference repo**

```bash
git -C ../voidrunner show cab6559:assets/breadheads_logo.png > assets/breadheads_logo.png
file assets/breadheads_logo.png
```

Expected: `PNG image data, 512 x 512, 8-bit/color RGBA`.

- [ ] **Step 3: Commit**

```bash
git add assets/breadheads_logo.png
git commit -m "assets: bundle Bread Heads studio logo for splash screen"
```

---

### Task 2: States — `StudioLogo`, `HowToPlay`, `Paused`

**Files:**
- Modify: `src/game/states.rs`

- [ ] **Step 1: Replace the file contents with:**

```rust
use bevy::prelude::*;

/// Top-level game flow.
#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum GameState {
    /// "BREAD HEADS STUDIOS PRESENTS" boot screen.
    #[default]
    StudioLogo,
    Menu,
    /// Once-per-session instruction screen (see src/ui/how_to_play.rs).
    HowToPlay,
    Playing,
    GameOver,
}

/// Whether the game is currently paused. Only meaningful in `Playing`.
#[derive(Resource, Default, PartialEq, Eq)]
pub struct Paused(pub bool);

/// Run condition: true when the game is NOT paused.
pub fn not_paused(paused: Res<Paused>) -> bool {
    !paused.0
}
```

(Adds `Copy` to the existing derive list — `ScreenFade` stores a `GameState` by value.)

- [ ] **Step 2: Verify it compiles**

Run: `cargo build`
Expected: success (dead-code warnings for the new variants are expected until Tasks 4-6 use them; `menu_input` still compiles because it matches only existing variants).

- [ ] **Step 3: Commit**

```bash
git add src/game/states.rs
git commit -m "feat: add StudioLogo/HowToPlay states and Paused resource"
```

---

### Task 3: `ScreenFade` transition module (verbatim port)

**Files:**
- Create: `src/ui/transition.rs`
- Modify: `src/ui/mod.rs`

- [ ] **Step 1: Extract the proven module verbatim**

```bash
git -C ../voidrunner show cab6559:src/ui/transition.rs > src/ui/transition.rs
```

This brings `ScreenFade` (boot/request/request_with/is_idle/alpha/tick with duration clamp), `FadeOverlay` + `spawn_fade_overlay`, `update_fade`, `Pulse` + `pulse_text`, and 4 unit tests. It only imports `bevy::prelude` and `crate::game::states::GameState`, both of which resolve identically in the template. Read the file after extraction to confirm it matches that description.

- [ ] **Step 2: Wire it — replace `src/ui/mod.rs` with:**

```rust
use bevy::prelude::*;

pub mod hud;
pub mod menu;
pub mod transition;

use crate::game::states::GameState;

/// Title screen, game-over screen, in-run HUD, and cross-state transitions.
pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app
            // Cross-state fade overlay + shared prompt pulse
            .insert_resource(transition::ScreenFade::boot())
            .add_systems(Startup, transition::spawn_fade_overlay)
            .add_systems(Update, (transition::update_fade, transition::pulse_text))
            // Menu / Game Over
            .add_systems(OnEnter(GameState::Menu), menu::spawn_menu)
            .add_systems(OnExit(GameState::Menu), menu::despawn_menu)
            .add_systems(OnEnter(GameState::GameOver), menu::spawn_game_over)
            .add_systems(OnExit(GameState::GameOver), menu::despawn_menu)
            .add_systems(Update, menu::menu_input)
            // In-run HUD
            .add_systems(OnEnter(GameState::Playing), hud::spawn_hud)
            .add_systems(Update, hud::update_hud.run_if(in_state(GameState::Playing)));
    }
}
```

- [ ] **Step 3: Test**

Run: `cargo test transition`
Expected: 4 tests pass (`boot_fade_in_reaches_idle`, `request_fades_out_emits_target_once_then_fades_in`, `request_while_busy_is_rejected`, `non_positive_duration_is_clamped_not_soft_locked`).

- [ ] **Step 4: Commit**

```bash
git add src/ui/transition.rs src/ui/mod.rs
git commit -m "feat: ScreenFade transition driver with fade overlay and Pulse"
```

---

### Task 4: Studio logo screen (verbatim port)

**Files:**
- Create: `src/ui/studio_logo.rs`
- Modify: `src/ui/mod.rs`

- [ ] **Step 1: Extract verbatim**

```bash
git -C ../voidrunner show cab6559:src/ui/studio_logo.rs > src/ui/studio_logo.rs
```

Brings `StudioLogoRoot`, `StudioLogoTimer`, `spawn_studio_logo` (loads `breadheads_logo.png`), `advance_studio_logo` with the `latch_and_should_advance` helper, `despawn_studio_logo` (removes the timer resource), and 3 unit tests. Imports resolve identically in the template.

- [ ] **Step 2: Wire in `src/ui/mod.rs`**

Add `pub mod studio_logo;` next to the other module declarations, and add inside `build`, before the Menu block:

```rust
            // Studio logo boot screen
            .add_systems(OnEnter(GameState::StudioLogo), studio_logo::spawn_studio_logo)
            .add_systems(
                Update,
                studio_logo::advance_studio_logo.run_if(in_state(GameState::StudioLogo)),
            )
            .add_systems(OnExit(GameState::StudioLogo), studio_logo::despawn_studio_logo)
```

- [ ] **Step 3: Test and run**

Run: `cargo test` — expected: 7 kit tests + any pre-existing tests pass.
`GameState::StudioLogo` is already the boot default (Task 2), so the game now boots into the logo screen. Do NOT `cargo run` (controller verifies visuals at the end).

- [ ] **Step 4: Commit**

```bash
git add src/ui/studio_logo.rs src/ui/mod.rs
git commit -m "feat: Bread Heads studio logo boot screen"
```

---

### Task 5: How-to-Play screen (template adaptation)

**Files:**
- Create: `src/ui/how_to_play.rs`
- Modify: `src/ui/mod.rs`

- [ ] **Step 1 (TDD): Create `src/ui/how_to_play.rs` with only the gating logic + test**

```rust
use bevy::prelude::*;

use crate::game::states::GameState;

/// Set once the player has seen the how-to-play screen this session; never
/// reset, so the screen shows exactly once per boot.
#[derive(Resource, Default)]
pub struct SeenHowToPlay(pub bool);

/// Where the title screen's start action goes: the how-to-play screen on the
/// first run of a session, straight into gameplay afterwards.
pub fn start_target(seen_how_to_play: bool) -> GameState {
    if seen_how_to_play {
        GameState::Playing
    } else {
        GameState::HowToPlay
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_start_shows_how_to_play_then_skips_it() {
        assert_eq!(start_target(false), GameState::HowToPlay);
        assert_eq!(start_target(true), GameState::Playing);
    }
}
```

Add `pub mod how_to_play;` to `src/ui/mod.rs`. Run `cargo test how_to_play` — expected: 1 test passes.

- [ ] **Step 2: Add the showcase screen**

Insert between `start_target` and the tests module (imports go to the top of the file):

```rust
use crate::ui::transition::{Pulse, ScreenFade};

/// Marker for every entity (3D and UI) spawned by the how-to-play screen.
#[derive(Component)]
pub struct HowToPlayScreen;

/// Slow Y-axis rotation for showcase items.
#[derive(Component)]
pub struct Spin;

/// UI label pinned each frame to a showcase item's screen position.
#[derive(Component)]
pub struct ItemLabel {
    pub target: Entity,
}

const LABEL_WIDTH: f32 = 170.0;

/// Marks the how-to-play screen as seen for the rest of the session.
pub fn mark_seen(mut seen: ResMut<SeenHowToPlay>) {
    seen.0 = true;
}

/// Spawns the showcase and screen chrome.
///
/// TEMPLATE NOTE: replace the placeholder item below with your game's real
/// entities — spawn each one from the same meshes/materials gameplay uses so
/// the key always matches what players see, give it `HowToPlayScreen + Spin`,
/// and pair it with a label. Grid positions sit in front of the Startup
/// camera at (0, 0, 20) looking at the origin.
pub fn spawn_how_to_play(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Headline and launch prompt.
    commands.spawn((
        HowToPlayScreen,
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceBetween,
            padding: UiRect::axes(Val::Px(0.0), Val::Px(40.0)),
            ..default()
        },
        children![
            (
                Text::new("HOW TO PLAY"),
                TextFont {
                    font_size: 48.0,
                    ..default()
                },
                TextColor(Color::srgb(0.2, 0.8, 1.0)),
            ),
            (
                Text::new("PRESS ENTER TO START"),
                TextFont {
                    font_size: 26.0,
                    ..default()
                },
                TextColor(Color::srgb(0.9, 0.95, 1.0)),
                Pulse {
                    speed: 3.0,
                    min: 0.25,
                    max: 1.0,
                },
            ),
        ],
    ));

    // Placeholder showcase item: swap for your game's real entities.
    let item = commands
        .spawn((
            HowToPlayScreen,
            Spin,
            Mesh3d(meshes.add(Cuboid::new(1.5, 1.5, 1.5))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::srgb(0.2, 0.8, 1.0),
                emissive: LinearRgba::new(0.1, 0.6, 1.0, 1.0),
                ..default()
            })),
            Transform::from_xyz(0.0, 1.0, 0.0),
        ))
        .id();
    commands.spawn((
        HowToPlayScreen,
        ItemLabel { target: item },
        Node {
            position_type: PositionType::Absolute,
            width: Val::Px(LABEL_WIDTH),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: Val::Px(2.0),
            ..default()
        },
        children![
            (
                Text::new("YOUR ITEM"),
                TextFont {
                    font_size: 17.0,
                    ..default()
                },
                TextColor(Color::srgb(0.2, 0.8, 1.0)),
            ),
            (
                Text::new("Describe it here"),
                TextFont {
                    font_size: 13.0,
                    ..default()
                },
                TextColor(Color::srgb(0.55, 0.6, 0.7)),
            ),
        ],
    ));
}

/// Slowly rotates showcase items around Y.
pub fn spin_items(time: Res<Time>, mut query: Query<&mut Transform, With<Spin>>) {
    for mut tf in &mut query {
        tf.rotate_y(0.6 * time.delta_secs());
    }
}

/// Pins each label under its 3D item by projecting the item's position to
/// viewport coordinates. Runs every frame so labels stay correct on resize.
///
/// Labels are placed from default (identity) GlobalTransforms on the first
/// frame after state entry; this is invisible only because HowToPlay is
/// always entered through a near-black ScreenFade. Keep it that way.
pub fn position_labels(
    camera_q: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    ui_scale: Res<UiScale>,
    items: Query<&GlobalTransform, (With<Spin>, Without<Camera3d>)>,
    mut labels: Query<(&ItemLabel, &mut Node)>,
) {
    let Ok((camera, cam_tf)) = camera_q.single() else {
        return;
    };
    for (label, mut node) in &mut labels {
        let Ok(item_tf) = items.get(label.target) else {
            continue;
        };
        // Anchor just below the item so the label clears the mesh.
        let anchor = item_tf.translation() - Vec3::Y * 1.6;
        let Ok(viewport) = camera.world_to_viewport(cam_tf, anchor) else {
            continue;
        };
        // Node Px values get multiplied by UiScale at layout time; divide so
        // the node lands on the viewport-pixel position we computed.
        node.left = Val::Px(viewport.x / ui_scale.0 - LABEL_WIDTH / 2.0);
        node.top = Val::Px(viewport.y / ui_scale.0);
    }
}

/// Launch input: Enter/Space or gamepad South, gated on the fade being idle.
pub fn how_to_play_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    mut fade: ResMut<ScreenFade>,
) {
    if !fade.is_idle() {
        return;
    }
    let mut start =
        keyboard.just_pressed(KeyCode::Enter) || keyboard.just_pressed(KeyCode::Space);
    if !start {
        for gamepad in &gamepads {
            if gamepad.just_pressed(GamepadButton::South) {
                start = true;
                break;
            }
        }
    }
    if start {
        let _ = fade.request(GameState::Playing);
    }
}

/// Despawns everything the how-to-play screen spawned.
pub fn despawn_how_to_play(mut commands: Commands, query: Query<Entity, With<HowToPlayScreen>>) {
    for entity in &query {
        commands.entity(entity).despawn();
    }
}
```

- [ ] **Step 3: Wire in `src/ui/mod.rs`** — add after the studio logo block:

```rust
            // How-to-play screen (shown once per session before the first game)
            .init_resource::<how_to_play::SeenHowToPlay>()
            .add_systems(
                OnEnter(GameState::HowToPlay),
                (how_to_play::spawn_how_to_play, how_to_play::mark_seen),
            )
            .add_systems(
                Update,
                (
                    how_to_play::spin_items,
                    how_to_play::position_labels,
                    how_to_play::how_to_play_input,
                )
                    .run_if(in_state(GameState::HowToPlay)),
            )
            .add_systems(OnExit(GameState::HowToPlay), how_to_play::despawn_how_to_play)
```

- [ ] **Step 4: Test**

Run: `cargo test` — expected: 8 kit tests pass. Screen unreachable until Task 6.

- [ ] **Step 5: Commit**

```bash
git add src/ui/how_to_play.rs src/ui/mod.rs
git commit -m "feat: once-per-session how-to-play screen with placeholder showcase"
```

---

### Task 6: Menu rework — pulsing title, fade-routed flow

**Files:**
- Modify: `src/ui/menu.rs`

- [ ] **Step 1: Replace `src/ui/menu.rs` with:**

```rust
use bevy::prelude::*;

use crate::game::states::GameState;
use crate::ui::how_to_play::{start_target, SeenHowToPlay};
use crate::ui::transition::{Pulse, ScreenFade};

#[derive(Component)]
pub struct MenuRoot;

/// Spawns the title screen: styled text title with a pulsing start prompt
/// over a bottom scrim.
///
/// TEMPLATE NOTE — artwork variant: if your game has full-bleed title art
/// (logotype baked in, like Voidrunner/Hunted), replace the title text with
/// a cover-fit image and preload the handle at Startup so it never pops in:
///
/// ```ignore
/// #[derive(Resource)]
/// pub struct TitleArtwork(pub Handle<Image>);
/// pub fn preload_title_artwork(mut commands: Commands, assets: Res<AssetServer>) {
///     commands.insert_resource(TitleArtwork(assets.load("title.png")));
/// }
/// // In spawn_menu (root gets Overflow::clip()):
/// //   (ImageNode::new(artwork.0.clone()),
/// //    Node { width: Val::Percent(100.0), flex_shrink: 0.0, ..default() }),
/// // and register preload_title_artwork in the plugin's Startup set.
/// ```
pub fn spawn_menu(mut commands: Commands) {
    commands.spawn((
        MenuRoot,
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            row_gap: Val::Px(20.0),
            ..default()
        },
        BackgroundColor(Color::srgb(0.01, 0.02, 0.05)),
        children![
            (
                Text::new("GAMEBIENT GAME"),
                TextFont {
                    font_size: 72.0,
                    ..default()
                },
                TextColor(Color::srgb(0.2, 0.8, 1.0)),
            ),
            // Bottom strip: scrim holding the pulsing prompt + controls line.
            (
                Node {
                    position_type: PositionType::Absolute,
                    bottom: Val::Px(0.0),
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(10.0),
                    padding: UiRect::axes(Val::Px(0.0), Val::Px(24.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
                children![
                    (
                        Text::new("PRESS ENTER TO START"),
                        TextFont {
                            font_size: 28.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.9, 0.95, 1.0)),
                        Pulse {
                            speed: 3.0,
                            min: 0.25,
                            max: 1.0,
                        },
                    ),
                    (
                        Text::new("WASD / Arrows: Move  |  Esc: Pause"),
                        TextFont {
                            font_size: 18.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.5, 0.6, 0.7)),
                    ),
                ],
            ),
        ],
    ));
}

/// Spawns the game-over screen.
pub fn spawn_game_over(mut commands: Commands) {
    commands.spawn((
        MenuRoot,
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            row_gap: Val::Px(20.0),
            ..default()
        },
        BackgroundColor(Color::srgb(0.05, 0.01, 0.02)),
        children![
            (
                Text::new("GAME OVER"),
                TextFont {
                    font_size: 64.0,
                    ..default()
                },
                TextColor(Color::srgb(1.0, 0.3, 0.2)),
            ),
            (
                Text::new("PRESS ENTER TO CONTINUE"),
                TextFont {
                    font_size: 24.0,
                    ..default()
                },
                TextColor(Color::srgb(0.9, 0.95, 1.0)),
                Pulse {
                    speed: 3.0,
                    min: 0.25,
                    max: 1.0,
                },
            ),
        ],
    ));
}

pub fn despawn_menu(mut commands: Commands, query: Query<Entity, With<MenuRoot>>) {
    for e in &query {
        commands.entity(e).despawn();
    }
}

/// ENTER routes `Menu -> HowToPlay/Playing` (once-per-session gate) and
/// `GameOver -> Menu`, always through the fade.
pub fn menu_input(
    input: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    state: Res<State<GameState>>,
    seen: Res<SeenHowToPlay>,
    mut fade: ResMut<ScreenFade>,
) {
    if !fade.is_idle() {
        return;
    }
    let mut confirm = input.just_pressed(KeyCode::Enter) || input.just_pressed(KeyCode::Space);
    if !confirm {
        for gamepad in &gamepads {
            if gamepad.just_pressed(GamepadButton::South) {
                confirm = true;
                break;
            }
        }
    }
    if !confirm {
        return;
    }
    match state.get() {
        GameState::Menu => {
            let _ = fade.request(start_target(seen.0));
        }
        GameState::GameOver => {
            let _ = fade.request(GameState::Menu);
        }
        _ => {}
    }
}
```

Note the flow change: `GameOver` now returns to `Menu` (kit-consistent) instead of straight to `Playing`.

- [ ] **Step 2: Test**

Run: `cargo test` — expected: all pass. `cargo build` — expected: no dead-code warnings remain for kit items.

- [ ] **Step 3: Commit**

```bash
git add src/ui/menu.rs
git commit -m "feat: pulsing title over scrim, fade-routed start and game-over flow"
```

---

### Task 7: Pause with quit-to-title

**Files:**
- Modify: `src/game/mod.rs`

- [ ] **Step 1: Add pause to `src/game/mod.rs`**

a) Add to `GamePlugin::build`, after the existing `.add_systems(OnExit(...))` call:

```rust
            .init_resource::<states::Paused>()
            .add_systems(OnEnter(GameState::Playing), reset_paused)
            .add_systems(
                Update,
                (toggle_pause, pause_quit)
                    .chain()
                    .run_if(in_state(GameState::Playing)),
            )
            .add_systems(OnExit(GameState::Playing), cleanup_pause_overlay)
```

b) Gate the existing gameplay systems: change the existing Update registration to

```rust
            .add_systems(
                Update,
                (player::move_player, scoring::handle_score_events)
                    .run_if(in_state(GameState::Playing).and(states::not_paused)),
            )
```

c) Append these items to the file:

```rust
/// Marker for the "PAUSED" overlay so it can be despawned on unpause.
#[derive(Component)]
struct PauseOverlay;

/// Resets pause state when a run starts.
fn reset_paused(mut paused: ResMut<states::Paused>) {
    paused.0 = false;
}

/// Despawns the pause overlay when leaving Playing (e.g. quit while paused).
fn cleanup_pause_overlay(mut commands: Commands, query: Query<Entity, With<PauseOverlay>>) {
    for entity in &query {
        commands.entity(entity).despawn();
    }
}

/// Toggles pause on Escape or gamepad Start and shows/hides the overlay.
fn toggle_pause(
    mut commands: Commands,
    keyboard: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    mut paused: ResMut<states::Paused>,
    overlay_query: Query<Entity, With<PauseOverlay>>,
    fade: Res<crate::ui::transition::ScreenFade>,
) {
    if !fade.is_idle() {
        return;
    }
    let mut pressed = keyboard.just_pressed(KeyCode::Escape);
    for gamepad in &gamepads {
        if gamepad.just_pressed(GamepadButton::Start) {
            pressed = true;
            break;
        }
    }
    if !pressed {
        return;
    }

    paused.0 = !paused.0;

    if paused.0 {
        commands
            .spawn((
                PauseOverlay,
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(16.0),
                    position_type: PositionType::Absolute,
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7)),
                GlobalZIndex(100),
            ))
            .with_children(|parent| {
                parent.spawn((
                    Text::new("PAUSED"),
                    TextFont {
                        font_size: 64.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.2, 0.8, 1.0)),
                ));
                parent.spawn((
                    Text::new("ESC: RESUME"),
                    TextFont {
                        font_size: 22.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.9, 0.95, 1.0)),
                ));
                parent.spawn((
                    Text::new("ENTER: QUIT TO TITLE"),
                    TextFont {
                        font_size: 22.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.5, 0.6, 0.7)),
                ));
            });
    } else {
        for entity in &overlay_query {
            commands.entity(entity).despawn();
        }
    }
}

/// While paused, Enter (or gamepad East) quits back to the title screen
/// through the fade. Run state resets on the next Playing entry
/// (`reset_paused`); OnExit(Playing) systems handle cleanup.
fn pause_quit(
    keyboard: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    paused: Res<states::Paused>,
    mut fade: ResMut<crate::ui::transition::ScreenFade>,
) {
    if !paused.0 || !fade.is_idle() {
        return;
    }
    let mut quit = keyboard.just_pressed(KeyCode::Enter);
    if !quit {
        for gamepad in &gamepads {
            if gamepad.just_pressed(GamepadButton::East) {
                quit = true;
                break;
            }
        }
    }
    if quit {
        let _ = fade.request(GameState::Menu);
    }
}
```

- [ ] **Step 2: Test**

Run: `cargo test` and `cargo clippy --all-targets -- -D warnings` — expected: pass/clean.

- [ ] **Step 3: Commit**

```bash
git add src/game/mod.rs
git commit -m "feat: pause overlay with quit-to-title through the fade"
```

---

### Task 8: `ClearColor` + web boot rework

**Files:**
- Modify: `src/main.rs`
- Modify: `index.html`

- [ ] **Step 1: ClearColor in `src/main.rs`** — add to the App builder before `.add_plugins((game::GamePlugin, ...))`:

```rust
        // Near-black clear color: visible wherever no geometry/UI covers the
        // viewport (notably the how-to-play showcase background).
        .insert_resource(ClearColor(Color::srgb(0.008, 0.012, 0.03)))
```

- [ ] **Step 2: Port the web boot rework to `index.html`**

Reference: `git -C ../voidrunner show cab6559:index.html` lines ~210-290 (read it). Apply the same restructure to the template's `index.html`, keeping the `gamebient-game` name tokens:

Replace the block that currently reads (locate by content, ~lines 211-260):

```js
        const { default: init } = await import('./gamebient-game.js');

        setProgress(50, 'Initializing engine...');
        // ... brotli probe ...
        try {
            await init(wasmUrl ? { module_or_path: wasmUrl } : undefined);
        } catch (e) { ... }
        setProgress(100, 'Ready');
        loadingOverlay.classList.add('hidden');
        audioUnlock.style.display = 'flex';
        const unlock = () => { ... };
        document.addEventListener('click', unlock);
        document.addEventListener('touchend', unlock);
        document.addEventListener('keydown', unlock);
```

with (keep the brotli `wasmUrl` probe exactly where it is, between the import and this block):

```js
        // Prefetch and compile the wasm bytes NOW (behind the progress bar) so
        // the network cost is paid before the user clicks — the engine start is
        // deferred to the unlock click so that the studio-logo screen plays only
        // after the user has seen "Click to Start" and interacted.
        let wasmModule;
        try {
            setProgress(30, 'Downloading game...');
            const wasmResponse = await fetch(wasmUrl || './gamebient-game_bg.wasm');
            if (!wasmResponse.ok) {
                throw new Error('HTTP ' + wasmResponse.status);
            }
            setProgress(60, 'Compiling...');
            wasmModule = await WebAssembly.compile(await wasmResponse.arrayBuffer());
            setProgress(100, 'Ready');
        } catch (e) {
            showError('Failed to download game: ' + (e?.message || e));
            throw e;
        }

        // Show click-to-start. The engine (init) runs only after the click so
        // the studio-logo screen timer begins when the user is actually watching.
        loadingOverlay.classList.add('hidden');
        audioUnlock.style.display = 'flex';

        let started = false;
        const unlock = async () => {
            // Guard against re-entry: set flag synchronously before any await.
            if (started) return;
            started = true;
            document.removeEventListener('click', unlock);
            document.removeEventListener('touchend', unlock);
            document.removeEventListener('keydown', unlock);
            userHasInteracted = true;
            trackedContexts.forEach(ctx => {
                if (ctx.state === 'suspended') ctx.resume();
            });
            audioUnlock.style.display = 'none';
            gameCanvas.focus();
            try {
                await init({ module_or_path: wasmModule });
            } catch (e) {
                // wasm-bindgen uses exceptions for control flow — not a real error
                if (e?.message?.includes('Using exceptions for control flow')) {
                    console.debug('[wasm-bindgen] Ignoring control-flow exception');
                } else {
                    showError('Failed to start: ' + e.message);
                    throw e;
                }
            }
        };
        document.addEventListener('click', unlock);
        document.addEventListener('touchend', unlock);
        document.addEventListener('keydown', unlock);
```

Keep everything else (audio-context patching, `userHasInteracted`, WebGL2 check, error overlay) untouched. Compare against the voidrunner reference to confirm structural parity.

- [ ] **Step 3: Verify**

`cargo build`, `cargo test`, `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings` — all clean. Eyeball the edited index.html block for balanced braces.

- [ ] **Step 4: Commit**

```bash
git add src/main.rs index.html
git commit -m "feat: void-dark clear color, start engine on web unlock click"
```

---

### Task 9: Conventions doc

**Files:**
- Modify: `docs/conventions.md`

- [ ] **Step 1: Append this section:**

```markdown
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
```

- [ ] **Step 2: Commit**

```bash
git add docs/conventions.md
git commit -m "docs: presentation kit conventions"
```

---

### Task 10: Full verification + PR

- [ ] **Step 1: Gates** — `cargo test` (8 kit tests + pre-existing), `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`.

- [ ] **Step 2: Web runtime verification** (controller does this — headless Chrome recipe from `../voidrunner`, see memory note): `PATH=/tmp/vr-verify/wbg/bin:$PATH ./build_web.sh`, serve `dist/`, drive: unlock → logo screenshot → title (pulse) → Enter → how-to-play (placeholder item + label) → Enter → gameplay → Esc pause overlay → Enter quit-to-title → Enter second start skips how-to-play. Plus wasm-404 error-overlay probe.

- [ ] **Step 3: Push + PR**

```bash
git push -u origin feat/presentation-polish
gh pr create --title "Presentation kit: studio logo, fades, how-to-play, pause quit-to-title" --body "..."
```

(PR body: summary of the kit + verification evidence; note this is the canonical source the game repos copy.)
