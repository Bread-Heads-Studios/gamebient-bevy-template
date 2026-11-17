//! Songs and sound effects as files. `content/audio/songs.json` replaces the
//! song of a named cue or layer (`audio::Cue::name`), `content/audio/sfx.json`
//! the chip program of a named event (`SfxEvent::name`). A name the file
//! leaves out keeps its Rust default (`songs.rs`, `audio::bake`); the
//! template ships neither file.
//!
//! Presentation only: the overrides live in [`SongOverrides`] and
//! [`SfxOverrides`], which exist only in a build that plays sound, are never
//! read by sim code and never enter the content hash. In the headless sim
//! and the verifier [`set_songs`] and [`set_sfx`] change nothing and return
//! an error.

use bevy::prelude::*;

use super::files;
use super::model::{ContentError, FxOp, InstrumentData, NoteOrHz, SfxFile, SongData, SongsFile};
use super::saved::ContentWarnings;
use crate::game::audio::chip::{NoiseMode, NoiseRate, note_hz};
use crate::game::audio::tracker::{self, Fx, Instrument, Song, SongError};
use crate::game::audio::{self, CurrentCue, MusicCache, SfxAssets, SfxEvent};

/// Songs from `songs.json`, by cue or layer name, in file order.
#[derive(Resource, Default)]
pub struct SongOverrides(pub Vec<(String, Song)>);

/// Sound effect programs from `sfx.json`, by event name, in file order.
#[derive(Resource, Default)]
pub struct SfxOverrides(pub Vec<(String, Fx)>);

fn no_sound(name: &str) -> ContentError {
    ContentError(format!("There is no sound called \"{name}\"."))
}

fn no_audio() -> ContentError {
    ContentError("This build plays no sound.".to_string())
}

/// `white` / `periodic`.
pub fn noise_mode(name: &str) -> Option<NoiseMode> {
    match name {
        "white" => Some(NoiseMode::White),
        "periodic" => Some(NoiseMode::Periodic),
        _ => None,
    }
}

/// `div512` / `div1024` / `div2048` / `tone3`.
pub fn noise_rate(name: &str) -> Option<NoiseRate> {
    match name {
        "div512" => Some(NoiseRate::Div512),
        "div1024" => Some(NoiseRate::Div1024),
        "div2048" => Some(NoiseRate::Div2048),
        "tone3" => Some(NoiseRate::Tone3),
        _ => None,
    }
}

fn mode_name(m: NoiseMode) -> &'static str {
    match m {
        NoiseMode::White => "white",
        NoiseMode::Periodic => "periodic",
    }
}

fn rate_name(r: NoiseRate) -> &'static str {
    match r {
        NoiseRate::Div512 => "div512",
        NoiseRate::Div1024 => "div1024",
        NoiseRate::Div2048 => "div2048",
        NoiseRate::Tone3 => "tone3",
    }
}

fn bad_noise() -> ContentError {
    ContentError(
        "A noise must be white or periodic, at div512, div1024, div2048 or tone3.".to_string(),
    )
}

fn leak_str(s: &str) -> &'static str {
    Box::leak(s.to_owned().into_boxed_str())
}

fn leak<T>(v: Vec<T>) -> &'static [T] {
    Box::leak(v.into_boxed_slice())
}

fn instrument_from(d: &InstrumentData) -> Result<Instrument, ContentError> {
    let noise = match &d.noise {
        None => None,
        Some((m, r)) => Some((
            noise_mode(m).ok_or_else(bad_noise)?,
            noise_rate(r).ok_or_else(bad_noise)?,
        )),
    };
    Ok(Instrument {
        env: leak(d.env.clone()),
        release: d.release,
        arp: leak(d.arp.clone()),
        vibrato: d.vibrato.map(|[c, hz]| (c, hz)),
        noise,
    })
}

fn instrument_data(i: &Instrument) -> InstrumentData {
    InstrumentData {
        env: i.env.to_vec(),
        release: i.release,
        arp: i.arp.to_vec(),
        vibrato: i.vibrato.map(|(c, hz)| [c, hz]),
        noise: i
            .noise
            .map(|(m, r)| (mode_name(m).to_string(), rate_name(r).to_string())),
    }
}

fn voice_words(voice: &str) -> &'static str {
    match voice {
        "tone1" => "voice 1",
        "tone2" => "voice 2",
        "tone3" => "voice 3",
        _ => "the noise part",
    }
}

/// The tracker's own complaint, as one sentence about `what`.
fn song_error(what: &str, e: SongError) -> ContentError {
    ContentError(match e {
        SongError::Parse { voice, token } => format!(
            "{what} has \"{token}\" in {}, which is not a note, a rest or a setting.",
            voice_words(voice)
        ),
        SongError::Length { voice, got, want } => format!(
            "{what} has {got} steps in {} but its length is {want} steps.",
            voice_words(voice)
        ),
        SongError::Instrument { voice, index } => format!(
            "{what} uses instrument {index} in {}, which it does not have.",
            voice_words(voice)
        ),
        SongError::Pitch { voice, token } => format!(
            "{what} has \"{token}\" in {}, which is too low for the chip.",
            voice_words(voice)
        ),
    })
}

fn song_named(what: &str, d: &SongData) -> Result<Song, ContentError> {
    let instruments = d
        .instruments
        .iter()
        .map(instrument_from)
        .collect::<Result<Vec<_>, _>>()?;
    let noise_instruments = d
        .noise_instruments
        .iter()
        .map(instrument_from)
        .collect::<Result<Vec<_>, _>>()?;
    let song = Song {
        bpm: d.bpm,
        beats_per_bar: d.beats_per_bar,
        steps_per_beat: d.steps_per_beat,
        bars: d.bars,
        voices: [
            leak_str(&d.voices[0]),
            leak_str(&d.voices[1]),
            leak_str(&d.voices[2]),
        ],
        noise: leak_str(&d.noise),
        instruments: leak(instruments),
        noise_instruments: leak(noise_instruments),
    };
    // The tracker checks every token, length, instrument and pitch while it
    // lays the song out. At this tempo the song is zero samples long, so
    // the check renders nothing.
    let check = Song { bpm: 1.0e9, ..song };
    tracker::render_song_samples(&check, 0.0).map_err(|e| song_error(what, e))?;
    Ok(song)
}

/// A file's song as a tracker [`Song`], checked by the tracker itself.
///
/// `Song` holds `&'static` data, so the converted strings and instrument
/// tables are leaked (`Box::leak`). That is bounded: one leak per loaded
/// file at startup, and in the editor build one per live edit, which is
/// acceptable for the length of an editing session.
pub fn song_from_data(d: &SongData) -> Result<Song, ContentError> {
    song_named("The song", d)
}

/// The file form of a song (the inverse of [`song_from_data`]).
pub fn data_from_song(s: &Song) -> SongData {
    SongData {
        bpm: s.bpm,
        beats_per_bar: s.beats_per_bar,
        steps_per_beat: s.steps_per_beat,
        bars: s.bars,
        voices: s.voices.map(str::to_string),
        noise: s.noise.to_string(),
        instruments: s.instruments.iter().map(instrument_data).collect(),
        noise_instruments: s.noise_instruments.iter().map(instrument_data).collect(),
    }
}

fn pitch(n: &NoteOrHz) -> Result<f32, ContentError> {
    match n {
        NoteOrHz::Hz(h) if h.is_finite() && *h > 0.0 => Ok(*h),
        NoteOrHz::Hz(h) => Err(ContentError(format!("{h} is not a pitch."))),
        NoteOrHz::Note(s) => note_hz(s).ok_or_else(|| {
            ContentError(format!("\"{s}\" is not a note name like c4, f#3 or bb2."))
        }),
    }
}

fn voice(v: u8) -> Result<usize, ContentError> {
    if v <= 2 {
        Ok(usize::from(v))
    } else {
        Err(ContentError(format!(
            "There is no tone voice {v}; use 0, 1 or 2."
        )))
    }
}

fn apply_op(fx: Fx, op: &FxOp) -> Result<Fx, ContentError> {
    Ok(match op {
        FxOp::Tone {
            voice: v,
            from,
            to,
            a0,
            a1,
            frames,
        } => fx.tone(voice(*v)?, pitch(from)?, pitch(to)?, *a0, *a1, *frames),
        FxOp::Hit {
            voice: v,
            note,
            frames,
        } => fx.hit(voice(*v)?, pitch(note)?, *frames),
        FxOp::Slide {
            voice: v,
            from,
            to,
            frames,
        } => fx.slide(voice(*v)?, pitch(from)?, pitch(to)?, *frames),
        FxOp::Noise {
            mode,
            rate,
            a0,
            a1,
            frames,
        } => fx.noise(
            noise_mode(mode).ok_or_else(bad_noise)?,
            noise_rate(rate).ok_or_else(bad_noise)?,
            *a0,
            *a1,
            *frames,
        ),
        FxOp::Rest { frames } => fx.rest(*frames),
    })
}

/// A file's sound effect program as a chip [`Fx`]. Parts play one after
/// another (`Fx::then`); within a part each sound is added to its voice the
/// way the `Fx` builder methods add it, so sounds on different voices play
/// together and a `rest` waits for everything before it.
pub fn fx_from_program(p: &[Vec<FxOp>]) -> Result<Fx, ContentError> {
    let mut out: Option<Fx> = None;
    for part in p {
        let mut fx = Fx::new();
        for op in part {
            fx = apply_op(fx, op)?;
        }
        out = Some(match out {
            None => fx,
            Some(before) => before.then(fx),
        });
    }
    Ok(out.unwrap_or_default())
}

/// Replaces `songs.json`: every song must name a cue or layer in `MUSIC`
/// (not the house sting) and pass the tracker, or nothing changes. Drops the
/// rendered audio of every name whose song changed, so the next play uses
/// the new one, and restarts the cue now playing if its audio changed.
pub fn set_songs(world: &mut World, bytes: &[u8]) -> Result<(), ContentError> {
    if !audio::audio_present(world) {
        return Err(no_audio());
    }
    let file = SongsFile::parse(bytes)?;
    let names = audio::cue_names();
    let mut songs: Vec<(String, Song)> = Vec::new();
    for (name, data) in &file.songs {
        if name == audio::HOUSE {
            return Err(ContentError(
                "The house sting is the same in every game, so a songs file cannot replace it."
                    .to_string(),
            ));
        }
        if !names.contains(&name.as_str()) {
            return Err(no_sound(name));
        }
        songs.push((
            name.clone(),
            song_named(&format!("The song \"{name}\""), data)?,
        ));
    }
    let old = world.remove_resource::<SongOverrides>().unwrap_or_default();
    let debug_of = |list: &[(String, Song)], n: &str| {
        list.iter()
            .find(|(k, _)| k == n)
            .map(|(_, s)| format!("{s:?}"))
    };
    let changed: Vec<&str> = names
        .iter()
        .copied()
        .filter(|n| debug_of(&old.0, n) != debug_of(&songs, n))
        .collect();
    world.insert_resource(SongOverrides(songs));
    let mut cache = world.resource_mut::<MusicCache>();
    for n in &changed {
        cache.forget_song(n);
    }
    if let Some(mut current) = world.get_resource_mut::<CurrentCue>()
        && let Some(cue) = current.cue
        && cue.parts().any(|p| changed.contains(&p.name))
    {
        current.restart = true;
    }
    Ok(())
}

/// Replaces `sfx.json`: every key must be an event name (`SfxEvent::name`)
/// and every program must build, or nothing changes. Re-renders every event
/// whose program changed, so its next play is the new sound.
pub fn set_sfx(world: &mut World, bytes: &[u8]) -> Result<(), ContentError> {
    if !audio::audio_present(world) {
        return Err(no_audio());
    }
    let file = SfxFile::parse(bytes)?;
    let mut programs: Vec<(String, Fx)> = Vec::new();
    for (name, program) in &file.sfx {
        if SfxEvent::from_name(name).is_none() {
            return Err(no_sound(name));
        }
        let fx = fx_from_program(program)
            .map_err(|e| ContentError(format!("The sound \"{name}\" could not be built: {e}")))?;
        programs.push((name.clone(), fx));
    }
    let old = world.remove_resource::<SfxOverrides>().unwrap_or_default();
    let find =
        |list: &[(String, Fx)], n: &str| list.iter().find(|(k, _)| k == n).map(|(_, f)| f.clone());
    let changed: Vec<SfxEvent> = SfxEvent::ALL
        .iter()
        .copied()
        .filter(|e| find(&old.0, &e.name()) != find(&programs, &e.name()))
        .collect();
    world.insert_resource(SfxOverrides(programs));
    if world.contains_resource::<SfxAssets>() {
        world.resource_scope(|world, mut assets: Mut<SfxAssets>| {
            world.resource_scope(|world, mut sources: Mut<Assets<AudioSource>>| {
                let overrides = world.get_resource::<SfxOverrides>();
                for e in &changed {
                    assets.rebake(&mut sources, *e, overrides);
                }
            });
        });
    }
    Ok(())
}

/// `Startup`, before `audio::setup_sfx`: loads the embedded `audio/songs.json`
/// and `audio/sfx.json` when the build plays sound. A file that does not
/// load is a content warning and the Rust defaults play.
pub fn load_embedded_audio(world: &mut World) {
    load_audio_from(
        world,
        files::embedded("audio/songs.json"),
        files::embedded("audio/sfx.json"),
    );
}

/// [`load_embedded_audio`] with the files' bytes injectable.
pub fn load_audio_from(world: &mut World, songs: Option<&[u8]>, sfx: Option<&[u8]>) {
    if !audio::audio_present(world) {
        return;
    }
    world.init_resource::<SongOverrides>();
    world.init_resource::<SfxOverrides>();
    type Setter = fn(&mut World, &[u8]) -> Result<(), ContentError>;
    let loads: [(&str, Option<&[u8]>, Setter); 2] = [
        ("audio/songs.json", songs, set_songs),
        ("audio/sfx.json", sfx, set_sfx),
    ];
    for (path, bytes, set) in loads {
        if let Some(bytes) = bytes
            && let Err(e) = set(world, bytes)
        {
            let msg = format!("content/{path}: {e}");
            warn!("{msg}");
            world.init_resource::<ContentWarnings>();
            world.resource_mut::<ContentWarnings>().0.push(msg);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::audio::songs;
    use crate::game::audio::{MENU_CUE, PLAY_CUE, PreviewVoice, preview};

    fn round_trip(s: &Song) -> Song {
        let json = serde_json::to_string(&data_from_song(s)).unwrap();
        let data: SongData = serde_json::from_str(&json).unwrap();
        song_from_data(&data).unwrap()
    }

    /// Every built-in song as one songs file, under the parser's caps.
    fn builtin_file() -> String {
        let mut file = SongsFile {
            v: 1,
            songs: Default::default(),
        };
        for (name, s) in [
            ("menu", &songs::MENU),
            ("play", &songs::PLAY),
            ("play_layer", &songs::PLAY_LAYER),
            ("game_over", &songs::GAME_OVER),
            ("demo_end", &songs::DEMO_END),
        ] {
            file.songs.insert(name.to_string(), data_from_song(s));
        }
        serde_json::to_string(&file).unwrap()
    }

    #[test]
    fn builtin_songs_fit_the_file_caps() {
        let text = builtin_file();
        let file = SongsFile::parse(text.as_bytes()).unwrap();
        assert_eq!(file.songs.len(), 5);
        for (name, d) in &file.songs {
            assert!(song_from_data(d).is_ok(), "{name}");
        }
    }

    #[test]
    fn songs_from_files_render_sample_identical() {
        for s in [&songs::MENU, &songs::PLAY, &songs::PLAY_LAYER] {
            let a = tracker::render_loop(s).unwrap();
            let b = tracker::render_loop(&round_trip(s)).unwrap();
            assert!(a.bytes == b.bytes);
        }
        for (s, tail) in [(&songs::GAME_OVER, 1.5), (&songs::DEMO_END, 1.0)] {
            let a = tracker::render_sting(s, tail).unwrap();
            let b = tracker::render_sting(&round_trip(s), tail).unwrap();
            assert!(a.bytes == b.bytes);
        }
    }

    #[test]
    fn bad_song_parts_answer_with_the_tracker_complaint() {
        let mut d = data_from_song(&songs::MENU);
        d.voices[0] = d.voices[0].replacen("e4:2", "h4:2", 1);
        let e = song_from_data(&d).unwrap_err();
        assert_eq!(
            e.0,
            "The song has \"h4:2\" in voice 1, which is not a note, a rest or a setting."
        );
        let mut d = data_from_song(&songs::MENU);
        d.noise = "x:4".to_string();
        let e = song_from_data(&d).unwrap_err();
        assert!(
            e.0.contains("4 steps in the noise part") && e.0.contains("128"),
            "{e}"
        );
        let mut d = data_from_song(&songs::MENU);
        d.instruments.truncate(1);
        assert!(song_from_data(&d).unwrap_err().0.contains("instrument 1"));
        let mut d = data_from_song(&songs::GAME_OVER);
        d.voices[2] = d.voices[2].replacen("c3:4", "c1:4", 1);
        assert!(song_from_data(&d).unwrap_err().0.contains("too low"));
    }

    fn program(json: &str) -> Vec<Vec<FxOp>> {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn programs_from_files_equal_the_built_fx() {
        let confirm = program(
            r#"[[{"op":"hit","voice":0,"note":"c5","frames":5}],[{"op":"hit","voice":0,"note":"g5","frames":9}]]"#,
        );
        assert_eq!(
            fx_from_program(&confirm).unwrap(),
            audio::program(SfxEvent::Confirm).unwrap()
        );
        let sweep = program(
            r#"[[{"op":"tone","voice":0,"from":"c4","to":"c6","a0":2,"a1":15,"frames":18},
                 {"op":"noise","mode":"white","rate":"div512","a0":6,"a1":15,"frames":12}]]"#,
        );
        assert_eq!(
            fx_from_program(&sweep).unwrap(),
            audio::program(SfxEvent::ScreenSweep).unwrap()
        );
        let bad = program(r#"[[{"op":"hit","voice":0,"note":"h4","frames":5}]]"#);
        assert!(fx_from_program(&bad).unwrap_err().0.contains("\"h4\""));
    }

    #[test]
    fn noise_names_map_to_the_chip() {
        assert_eq!(noise_mode("white"), Some(NoiseMode::White));
        assert_eq!(noise_mode("periodic"), Some(NoiseMode::Periodic));
        assert_eq!(noise_rate("div512"), Some(NoiseRate::Div512));
        assert_eq!(noise_rate("div1024"), Some(NoiseRate::Div1024));
        assert_eq!(noise_rate("div2048"), Some(NoiseRate::Div2048));
        assert_eq!(noise_rate("tone3"), Some(NoiseRate::Tone3));
        assert_eq!(noise_mode("White"), None);
        assert_eq!(noise_rate("low"), None);
        for m in [NoiseMode::White, NoiseMode::Periodic] {
            assert_eq!(noise_mode(mode_name(m)), Some(m));
        }
        for r in [
            NoiseRate::Div512,
            NoiseRate::Div1024,
            NoiseRate::Div2048,
            NoiseRate::Tone3,
        ] {
            assert_eq!(noise_rate(rate_name(r)), Some(r));
        }
    }

    /// An app with sound stores (no device needed) and baked SFX.
    fn sound_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<AudioSource>()
            .add_message::<SfxEvent>()
            .init_resource::<MusicCache>()
            .init_resource::<CurrentCue>()
            .add_systems(Startup, (load_embedded_audio, audio::setup_sfx).chain());
        app.update();
        app
    }

    const ONE_BAR: &str = r#"{"bpm":120,"beats_per_bar":4,"steps_per_beat":4,"bars":1,
        "voices":["c4:16",".:16",".:16"],"noise":".:16","instruments":[{"env":[0,4,8]}]}"#;

    #[test]
    fn unknown_names_and_the_house_sting_are_refused() {
        let mut app = sound_app();
        let w = app.world_mut();
        let e = set_songs(
            w,
            format!(r#"{{"v":1,"songs":{{"nope":{ONE_BAR}}}}}"#).as_bytes(),
        )
        .unwrap_err();
        assert_eq!(e.0, "There is no sound called \"nope\".");
        let e = set_sfx(w, br#"{"v":1,"sfx":{"nope":[[{"op":"rest","frames":1}]]}}"#).unwrap_err();
        assert_eq!(e.0, "There is no sound called \"nope\".");
        let e = set_songs(
            w,
            format!(r#"{{"v":1,"songs":{{"house":{ONE_BAR}}}}}"#).as_bytes(),
        )
        .unwrap_err();
        assert!(e.0.contains("house sting"));
        assert!(w.resource::<SongOverrides>().0.is_empty());
        assert!(w.resource::<SfxOverrides>().0.is_empty());
    }

    #[test]
    fn headless_worlds_change_nothing() {
        let mut world = World::new();
        assert!(set_songs(&mut world, br#"{"v":1,"songs":{}}"#).is_err());
        assert!(set_sfx(&mut world, br#"{"v":1,"sfx":{}}"#).is_err());
        load_audio_from(&mut world, Some(b"{}"), Some(b"{}"));
        assert!(!world.contains_resource::<SongOverrides>());
        assert!(!world.contains_resource::<ContentWarnings>());
        assert!(!preview(&mut world, "confirm"));
    }

    fn handle_for(world: &mut World, name: &str) -> Handle<AudioSource> {
        let part = audio::find_part(name).unwrap();
        world.resource_scope(|world, mut cache: Mut<MusicCache>| {
            world.resource_scope(|world, mut sources: Mut<Assets<AudioSource>>| {
                let server = world.resource::<AssetServer>();
                let o = world.get_resource::<SongOverrides>();
                cache.handle(&mut sources, server, part, o)
            })
        })
    }

    fn bytes_of(world: &World, h: &Handle<AudioSource>) -> Vec<u8> {
        world
            .resource::<Assets<AudioSource>>()
            .get(h)
            .unwrap()
            .bytes
            .to_vec()
    }

    #[test]
    fn set_songs_swaps_the_cue_audio_and_restarts_the_cue_playing() {
        let mut app = sound_app();
        let w = app.world_mut();
        w.resource_mut::<CurrentCue>().cue = Some(&MENU_CUE);
        let menu = handle_for(w, "menu");
        let layer = handle_for(w, "play_layer");

        let file = format!(r#"{{"v":1,"songs":{{"play_layer":{ONE_BAR}}}}}"#);
        set_songs(w, file.as_bytes()).unwrap();
        assert!(!w.resource::<CurrentCue>().restart, "menu did not change");
        assert_eq!(handle_for(w, "menu"), menu);
        let new_layer = handle_for(w, "play_layer");
        assert_ne!(new_layer, layer);
        let data: SongData = serde_json::from_str(ONE_BAR).unwrap();
        let want = tracker::render_loop(&song_from_data(&data).unwrap()).unwrap();
        assert_eq!(bytes_of(w, &new_layer), want.bytes.to_vec());

        // The same file again changes nothing.
        set_songs(w, file.as_bytes()).unwrap();
        assert_eq!(handle_for(w, "play_layer"), new_layer);

        let file = format!(r#"{{"v":1,"songs":{{"menu":{ONE_BAR}}}}}"#);
        set_songs(w, file.as_bytes()).unwrap();
        assert!(w.resource::<CurrentCue>().restart);
        assert_ne!(handle_for(w, "menu"), menu);
        // Back to the built-in layer, already rendered once.
        assert_eq!(handle_for(w, "play_layer"), layer);

        w.resource_mut::<CurrentCue>().restart = false;
        set_songs(w, br#"{"v":1,"songs":{}}"#).unwrap();
        assert!(w.resource::<CurrentCue>().restart);
        assert_eq!(handle_for(w, "menu"), menu);
        // Stings from a file render with a release tail.
        let file = format!(r#"{{"v":1,"songs":{{"game_over":{ONE_BAR}}}}}"#);
        set_songs(w, file.as_bytes()).unwrap();
        let sting = handle_for(w, "game_over");
        let want =
            tracker::render_sting(&song_from_data(&data).unwrap(), audio::OVERRIDE_STING_TAIL)
                .unwrap();
        assert_eq!(bytes_of(w, &sting), want.bytes.to_vec());
        let _ = PLAY_CUE.name;
    }

    #[test]
    fn set_sfx_rebakes_the_changed_events() {
        let mut app = sound_app();
        let w = app.world_mut();
        let before_pause = w
            .resource::<SfxAssets>()
            .get(SfxEvent::Pause)
            .unwrap()
            .0
            .clone();
        let file = br#"{"v":1,"sfx":{"confirm":[[{"op":"hit","voice":0,"note":"g4","frames":5}],[{"op":"hit","voice":0,"note":"c4","frames":9}]]}}"#;
        set_sfx(w, file).unwrap();
        let confirm = w
            .resource::<SfxAssets>()
            .get(SfxEvent::Confirm)
            .unwrap()
            .0
            .clone();
        assert_eq!(
            bytes_of(w, &confirm),
            audio::bake(SfxEvent::Pause).bytes.to_vec(),
            "confirm now sounds like pause"
        );
        let pause = w
            .resource::<SfxAssets>()
            .get(SfxEvent::Pause)
            .unwrap()
            .0
            .clone();
        assert_eq!(pause, before_pause, "pause was not rebaked");
        set_sfx(w, br#"{"v":1,"sfx":{}}"#).unwrap();
        let confirm = w
            .resource::<SfxAssets>()
            .get(SfxEvent::Confirm)
            .unwrap()
            .0
            .clone();
        assert_eq!(
            bytes_of(w, &confirm),
            audio::bake(SfxEvent::Confirm).bytes.to_vec()
        );
    }

    #[test]
    fn embedded_files_load_at_startup_and_bad_ones_warn() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<AudioSource>()
            .init_resource::<MusicCache>();
        let w = app.world_mut();
        let songs = format!(r#"{{"v":1,"songs":{{"menu":{ONE_BAR}}}}}"#);
        load_audio_from(w, Some(songs.as_bytes()), Some(b"not json"));
        assert_eq!(w.resource::<SongOverrides>().0.len(), 1);
        assert!(w.resource::<SfxOverrides>().0.is_empty());
        let warnings = &w.resource::<ContentWarnings>().0;
        assert_eq!(warnings.len(), 1);
        assert!(
            warnings[0].starts_with("content/audio/sfx.json: "),
            "{warnings:?}"
        );
    }

    #[test]
    fn preview_plays_events_and_cues_by_name() {
        let mut app = sound_app();
        let w = app.world_mut();
        assert!(preview(w, "confirm"));
        let sent: Vec<SfxEvent> = w.resource_mut::<Messages<SfxEvent>>().drain().collect();
        assert_eq!(sent, vec![SfxEvent::Confirm]);
        assert!(preview(w, "menu"));
        assert!(preview(w, "play_layer"));
        let n = w
            .query_filtered::<Entity, With<PreviewVoice>>()
            .iter(w)
            .count();
        assert_eq!(n, 1, "a preview replaces the last one");
        assert!(!preview(w, "nope"));
    }
}
