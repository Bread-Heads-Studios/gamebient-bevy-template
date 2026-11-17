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
//! * **Files.** Every cue and layer has a `name`; `content/audio/songs.json`
//!   may replace a named cue's song and `content/audio/sfx.json` an event's
//!   program ([`crate::game::content::audio`]). The Rust songs and programs
//!   here stay the defaults. Overrides are presentation only: no sim code
//!   reads them. [`preview`] plays any event or cue by name (the editor's
//!   `sound.preview`).
//!
//! Port checklist: replace the three template events and their `bake` arms
//! with the game's own. `bake` is the per-game hook and may return any
//! `AudioSource` (a chip program, a `synth.rs` voice). Where an event *is*
//! a chip program, also return it from [`program`] (the editor shows and
//! tests it); return `None` for one that is not. A `sfx.json` program
//! replaces either kind at runtime ([`bake_with`]). Then compose `songs.rs`, point `MUSIC` at it (every
//! cue and layer named, unique, snake_case), and write
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

use crate::game::content::audio::{SfxOverrides, SongOverrides};
use crate::game::states::GameState;
use chip::{NoiseMode, NoiseRate, note_hz};
use tracker::{Fx, Song, wav_secs};

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

    /// Lower-case variant name, used for export file names, the keys of
    /// `content/audio/sfx.json` and `sound.preview`.
    pub fn name(self) -> String {
        format!("{self:?}").to_lowercase()
    }

    /// The event called `name` (see [`SfxEvent::name`]).
    pub fn from_name(name: &str) -> Option<SfxEvent> {
        Self::ALL.iter().copied().find(|e| e.name() == name)
    }
}

fn hz(name: &str) -> f32 {
    note_hz(name).expect("note name")
}

/// One event's built-in chip program, when its sound is one; `None` for an
/// event whose `bake` arm is not a chip program (a `synth.rs` voice). The
/// template speaks C major: confirm is a rising fifth, pause a falling one,
/// the screen sweep a rising glide with a noise shimmer. A game replaces
/// all three in its own key.
pub fn program(event: SfxEvent) -> Option<Fx> {
    Some(match event {
        SfxEvent::Confirm => Fx::new()
            .hit(0, hz("c5"), 5)
            .then(Fx::new().hit(0, hz("g5"), 9)),
        SfxEvent::Pause => Fx::new()
            .hit(0, hz("g4"), 5)
            .then(Fx::new().hit(0, hz("c4"), 9)),
        SfxEvent::ScreenSweep => Fx::new()
            .tone(0, hz("c4"), hz("c6"), 2, 15, 18)
            .with(Fx::new().noise(NoiseMode::White, NoiseRate::Div512, 6, 15, 12)),
    })
}

/// Renders one event's built-in sound: the per-game hook. The template's
/// are all chip programs, so it renders [`program`]; a game may return any
/// `AudioSource` here.
pub fn bake(event: SfxEvent) -> AudioSource {
    match event {
        SfxEvent::Confirm | SfxEvent::Pause | SfxEvent::ScreenSweep => program(event)
            .expect("the template's events are chip programs")
            .render(),
    }
}

/// Renders one event: its `sfx.json` program when there is one, else
/// [`bake`].
pub fn bake_with(event: SfxEvent, overrides: Option<&SfxOverrides>) -> AudioSource {
    let name = event.name();
    match overrides.and_then(|o| o.0.iter().find(|(n, _)| *n == name)) {
        Some((_, fx)) => fx.render(),
        None => bake(event),
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
    pub fn get(&self, event: SfxEvent) -> Option<(&Handle<AudioSource>, f32)> {
        self.clips
            .iter()
            .find(|(e, _, _)| *e == event)
            .map(|(_, h, secs)| (h, *secs))
    }

    /// Re-renders `event` (override or built-in) so its next play is new.
    pub fn rebake(
        &mut self,
        sources: &mut Assets<AudioSource>,
        event: SfxEvent,
        overrides: Option<&SfxOverrides>,
    ) {
        let source = bake_with(event, overrides);
        let secs = wav_secs(&source);
        let handle = sources.add(source);
        match self.clips.iter_mut().find(|(e, _, _)| *e == event) {
            Some(clip) => *clip = (event, handle, secs),
            None => self.clips.push((event, handle, secs)),
        }
    }
}

/// `Startup`, after `content::audio::load_embedded_audio`: bakes every
/// event, from `sfx.json` where it has one.
pub fn setup_sfx(
    mut commands: Commands,
    mut sources: ResMut<Assets<AudioSource>>,
    overrides: Option<Res<SfxOverrides>>,
) {
    let mut assets = SfxAssets::default();
    for event in SfxEvent::ALL {
        assets.rebake(&mut sources, *event, overrides.as_deref());
    }
    commands.insert_resource(assets);
}

/// Spawns a one-shot player per received event; stings start a duck.
pub fn play_sfx(
    mut commands: Commands,
    mut events: MessageReader<SfxEvent>,
    sfx: Res<SfxAssets>,
    mut duck: ResMut<Duck>,
    look: Option<Res<crate::look::LookAssets>>,
) {
    for event in events.read() {
        let Some((handle, secs)) = sfx.get(*event) else {
            continue;
        };
        let handle = look
            .as_ref()
            .and_then(|l| l.audio(&format!("sfx.{}", event.name())))
            .unwrap_or_else(|| handle.clone());
        commands.spawn((AudioPlayer(handle), PlaybackSettings::DESPAWN));
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

/// An extra voice of a cue (see [`Cue::layers`]), named like a cue so
/// `songs.json` can replace it.
#[derive(Debug)]
pub struct Layer {
    pub name: &'static str,
    pub source: Source,
}

impl Layer {
    pub const fn baked(name: &'static str, f: fn() -> AudioSource) -> Self {
        Self {
            name,
            source: Source::Baked(f),
        }
    }

    pub const fn file(name: &'static str, path: &'static str) -> Self {
        Self {
            name,
            source: Source::File(path),
        }
    }
}

/// Music for one state (or one switch): a source, how it plays, its layers.
#[derive(Debug)]
pub struct Cue {
    /// The cue's name: its key in `content/audio/songs.json` and for
    /// `sound.preview`. Unique across `MUSIC` and its layers (a test checks).
    pub name: &'static str,
    pub source: Source,
    pub looping: bool,
    /// Seconds to full volume when the cue starts.
    pub fade_in: f32,
    /// Seconds to silence when the cue is replaced.
    pub fade_out: f32,
    pub volume: f32,
    /// Extra voices of the same length and tempo, started with the cue at
    /// zero volume; `LayerEvent::Start(i)` fades layer `i` in.
    pub layers: &'static [Layer],
}

/// Layer fade time, both ways.
pub const LAYER_FADE: f32 = 0.8;

impl Cue {
    pub const fn loop_baked(name: &'static str, f: fn() -> AudioSource) -> Self {
        Self {
            name,
            source: Source::Baked(f),
            looping: true,
            fade_in: 2.0,
            fade_out: 1.5,
            volume: 1.0,
            layers: &[],
        }
    }

    pub const fn sting_baked(name: &'static str, f: fn() -> AudioSource) -> Self {
        Self {
            name,
            source: Source::Baked(f),
            looping: false,
            fade_in: 0.0,
            fade_out: 0.3,
            volume: 1.0,
            layers: &[],
        }
    }

    pub const fn loop_file(name: &'static str, path: &'static str) -> Self {
        Self {
            name,
            source: Source::File(path),
            looping: true,
            fade_in: 2.0,
            fade_out: 1.5,
            volume: 1.0,
            layers: &[],
        }
    }

    pub const fn sting_file(name: &'static str, path: &'static str) -> Self {
        Self {
            name,
            source: Source::File(path),
            looping: false,
            fade_in: 0.0,
            fade_out: 0.3,
            volume: 1.0,
            layers: &[],
        }
    }

    pub const fn with_layers(mut self, layers: &'static [Layer]) -> Self {
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
/// Its name is `"house"`; `songs.json` cannot replace it.
pub static HOUSE_CUE: Cue = Cue::sting_baked(HOUSE, house::house_sting);
pub static MENU_CUE: Cue = Cue::loop_baked("menu", || {
    tracker::render_loop(&songs::MENU).expect("MENU is well-formed")
});
pub static PLAY_CUE: Cue = Cue::loop_baked("play", || {
    tracker::render_loop(&songs::PLAY).expect("PLAY is well-formed")
})
.with_layers(&[Layer::baked("play_layer", || {
    tracker::render_loop(&songs::PLAY_LAYER).expect("PLAY_LAYER is well-formed")
})]);
pub static GAME_OVER_CUE: Cue = Cue::sting_baked("game_over", songs::game_over_sting);
pub static DEMO_END_CUE: Cue = Cue::sting_baked("demo_end", songs::demo_end_tag);

/// The house sting's cue name. The one sound a game's files cannot replace.
pub const HOUSE: &str = "house";

/// Release tail of a sting whose song comes from `songs.json`.
pub const OVERRIDE_STING_TAIL: f32 = 1.5;

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

/// One playable part of a cue: the cue itself or one of its layers.
#[derive(Clone, Copy, Debug)]
pub struct Part<'a> {
    pub name: &'a str,
    pub source: &'a Source,
    pub looping: bool,
}

impl Cue {
    /// The cue's own part.
    pub fn part(&self) -> Part<'_> {
        Part {
            name: self.name,
            source: &self.source,
            looping: self.looping,
        }
    }

    /// Layer `i` as a part (it plays the way its cue does).
    pub fn layer_part(&self, i: usize) -> Option<Part<'_>> {
        self.layers.get(i).map(|l| Part {
            name: l.name,
            source: &l.source,
            looping: self.looping,
        })
    }

    /// The cue's part and its layers', in order.
    pub fn parts(&self) -> impl Iterator<Item = Part<'_>> {
        std::iter::once(self.part())
            .chain((0..self.layers.len()).filter_map(|i| self.layer_part(i)))
    }
}

/// Every distinct cue in `MUSIC`, in table order.
pub fn music_cues() -> Vec<&'static Cue> {
    let mut out: Vec<&'static Cue> = Vec::new();
    for cue in MUSIC.iter().filter_map(|(_, c)| *c) {
        if !out.iter().any(|c| std::ptr::eq(*c, cue)) {
            out.push(cue);
        }
    }
    out
}

/// Every cue and layer name in `MUSIC`, in table order, once each.
pub fn cue_names() -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    for cue in music_cues() {
        for name in std::iter::once(cue.name).chain(cue.layers.iter().map(|l| l.name)) {
            if !out.contains(&name) {
                out.push(name);
            }
        }
    }
    out
}

/// The part called `name` (a cue or a layer in `MUSIC`).
pub fn find_part(name: &str) -> Option<Part<'static>> {
    music_cues()
        .into_iter()
        .flat_map(|c| c.parts())
        .find(|p| p.name == name)
}

/// Every name `sound.preview` takes: the events, then the cues and layers.
pub fn sound_names() -> Vec<String> {
    SfxEvent::ALL
        .iter()
        .map(|e| e.name())
        .chain(cue_names().into_iter().map(str::to_string))
        .collect()
}

#[derive(Clone, Debug, PartialEq)]
enum CacheKey {
    /// A built-in baked source, by its render function.
    Baked(usize),
    /// A `songs.json` song, by cue (or layer) name.
    Song(String),
}

/// Rendered music, kept: built-in baked sources by render function and
/// `songs.json` songs by name. Overrides win over the built-in source.
#[derive(Resource, Default)]
pub struct MusicCache(Vec<(CacheKey, Handle<AudioSource>)>);

fn override_for<'a>(overrides: Option<&'a SongOverrides>, name: &str) -> Option<&'a Song> {
    overrides.and_then(|o| o.0.iter().find(|(n, _)| n == name).map(|(_, s)| s))
}

impl MusicCache {
    fn key_for(part: &Part, overrides: Option<&SongOverrides>) -> Option<CacheKey> {
        if override_for(overrides, part.name).is_some() {
            return Some(CacheKey::Song(part.name.to_string()));
        }
        part.source.key().map(CacheKey::Baked)
    }

    fn cached(&self, key: &CacheKey) -> Option<&Handle<AudioSource>> {
        self.0.iter().find(|(k, _)| k == key).map(|(_, h)| h)
    }

    /// The part's audio: its `songs.json` song when it has one, else its
    /// source. Rendered on first use and kept.
    pub fn handle(
        &mut self,
        sources: &mut Assets<AudioSource>,
        asset_server: &AssetServer,
        part: Part,
        overrides: Option<&SongOverrides>,
    ) -> Handle<AudioSource> {
        if let Some(song) = override_for(overrides, part.name) {
            let key = CacheKey::Song(part.name.to_string());
            if let Some(h) = self.cached(&key) {
                return h.clone();
            }
            let rendered = if part.looping {
                tracker::render_loop(song)
            } else {
                tracker::render_sting(song, OVERRIDE_STING_TAIL)
            };
            match rendered {
                Ok(source) => {
                    let handle = sources.add(source);
                    self.0.push((key, handle.clone()));
                    return handle;
                }
                // `set_songs` checked it; never expected.
                Err(e) => error!("song \"{}\": {e:?}", part.name),
            }
        }
        match part.source {
            Source::Baked(f) => {
                let key = CacheKey::Baked(*f as usize);
                if let Some(h) = self.cached(&key) {
                    return h.clone();
                }
                let handle = sources.add(f());
                self.0.push((key, handle.clone()));
                handle
            }
            Source::File(path) => asset_server.load(*path),
        }
    }

    /// Drops the rendered `songs.json` song for `name`, so the next play
    /// renders the new one (or the built-in source).
    pub fn forget_song(&mut self, name: &str) {
        self.0
            .retain(|(k, _)| !matches!(k, CacheKey::Song(n) if n == name));
    }

    /// Every part in `MUSIC` that renders in-process, in table order.
    fn renderable_parts(overrides: Option<&SongOverrides>) -> Vec<(Part<'static>, CacheKey)> {
        music_cues()
            .into_iter()
            .flat_map(|c| c.parts())
            .filter_map(|p| Self::key_for(&p, overrides).map(|k| (p, k)))
            .collect()
    }

    /// Renders every baked part in `MUSIC` and its layers.
    pub fn prewarm(
        &mut self,
        sources: &mut Assets<AudioSource>,
        asset_server: &AssetServer,
        overrides: Option<&SongOverrides>,
    ) {
        for (part, _) in Self::renderable_parts(overrides) {
            self.handle(sources, asset_server, part, overrides);
        }
    }

    /// Renders one not-yet-cached part; false when all are done.
    pub fn prewarm_step(
        &mut self,
        sources: &mut Assets<AudioSource>,
        asset_server: &AssetServer,
        overrides: Option<&SongOverrides>,
    ) -> bool {
        let next = Self::renderable_parts(overrides)
            .into_iter()
            .find(|(_, key)| self.cached(key).is_none());
        match next {
            Some((part, _)) => {
                self.handle(sources, asset_server, part, overrides);
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
    overrides: Option<Res<SongOverrides>>,
) {
    cache.prewarm_step(&mut sources, &asset_server, overrides.as_deref());
}

/// What the director is playing and whether a switch overrides the table.
#[derive(Resource, Default)]
pub struct CurrentCue {
    pub cue: Option<&'static Cue>,
    pub switched: Option<&'static Cue>,
    /// Set when the current cue's song changed (`set_songs`): the director
    /// starts it again from the new audio.
    pub restart: bool,
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

/// The Look Pack cue name for a state's music: `music.<state, lower case>`.
pub fn music_key(state: &GameState) -> String {
    format!("music.{}", format!("{state:?}").to_lowercase())
}

/// The baked layers to spawn for a cue: none when a pack replaced its track.
pub fn layers_for(cue: &Cue, pack_override: bool) -> &'static [Layer] {
    if pack_override { &[] } else { cue.layers }
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
    overrides: Option<Res<SongOverrides>>,
    look: Option<Res<crate::look::LookAssets>>,
) {
    let restart = std::mem::take(&mut current.restart);
    let mut changed = restart;
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
    if same && !restart {
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
    let overrides = overrides.as_deref();
    // A pack may replace a state's own music (not a mid-state switch). It
    // wins over a `songs.json` song: the player picked the pack.
    let pack_track = current
        .switched
        .is_none()
        .then(|| look.as_ref().and_then(|l| l.audio(&music_key(state.get()))))
        .flatten();
    let pack_track_used = pack_track.is_some();
    let handle = pack_track
        .unwrap_or_else(|| cache.handle(&mut sources, &asset_server, cue.part(), overrides));
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
    // A pack's track replaces the whole cue: the baked layers are in another
    // key and tempo. A `LayerEvent` then finds no voice, which is harmless.
    for i in 0..layers_for(cue, pack_track_used).len() {
        let Some(part) = cue.layer_part(i) else {
            continue;
        };
        let handle = cache.handle(&mut sources, &asset_server, part, overrides);
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

// --- Preview ------------------------------------------------------------------

/// A cue or layer playing for `sound.preview`; the next preview replaces it.
#[derive(Component)]
pub struct PreviewVoice;

/// True in a build that plays sound (the windowed game). The headless sim
/// and the verifier have none of these, and no song or sound override.
pub fn audio_present(world: &World) -> bool {
    world.contains_resource::<Assets<AudioSource>>()
        && world.contains_resource::<AssetServer>()
        && world.contains_resource::<MusicCache>()
}

/// Plays the event or cue called `name` once. An event goes through the
/// bus like gameplay's (a sting ducks the music); a cue or layer plays its
/// current audio (its `songs.json` song when it has one) once, over the
/// music, replacing the last preview. False for an unknown name or in a
/// build without sound.
pub fn preview(world: &mut World, name: &str) -> bool {
    if !audio_present(world) {
        return false;
    }
    if let Some(event) = SfxEvent::from_name(name) {
        world.write_message(event);
        return true;
    }
    let Some(part) = find_part(name) else {
        return false;
    };
    let handle = world.resource_scope(|world, mut cache: Mut<MusicCache>| {
        world.resource_scope(|world, mut sources: Mut<Assets<AudioSource>>| {
            let server = world.resource::<AssetServer>();
            let overrides = world.get_resource::<SongOverrides>();
            cache.handle(&mut sources, server, part, overrides)
        })
    });
    let old: Vec<Entity> = world
        .query_filtered::<Entity, With<PreviewVoice>>()
        .iter(world)
        .collect();
    for e in old {
        world.despawn(e);
    }
    world.spawn((
        PreviewVoice,
        AudioPlayer::new(handle),
        PlaybackSettings::DESPAWN,
    ));
    true
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
            push_baked(&mut out, format!("music/{state}-layer-{i}"), &layer.source);
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
    fn a_pack_track_drops_the_baked_layers() {
        assert!(layers_for(&PLAY_CUE, true).is_empty());
        assert_eq!(layers_for(&PLAY_CUE, false).len(), PLAY_CUE.layers.len());
        assert!(!PLAY_CUE.layers.is_empty());
    }

    #[test]
    fn music_keys_name_the_state_in_lower_case() {
        assert_eq!(music_key(&GameState::GameOver), "music.gameover");
        assert_eq!(music_key(&GameState::Menu), "music.menu");
    }

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
        let a = cache.handle(&mut sources, &asset_server, HOUSE_CUE.part(), None);
        let b = cache.handle(&mut sources, &asset_server, HOUSE_CUE.part(), None);
        assert_eq!(a, b);
        assert_eq!(sources.len(), 1);
        cache.prewarm(&mut sources, &asset_server, None);
        // house, menu, play, play layer, game over, demo end
        assert_eq!(sources.len(), 6);
        assert!(!cache.prewarm_step(&mut sources, &asset_server, None));
    }

    #[test]
    fn every_cue_and_layer_has_a_unique_name() {
        let mut seen: Vec<(&str, *const Source)> = Vec::new();
        for cue in MUSIC.iter().filter_map(|(_, c)| *c) {
            for part in cue.parts() {
                assert!(!part.name.is_empty());
                match seen.iter().find(|(n, _)| *n == part.name) {
                    // The same cue in two rows (menu, how-to-play) is fine.
                    Some((_, src)) => assert!(
                        std::ptr::eq(*src, part.source),
                        "two parts are called {}",
                        part.name
                    ),
                    None => seen.push((part.name, part.source)),
                }
            }
        }
        assert_eq!(
            cue_names(),
            [
                "house",
                "menu",
                "play",
                "play_layer",
                "game_over",
                "demo_end"
            ]
        );
        for e in SfxEvent::ALL {
            assert!(!cue_names().contains(&e.name().as_str()), "{e:?}");
            assert_eq!(SfxEvent::from_name(&e.name()), Some(*e));
        }
        let names = sound_names();
        let mut sorted = names.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), names.len());
        assert_eq!(names.len(), SfxEvent::ALL.len() + 6);
        assert_eq!(find_part("play_layer").map(|p| p.looping), Some(true));
        assert_eq!(find_part("game_over").map(|p| p.looping), Some(false));
        assert!(find_part("nope").is_none());
    }

    #[test]
    fn bake_renders_the_program() {
        for e in SfxEvent::ALL {
            // Every template event is a chip program; a game's may be None.
            let fx = program(*e).expect("template events are chip programs");
            assert!(bake(*e).bytes == fx.render().bytes);
            assert!(bake_with(*e, None).bytes == bake(*e).bytes);
        }
    }
}
