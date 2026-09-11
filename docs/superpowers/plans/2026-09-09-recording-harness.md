# Recording Harness Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give every Gamebient game a `record` cargo feature that renders the autopilot tour offline into a 60 fps MP4, beat clips, an event log and a chapters table, plus a workspace skill that rolls it out, runs it and writes per-game video notes.

**Architecture:** `src/game/record.rs` layers on the existing autopilot: a fixed 1/60 s manual clock, one `Screenshot` per frame, and an `events.jsonl` fed by generic loggers (`log_state`, `log_messages`, `log_value`) plus `RecordBeat` markers written by the autopilot's `shot()`. `tools/record.sh` runs the build and ffmpeg; `tools/cut_clips.py` cuts clips and stills and renders `chapters.md`. `tools/rollout-record.sh` copies the feature into a game. The skill `.claude/skills/recording-game-footage/` drives rollout, recording, verification and the editorial notes.

**Tech Stack:** Rust 2024, Bevy 0.18 (`bevy::render::view::screenshot`, `bevy::time::TimeUpdateStrategy`), bash, Python 3 stdlib, ffmpeg 9.

Spec: `docs/superpowers/specs/2026-09-09-recording-harness-design.md`.

## Global Constraints

- Bevy `0.18`, `default-features = false`; add no new crate dependencies (JSON is hand-rendered).
- The `record` feature is dev-only: `record = ["autopilot"]`, never enabled in shipping builds, never in `build_web.sh`.
- Output root is `$RECORD_DIR`, default `build/record` (already gitignored via `/build/`).
- Frame rate is 60 fps; frames are `frames/000001.png` (six digits, starting at 1); default `AUTOPILOT_SCALE=1.5` (1920x1080).
- Beat names keep the autopilot contract `01-studio-logo` … `09-game-over`.
- Every event line is `{"frame":N,"t":S,"kind":"...","data":{...}}`.
- CI already runs `cargo clippy --all-targets --all-features -- -D warnings`, so the feature is compile-checked; keep it warning-free.
- Commits in each repo end with `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`.
- Template repo: `libs/gamebient-bevy-template`. Game repos: `games/<name>`. Workspace root (`<workspace>`) is not a git repo; skills live in its `.claude/skills/`.

---

## File map

Template (`libs/gamebient-bevy-template`):

| File | Responsibility |
|---|---|
| `Cargo.toml` | `record = ["autopilot"]` feature |
| `src/game/record.rs` | `RecordPlugin`, `Recorder`, `RecordBeat`, generic loggers, manifest; pure JSON/naming helpers with tests |
| `src/game/mod.rs` | `pub mod record` + wiring block |
| `src/game/autopilot.rs` | `shot()` writes `RecordBeat` under `record` |
| `tools/record.sh` | build → run → ffmpeg → cutter → cleanup |
| `tools/cut_clips.py` | clips, stills, `chapters.md` |
| `tools/test_cut_clips.py` | unit tests for the cutter's pure helpers |
| `tools/rollout-record.sh` | copy + wire the feature into a game dir |
| `README.md`, `AGENTS.md` | usage + contract |

Workspace: `.claude/skills/recording-game-footage/{SKILL.md, references/notes-template.md, references/catalog-2026-09-09.md, references/irregular-games.md}`.

Games: the four files above copied in, plus wiring edits, plus `docs/video-notes.md` after the first run.

---

### Task 1: Feature flag and the recorder's pure helpers

**Files:**
- Modify: `Cargo.toml:6-9`
- Create: `src/game/record.rs`
- Modify: `src/game/mod.rs:3-10`

**Interfaces:**
- Produces: `record::FPS: u32`, `record::record_dir() -> PathBuf`, `record::frame_filename(u64) -> String`, `record::json_escape(&str) -> String`, `record::json_str(&str) -> String`, `record::json_obj(&[(&str, String)]) -> String`, `record::event_line(frame: u64, fps: u32, kind: &str, data: &str) -> String`, `record::manifest_json(&Manifest) -> String`, `record::name_from_info_json(&str) -> Option<String>`, `pub struct Manifest { name, fps, width, height, frames, beats: Vec<(String, u64)> }`.

- [ ] **Step 1: Add the feature**

In `Cargo.toml` after `autopilot = []`:

```toml
# Offline footage recorder layered on the autopilot tour: fixed 1/60 s clock,
# one screenshot per frame, events.jsonl (src/game/record.rs, tools/record.sh).
# Dev-only; never enabled in shipping builds.
record = ["autopilot"]
```

- [ ] **Step 2: Declare the module**

In `src/game/mod.rs`, after the `autopilot` declaration:

```rust
#[cfg(feature = "record")]
pub mod record;
```

- [ ] **Step 3: Write the failing tests**

Create `src/game/record.rs` with only the test module for now:

```rust
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
        let line = event_line(120, 60, "beat", &json_obj(&[("name", json_str("02-title"))]));
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
```

- [ ] **Step 4: Run the tests to verify they fail**

Run: `cargo test --features record record::tests`
Expected: compile errors, `frame_filename` and friends not found.

- [ ] **Step 5: Implement the helpers**

Above the test module in `src/game/record.rs`:

```rust
use std::path::PathBuf;

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
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test --features record record::tests`
Expected: 6 passed. Also `cargo test` (no feature) still passes.

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml src/game/record.rs src/game/mod.rs
git commit -m "feat(record): record feature flag and recorder helpers

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 2: RecordPlugin — manual clock, per-frame capture, loggers, manifest

**Files:**
- Modify: `src/game/record.rs`
- Modify: `src/game/mod.rs` (the `#[cfg(feature = "autopilot")] app.add_plugins(...)` block near line 60)
- Modify: `src/game/autopilot.rs:43-47` (imports) and `:119-125` (`fn shot`)

**Interfaces:**
- Consumes: Task 1 helpers.
- Produces: `pub struct RecordPlugin`, `pub struct RecordBeat(pub String)` (Message), `pub struct Recorder { pub frame: u64, pub fps: u32, .. }` with `pub fn log(&mut self, kind: &str, data: &str)`, `pub fn log_state<S: States + std::fmt::Debug>(app: &mut App)`, `pub fn log_messages<T: Message + std::fmt::Debug>(app: &mut App)`, `pub fn log_value<R: Resource>(app: &mut App, kind: &'static str, read: fn(&R) -> i64)`, `pub enum RecordSet { Capture, Log }`.

- [ ] **Step 1: Add the runtime part of `record.rs`**

Insert between the helpers and the test module:

```rust
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::Path;
use std::time::Duration;

use bevy::app::AppExit;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use bevy::time::TimeUpdateStrategy;
use bevy::window::PrimaryWindow;

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
    fn system<T: Message + std::fmt::Debug>(mut reader: MessageReader<T>, mut rec: ResMut<Recorder>) {
        let kind = std::any::type_name::<T>().rsplit("::").next().unwrap_or("message");
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
```

Move the `use std::path::PathBuf;` from Task 1 into this import block (one `use` list at the top of the file).

- [ ] **Step 2: Wire the plugin in `GamePlugin`**

In `src/game/mod.rs`, directly after `app.add_plugins(autopilot::AutopilotPlugin);`:

```rust
        #[cfg(feature = "record")]
        {
            app.add_plugins(record::RecordPlugin);
            record::log_state::<GameState>(app);
            record::log_messages::<audio::SfxEvent>(app);
            record::log_value::<scoring::GameData>(app, "score", |d| i64::from(d.score));
            record::log_value::<states::Paused>(app, "pause", |p| i64::from(p.0));
        }
```

- [ ] **Step 3: Make the autopilot's `shot()` a beat marker under `record`**

In `src/game/autopilot.rs` change the screenshot import and `shot`:

```rust
#[cfg(not(feature = "record"))]
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
```

```rust
/// Captures a named beat. Under `record` every frame is already captured
/// (and a second `Screenshot` of the same window in one frame is dropped as
/// a duplicate), so the beat is logged instead and `tools/cut_clips.py`
/// extracts the still from `tour.mp4`.
fn shot(commands: &mut Commands, name: &str) {
    #[cfg(feature = "record")]
    {
        info!("autopilot: beat {name}");
        commands.write_message(super::record::RecordBeat(name.to_string()));
    }
    #[cfg(not(feature = "record"))]
    {
        let path = format!("{}/{name}.png", shot_dir());
        info!("autopilot: screenshot {path}");
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path));
    }
}
```

- [ ] **Step 4: Compile and lint both feature sets**

Run: `cargo clippy --all-targets --all-features -- -D warnings && cargo clippy --all-targets --features autopilot -- -D warnings && cargo test --all-features`
Expected: clean; the six tests pass. If clippy flags `shot_dir` as unused under `record`, add `#[cfg_attr(feature = "record", allow(dead_code))]` to it.

- [ ] **Step 5: Smoke-run the recorder for a real capture**

Run: `RECORD_DIR=/tmp/rec-smoke AUTOPILOT_SCALE=1.0 timeout 120 cargo run --release --features record; ls /tmp/rec-smoke/frames | wc -l; head -5 /tmp/rec-smoke/events.jsonl; cat /tmp/rec-smoke/manifest.json`
Expected: the window plays the tour and exits by itself (the first release build takes a few minutes; raise the timeout if it is still compiling). Frames count in the thousands, `events.jsonl` starts with `{"frame":1,...,"kind":"state","data":{"name":"StudioLogo"}}`, a `beat` line for `01-studio-logo`, and `manifest.json` lists nine beats with `width` 1280 (scale 1.0). Delete `/tmp/rec-smoke` afterwards.

- [ ] **Step 6: Commit**

```bash
git add src/game/record.rs src/game/mod.rs src/game/autopilot.rs
git commit -m "feat(record): RecordPlugin with per-frame capture, event log and manifest

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 3: `tools/cut_clips.py` — clips, stills and chapters

**Files:**
- Create: `tools/cut_clips.py`
- Create: `tools/test_cut_clips.py`

**Interfaces:**
- Consumes: `manifest.json` and `events.jsonl` from Task 2.
- Produces: `clips/<beat>.mp4`, `clips/signature.mp4`, `clips/game-over.mp4`, `shots/<beat>.png`, `chapters.md`; pure helpers `clip_window(beat_t, duration, pre=2.0, post=4.0) -> (start, length)`, `fmt_time(t) -> str`, `milestones(scores) -> list[(t, value)]`, `render_chapters(manifest, events, clips) -> str`.

- [ ] **Step 1: Write the failing tests**

`tools/test_cut_clips.py`:

```python
import unittest

from cut_clips import clip_window, fmt_time, milestones, render_chapters


class ClipWindowTests(unittest.TestCase):
    def test_window_is_two_before_four_after(self):
        self.assertEqual(clip_window(10.0, 60.0), (8.0, 6.0))

    def test_window_clamps_to_start(self):
        self.assertEqual(clip_window(1.0, 60.0), (0.0, 5.0))

    def test_window_clamps_to_end(self):
        self.assertEqual(clip_window(58.0, 60.0), (56.0, 4.0))


class FormatTests(unittest.TestCase):
    def test_fmt_time(self):
        self.assertEqual(fmt_time(0.0), "00:00.00")
        self.assertEqual(fmt_time(65.5), "01:05.50")


class MilestoneTests(unittest.TestCase):
    def test_first_score_and_power_of_ten_crossings(self):
        scores = [(1.0, 0), (2.0, 100), (3.0, 500), (4.0, 1000), (5.0, 1200), (6.0, 10500)]
        self.assertEqual(milestones(scores), [(2.0, 100), (4.0, 1000), (6.0, 10500)])

    def test_no_scores_gives_nothing(self):
        self.assertEqual(milestones([]), [])


class ChaptersTests(unittest.TestCase):
    def test_renders_table_and_clip_list(self):
        manifest = {"name": "Tire Stack", "fps": 60, "duration_s": 62.0, "frames": 3720}
        events = [
            {"frame": 1, "t": 0.017, "kind": "state", "data": {"name": "StudioLogo"}},
            {"frame": 72, "t": 1.2, "kind": "beat", "data": {"name": "01-studio-logo"}},
            {"frame": 400, "t": 6.667, "kind": "score", "data": {"value": 0}},
            {"frame": 520, "t": 8.667, "kind": "score", "data": {"value": 100}},
            {"frame": 3000, "t": 50.0, "kind": "pause", "data": {"value": 1}},
            {"frame": 3010, "t": 50.167, "kind": "SfxEvent", "data": {"debug": "Pause"}},
        ]
        out = render_chapters(manifest, events, ["clips/01-studio-logo.mp4"])
        self.assertIn("# Tire Stack — chapters", out)
        self.assertIn("| 00:00.02 | state | StudioLogo |", out)
        self.assertIn("| 00:01.20 | beat | 01-studio-logo |", out)
        self.assertIn("| 00:08.67 | score | 100 |", out)
        self.assertIn("| 00:50.00 | pause | on |", out)
        self.assertNotIn("SfxEvent", out)
        self.assertIn("- clips/01-studio-logo.mp4", out)
        self.assertIn("Tour length: 01:02.00 (3720 frames @ 60 fps)", out)


if __name__ == "__main__":
    unittest.main()
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cd tools && python3 -m unittest test_cut_clips -v`
Expected: `ModuleNotFoundError: No module named 'cut_clips'`.

- [ ] **Step 3: Implement the cutter**

`tools/cut_clips.py`:

```python
#!/usr/bin/env python3
"""Cut beat clips and stills from a recording and render chapters.md.

Usage: cut_clips.py <record_dir>   (the RECORD_DIR that record.sh produced)

Reads manifest.json + events.jsonl, writes:
  clips/<beat>.mp4     2 s before to 4 s after every logged beat
  clips/signature.mp4  copy of the 06-* beat clip
  clips/game-over.mp4  copy of the 09-game-over clip
  shots/<beat>.png     the exact beat frame (the autopilot's screenshot contract)
  chapters.md          timestamped table of states, beats, pause, score milestones
Stdlib only; needs ffmpeg on PATH.
"""

import json
import pathlib
import shutil
import subprocess
import sys

PRE_S = 2.0
POST_S = 4.0


def clip_window(beat_t, duration, pre=PRE_S, post=POST_S):
    """(start, length) in seconds, clamped to [0, duration]."""
    start = max(0.0, beat_t - pre)
    end = min(duration, beat_t + post)
    return (start, max(0.0, end - start))


def fmt_time(t):
    minutes = int(t // 60)
    seconds = t - 60 * minutes
    return f"{minutes:02d}:{seconds:05.2f}"


def milestones(scores):
    """First non-zero score, then every crossing of a power of ten."""
    out = []
    threshold = None
    for t, value in scores:
        if value <= 0:
            continue
        if threshold is None:
            out.append((t, value))
            threshold = 10 ** len(str(value))
            continue
        if value >= threshold:
            out.append((t, value))
            while value >= threshold:
                threshold *= 10
    return out


def load_events(path):
    with open(path, encoding="utf-8") as f:
        return [json.loads(line) for line in f if line.strip()]


def render_chapters(manifest, events, clips):
    rows = []
    scores = []
    for e in events:
        kind, data, t = e["kind"], e["data"], e["t"]
        if kind == "state":
            rows.append((t, "state", data["name"]))
        elif kind == "beat":
            rows.append((t, "beat", data["name"]))
        elif kind == "pause":
            rows.append((t, "pause", "on" if data["value"] else "off"))
        elif kind == "score":
            scores.append((t, data["value"]))
    for t, value in milestones(scores):
        rows.append((t, "score", str(value)))
    rows.sort(key=lambda r: r[0])
    lines = [f"# {manifest['name']} — chapters", ""]
    lines.append(
        f"Tour length: {fmt_time(manifest['duration_s'])} "
        f"({manifest['frames']} frames @ {manifest['fps']} fps)"
    )
    lines += ["", "| time | kind | detail |", "|---|---|---|"]
    lines += [f"| {fmt_time(t)} | {kind} | {detail} |" for t, kind, detail in rows]
    lines += ["", "## Clips", ""]
    lines += [f"- {c}" for c in clips]
    lines.append("")
    return "\n".join(lines)


def ffmpeg(*args):
    subprocess.run(["ffmpeg", "-y", "-loglevel", "error", *args], check=True)


def main(record_dir):
    root = pathlib.Path(record_dir)
    manifest = json.loads((root / "manifest.json").read_text(encoding="utf-8"))
    events = load_events(root / "events.jsonl")
    tour = root / "tour.mp4"
    clips_dir = root / "clips"
    shots_dir = root / "shots"
    clips_dir.mkdir(exist_ok=True)
    shots_dir.mkdir(exist_ok=True)
    duration = manifest["duration_s"]
    fps = manifest["fps"]
    written = []
    for beat in manifest["beats"]:
        name, t = beat["name"], beat["frame"] / fps
        start, length = clip_window(t, duration)
        out = clips_dir / f"{name}.mp4"
        ffmpeg("-ss", f"{start:.3f}", "-i", str(tour), "-t", f"{length:.3f}",
               "-c:v", "libx264", "-crf", "18", "-pix_fmt", "yuv420p", "-an", str(out))
        written.append(f"clips/{name}.mp4")
        ffmpeg("-ss", f"{t:.3f}", "-i", str(tour), "-frames:v", "1", str(shots_dir / f"{name}.png"))
        if name.startswith("06-"):
            shutil.copyfile(out, clips_dir / "signature.mp4")
            written.append("clips/signature.mp4")
        if name == "09-game-over":
            shutil.copyfile(out, clips_dir / "game-over.mp4")
            written.append("clips/game-over.mp4")
    (root / "chapters.md").write_text(render_chapters(manifest, events, written), encoding="utf-8")
    print(f"cut_clips: {len(manifest['beats'])} beats -> {clips_dir}")


if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    main(sys.argv[1])
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cd tools && python3 -m unittest test_cut_clips -v`
Expected: 7 tests OK.

- [ ] **Step 5: Commit**

```bash
git add tools/cut_clips.py tools/test_cut_clips.py
git commit -m "feat(record): clip cutter with chapters rendering

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 4: `tools/record.sh`, docs, end-to-end run

**Files:**
- Create: `tools/record.sh`
- Modify: `README.md:46-56` (after the autopilot section), `AGENTS.md:40-46` (after the visual-check bullet)

**Interfaces:**
- Consumes: Tasks 2 and 3.
- Produces: `build/record/{tour.mp4, clips/, shots/, events.jsonl, manifest.json, chapters.md}`; env contract `RECORD_DIR`, `AUTOPILOT_SCALE`; flag `--keep-frames`.

- [ ] **Step 1: Write the script**

`tools/record.sh`:

```bash
#!/usr/bin/env bash
# Records the autopilot tour offline into build/record/:
#   tour.mp4 (60 fps), clips/<beat>.mp4, shots/<beat>.png, events.jsonl,
#   manifest.json, chapters.md.  See src/game/record.rs and tools/cut_clips.py.
#
# Usage: tools/record.sh [--keep-frames]
# Env:   RECORD_DIR (default build/record), AUTOPILOT_SCALE (default 1.5 = 1920x1080)
set -euo pipefail
cd "$(dirname "$0")/.."

KEEP=0
for arg in "$@"; do
  case "$arg" in
    --keep-frames) KEEP=1 ;;
    *) echo "usage: $0 [--keep-frames]" >&2; exit 2 ;;
  esac
done

command -v ffmpeg >/dev/null 2>&1 || { echo "record.sh: ffmpeg not on PATH (brew install ffmpeg)" >&2; exit 1; }
command -v python3 >/dev/null 2>&1 || { echo "record.sh: python3 not on PATH" >&2; exit 1; }

export RECORD_DIR="${RECORD_DIR:-build/record}"
export AUTOPILOT_DIR="$RECORD_DIR/autopilot"
export AUTOPILOT_SCALE="${AUTOPILOT_SCALE:-1.5}"

rm -rf "$RECORD_DIR"
mkdir -p "$RECORD_DIR"
[ -w "$RECORD_DIR" ] || { echo "record.sh: $RECORD_DIR is not writable" >&2; exit 1; }

echo "record.sh: recording the autopilot tour into $RECORD_DIR (scale $AUTOPILOT_SCALE)"
cargo run --release --features record

frames=$(find "$RECORD_DIR/frames" -name '*.png' | wc -l | tr -d ' ')
[ "$frames" -gt 0 ] || { echo "record.sh: no frames captured" >&2; exit 1; }

echo "record.sh: encoding $frames frames"
ffmpeg -y -loglevel error -framerate 60 -pattern_type glob -i "$RECORD_DIR/frames/*.png" \
  -c:v libx264 -crf 18 -pix_fmt yuv420p "$RECORD_DIR/tour.mp4"

python3 tools/cut_clips.py "$RECORD_DIR"

[ "$KEEP" = 1 ] || rm -rf "$RECORD_DIR/frames"
echo "record.sh: done -> $RECORD_DIR/tour.mp4 ($frames frames)"
```

`chmod +x tools/record.sh`.

- [ ] **Step 2: Document it**

README, new subsection after "Visual check (autopilot)":

````markdown
### Footage (record)

```bash
tools/record.sh                       # 60 fps tour video + beat clips -> build/record/
AUTOPILOT_SCALE=1.0 tools/record.sh   # 1280x720 instead of 1920x1080
tools/record.sh --keep-frames         # keep the PNG frames after encoding
```

Runs the autopilot tour on a fixed 1/60 s clock, captures every frame, and
encodes `build/record/tour.mp4` with ffmpeg. `events.jsonl` logs state
changes, beats, score, pause and every `SfxEvent`; `tools/cut_clips.py` cuts
`clips/<beat>.mp4` (2 s before to 4 s after each beat), `shots/<beat>.png`,
and `chapters.md`. Needs `ffmpeg` and `python3`. A tour is ~3,700 frames of
PNG (several GB) until the script deletes them. Dev-only; never ships.
````

AGENTS.md, a bullet after the visual-check bullet:

```markdown
- **Footage:** `tools/record.sh` records the same tour offline (fixed 1/60 s
  clock, one `Screenshot` per frame, `events.jsonl`) into `build/record/`
  and cuts beat clips + `chapters.md` (`src/game/record.rs`,
  `tools/cut_clips.py`). Under `record` the autopilot's `shot()` logs a
  `RecordBeat` instead of taking its own screenshot (Bevy drops a second
  `Screenshot` of the same window in one frame). Add game messages to the log
  with `record::log_messages::<T>(app)` in `GamePlugin`. The
  `recording-game-footage` skill rolls this out and writes
  `docs/video-notes.md`.
```

- [ ] **Step 3: Run it end to end**

Run: `tools/record.sh`
Expected: exits 0; `build/record/` holds `tour.mp4`, nine `clips/0*.mp4` plus `signature.mp4` and `game-over.mp4`, nine `shots/*.png`, `chapters.md`, `manifest.json`, `events.jsonl`; no `frames/`. Check: `ffprobe -v error -select_streams v -show_entries stream=width,height,r_frame_rate,nb_frames -of csv=p=0 build/record/tour.mp4` prints `1920,1080,60/1,<frames>` and the frame count equals `manifest.json`'s `frames` within 5. Read `build/record/shots/06-first-score.png` and confirm it shows gameplay with a score.

- [ ] **Step 4: Commit**

```bash
git add tools/record.sh README.md AGENTS.md
git commit -m "feat(record): record.sh pipeline and docs

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 5: `tools/rollout-record.sh` and first game rollout (Tire Stack)

**Files:**
- Create: `tools/rollout-record.sh` (template)
- Modify in `games/tire-stack`: `Cargo.toml`, `src/game/mod.rs`, `src/game/autopilot.rs`; create `src/game/record.rs`, `tools/record.sh`, `tools/cut_clips.py`, `tools/test_cut_clips.py`

**Interfaces:**
- Consumes: the four template files.
- Produces: `tools/rollout-record.sh <game-dir>`; idempotent (re-running changes nothing).

- [ ] **Step 1: Write the rollout script**

`tools/rollout-record.sh`:

```bash
#!/usr/bin/env bash
# Copies the record feature from this template into a game checkout and wires
# it: feature flag, `pub mod record`, the RecordPlugin block in GamePlugin,
# the RecordBeat branch in the autopilot's shot(), Debug on SfxEvent.
# Idempotent. Games whose layout differs from the template (autopilot under
# src/, no GameData, SfxEvent elsewhere) print what still needs a hand edit.
#
# Usage: tools/rollout-record.sh <game-dir>
set -euo pipefail
TEMPLATE="$(cd "$(dirname "$0")/.." && pwd)"
GAME="$(cd "${1:?usage: $0 <game-dir>}" && pwd)"

cp "$TEMPLATE/src/game/record.rs" "$GAME/src/game/record.rs"
mkdir -p "$GAME/tools"
cp "$TEMPLATE/tools/record.sh" "$TEMPLATE/tools/cut_clips.py" "$TEMPLATE/tools/test_cut_clips.py" "$GAME/tools/"
chmod +x "$GAME/tools/record.sh"

# Cargo feature.
if ! grep -q '^record = ' "$GAME/Cargo.toml"; then
  perl -0pi -e 's/^autopilot = \[\]\n/autopilot = []\n# Offline footage recorder layered on the autopilot tour (src\/game\/record.rs,\n# tools\/record.sh). Dev-only; never enabled in shipping builds.\nrecord = ["autopilot"]\n/m' "$GAME/Cargo.toml"
fi
grep -q '^record = ' "$GAME/Cargo.toml" || echo "HAND EDIT: add 'record = [\"autopilot\"]' under [features] in Cargo.toml"

# Module declaration + plugin block.
MOD="$GAME/src/game/mod.rs"
if [ -f "$MOD" ]; then
  if ! grep -q 'pub mod record' "$MOD"; then
    perl -0pi -e 's/(#\[cfg\(feature = "autopilot"\)\]\npub mod autopilot;\n)/$1#[cfg(feature = "record")]\npub mod record;\n/' "$MOD"
  fi
  if ! grep -q 'record::RecordPlugin' "$MOD"; then
    perl -0pi -e 's/^([ \t]*)(app\.add_plugins\(autopilot::AutopilotPlugin\);\n)/$1$2$1#[cfg(feature = "record")]\n$1\{\n$1    app.add_plugins(record::RecordPlugin);\n$1    record::log_state::<GameState>(app);\n$1    record::log_messages::<audio::SfxEvent>(app);\n$1    record::log_value::<scoring::GameData>(app, "score", |d| i64::from(d.score));\n$1    record::log_value::<states::Paused>(app, "pause", |p| i64::from(p.0));\n$1\}\n/m' "$MOD"
  fi
  grep -q 'pub mod record' "$MOD" || echo "HAND EDIT: declare 'pub mod record' in $MOD"
  grep -q 'record::RecordPlugin' "$MOD" || echo "HAND EDIT: add the RecordPlugin block next to AutopilotPlugin in $MOD"
else
  echo "HAND EDIT: no src/game/mod.rs; declare 'mod record' and add the RecordPlugin block where AutopilotPlugin is registered"
fi

# Autopilot shot().
AUTO="$GAME/src/game/autopilot.rs"
[ -f "$AUTO" ] || AUTO="$GAME/src/autopilot.rs"
if [ -f "$AUTO" ] && ! grep -q 'RecordBeat' "$AUTO"; then
  perl -0pi -e 's/^use bevy::render::view::screenshot::/#[cfg(not(feature = "record"))]\nuse bevy::render::view::screenshot::/m' "$AUTO"
  perl -0pi -e 's/fn shot\(commands: &mut Commands, name: &str\) \{\n    let path = format!\("\{\}\/\{name\}\.png", shot_dir\(\)\);\n    info!\("autopilot: screenshot \{path\}"\);\n    commands\n        \.spawn\(Screenshot::primary_window\(\)\)\n        \.observe\(save_to_disk\(path\)\);\n\}/fn shot(commands: &mut Commands, name: &str) {\n    #[cfg(feature = "record")]\n    {\n        info!("autopilot: beat {name}");\n        commands.write_message(super::record::RecordBeat(name.to_string()));\n    }\n    #[cfg(not(feature = "record"))]\n    {\n        let path = format!("{}\/{name}.png", shot_dir());\n        info!("autopilot: screenshot {path}");\n        commands\n            .spawn(Screenshot::primary_window())\n            .observe(save_to_disk(path));\n    }\n}/' "$AUTO"
fi
grep -q 'RecordBeat' "$AUTO" 2>/dev/null || echo "HAND EDIT: make shot() write RecordBeat under cfg(feature = \"record\") in ${AUTO:-the autopilot file}"

# SfxEvent must be Debug for log_messages.
SFX="$(grep -rl 'pub enum SfxEvent' "$GAME/src" | head -1 || true)"
if [ -n "$SFX" ]; then
  perl -0pi -e 's/#\[derive\(([^)]*)\)\]\npub enum SfxEvent/my $d=$1; $d =~ \/\\bDebug\\b\/ ? "#[derive($d)]\npub enum SfxEvent" : "#[derive($d, Debug)]\npub enum SfxEvent"/e' "$SFX"
else
  echo "HAND EDIT: no 'pub enum SfxEvent' found; drop or retarget the log_messages line"
fi

echo "rollout-record: files in place for $GAME"
echo "next: (cd $GAME && cargo check --features record && cargo clippy --all-targets --all-features -- -D warnings)"
```

`chmod +x tools/rollout-record.sh`.

- [ ] **Step 2: Run it against Tire Stack and check**

Run: `tools/rollout-record.sh ../../games/tire-stack && cd ../../games/tire-stack && git diff --stat && cargo check --features record && cargo clippy --all-targets --all-features -- -D warnings && cargo test --all-features record::tests`
Expected: no `HAND EDIT` lines; diff touches `Cargo.toml`, `src/game/mod.rs`, `src/game/autopilot.rs`; check and clippy clean; 6 tests pass. If `scoring::GameData` has no `score` field or `states::Paused` is missing in this game, edit the `log_value` lines to the game's real types (this is the expected per-game fix-up).

- [ ] **Step 3: Re-run for idempotency**

Run: `cd ../../libs/gamebient-bevy-template && tools/rollout-record.sh ../../games/tire-stack && cd ../../games/tire-stack && git diff --stat`
Expected: identical diff stat to Step 2 (no duplicated blocks).

- [ ] **Step 4: Record Tire Stack**

Run: `cd games/tire-stack && tools/record.sh && cat build/record/chapters.md`
Expected: `tour.mp4` at 1920x1080, `SfxEvent` lines present in `events.jsonl` (`grep -c SfxEvent build/record/events.jsonl` > 0), chapters table shows `state Playing`, beats, and score milestones.

- [ ] **Step 5: Commit both repos**

```bash
cd libs/gamebient-bevy-template
git add tools/rollout-record.sh
git commit -m "feat(record): rollout script for game repos

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
cd ../../games/tire-stack
git add Cargo.toml src/game/record.rs src/game/mod.rs src/game/autopilot.rs tools/record.sh tools/cut_clips.py tools/test_cut_clips.py
git commit -m "feat(record): offline footage recorder from the template

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 6: The `recording-game-footage` skill

**Files:**
- Create: `<workspace>/.claude/skills/recording-game-footage/SKILL.md`
- Create: `.../references/notes-template.md`
- Create: `.../references/irregular-games.md`
- Create: `.../references/catalog-2026-09-09.md`

**Interfaces:**
- Consumes: `tools/rollout-record.sh`, `tools/record.sh`, `build/record/*` from Tasks 4-5.
- Produces: the procedure that Task 7-9 executors follow; `docs/video-notes.md` per game.

- [ ] **Step 1: Write `SKILL.md`**

```markdown
---
name: recording-game-footage
description: Use when a Gamebient game needs play-video footage, trailer clips, or video notes; when the user says "record footage", "make the play video", "video notes", or "roll out the recorder"; or when a game lacks docs/video-notes.md or its notes predate a gameplay change
---

# Recording Game Footage

## Overview

Every template-derived game can record its own autopilot tour offline:
`tools/record.sh` runs the game with `--features record` on a fixed 1/60 s
clock, captures every frame, encodes `build/record/tour.mp4` (60 fps,
1920x1080), cuts `clips/<beat>.mp4` and `shots/<beat>.png` around each
autopilot beat, and writes `events.jsonl`, `manifest.json`, `chapters.md`.
The skill's job is to get a game to that point, verify it, and turn the
result plus a code read into `docs/video-notes.md` for whoever edits the
video. The recorder lives in the template
(`libs/gamebient-bevy-template/src/game/record.rs`, design spec
`docs/superpowers/specs/2026-09-09-recording-harness-design.md`).

Supporting files: [notes-template.md](references/notes-template.md) (the
section skeleton), [irregular-games.md](references/irregular-games.md)
(games whose layout differs from the template),
[catalog-2026-09-09.md](references/catalog-2026-09-09.md) (per-game beats
already researched for the 16 published games; a starting corpus, verify
against the code before reuse).

## Process

1. **Roll out if missing.** `grep -q '^record = ' games/<g>/Cargo.toml` or
   run `libs/gamebient-bevy-template/tools/rollout-record.sh games/<g>`.
   Read every `HAND EDIT` line it prints and fix them (see
   irregular-games.md). Then in the game:
   `cargo check --features record && cargo clippy --all-targets --all-features -- -D warnings && cargo test --all-features`.
   Commit: `feat(record): offline footage recorder from the template`.
2. **Record.** `tools/record.sh` in the game dir. First release build takes
   minutes; the tour itself takes 3-6 minutes because every frame is read
   back from the GPU. Do not run two recordings at once.
3. **Verify** before writing anything:
   - `manifest.json` `frames` is within 5 of
     `ffprobe -v error -select_streams v -show_entries stream=nb_frames -of csv=p=0 build/record/tour.mp4`;
   - every beat in `manifest.json` has `clips/<beat>.mp4` and `shots/<beat>.png`;
   - `grep -c SfxEvent build/record/events.jsonl` is greater than zero (or the
     game's equivalent message);
   - Read `shots/06-*.png` and `shots/05-mid-play.png` as images: gameplay
     is visible, HUD shows a score or progress, no letterbox band. If the
     bot idles or the signature beat never fires, fix
     `src/game/autopilot.rs` first (the `designing-cartridge-covers` skill's
     autopilot.md covers this) and re-record.
4. **Write `docs/video-notes.md`** from notes-template.md. Paste
   `chapters.md`'s table verbatim. Fill the editorial sections by reading
   the README, `docs/superpowers/specs/*-design.md`, the gameplay `src/game/*.rs`
   files that define scoring, waves, levels, bosses, upgrades, and any
   copy/flavor-text file. Every claim cites a file. Quote flavor text
   verbatim. Numbers (thresholds, counts, timings) come from constants in
   the code, not from memory.
5. **Report.** Clip paths, the lead clip you'd cut first, and everything the
   tour did not reach (a boss, a fever mode, a rare drop) as candidates for
   a custom recording scenario. Commit `docs/video-notes.md`:
   `docs: video notes for <Game>`.

## Common mistakes

| Mistake | Fix |
|---------|-----|
| Recording with `cargo run --features autopilot` | That saves nine stills only. Use `tools/record.sh`. |
| Two `Screenshot`s in one frame | Under `record`, `shot()` must write `RecordBeat`, never spawn a Screenshot; Bevy drops the duplicate. |
| Notes written from the catalog file without checking the code | The catalog is a 2026-09-09 snapshot. Re-verify constants and names in `src/`. |
| Frame count far below expected | The run crashed or the bot skipped a state; read the cargo output, check `events.jsonl`'s last lines. |
| Committing `build/record` | It is gitignored; never force-add it. Deliver clips by path or SendUserFile. |
| Running on the web build | The recorder is native-only; web builds never include it. |
```

- [ ] **Step 2: Write `references/notes-template.md`**

```markdown
# <Game> — video notes

Recorded <YYYY-MM-DD> with `tools/record.sh` (tour: `build/record/tour.mp4`).

## Hook

One sentence a viewer hears in the first two seconds: the premise, not the genre.

## Lead clips

1. `clips/<beat>.mp4` — why it goes first
2. `clips/<beat>.mp4` — …

## Chapters (auto)

<paste build/record/chapters.md's table>

## Controls

| Action | Keys | Source |
|---|---|---|

## Escalation

What changes as the run goes on, with the real numbers (waves, levels, timers, caps). Cite the constant.

## Named content

Enemies, upgrades, holes, missions, cards, flavors: list them with the file they come from.

## Spectacle moments

| Moment | When it happens | Reached by the tour? |
|---|---|---|

## Flavor text to caption

Verbatim quotes with file:line.

## Hidden mechanics

Tricks a casual player misses; exploits; debug keys.

## Rough edges to keep off camera

Placeholder art, known bugs, stale metadata on the demo page.

## Not reached by the tour

Payoffs the autopilot never gets to (boss, fever, rare drop) and how a custom scenario would reach them.

## Sources

Files read for this note.
```

- [ ] **Step 3: Write `references/irregular-games.md`**

```markdown
# Games whose layout differs from the template

`rollout-record.sh` assumes `src/game/{mod,autopilot,states,scoring}.rs` and
`src/game/audio/mod.rs::SfxEvent`. These games need hand edits after the
script runs; `cargo check --features record` names the exact lines.

| Game | Difference | What to do |
|---|---|---|
| `Hunted` | No `src/game/`; `autopilot.rs`, `audio.rs`, `GameState` live under `src/`; `AutopilotPlugin` is registered in `main.rs`; no `GameData` score, a run timer instead | Copy `record.rs` to `src/record.rs`; declare `mod record;` in `main.rs`; add the plugin block next to `AutopilotPlugin` using `crate::GameState`, `crate::audio::SfxEvent`; drop the score line; in `shot()` use `crate::record::RecordBeat` |
| `pack-the-ripper` | `autopilot.rs` under `src/`, registered in `main.rs`; `GameData` and `SfxEvent` are under `src/game/` | Keep `record.rs` in `src/game/`; declare and wire it from `main.rs`; in `src/autopilot.rs` use `crate::game::record::RecordBeat` |
| `voidrunner` | No `SfxEvent` enum (`src/assets/audio.rs` holds `SfxAssets` only); score arrives via `ScoreEvent` | Drop the `log_messages::<SfxEvent>` line; add `record::log_messages::<scoring::ScoreEvent>(app)` (derive `Debug` on it) |
| `Gravestone_Gauntlet` | `SfxEvent` in `src/assets/audio.rs`; score in `ScoreBoard`, not `GameData` | `record::log_messages::<crate::assets::audio::SfxEvent>(app)`; `log_value::<scoring::ScoreBoard>(app, "score", |s| i64::from(s.score))` (check the field name) |
| `BeerPong` (Table Titans) | Two-player: `MatchData` with per-player cups, no single score | Log `MatchData` twice: `"cups_p1"` / `"cups_p2"` remaining |

Every other published game (attic-excavator, cannonball-putt, dive-rise,
dough-io, grand-theft-auto-reply, grand-theft-otto, gulper, ladder-legend,
pizza-pinball, sundae-shooter, tire-stack) matches the template layout; the
script needs no follow-up beyond `cargo check`.
```

- [ ] **Step 4: Write `references/catalog-2026-09-09.md`**

The per-game beats researched on 2026-09-09 from each game's README, design specs and source. Verify constants against `src/` before reusing.

```markdown
# Published catalog — video beats (snapshot 2026-09-09)

16 games on chain (Ladder Legend is listed twice in /library under two PDAs).
Tire Stack's on-chain description is still the template placeholder.

## Voidrunner — 3D neon rail shooter
- Three-phase loop: corridor (rifts, sentinels, drift cells, speed pads), open-space waves, boss every 3rd level (`src/game/level.rs is_boss_level`).
- Wraiths: Flankers, Divers, Phasers (blink), Bombers; Void Echoes fire a parting shot (`src/game/enemies.rs`).
- Void Leviathan: tendril tips 500, cores 1000, kill 5000, rage phase (`src/game/boss.rs`, README scoring). Extra life every 50,000.
- Five authored music tracks (menu, corridor, combat, boss, game over) — keep the audio.

## Gravestone Gauntlet — gravity-flip wave survival
- Flip floor/ceiling instantly, afterimage trail, ~200 ms invincibility you kill through (design doc).
- Enemies unlock per wave: Drifter, Bouncer, Tracker, Gunner (barrel glows first), Splitter (`src/game/enemies.rs`).
- Combo to x10 for kills under 1 s apart; bosses at waves 10/20/30: The Sentinel, The Mirror, The Storm (`src/game/boss.rs`, `waves.rs`).

## Hunted — first-person survival horror
- Flashlight cone only; sprint is audible from twice the distance; creatures track footsteps (`src/creatures.rs`, `src/audio.rs`).
- Three creatures as eyes + light-absorbing shape: deep red (wet breathing), orange (bone scraping), crimson (insect clicking).
- Staff pickup flips to HUNT: creatures flee, flashlight tints blue-purple, auto-aim bolts light corridors; three keys, exit door.
- "YOU WERE FOUND" / "YOU ESCAPED" with "NEW BEST TIME" (`src/screens.rs`); F3 debug minimap for a reveal shot.

## Pizza Pinball
- Plunger, pepperoni/olive/pepper bumpers, drain at the slice tip. Combo window 2 s, cap x5, ding pitch rises a tone per level (`src/game/scoring.rs`, juice-pass-2 spec).
- Extra balls at 5,000 / 20,000 / 50,000 (`EXTRA_BALL_AT`); ball-save after launch; "NEW HIGH SCORE!"; procedural chiptune loops.

## Table Titans (BeerPong) — two-player hot-seat cup pong
- Hold Z to charge, X instant throw; ten cups a side. Streak popups: SPLASH!, DOUBLE DIP!, HEATING UP!, ON FIRE!; trail turns to fire at streak 3 (juice-pass spec, `src/game/fx.rs`).
- Party lights, neon table strips, orbiting menu camera, confetti on the winner screen. Film with a real second player.

## Tire Stack — catch, stack, deliver
- Off-center catches shift the center of mass; inverted-pendulum lean; drive under it to save. Topple past ~0.35 rad (`src/game/stack.rs TIP_ANGLE`).
- Delivery pays 10·n(n+1)/2 × combo (cap x5); stack cap 8; X dumps safely. Types: standard, heavy truck (1.8x lean, 2x pts), kart (fast, 2x pts). Bay pile ships out at 12.

## Ladder Legend — agility-ladder rhythm
- Perfect ≤60 ms, Good ≤120, OK ≤180; Miss −12 HP, Perfect +2 (`src/game/judgment.rs`, `scoring.rs`).
- Drills: FORWARD RUN, SIDE SHUFFLE, IN & OUT, CROSSOVER, ICKY SHUFFLE, ALI SHUFFLE, QUICK FEET (`src/game/chart.rs`). BPM 90 + 4/bar to 170.
- FEVER at combo 32: x5, bloom and pulse amplified (`FEVER_COMBO`, `src/game/pulse.rs`). The autopilot tour reaches FEVER.

## Attic Excavator — Dig Dug in an attic
- Four strata; tiers MARBLE BAG 100, TIN ROBOT 250, MODEL TRAIN 500, ARCADE CART 1000, GRAIL TOY 2500 + 500/level (design spec, `src/game/treasure.rs`).
- Cats wake when a tunnel connects; camera flash stuns 2.5 s on a 5 s cooldown; undermine heavy junk to crush a cat for 500 (`src/game/heavy.rs`, `cat.rs`).
- "ATTIC CLEARED" banner; cat count 1 + (level−1), cap 4.

## Cannonball Putt — nine-hole arcade golf
- Two-tap shot (aim, stop the meter); stroke cap 8; water +1.
- Holes: The Plank, Lagoon Bend, The Falls, Smuggler's Cave, Sandy Shoals, Twin Falls, The Grotto, Shipwreck Deck (tilting), Captain's Gauntlet (`src/game/course.rs`).
- Every hole has a "find it on the real course" plaque; cut to the real hole if it exists. Ends on a scorecard.

## Dive Rise — survivors-like about the diel vertical migration
- Three cycles of ~6 min: dusk, night feast, dawn escape, day refuge. Three kits: lanternfish, hatchetfish, firefly squid.
- Behaviors: Photophores, Swim Burst, Schooling, Filter Feeding, Ink Cloud, Lure, Spines, Lateral Line; companions: Krill Swarm, Hatchetfish, Jellyfish, Vampire Squid, Siphonophore, Cleaner Shrimp; combos: Counter-Illuminated Shoal, False Dawn, Feeding Frenzy, Living Net, Venom Spines (`src/game/behaviors/`, `companions/`, `combos.rs`).
- Bosses: squid pack night 1, tuna at every dawn, dragonfish/anglerfish by day, swordfish night 2, whale at the final dawn (`src/game/events/`). `cargo run -- --playtest --start-at 1024` jumps to the whale.
- Real fact cards (`src/game/facts.rs`): read one aloud.

## Dough.io — agar.io on a bakery floor
- Body-language telegraphs: "Trembling? Snack. Jaws open? Run!"
- Ingredient pairs auto-bake: Baguette (homing javelin), Croissant (bodyguard, bites back), Pretzel (trailing snare), Brioche (shockwave spare life), Sourdough (foraging pet) (`src/game/minions.rs`, `announce.rs`).
- Toasts: "CROISSANT BLOCKED & BIT BACK!", "BRIOCHE BLAST! YOU'RE SAVED!", "CLOSE CALL!"; size titles DINNER ROLL → BATARD → BOULE → MEGALOAF (`milestones.rs`). Combo x5, golden crumbs, giants decay above radius 6.

## Grand Theft Auto-Reply — inbox crime sim
- Synergy Dynamics LLC; HR are the cops. Crimes unlock: Thanks!, Gentle Reminder, Reply All, k., Out of Office, CC the CEO, Per My Last Email (screen goes red) (`src/game/crimes.rs`).
- Wanted captions: Model employee → Noted by HR → A concerned Slack DM → Calendar invite: Quick chat? → Performance Improvement Plan → HR RAID (10 s to get under 4 stars or FIRED). Combo callouts SYNERGY … UNHINGED (`src/game/copy.rs`).
- Emails to read on camera: "the 3rd floor printer has been renamed 'Gary'. Please respect Gary."; "Anything unlabeled will be discarded. Anything labeled will also be discarded."; boss "RE(38): RE: RE: RE: Lunch?" from ALL COMPANY (38,112).
- Ending: "YOU HAVE BEEN PROMOTED TO MIDDLE MANAGEMENT." `--playtest --mission 5` starts at the boss.

## Grand Theft Otto — GTA parody where you steal nothing
- Otto Kleinschmidt, 52, tax auditor; hold Z to document, 8 pages, file at the Finanzamt (x1.7 for a full notebook) (`src/game/notebook.rs`, `filing.rs`).
- Your stars come only from your own infractions: jaywalking +100, red light +60, grass +30 then +15/s, tram leap +100; police at 3 stars, arrest at 5 (`src/game/wanted.rs`, `infractions.rs`).
- Crimes with citations: Unlicensed Currywurst "no permit, suspicious sauce", Tax Evasion "Otto's white whale" (`src/game/crimes/kinds.rs`). Otto: "Ordnung muss sein.", "I steal nothing. I see everything."
- Arrest report: "Vehicles stolen: 0, Money stolen: 0", "Subject's statement: 'I was only documenting.'" (`src/ui/game_over.rs`). `--playtest --policy reckless` gets arrested fast.

## Gulper — Snake meets racecar tuner
- Hold Z to unhinge the jaws; segments are health; lure on the tail tip.
- Species by size: krill, bristlemouth, hatchetfish, lanternfish, jelly, glass squid, snipe eel, rival gulper, viperfish, barracudina, anglerfish (fake lure), leviathan (`src/game/creatures/species.rs`).
- Digest screen: Speed, Thrust, Mouth, Stomach, Lure, Armor; spec sheet with mass, power-to-weight, burn rate; costs x1.6 per level.
- Deaths: "EATEN", "STARVED IN THE DARK" (`src/game/death.rs`). `--playtest --start-band 3` skips deep.

## Pack The Ripper — booster-pack sorting
- Mash Z to rip; Left/Down/Right into COMMONS/RARES/FOILS; three Collector Trust strikes.
- Odds: common 70%, rare 24%, foil 5%, chase 1% = $500 with slow-mo, gold flash, confetti (`src/game/cards.rs`, `juice.rs`).
- Names: "GRUBLIN, THE MOIST", "SKRUNKLE, DESTROYER OF SLEEVES", "GORP, BANNED IN THREE FORMATS", "BLIMBUS, FIRST EDITION (TRUST ME)".
- Streak to x5 "MAX!"; "THE COLLECTORS HAVE LOST FAITH" appraisal on game over.

## Sundae Shooter — Puzzle Bobble meets Overcooked
- Aim the cone cannon; wrong flavor −10; finished sundae +150 with a cherry and a pop.
- Flavors Vanilla, Chocolate, Strawberry, Mint, Blueberry grow across levels; bowls 4 + level (cap 10); recipes to 4 scoops; timer red under 10 s (`src/game/level.rs`, `flavor.rs`).
- Combo x5 with rising dings, confetti pop, wall-bounce bank shots along the dotted guide; "LEVEL CLEAR" time bonus.
```

- [ ] **Step 5: Verify the skill loads**

Run: `ls -R <workspace>/.claude/skills/recording-game-footage && head -4 <workspace>/.claude/skills/recording-game-footage/SKILL.md`
Expected: four files; frontmatter has `name` and `description`. (The workspace root is not a git repo; nothing to commit.)

---

### Task 7: Roll out to the eleven template-shaped games

**Files:**
- In each of `games/{attic-excavator, cannonball-putt, dive-rise, dough-io, grand-theft-auto-reply, grand-theft-otto, gulper, ladder-legend, pizza-pinball, sundae-shooter}` (tire-stack done in Task 5): the same four files and three edits as Task 5.

**Interfaces:**
- Consumes: `tools/rollout-record.sh`.
- Produces: each game compiles with `--features record` and is committed.

- [ ] **Step 1: Run the rollout loop**

```bash
cd <workspace>
for g in attic-excavator cannonball-putt dive-rise dough-io grand-theft-auto-reply grand-theft-otto gulper ladder-legend pizza-pinball sundae-shooter; do
  echo "=== $g"
  libs/gamebient-bevy-template/tools/rollout-record.sh games/$g
done
```

Expected: no `HAND EDIT` lines. Any that appear: fix per `irregular-games.md`.

- [ ] **Step 2: Check each game**

```bash
for g in attic-excavator cannonball-putt dive-rise dough-io grand-theft-auto-reply grand-theft-otto gulper ladder-legend pizza-pinball sundae-shooter; do
  echo "=== $g"
  (cd games/$g && cargo check --features record 2>&1 | tail -3 && cargo clippy --all-targets --all-features -- -D warnings 2>&1 | tail -2 && cargo test --all-features record::tests 2>&1 | grep 'test result')
done
```

Expected: every game clean, 6 tests pass. Typical fix-ups: `GameData` has no `score` (retarget the `log_value` line to the game's score holder, e.g. `ScoreBoard`, `RunStats.banked`, `MatchData`), `Paused` lives elsewhere (drop the line), `SfxEvent` lacks `Debug` in a derive the script's regex missed (add it).

- [ ] **Step 3: Commit each game**

```bash
for g in attic-excavator cannonball-putt dive-rise dough-io grand-theft-auto-reply grand-theft-otto gulper ladder-legend pizza-pinball sundae-shooter; do
  (cd games/$g && git add -A Cargo.toml src tools && git commit -q -m "feat(record): offline footage recorder from the template

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>" && git log --oneline -1)
done
```

Expected: ten new commits. Do not push; the owner pushes.

---

### Task 8: Roll out to the irregular games by hand

**Files:**
- `games/Hunted`: create `src/record.rs`; modify `Cargo.toml`, `src/main.rs`, `src/autopilot.rs`, `src/audio.rs`; add `tools/`.
- `games/pack-the-ripper`: create `src/game/record.rs`; modify `Cargo.toml`, `src/main.rs`, `src/game/mod.rs`, `src/autopilot.rs`, `src/game/audio/mod.rs`; add `tools/`.
- `games/voidrunner`: create `src/game/record.rs`; modify `Cargo.toml`, `src/game/mod.rs`, `src/game/autopilot.rs`, `src/game/scoring.rs`; add `tools/`.
- `games/Gravestone_Gauntlet`: create `src/game/record.rs`; modify `Cargo.toml`, `src/game/mod.rs`, `src/game/autopilot.rs`, `src/assets/audio.rs`; add `tools/`.
- `games/BeerPong`: create `src/game/record.rs`; modify `Cargo.toml`, `src/game/mod.rs`, `src/game/autopilot.rs`; add `tools/`.

**Interfaces:**
- Consumes: `tools/rollout-record.sh` (for the file copies and whatever edits match), `references/irregular-games.md`.
- Produces: five more games compiling with `--features record`.

- [ ] **Step 1: Run the script and collect the HAND EDIT lines**

```bash
cd <workspace>
for g in Hunted pack-the-ripper voidrunner Gravestone_Gauntlet BeerPong; do echo "=== $g"; libs/gamebient-bevy-template/tools/rollout-record.sh games/$g; done
```

- [ ] **Step 2: Hunted**

`record.rs` was copied to `src/game/record.rs` into a `src/game/` dir that does not otherwise exist: move it to `src/record.rs` and delete the empty dir. In `src/main.rs`: `#[cfg(feature = "record")] mod record;` beside the autopilot module line, and next to `app.add_plugins(autopilot::AutopilotPlugin);`:

```rust
    #[cfg(feature = "record")]
    {
        app.add_plugins(record::RecordPlugin);
        record::log_state::<GameState>(app);
        record::log_messages::<audio::SfxEvent>(app);
    }
```

In `src/autopilot.rs` `shot()` use `crate::record::RecordBeat`. Ensure `SfxEvent` in `src/audio.rs` derives `Debug`. If Hunted has a `Paused` resource, add `record::log_value::<Paused>(app, "pause", |p| i64::from(p.0));` with its real path.

- [ ] **Step 3: pack-the-ripper**

Keep `src/game/record.rs` (the script declared `pub mod record` in `src/game/mod.rs`, which is right). The plugin block goes in `src/main.rs` next to `AutopilotPlugin` with `game::record::…` paths and `game::states::GameState`, `game::audio::SfxEvent`, `game::scoring::GameData`. In `src/autopilot.rs` `shot()` use `crate::game::record::RecordBeat`.

- [ ] **Step 4: voidrunner**

Remove the `log_messages::<audio::SfxEvent>` line the script added (no such enum). Add `#[derive(Debug)]` to `ScoreEvent` in `src/game/scoring.rs` if missing and use `record::log_messages::<scoring::ScoreEvent>(app);`. Keep the `score` `log_value` line only if `GameData.score` exists; otherwise point it at the real score resource.

- [ ] **Step 5: Gravestone_Gauntlet**

Change the log lines to `record::log_messages::<crate::assets::audio::SfxEvent>(app);` and `record::log_value::<scoring::ScoreBoard>(app, "score", |s| i64::from(s.score));` (confirm the field name with `grep -n 'pub struct ScoreBoard' -A6 src/game/scoring.rs`). Ensure that `SfxEvent` derives `Debug`.

- [ ] **Step 6: BeerPong (Table Titans)**

Replace the `score` line with two lines reading the match state (`grep -n 'pub struct MatchData' -A10 src/game/scoring.rs` for the field names), e.g.
`record::log_value::<scoring::MatchData>(app, "cups_p1", |m| i64::from(m.cups_p1));` and the `p2` twin.

- [ ] **Step 7: Check and commit all five**

```bash
for g in Hunted pack-the-ripper voidrunner Gravestone_Gauntlet BeerPong; do
  echo "=== $g"
  (cd games/$g && cargo check --features record && cargo clippy --all-targets --all-features -- -D warnings && cargo test --all-features record::tests | grep 'test result' \
   && git add -A Cargo.toml src tools && git commit -q -m "feat(record): offline footage recorder from the template

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>" && git log --oneline -1)
done
```

Expected: five clean checks and five commits. Also update `irregular-games.md` with anything the code contradicted.

---

### Task 9: First catalog run — footage and notes for all 16 games

**Files:**
- Create `games/<g>/docs/video-notes.md` for each of the 16 published games.

**Interfaces:**
- Consumes: the skill from Task 6, the rollouts from Tasks 5, 7, 8.

- [ ] **Step 1: Record and write notes, one game at a time**

For each game, in this order (short tours first): tire-stack, pizza-pinball, sundae-shooter, pack-the-ripper, ladder-legend, attic-excavator, cannonball-putt, dough-io, gulper, grand-theft-auto-reply, grand-theft-otto, dive-rise, voidrunner, Gravestone_Gauntlet, Hunted, BeerPong:

1. Follow the `recording-game-footage` skill, steps 2-5 (record, verify, notes, report).
2. Confirm with `ls build/record/clips | wc -l` (≥ 9) and `test -s docs/video-notes.md`.
3. Commit `docs/video-notes.md` as `docs: video notes for <Game>`.

Expected per game: verification passes; notes cite files; the "Not reached by the tour" section lists real gaps (Voidrunner level-3 boss, Gravestone wave-10 boss, Dive Rise whale, Pack The Ripper chase card, etc.).

- [ ] **Step 2: Summarize**

Write `<workspace>/docs/video-notes-index.md`: a table of game, tour length, lead clip path, and the top "not reached" payoff, so the owner has one place to start editing from.

---

## Self-review

- **Spec coverage:** capture (T1-2), assembly (T3-4), rollout (T5, T7, T8), skill (T6), tests and failure modes (T1 tests, T3 tests, `record.sh` guards, CI via `--all-features`), success criteria 1-3 (T4 step 3, T5 step 4, T9). Deviation from spec: clips are always re-encoded rather than stream-copied; cheaper to reason about and clips are six seconds. `shots/<beat>.png` are extracted by the cutter because a second `Screenshot` per frame is dropped by Bevy; the autopilot contract's file names are preserved.
- **Placeholders:** none; every code step is complete.
- **Type consistency:** `Recorder::log(&mut self, kind: &str, data: &str)`, `json_obj(&[(&str, String)])`, `RecordBeat(pub String)`, `log_value(app, kind: &'static str, read: fn(&R) -> i64)` are used identically in T2, T5's script, and T8.
