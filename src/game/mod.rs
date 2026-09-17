use bevy::prelude::*;

pub mod audio;
#[cfg(feature = "autopilot")]
pub mod autopilot;
pub mod host;
pub mod input;
pub mod player;
#[cfg(feature = "record")]
pub mod record;
pub mod replay;
pub mod scoring;
pub mod sim;
pub mod states;

use states::GameState;

/// Marker for entities spawned during a run; despawned on cleanup.
#[derive(Component)]
pub struct GameEntity;

/// `headless: true` builds the sim only (no scene, audio, overlay or fade)
/// for the replay verifier. The windowed game uses `GamePlugin::default()`.
#[derive(Default)]
pub struct GamePlugin {
    pub headless: bool,
}

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        let input = if self.headless {
            gamebient_input::GxInputPlugin::headless("Gamebient Game")
        } else {
            let mut p = gamebient_input::GxInputPlugin::named("Gamebient Game");
            p.config.tick_input = true;
            p
        };
        app.init_state::<GameState>()
            // Canon input + web glue. StateEvents posts every GameState
            // transition to the embedding host (website analytics, cabinet).
            .add_plugins(input)
            .add_plugins(gamebient_input::StateEvents::<GameState>::default())
            // Host commands (pause / resume / mute) and the events hosts act
            // on (started, gameover, score, paused).
            .add_plugins(host::HostBridgePlugin)
            .insert_resource(Time::<Fixed>::from_duration(sim::tick_duration()))
            .init_resource::<scoring::GameData>()
            .init_resource::<states::Paused>()
            .init_resource::<sim::SimTick>()
            .init_resource::<sim::RunSeed>()
            .init_resource::<sim::PendingSeed>()
            .init_resource::<sim::GameRng>()
            .init_resource::<sim::Checksum>()
            .init_resource::<sim::SimPrev>()
            .init_resource::<sim::RunOver>()
            .init_resource::<replay::recorder::ReplayRecorder>()
            .add_message::<scoring::ScoreEvent>()
            .add_message::<audio::SfxEvent>()
            // Three clauses, and every game needs all three: the run is
            // live, it is not paused, and it is not already over.
            // `sim::run_not_over` freezes the sim on the tick the run ends
            // so the windowed game's game-over fade (`Update`, frame delta)
            // cannot add a frame-rate-dependent tail of ticks the headless
            // verifier can't reproduce. See `sim::RunOver` / `sim::end_run`.
            .configure_sets(
                FixedUpdate,
                sim::SimSet.run_if(
                    in_state(GameState::Playing)
                        .and(states::not_paused)
                        .and(sim::run_not_over),
                ),
            )
            .add_systems(
                OnEnter(GameState::Playing),
                (
                    reset_paused,
                    reset_game_data,
                    sim::begin_run,
                    replay::recorder::begin_recording,
                    player::spawn_player,
                )
                    .chain(),
            )
            .add_systems(
                FixedUpdate,
                toggle_pause
                    .run_if(in_state(GameState::Playing))
                    .before(sim::SimSet),
            )
            .add_systems(
                FixedUpdate,
                (
                    sim::advance_tick,
                    player::move_player,
                    scoring::handle_score_events,
                    sim::checksum_tick,
                    player::checksum_player,
                    replay::recorder::record_tick,
                    sim::remember_sim_prev,
                )
                    .chain()
                    .in_set(sim::SimSet),
            )
            // Keeps a paused stretch from moving the press edges the replay
            // reproduces: see `sim::restore_tick_frame_while_paused`. Runs in
            // both modes — the headless verifier never pauses (the recorder
            // masks PAUSE), but the two apps must be built the same way.
            .add_systems(
                FixedUpdate,
                sim::restore_tick_frame_while_paused
                    .after(sim::SimSet)
                    .run_if(in_state(GameState::Playing).and(|p: Res<states::Paused>| p.0)),
            )
            .add_systems(
                OnExit(GameState::Playing),
                (replay::recorder::seal_run, cleanup_game_entities).chain(),
            );

        if !self.headless {
            app.init_resource::<audio::CurrentTrack>()
                .add_systems(Startup, (setup_scene, audio::setup_sfx))
                .add_systems(
                    Update,
                    (
                        audio::play_sfx,
                        audio::music_director,
                        audio::update_music_fades,
                    ),
                )
                .add_systems(
                    Update,
                    (pause_quit, sync_pause_overlay)
                        .chain()
                        .run_if(in_state(GameState::Playing)),
                )
                .add_systems(OnExit(GameState::Playing), cleanup_pause_overlay);

            // The autopilot/record dev harnesses drive a real window
            // (screenshots, `ScreenFade`, a manual render clock) and have no
            // meaning for the headless sim; keep them out of headless apps
            // even when the feature is enabled (CI runs `cargo test
            // --all-features`, which would otherwise build a headless app
            // with these systems wired in).
            #[cfg(feature = "autopilot")]
            app.add_plugins(autopilot::AutopilotPlugin);
            #[cfg(feature = "record")]
            {
                app.add_plugins(record::RecordPlugin);
                record::log_state::<GameState>(app);
                record::log_messages::<audio::SfxEvent>(app);
                record::log_value::<scoring::GameData>(app, "score", |d| i64::from(d.score));
                record::log_value::<states::Paused>(app, "pause", |p| i64::from(p.0));
            }
        }
    }
}

/// One-time scene setup: a camera and a directional light.
fn setup_scene(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 0.0, 20.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.spawn((
        DirectionalLight::default(),
        Transform::from_xyz(4.0, 8.0, 8.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

/// Despawns all run entities when leaving `Playing`.
fn cleanup_game_entities(mut commands: Commands, query: Query<Entity, With<GameEntity>>) {
    for e in &query {
        commands.entity(e).despawn();
    }
}

/// Marker for the "PAUSED" overlay so it can be despawned on unpause.
#[derive(Component)]
struct PauseOverlay;

/// Resets pause state when a run starts.
fn reset_paused(mut paused: ResMut<states::Paused>) {
    paused.0 = false;
}

/// Resets per-run data when a run starts (preserving the cross-run high score).
fn reset_game_data(mut data: ResMut<scoring::GameData>) {
    let high_score = data.high_score;
    *data = scoring::GameData::default();
    data.high_score = high_score;
}

/// Despawns the pause overlay when leaving Playing (e.g. quit while paused).
fn cleanup_pause_overlay(mut commands: Commands, query: Query<Entity, With<PauseOverlay>>) {
    for entity in &query {
        commands.entity(entity).despawn();
    }
}

/// Toggles pause on the canon Pause action (Escape, gamepad Start, pad
/// Pause). The overlay is driven by `sync_pause_overlay`, so a host pause
/// (gx:set) looks exactly the same. Runs in `FixedUpdate` before `SimSet` so
/// a pause takes effect before that tick's gameplay systems run; headless
/// builds have no `ScreenFade` (no UI), so it's read as optional.
fn toggle_pause(
    input: Res<gamebient_input::TickInput>,
    mut paused: ResMut<states::Paused>,
    fade: Option<Res<crate::ui::transition::ScreenFade>>,
    mut sfx: MessageWriter<audio::SfxEvent>,
) {
    if fade.as_ref().is_some_and(|f| !f.is_idle()) || !input.pause_just_pressed {
        return;
    }
    paused.0 = !paused.0;
    sfx.write(audio::SfxEvent::Pause);
}

/// Shows or hides the "PAUSED" overlay whenever `Paused` changes, whoever
/// changed it (player input or a host command).
fn sync_pause_overlay(
    mut commands: Commands,
    paused: Res<states::Paused>,
    overlay_query: Query<Entity, With<PauseOverlay>>,
) {
    if !paused.is_changed() {
        return;
    }
    if paused.0 {
        if !overlay_query.is_empty() {
            return;
        }
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

/// While paused, Start (Enter / pad Start) or B (X / gamepad East) quits
/// back to the title screen through the fade. Run state resets on the next
/// Playing entry (`reset_paused`); OnExit(Playing) systems handle cleanup.
fn pause_quit(
    input: Res<input::GameInput>,
    paused: Res<states::Paused>,
    mut fade: ResMut<crate::ui::transition::ScreenFade>,
) {
    if !paused.0 || !fade.is_idle() {
        return;
    }
    if input.start_just_pressed || input.secondary_just_pressed {
        let _ = fade.request(GameState::Menu);
    }
}

#[cfg(test)]
mod tests {
    use bevy::input::InputPlugin;
    use bevy::state::app::StatesPlugin;
    use bevy::time::TimeUpdateStrategy;

    use super::*;

    /// Boots `GamePlugin { headless: true }` under `MinimalPlugins` (no
    /// window, no audio, no renderer) exactly as the future replay verifier
    /// will, and proves the fixed-tick chain actually runs: `SimTick`
    /// advances and `Checksum` moves off its default once a run starts.
    ///
    /// `InputPlugin` is added alongside `MinimalPlugins`: it owns
    /// `ButtonInput<KeyCode>` / `ButtonInput<GamepadButton>`, which
    /// `gamebient_input`'s `accumulate_input`/`collect_input` read
    /// unconditionally (headless or not, so a replay feeder can sit
    /// alongside live input) and which `MinimalPlugins` alone does not
    /// provide. It's a plain resource/event registration with no window or
    /// OS dependency, so it's headless-safe.
    #[test]
    fn headless_game_plugin_boots_and_ticks_on_minimal_plugins() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(StatesPlugin)
            .add_plugins(InputPlugin)
            .insert_resource(TimeUpdateStrategy::ManualDuration(sim::tick_duration()))
            .add_plugins(GamePlugin { headless: true });
        app.update(); // first frame: zero delta, no fixed tick
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Playing);
        app.update(); // transition applies; OnEnter(Playing) runs begin_run + spawn_player
        for _ in 0..10 {
            app.update();
        }
        let tick = app.world().resource::<sim::SimTick>().0;
        assert!(
            tick >= 10,
            "sim ticks should advance under the manual clock, got {tick}"
        );
        assert_ne!(
            app.world().resource::<sim::Checksum>().0,
            sim::Checksum::default().0
        );
    }
}
