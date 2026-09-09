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

use std::path::PathBuf;

/// Frames per second of the recording and the sim step (1/FPS s per frame).
#[allow(dead_code)]
pub const FPS: u32 = 60;

/// Output root; override with `RECORD_DIR`. `build/` is gitignored.
#[allow(dead_code)]
pub fn record_dir() -> PathBuf {
    std::env::var("RECORD_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("build/record"))
}

/// `frames/<this>`; six digits so ffmpeg's glob sorts them.
#[allow(dead_code)]
pub fn frame_filename(frame: u64) -> String {
    format!("{frame:06}.png")
}

#[allow(dead_code)]
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
#[allow(dead_code)]
pub fn json_str(s: &str) -> String {
    format!("\"{}\"", json_escape(s))
}

/// A JSON object from already-rendered values (`json_str`, numbers as text).
#[allow(dead_code)]
pub fn json_obj(fields: &[(&str, String)]) -> String {
    let body: Vec<String> = fields
        .iter()
        .map(|(k, v)| format!("{}:{v}", json_str(k)))
        .collect();
    format!("{{{}}}", body.join(","))
}

/// One `events.jsonl` line. `data` is an already-rendered JSON object.
#[allow(dead_code)]
pub fn event_line(frame: u64, fps: u32, kind: &str, data: &str) -> String {
    let t = frame as f64 / f64::from(fps);
    format!(
        "{{\"frame\":{frame},\"t\":{t:.3},\"kind\":{},\"data\":{data}}}",
        json_str(kind)
    )
}

/// What `manifest.json` carries; written when the tour exits.
#[allow(dead_code)]
pub struct Manifest {
    pub name: String,
    pub fps: u32,
    pub width: u32,
    pub height: u32,
    pub frames: u64,
    pub beats: Vec<(String, u64)>,
}

#[allow(dead_code)]
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
#[allow(dead_code)]
pub fn name_from_info_json(text: &str) -> Option<String> {
    let idx = text.find("\"name\"")?;
    let rest = &text[idx + "\"name\"".len()..];
    let rest = rest.trim_start().strip_prefix(':')?.trim_start();
    let rest = rest.strip_prefix('"')?;
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
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
