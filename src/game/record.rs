//! Feature-gated offline recorder (`cargo run --features record`).
//!
//! Layers on the autopilot tour: the app runs on a fixed 1/60 s manual clock
//! (`TimeUpdateStrategy::ManualDuration`), every frame is captured with
//! `Screenshot` into `$RECORD_DIR/frames/000001.png…`, and an `events.jsonl`
//! records state changes, autopilot beats, score, pause and any game message
//! registered through the generic loggers. `tools/record.sh` turns the frames
//! into `tour.mp4`; `tools/cut_clips.py` cuts beat clips and renders
//! `chapters.md`. Never compiled into shipping builds.
//!
//! Wiring (see `GamePlugin`): `app.add_plugins(RecordPlugin)`, then
//! `log_state::<GameState>(app)`, `log_messages::<SfxEvent>(app)`,
//! `log_value::<GameData>(app, "score", |d| i64::from(d.score))`. Under this
//! feature the autopilot's `shot()` writes a `RecordBeat` instead of its own
//! screenshot (a second `Screenshot` on the same window in one frame is
//! dropped as a duplicate by Bevy).

use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use bevy::app::AppExit;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use bevy::time::TimeUpdateStrategy;
use bevy::window::PrimaryWindow;

/// Frames per second of the recording and the sim step (1/FPS s per frame).
pub const FPS: u32 = 60;

/// Output root; override with `RECORD_DIR`. `build/` is gitignored.
pub fn record_dir() -> PathBuf {
    std::env::var("RECORD_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("build/record"))
}

/// `frames/<this>`; six digits so ffmpeg's glob sorts them.
pub fn frame_filename(frame: u64) -> String {
    format!("{frame:06}.png")
}

pub fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// A quoted, escaped JSON string.
pub fn json_str(s: &str) -> String {
    format!("\"{}\"", json_escape(s))
}

/// A JSON object from already-rendered values (`json_str`, numbers as text).
pub fn json_obj(fields: &[(&str, String)]) -> String {
    let body: Vec<String> = fields
        .iter()
        .map(|(k, v)| format!("{}:{v}", json_str(k)))
        .collect();
    format!("{{{}}}", body.join(","))
}

/// One `events.jsonl` line. `data` is an already-rendered JSON object.
pub fn event_line(frame: u64, fps: u32, kind: &str, data: &str) -> String {
    let t = frame as f64 / f64::from(fps);
    format!(
        "{{\"frame\":{frame},\"t\":{t:.3},\"kind\":{},\"data\":{data}}}",
        json_str(kind)
    )
}

/// What `manifest.json` carries; written when the tour exits.
pub struct Manifest {
    pub name: String,
    pub fps: u32,
    pub width: u32,
    pub height: u32,
    pub frames: u64,
    pub beats: Vec<(String, u64)>,
}

pub fn manifest_json(m: &Manifest) -> String {
    let beats: Vec<String> = m
        .beats
        .iter()
        .map(|(name, frame)| json_obj(&[("name", json_str(name)), ("frame", frame.to_string())]))
        .collect();
    let duration = m.frames as f64 / f64::from(m.fps);
    format!(
        "{{\"name\":{},\"fps\":{},\"width\":{},\"height\":{},\"frames\":{},\"duration_s\":{:.3},\"beats\":[{}]}}",
        json_str(&m.name),
        m.fps,
        m.width,
        m.height,
        m.frames,
        duration,
        beats.join(",")
    )
}

/// The `"name"` field of `assets/info.json`, without a JSON dependency.
pub fn name_from_info_json(text: &str) -> Option<String> {
    let idx = text.find("\"name\"")?;
    let rest = &text[idx + "\"name\"".len()..];
    let rest = rest.trim_start().strip_prefix(':')?.trim_start();
    let rest = rest.strip_prefix('"')?;
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

/// The autopilot fired a named beat on this frame (`01-studio-logo` …).
#[derive(Message, Debug, Clone)]
pub struct RecordBeat(pub String);

/// Capture runs first so every logger in the same frame stamps the frame
/// that was just captured.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RecordSet {
    Capture,
    Log,
}

#[derive(Resource)]
pub struct Recorder {
    /// Frames captured so far; the current frame's number after `Capture`.
    pub frame: u64,
    pub fps: u32,
    dir: PathBuf,
    log: BufWriter<File>,
    beats: Vec<(String, u64)>,
    finished: bool,
}

impl Recorder {
    fn open(dir: &Path) -> Self {
        fs::create_dir_all(dir.join("frames")).expect("record: create frames dir");
        let file = File::create(dir.join("events.jsonl")).expect("record: create events.jsonl");
        Self {
            frame: 0,
            fps: FPS,
            dir: dir.to_path_buf(),
            log: BufWriter::new(file),
            beats: Vec::new(),
            finished: false,
        }
    }

    /// Append one event at the current frame. `data` is a rendered JSON object.
    pub fn log(&mut self, kind: &str, data: &str) {
        let line = event_line(self.frame, self.fps, kind, data);
        writeln!(self.log, "{line}").expect("record: write events.jsonl");
    }

    fn finish(&mut self, width: u32, height: u32) {
        if self.finished {
            return;
        }
        self.finished = true;
        self.log.flush().expect("record: flush events.jsonl");
        let name = fs::read_to_string("assets/info.json")
            .ok()
            .and_then(|s| name_from_info_json(&s))
            .unwrap_or_else(|| env!("CARGO_PKG_NAME").to_string());
        let manifest = Manifest {
            name,
            fps: self.fps,
            width,
            height,
            frames: self.frame,
            beats: self.beats.clone(),
        };
        fs::write(self.dir.join("manifest.json"), manifest_json(&manifest))
            .expect("record: write manifest.json");
        info!("record: {} frames -> {}", self.frame, self.dir.display());
    }
}

impl Drop for Recorder {
    fn drop(&mut self) {
        let _ = self.log.flush();
    }
}

pub struct RecordPlugin;

impl Plugin for RecordPlugin {
    fn build(&self, app: &mut App) {
        let dir = record_dir();
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
            1.0 / f64::from(FPS),
        )))
        .insert_resource(Recorder::open(&dir))
        .add_message::<RecordBeat>()
        .configure_sets(PostUpdate, RecordSet::Capture.before(RecordSet::Log))
        .add_systems(PostUpdate, capture_frame.in_set(RecordSet::Capture))
        .add_systems(PostUpdate, log_beats.in_set(RecordSet::Log))
        .add_systems(Last, finish_on_exit);
    }
}

/// One screenshot per frame, named by frame number.
fn capture_frame(mut commands: Commands, mut rec: ResMut<Recorder>) {
    rec.frame += 1;
    let path = rec.dir.join("frames").join(frame_filename(rec.frame));
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(path));
}

fn log_beats(mut beats: MessageReader<RecordBeat>, mut rec: ResMut<Recorder>) {
    for RecordBeat(name) in beats.read() {
        let frame = rec.frame;
        rec.beats.push((name.clone(), frame));
        rec.log("beat", &json_obj(&[("name", json_str(name))]));
    }
}

/// Writes `manifest.json` on the frame the autopilot requests exit.
fn finish_on_exit(
    mut exits: MessageReader<AppExit>,
    mut rec: ResMut<Recorder>,
    windows: Query<&Window, With<PrimaryWindow>>,
) {
    if exits.read().next().is_none() {
        return;
    }
    let (w, h) = windows
        .single()
        .map(|w| (w.physical_width(), w.physical_height()))
        .unwrap_or((0, 0));
    rec.finish(w, h);
}

/// Logs `{"kind":"state","data":{"name":"Playing"}}` whenever `S` changes.
pub fn log_state<S: States + std::fmt::Debug>(app: &mut App) {
    fn system<S: States + std::fmt::Debug>(
        state: Res<State<S>>,
        mut last: Local<Option<String>>,
        mut rec: ResMut<Recorder>,
    ) {
        let now = format!("{:?}", state.get());
        if last.as_deref() == Some(now.as_str()) {
            return;
        }
        rec.log("state", &json_obj(&[("name", json_str(&now))]));
        *last = Some(now);
    }
    app.add_systems(PostUpdate, system::<S>.in_set(RecordSet::Log));
}

/// Logs every `T` message as `{"kind":"<TypeName>","data":{"debug":"..."}}`.
pub fn log_messages<T: Message + std::fmt::Debug>(app: &mut App) {
    fn system<T: Message + std::fmt::Debug>(
        mut reader: MessageReader<T>,
        mut rec: ResMut<Recorder>,
    ) {
        let kind = std::any::type_name::<T>()
            .rsplit("::")
            .next()
            .unwrap_or("message");
        for msg in reader.read() {
            rec.log(kind, &json_obj(&[("debug", json_str(&format!("{msg:?}")))]));
        }
    }
    app.add_systems(PostUpdate, system::<T>.in_set(RecordSet::Log));
}

/// Logs `{"kind":<kind>,"data":{"value":N}}` whenever `read(&R)` changes.
pub fn log_value<R: Resource>(app: &mut App, kind: &'static str, read: fn(&R) -> i64) {
    let system = move |res: Res<R>, mut last: Local<Option<i64>>, mut rec: ResMut<Recorder>| {
        let now = read(&res);
        if *last == Some(now) {
            return;
        }
        rec.log(kind, &json_obj(&[("value", now.to_string())]));
        *last = Some(now);
    };
    app.add_systems(PostUpdate, system.in_set(RecordSet::Log));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_filenames_are_six_digit_and_one_based() {
        assert_eq!(frame_filename(1), "000001.png");
        assert_eq!(frame_filename(3712), "003712.png");
    }

    #[test]
    fn json_escape_handles_quotes_backslashes_and_newlines() {
        assert_eq!(json_escape(r#"a"b\c"#), r#"a\"b\\c"#);
        assert_eq!(json_escape("x\ny"), r"x\ny");
        assert_eq!(json_str("hi"), "\"hi\"");
    }

    #[test]
    fn event_line_has_frame_time_kind_and_data() {
        let line = event_line(
            120,
            60,
            "beat",
            &json_obj(&[("name", json_str("02-title"))]),
        );
        assert_eq!(
            line,
            r#"{"frame":120,"t":2.000,"kind":"beat","data":{"name":"02-title"}}"#
        );
    }

    #[test]
    fn json_obj_renders_empty_and_multi_field_objects() {
        assert_eq!(json_obj(&[]), "{}");
        assert_eq!(
            json_obj(&[("a", "1".into()), ("b", json_str("x"))]),
            r#"{"a":1,"b":"x"}"#
        );
    }

    #[test]
    fn manifest_json_lists_beats_in_order() {
        let m = Manifest {
            name: "Gamebient Game".into(),
            fps: 60,
            width: 1920,
            height: 1080,
            frames: 3600,
            beats: vec![("01-studio-logo".into(), 72), ("02-title".into(), 150)],
        };
        assert_eq!(
            manifest_json(&m),
            r#"{"name":"Gamebient Game","fps":60,"width":1920,"height":1080,"frames":3600,"duration_s":60.000,"beats":[{"name":"01-studio-logo","frame":72},{"name":"02-title","frame":150}]}"#
        );
    }

    #[test]
    fn name_from_info_json_reads_the_name_field() {
        let info = "{\n    \"name\": \"Tire Stack\",\n    \"image\": \"x\"\n}";
        assert_eq!(name_from_info_json(info).as_deref(), Some("Tire Stack"));
        assert_eq!(name_from_info_json("{}"), None);
    }
}
