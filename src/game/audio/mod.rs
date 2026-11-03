//! The audio bus: sound effects as messages, music as per-state cues.
//!
//! * **SFX.** Gameplay and UI write [`SfxEvent`]s; [`play_sfx`] spawns a
//!   despawn-on-finish player for each. Every event is baked once at startup
//!   by [`bake`], on the house chip ([`chip`], [`tracker::Fx`]), so a game's
//!   sounds share one timbre with its music. Events that are stings
//!   ([`SfxEvent::is_sting`]) duck the music while they play.
//! * **Music.** [`MUSIC`] maps every `GameState` to a [`Cue`]: a looping or
//!   one-shot [`Source`] (a baked song or an `.ogg` under `assets/`), its
//!   fades and volume, and optional **layers** of the same length that start
//!   with it at zero volume and fade in on [`LayerEvent`]. Gameplay may also
//!   **switch** the music within a state (a boss, a race leg) with
//!   [`CueEvent::Switch`]; the table wins again on the next state change.
//!   Baked cues render lazily into [`MusicCache`]; [`prewarm_music`] renders
//!   them all while the player reads the title screen.
//!
//! Port checklist: replace the three template events and their `bake` arms
//! with the game's own, compose `songs.rs`, point `MUSIC` at it, and write
//! `docs/audio-identity.md`. Never play audio directly from a gameplay
//! system; emit a message. `chip.rs`, `tracker.rs` and `house.rs` are copied
//! verbatim from the template.

pub mod chip;
pub mod house;
pub mod songs;
pub mod synth;
pub mod tracker;

use bevy::audio::{PlaybackMode, Volume};
use bevy::prelude::*;

use crate::game::states::GameState;
use chip::{NoiseMode, NoiseRate, note_hz};
use tracker::{Fx, wav_secs};

// --- Sound effects ------------------------------------------------------------

/// Game-specific sound effects. Emit via `MessageWriter<SfxEvent>`. Extend
/// per game; keep `ALL` and `bake` in step (a test checks every variant).
#[derive(Message, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum SfxEvent {
    Confirm,
    Pause,
    ScreenSweep,
}

impl SfxEvent {
    pub const ALL: &[SfxEvent] = &[SfxEvent::Confirm, SfxEvent::Pause, SfxEvent::ScreenSweep];

    /// Stings (fanfares, big rewards) duck the music by 6 dB while they play.
    /// The template has none; a game marks its own here.
    pub fn is_sting(self) -> bool {
        false
    }

    /// Lower-case variant name, used for export file names.
    pub fn name(self) -> String {
        format!("{self:?}").to_lowercase()
    }
}

fn hz(name: &str) -> f32 {
    note_hz(name).expect("note name")
}

/// Renders one event on the house chip. The template speaks C major:
/// confirm is a rising fifth, pause a falling one, the screen sweep a rising
/// glide with a noise shimmer. A game replaces all three in its own key.
pub fn bake(event: SfxEvent) -> AudioSource {
    match event {
        SfxEvent::Confirm => Fx::new()
            .hit(0, hz("c5"), 5)
            .then(Fx::new().hit(0, hz("g5"), 9))
            .render(),
        SfxEvent::Pause => Fx::new()
            .hit(0, hz("g4"), 5)
            .then(Fx::new().hit(0, hz("c4"), 9))
            .render(),
        SfxEvent::ScreenSweep => Fx::new()
            .tone(0, hz("c4"), hz("c6"), 2, 15, 18)
            .with(Fx::new().noise(NoiseMode::White, NoiseRate::Div512, 6, 15, 12))
            .render(),
    }
}

/// Handles baked once at startup, with each clip's length for ducking.
/// (A `Vec` keyed by event, not a map: `src/game/` bans hash maps for the
/// sim's sake and the scan does not know this is presentation.)
#[derive(Resource, Default)]
pub struct SfxAssets {
    clips: Vec<(SfxEvent, Handle<AudioSource>, f32)>,
}

impl SfxAssets {
    fn get(&self, event: SfxEvent) -> Option<(&Handle<AudioSource>, f32)> {
        self.clips
            .iter()
            .find(|(e, _, _)| *e == event)
            .map(|(_, h, secs)| (h, *secs))
    }
}

pub fn setup_sfx(mut commands: Commands, mut sources: ResMut<Assets<AudioSource>>) {
    let mut assets = SfxAssets::default();
    for event in SfxEvent::ALL {
        let source = bake(*event);
        let secs = wav_secs(&source);
        assets.clips.push((*event, sources.add(source), secs));
    }
    commands.insert_resource(assets);
}

/// Spawns a one-shot player per received event; stings start a duck.
pub fn play_sfx(
    mut commands: Commands,
    mut events: MessageReader<SfxEvent>,
    sfx: Res<SfxAssets>,
    mut duck: ResMut<Duck>,
) {
    for event in events.read() {
        let Some((handle, secs)) = sfx.get(*event) else {
            continue;
        };
        commands.spawn((AudioPlayer(handle.clone()), PlaybackSettings::DESPAWN));
        if event.is_sting() {
            duck.remaining = duck.remaining.max(secs);
        }
    }
}

// --- Cues ---------------------------------------------------------------------

/// Where a cue's audio comes from.
#[derive(Clone, Copy, Debug)]
pub enum Source {
    /// Rendered in-process on first use (a `songs.rs` function).
    Baked(fn() -> AudioSource),
    /// An authored file under `assets/`, e.g. `"audio/music/menu.ogg"`.
    File(&'static str),
}

impl Source {
    fn key(&self) -> Option<usize> {
        match self {
            Source::Baked(f) => Some(*f as usize),
            Source::File(_) => None,
        }
    }
}

/// Music for one state (or one switch): a source, how it plays, its layers.
#[derive(Debug)]
pub struct Cue {
    pub source: Source,
    pub looping: bool,
    /// Seconds to full volume when the cue starts.
    pub fade_in: f32,
    /// Seconds to silence when the cue is replaced.
    pub fade_out: f32,
    pub volume: f32,
    /// Extra voices of the same length and tempo, started with the cue at
    /// zero volume; `LayerEvent::Start(i)` fades layer `i` in.
    pub layers: &'static [Source],
}

/// Layer fade time, both ways.
pub const LAYER_FADE: f32 = 0.8;

impl Cue {
    pub const fn loop_baked(f: fn() -> AudioSource) -> Self {
        Self {
            source: Source::Baked(f),
            looping: true,
            fade_in: 2.0,
            fade_out: 1.5,
            volume: 1.0,
            layers: &[],
        }
    }

    pub const fn sting_baked(f: fn() -> AudioSource) -> Self {
        Self {
            source: Source::Baked(f),
            looping: false,
            fade_in: 0.0,
            fade_out: 0.3,
            volume: 1.0,
            layers: &[],
        }
    }

    pub const fn loop_file(path: &'static str) -> Self {
        Self {
            source: Source::File(path),
            looping: true,
            fade_in: 2.0,
            fade_out: 1.5,
            volume: 1.0,
            layers: &[],
        }
    }

    pub const fn sting_file(path: &'static str) -> Self {
        Self {
            source: Source::File(path),
            looping: false,
            fade_in: 0.0,
            fade_out: 0.3,
            volume: 1.0,
            layers: &[],
        }
    }

    pub const fn with_layers(mut self, layers: &'static [Source]) -> Self {
        self.layers = layers;
        self
    }

    pub const fn at(mut self, volume: f32) -> Self {
        self.volume = volume;
        self
    }

    pub const fn fades(mut self, fade_in: f32, fade_out: f32) -> Self {
        self.fade_in = fade_in;
        self.fade_out = fade_out;
        self
    }
}

/// The house sting under the studio logo. Same in every game.
pub static HOUSE_CUE: Cue = Cue::sting_baked(house::house_sting);
pub static MENU_CUE: Cue =
    Cue::loop_baked(|| tracker::render_loop(&songs::MENU).expect("MENU is well-formed"));
pub static PLAY_CUE: Cue =
    Cue::loop_baked(|| tracker::render_loop(&songs::PLAY).expect("PLAY is well-formed"))
        .with_layers(&[Source::Baked(|| {
            tracker::render_loop(&songs::PLAY_LAYER).expect("PLAY_LAYER is well-formed")
        })]);
pub static GAME_OVER_CUE: Cue = Cue::sting_baked(songs::game_over_sting);
pub static DEMO_END_CUE: Cue = Cue::sting_baked(songs::demo_end_tag);

/// Per-state music. Every `GameState` has a row (a test checks); `None` is a
/// deliberate silence, written up in `docs/audio-identity.md`. `HowToPlay`
/// pointing at the menu cue means the menu loop continues uninterrupted.
pub const MUSIC: &[(GameState, Option<&Cue>)] = &[
    (GameState::StudioLogo, Some(&HOUSE_CUE)),
    (GameState::Menu, Some(&MENU_CUE)),
    (GameState::HowToPlay, Some(&MENU_CUE)),
    (GameState::Playing, Some(&PLAY_CUE)),
    (GameState::GameOver, Some(&GAME_OVER_CUE)),
    (GameState::DemoEnd, Some(&DEMO_END_CUE)),
];

/// Gameplay's handle on the music within a state.
#[derive(Message, Clone, Copy, Debug)]
pub enum CueEvent {
    /// Crossfade to this cue until the next state change (or `Resume`).
    Switch(&'static Cue),
    /// Back to the state table's cue.
    Resume,
}

/// Fade a layer of the current cue in or out (index into `Cue::layers`).
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayerEvent {
    Start(usize),
    Stop(usize),
}

/// Baked cues, rendered once and kept (keyed by the render function).
#[derive(Resource, Default)]
pub struct MusicCache(Vec<(usize, Handle<AudioSource>)>);

impl MusicCache {
    pub fn handle(
        &mut self,
        sources: &mut Assets<AudioSource>,
        asset_server: &AssetServer,
        source: &Source,
    ) -> Handle<AudioSource> {
        match source {
            Source::Baked(f) => {
                let key = *f as usize;
                if let Some((_, h)) = self.0.iter().find(|(k, _)| *k == key) {
                    return h.clone();
                }
                let handle = sources.add(f());
                self.0.push((key, handle.clone()));
                handle
            }
            Source::File(path) => asset_server.load(*path),
        }
    }

    /// Every baked source in `MUSIC` and its layers, in table order.
    fn baked_sources() -> impl Iterator<Item = &'static Source> {
        MUSIC
            .iter()
            .filter_map(|(_, cue)| *cue)
            .flat_map(|cue| std::iter::once(&cue.source).chain(cue.layers.iter()))
            .filter(|s| matches!(s, Source::Baked(_)))
    }

    /// Renders every baked source in `MUSIC` and its layers.
    pub fn prewarm(&mut self, sources: &mut Assets<AudioSource>, asset_server: &AssetServer) {
        for source in Self::baked_sources() {
            self.handle(sources, asset_server, source);
        }
    }

    /// Renders one not-yet-cached baked source; false when all are done.
    pub fn prewarm_step(
        &mut self,
        sources: &mut Assets<AudioSource>,
        asset_server: &AssetServer,
    ) -> bool {
        let next = Self::baked_sources().find(|s| {
            s.key()
                .is_some_and(|key| !self.0.iter().any(|(k, _)| *k == key))
        });
        match next {
            Some(source) => {
                self.handle(sources, asset_server, source);
                true
            }
            None => false,
        }
    }
}

/// Renders the baked cues one per frame while the title screen is up, so no
/// single frame pays for a 30 s loop and entering a state never waits.
pub fn prewarm_music(
    mut cache: ResMut<MusicCache>,
    mut sources: ResMut<Assets<AudioSource>>,
    asset_server: Res<AssetServer>,
) {
    cache.prewarm_step(&mut sources, &asset_server);
}

/// What the director is playing and whether a switch overrides the table.
#[derive(Resource, Default)]
pub struct CurrentCue {
    pub cue: Option<&'static Cue>,
    pub switched: Option<&'static Cue>,
}

/// Music ducks to half while a sting plays.
#[derive(Resource)]
pub struct Duck {
    pub remaining: f32,
    level: f32,
}

impl Default for Duck {
    fn default() -> Self {
        Self {
            remaining: 0.0,
            level: 1.0,
        }
    }
}

impl Duck {
    const DEPTH: f32 = 0.5;
    const RATE: f32 = 6.0;

    /// Advances the duck and returns the music gain to apply this frame.
    pub fn tick(&mut self, dt: f32) -> f32 {
        self.remaining = (self.remaining - dt).max(0.0);
        let target = if self.remaining > 0.0 {
            Self::DEPTH
        } else {
            1.0
        };
        let step = Self::RATE * dt;
        self.level = if self.level < target {
            (self.level + step).min(target)
        } else {
            (self.level - step).max(target)
        };
        self.level
    }
}

/// Marks the entities of the cue now playing (its track and its layers).
#[derive(Component)]
pub struct MusicTrack;

/// A playing music entity's volume automation.
#[derive(Component, Clone, Copy, Debug)]
pub struct MusicVoice {
    pub level: f32,
    pub target: f32,
    /// Level units per second while fading in (or a layer fading either way).
    pub rate: f32,
    /// Level units per second once the cue is replaced.
    pub out_rate: f32,
    pub base: f32,
    pub looping: bool,
    pub despawn_when_silent: bool,
    pub layer: Option<usize>,
}

impl MusicVoice {
    /// Moves `level` toward `target`; true when this voice should despawn.
    pub fn step(&mut self, dt: f32) -> bool {
        let step = self.rate * dt;
        self.level = if self.level < self.target {
            (self.level + step).min(self.target)
        } else {
            (self.level - step).max(self.target)
        };
        self.despawn_when_silent && self.level <= 0.0
    }

    /// Switches to the fade-out and marks the voice for despawn.
    pub fn release(&mut self) {
        self.target = 0.0;
        self.rate = self.out_rate;
        self.despawn_when_silent = true;
    }
}

fn rate_for(secs: f32) -> f32 {
    if secs <= 0.0 {
        f32::INFINITY
    } else {
        1.0 / secs
    }
}

/// Crossfades to the wanted cue on every state change or `CueEvent`.
pub fn music_director(
    mut commands: Commands,
    state: Res<State<GameState>>,
    mut current: ResMut<CurrentCue>,
    mut cue_events: MessageReader<CueEvent>,
    mut cache: ResMut<MusicCache>,
    mut sources: ResMut<Assets<AudioSource>>,
    asset_server: Res<AssetServer>,
    mut playing: Query<(Entity, &mut MusicVoice), With<MusicTrack>>,
) {
    let mut changed = false;
    if state.is_changed() {
        current.switched = None;
        changed = true;
    }
    for event in cue_events.read() {
        current.switched = match event {
            CueEvent::Switch(cue) => Some(*cue),
            CueEvent::Resume => None,
        };
        changed = true;
    }
    if !changed {
        return;
    }
    let table = MUSIC
        .iter()
        .find(|(s, _)| s == state.get())
        .and_then(|(_, cue)| *cue);
    let want = current.switched.or(table);
    let same = match (want, current.cue) {
        (Some(a), Some(b)) => std::ptr::eq(a, b),
        (None, None) => true,
        _ => false,
    };
    if same {
        return;
    }
    current.cue = want;

    for (entity, mut voice) in &mut playing {
        voice.release();
        commands.entity(entity).remove::<MusicTrack>();
    }

    let Some(cue) = want else {
        return;
    };
    let mode = if cue.looping {
        PlaybackMode::Loop
    } else {
        PlaybackMode::Once
    };
    let handle = cache.handle(&mut sources, &asset_server, &cue.source);
    commands.spawn((
        MusicTrack,
        AudioPlayer::new(handle),
        PlaybackSettings {
            mode,
            volume: Volume::Linear(0.0),
            ..default()
        },
        MusicVoice {
            level: if cue.fade_in <= 0.0 { 1.0 } else { 0.0 },
            target: 1.0,
            rate: rate_for(cue.fade_in),
            out_rate: rate_for(cue.fade_out),
            base: cue.volume,
            looping: cue.looping,
            despawn_when_silent: false,
            layer: None,
        },
    ));
    for (i, layer) in cue.layers.iter().enumerate() {
        let handle = cache.handle(&mut sources, &asset_server, layer);
        commands.spawn((
            MusicTrack,
            AudioPlayer::new(handle),
            PlaybackSettings {
                mode,
                volume: Volume::Linear(0.0),
                ..default()
            },
            MusicVoice {
                level: 0.0,
                target: 0.0,
                rate: rate_for(LAYER_FADE),
                out_rate: rate_for(cue.fade_out),
                base: cue.volume,
                looping: cue.looping,
                despawn_when_silent: false,
                layer: Some(i),
            },
        ));
    }
}

/// Fades the current cue's layers on `LayerEvent`.
pub fn layer_control(
    mut events: MessageReader<LayerEvent>,
    mut layers: Query<&mut MusicVoice, With<MusicTrack>>,
) {
    for event in events.read() {
        let (index, target) = match event {
            LayerEvent::Start(i) => (*i, 1.0),
            LayerEvent::Stop(i) => (*i, 0.0),
        };
        for mut voice in &mut layers {
            if voice.layer == Some(index) {
                voice.target = target;
            }
        }
    }
}

/// Applies every music voice's level, the duck and its base volume to its
/// sink; despawns faded-out voices and finished one-shots.
pub fn update_music_voices(
    mut commands: Commands,
    time: Res<Time>,
    mut duck: ResMut<Duck>,
    mut voices: Query<(Entity, &mut MusicVoice, &mut AudioSink)>,
) {
    let dt = time.delta_secs();
    let duck_gain = duck.tick(dt);
    for (entity, mut voice, mut sink) in &mut voices {
        let gone = voice.step(dt);
        sink.set_volume(Volume::Linear(voice.level * voice.base * duck_gain));
        if gone || (!voice.looping && sink.empty()) {
            commands.entity(entity).despawn();
        }
    }
}

// --- Export -------------------------------------------------------------------

/// Every baked cue, layer and sound effect, named for a WAV dump
/// (`tests/audio_export.rs`): the owner listens without running the game.
pub fn export_inventory() -> Vec<(String, AudioSource)> {
    let mut out: Vec<(String, AudioSource)> = Vec::new();
    let mut seen: Vec<usize> = Vec::new();
    let mut push_baked = |out: &mut Vec<(String, AudioSource)>, name: String, src: &Source| {
        if let (Some(key), Source::Baked(f)) = (src.key(), src)
            && !seen.contains(&key)
        {
            seen.push(key);
            out.push((name, f()));
        }
    };
    for (state, cue) in MUSIC {
        let Some(cue) = cue else {
            continue;
        };
        let state = format!("{state:?}").to_lowercase();
        push_baked(&mut out, format!("music/{state}"), &cue.source);
        for (i, layer) in cue.layers.iter().enumerate() {
            push_baked(&mut out, format!("music/{state}-layer-{i}"), layer);
        }
    }
    for event in SfxEvent::ALL {
        out.push((format!("sfx/{}", event.name()), bake(*event)));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn music_table_covers_every_state() {
        for s in [
            GameState::StudioLogo,
            GameState::Menu,
            GameState::HowToPlay,
            GameState::Playing,
            GameState::GameOver,
            GameState::DemoEnd,
        ] {
            assert!(MUSIC.iter().any(|(st, _)| *st == s), "{s:?} has no row");
        }
    }

    #[test]
    fn studio_logo_plays_the_house_sting() {
        let (_, cue) = MUSIC
            .iter()
            .find(|(s, _)| *s == GameState::StudioLogo)
            .unwrap();
        assert!(std::ptr::eq(cue.unwrap(), &HOUSE_CUE));
        assert!(!HOUSE_CUE.looping);
    }

    #[test]
    fn every_sfx_bakes_and_is_short() {
        for e in SfxEvent::ALL {
            let s = bake(*e);
            let secs = wav_secs(&s);
            assert!(secs > 0.02 && secs <= 6.0, "{e:?}: {secs}s");
        }
    }

    #[test]
    fn export_inventory_is_complete_and_unique() {
        let inv = export_inventory();
        let mut names: Vec<_> = inv.iter().map(|(n, _)| n.clone()).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), inv.len());
        assert!(names.iter().all(|n| n.contains('/')));
        // The menu cue is shared by Menu and HowToPlay: exported once.
        assert!(names.contains(&"music/menu".to_string()));
        assert!(!names.contains(&"music/howtoplay".to_string()));
        assert!(names.contains(&"music/playing-layer-0".to_string()));
        assert!(names.contains(&"music/studiologo".to_string()));
        for e in SfxEvent::ALL {
            assert!(names.contains(&format!("sfx/{}", e.name())));
        }
    }

    #[test]
    fn voice_level_moves_toward_target_and_despawns() {
        let mut v = MusicVoice {
            level: 1.0,
            target: 1.0,
            rate: 1.0,
            out_rate: 2.0,
            base: 1.0,
            looping: true,
            despawn_when_silent: false,
            layer: None,
        };
        assert!(!v.step(1.0));
        v.release();
        assert!(!v.step(0.25));
        assert!((v.level - 0.5).abs() < 1e-6);
        assert!(v.step(0.3));
    }

    #[test]
    fn instant_fade_in_starts_at_full_level() {
        assert!(rate_for(0.0).is_infinite());
        let mut v = MusicVoice {
            level: 0.0,
            target: 1.0,
            rate: rate_for(0.0),
            out_rate: 1.0,
            base: 1.0,
            looping: false,
            despawn_when_silent: false,
            layer: None,
        };
        v.step(0.001);
        assert_eq!(v.level, 1.0);
    }

    #[test]
    fn duck_dips_and_recovers() {
        let mut d = Duck::default();
        assert_eq!(d.tick(0.016), 1.0);
        d.remaining = 0.5;
        let dipped = d.tick(0.2);
        assert!((0.5..1.0).contains(&dipped));
        for _ in 0..30 {
            d.tick(0.1);
        }
        assert_eq!(d.tick(0.1), 1.0);
    }

    #[test]
    fn cache_renders_each_baked_source_once() {
        let mut sources = Assets::<AudioSource>::default();
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()));
        let asset_server = app.world().resource::<AssetServer>().clone();
        let mut cache = MusicCache::default();
        let a = cache.handle(&mut sources, &asset_server, &HOUSE_CUE.source);
        let b = cache.handle(&mut sources, &asset_server, &HOUSE_CUE.source);
        assert_eq!(a, b);
        assert_eq!(sources.len(), 1);
        cache.prewarm(&mut sources, &asset_server);
        // house, menu, play, play layer, game over, demo end
        assert_eq!(sources.len(), 6);
    }
}
