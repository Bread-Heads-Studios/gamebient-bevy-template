pub mod synth;

use bevy::audio::{PlaybackMode, Volume};
use bevy::prelude::*;

use crate::game::states::GameState;

/// Game-specific sound effects. Emit via `MessageWriter<SfxEvent>`; the
/// dispatcher spawns a despawn-on-finish player. Extend per game.
#[derive(Message, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum SfxEvent {
    Confirm,
    Pause,
    ScreenSweep,
}

/// Handles baked once at startup — synthesis happens exactly once.
#[derive(Resource)]
pub struct SfxAssets {
    confirm: Handle<AudioSource>,
    pause: Handle<AudioSource>,
    screen_sweep: Handle<AudioSource>,
}

impl SfxAssets {
    fn get(&self, event: SfxEvent) -> Handle<AudioSource> {
        match event {
            SfxEvent::Confirm => self.confirm.clone(),
            SfxEvent::Pause => self.pause.clone(),
            SfxEvent::ScreenSweep => self.screen_sweep.clone(),
        }
    }
}

pub fn setup_sfx(mut commands: Commands, mut sources: ResMut<Assets<AudioSource>>) {
    commands.insert_resource(SfxAssets {
        confirm: sources.add(synth::blip(880.0)),
        pause: sources.add(synth::blip(440.0)),
        screen_sweep: sources.add(synth::sweep(300.0, 900.0, 0.3)),
    });
}

/// Spawns a one-shot player per received event.
pub fn play_sfx(mut commands: Commands, mut events: MessageReader<SfxEvent>, sfx: Res<SfxAssets>) {
    for event in events.read() {
        commands.spawn((AudioPlayer(sfx.get(*event)), PlaybackSettings::DESPAWN));
    }
}

// --- Music slots -----------------------------------------------------------

/// Per-state music table. `None` = silence in that state (nothing is loaded,
/// no missing-asset errors). To add music: drop an .ogg under
/// assets/audio/music/ and name it here.
const MUSIC: &[(GameState, Option<&str>)] = &[
    (GameState::StudioLogo, None),
    (GameState::Menu, None),
    (GameState::HowToPlay, None),
    (GameState::Playing, None),
    (GameState::GameOver, None),
];

/// Currently-playing track path, if any.
#[derive(Resource, Default)]
pub struct CurrentTrack(Option<&'static str>);

/// Crossfades to the entering state's slot when it differs from the current
/// track. Runs on every state change; no-ops while the slot matches.
pub fn music_director(
    mut commands: Commands,
    state: Res<State<GameState>>,
    mut current: ResMut<CurrentTrack>,
    asset_server: Res<AssetServer>,
    playing: Query<(Entity, &AudioSink), With<MusicTrack>>,
) {
    if !state.is_changed() {
        return;
    }
    let want = MUSIC
        .iter()
        .find(|(s, _)| s == state.get())
        .and_then(|(_, path)| *path);
    if want == current.0 {
        return;
    }
    current.0 = want;
    crossfade_to(&mut commands, &playing, want.map(|p| asset_server.load(p)));
}

// --- Voidrunner music fade lift (adapted: Option<Handle> signature) ---------

/// Marker component for the currently active music entity.
#[derive(Component)]
pub struct MusicTrack;

/// Fades music volume from 0 to target over `duration` seconds.
#[derive(Component)]
pub struct MusicFadeIn {
    pub elapsed: f32,
    pub duration: f32,
    pub target_volume: f32,
}

/// Fades music volume to 0 then despawns the entity.
#[derive(Component)]
pub struct MusicFadeOut {
    pub elapsed: f32,
    pub duration: f32,
    pub start_volume: f32,
}

/// Crossfades from any current music to a new track.
/// `handle` is `None` to fade out everything and start nothing (silence slot).
///
/// The template's `MUSIC` table carries no per-track parameters, so the fade
/// timings, loop mode, and target volume are fixed here (1.5 s out, 2.0 s in,
/// looping, full volume). If a game needs per-track control — e.g. a one-shot
/// game-over sting (`PlaybackMode::Once`) or a boosted boss theme — reintroduce
/// voidrunner's `looping` / `fade_out_secs` / `fade_in_secs` / `target_volume`
/// parameters and extend the table.
fn crossfade_to(
    commands: &mut Commands,
    music_query: &Query<(Entity, &AudioSink), With<MusicTrack>>,
    handle: Option<Handle<AudioSource>>,
) {
    // Fade out all current music entities. Sampling the sink's current volume
    // (rather than assuming 1.0) keeps a track interrupted mid-fade-in from
    // popping to full volume before it fades out.
    for (entity, sink) in music_query.iter() {
        let current_vol = sink.volume().to_linear();
        commands
            .entity(entity)
            .remove::<MusicTrack>()
            .remove::<MusicFadeIn>()
            .insert(MusicFadeOut {
                elapsed: 0.0,
                duration: 1.5,
                start_volume: current_vol,
            });
    }

    // Spawn new track at zero volume, fading in (only if a path was given)
    if let Some(h) = handle {
        commands.spawn((
            MusicTrack,
            AudioPlayer::new(h),
            PlaybackSettings {
                mode: PlaybackMode::Loop,
                volume: Volume::Linear(0.0),
                ..default()
            },
            MusicFadeIn {
                elapsed: 0.0,
                duration: 2.0,
                target_volume: 1.0,
            },
        ));
    }
}

/// Ticks all music fades: increases volume on fade-ins, decreases on fade-outs.
pub fn update_music_fades(
    mut commands: Commands,
    time: Res<Time>,
    mut fade_in_q: Query<(Entity, &mut MusicFadeIn, &mut AudioSink)>,
    mut fade_out_q: Query<(Entity, &mut MusicFadeOut, &mut AudioSink), Without<MusicFadeIn>>,
) {
    let dt = time.delta_secs();

    for (entity, mut fade, mut sink) in &mut fade_in_q {
        fade.elapsed += dt;
        let t = (fade.elapsed / fade.duration).min(1.0);
        sink.set_volume(Volume::Linear(t * fade.target_volume));
        if t >= 1.0 {
            commands.entity(entity).remove::<MusicFadeIn>();
        }
    }

    for (entity, mut fade, mut sink) in &mut fade_out_q {
        fade.elapsed += dt;
        let t = 1.0 - (fade.elapsed / fade.duration).min(1.0);
        sink.set_volume(Volume::Linear(t * fade.start_volume));
        if t <= 0.0 {
            commands.entity(entity).despawn();
        }
    }
}
