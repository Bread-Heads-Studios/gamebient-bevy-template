//! Parse models for the content files, with the size limits and patterns the
//! JSON schemas in `content/schema/` state. Every `parse` returns one plain
//! sentence on failure so the editor can show it as is.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize, de::DeserializeOwned};

pub const LEVEL_MAX_BYTES: usize = 256 * 1024;
pub const LEVEL_MAX_THINGS: usize = 2000;
pub const TUNABLES_MAX_BYTES: usize = 64 * 1024;
pub const SPRITES_MAX_BYTES: usize = 16 * 1024;
pub const SONGS_MAX_BYTES: usize = 128 * 1024;
pub const SFX_MAX_BYTES: usize = 64 * 1024;

/// One plain-English sentence ending with a period.
#[derive(Debug, Clone, PartialEq)]
pub struct ContentError(pub String);

impl std::fmt::Display for ContentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ContentError {}

fn err<T>(msg: impl Into<String>) -> Result<T, ContentError> {
    let mut s: String = msg.into();
    if !s.ends_with('.') {
        s.push('.');
    }
    Err(ContentError(s))
}

fn parse_json<T: DeserializeOwned>(
    bytes: &[u8],
    max: usize,
    what: &str,
) -> Result<T, ContentError> {
    if bytes.len() > max {
        return err(format!("The {what} file is larger than {} KB", max / 1024));
    }
    serde_json::from_slice(bytes).or_else(|e| {
        err(format!(
            "The {what} file could not be read ({})",
            e.to_string().trim_end_matches('.')
        ))
    })
}

fn check_v(v: u8, what: &str) -> Result<(), ContentError> {
    if v == 1 {
        Ok(())
    } else {
        err(format!("The {what} file must have \"v\" set to 1"))
    }
}

/// `^[a-z0-9_-]{1,32}$`
pub fn valid_id(s: &str) -> bool {
    (1..=32).contains(&s.len())
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
}

/// `^[A-Za-z][A-Za-z0-9_-]{0,31}$`
pub fn valid_kind(s: &str) -> bool {
    let b = s.as_bytes();
    (1..=32).contains(&b.len())
        && b[0].is_ascii_alphabetic()
        && b[1..]
            .iter()
            .all(|c| c.is_ascii_alphanumeric() || *c == b'_' || *c == b'-')
}

fn pos_pair(p: [f32; 2]) -> bool {
    p.iter().all(|v| v.is_finite() && *v > 0.0)
}

// ---- level ----------------------------------------------------------------

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct LevelFile {
    pub v: u8,
    pub things: Vec<PlacedThing>,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PlacedThing {
    pub id: String,
    pub kind: String,
    pub at: [f32; 2],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<[f32; 2]>,
}

impl LevelFile {
    pub fn parse(bytes: &[u8]) -> Result<LevelFile, ContentError> {
        let f: LevelFile = parse_json(bytes, LEVEL_MAX_BYTES, "level")?;
        check_v(f.v, "level")?;
        if f.things.len() > LEVEL_MAX_THINGS {
            return err("A level holds at most 2000 things.");
        }
        let mut seen = std::collections::BTreeSet::new();
        for t in &f.things {
            if !valid_id(&t.id) {
                return err(format!(
                    "The id \"{}\" must be 1 to 32 characters of a-z, 0-9, _ or -",
                    t.id
                ));
            }
            if !seen.insert(t.id.as_str()) {
                return err(format!(
                    "The id \"{}\" is used by more than one thing",
                    t.id
                ));
            }
            if !valid_kind(&t.kind) {
                return err(format!(
                    "The kind \"{}\" must start with a letter and use at most 32 letters, digits, _ or -",
                    t.kind
                ));
            }
            if !t.at.iter().all(|v| v.is_finite()) {
                return err(format!(
                    "The thing \"{}\" has a position that is not a number",
                    t.id
                ));
            }
            if let Some(s) = t.size
                && !pos_pair(s)
            {
                return err(format!("The thing \"{}\" needs a size above zero", t.id));
            }
        }
        Ok(f)
    }
}

// ---- tunables -------------------------------------------------------------

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TunablesFile {
    pub v: u8,
    pub values: BTreeMap<String, serde_json::Value>,
}

impl TunablesFile {
    pub fn parse(bytes: &[u8]) -> Result<TunablesFile, ContentError> {
        let f: TunablesFile = parse_json(bytes, TUNABLES_MAX_BYTES, "settings")?;
        check_v(f.v, "settings")?;
        for (k, val) in &f.values {
            if k.is_empty() || k.len() > 128 {
                return err("A setting name must be 1 to 128 characters long");
            }
            use serde_json::Value as V;
            let ok = match val {
                V::Bool(_) | V::Number(_) | V::String(_) => true,
                V::Array(a) => a.iter().all(V::is_number),
                V::Null | V::Object(_) => false,
            };
            if !ok {
                return err(format!(
                    "The setting \"{k}\" must be a number, text, true or false, or a list of numbers"
                ));
            }
        }
        Ok(f)
    }
}

// ---- sprites --------------------------------------------------------------

fn one() -> u16 {
    1
}
fn eight() -> f32 {
    8.0
}
fn unit() -> f32 {
    1.0
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SpritesFile {
    pub v: u8,
    pub sprites: BTreeMap<String, SpriteMeta>,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SpriteMeta {
    #[serde(default = "one")]
    pub frames: u16,
    #[serde(default = "eight")]
    pub fps: f32,
    #[serde(default = "unit")]
    pub scale: f32,
    #[serde(default)]
    pub anchor: Option<[f32; 2]>,
}

impl SpritesFile {
    pub fn parse(bytes: &[u8]) -> Result<SpritesFile, ContentError> {
        let f: SpritesFile = parse_json(bytes, SPRITES_MAX_BYTES, "sprites")?;
        check_v(f.v, "sprites")?;
        for (k, s) in &f.sprites {
            if !valid_kind(k) {
                return err(format!("The sprite name \"{k}\" is not a valid kind name"));
            }
            if !(1..=256).contains(&s.frames) {
                return err(format!("The sprite \"{k}\" needs 1 to 256 frames"));
            }
            if !(s.fps.is_finite() && s.fps > 0.0 && s.fps <= 120.0) {
                return err(format!(
                    "The sprite \"{k}\" needs a speed above 0 and up to 120 frames per second"
                ));
            }
            if !(s.scale.is_finite() && s.scale > 0.0 && s.scale <= 64.0) {
                return err(format!(
                    "The sprite \"{k}\" needs a scale above 0 and up to 64"
                ));
            }
            if let Some(a) = s.anchor
                && !a.iter().all(|v| v.is_finite() && (0.0..=1.0).contains(v))
            {
                return err(format!(
                    "The sprite \"{k}\" needs an anchor between 0 and 1"
                ));
            }
        }
        Ok(f)
    }
}

// ---- songs ----------------------------------------------------------------

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SongsFile {
    pub v: u8,
    pub songs: BTreeMap<String, SongData>,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SongData {
    pub bpm: f32,
    pub beats_per_bar: u8,
    pub steps_per_beat: u8,
    pub bars: u8,
    pub voices: [String; 3],
    #[serde(default)]
    pub noise: String,
    pub instruments: Vec<InstrumentData>,
    #[serde(default)]
    pub noise_instruments: Vec<InstrumentData>,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct InstrumentData {
    pub env: Vec<u8>,
    #[serde(default)]
    pub release: u8,
    #[serde(default)]
    pub arp: Vec<i8>,
    #[serde(default)]
    pub vibrato: Option<[f32; 2]>,
    #[serde(default)]
    pub noise: Option<(String, String)>,
}

const MAX_PATTERN_CHARS: usize = 16384;

fn check_instruments(cue: &str, list: &[InstrumentData]) -> Result<(), ContentError> {
    if list.len() > 64 {
        return err(format!("The song \"{cue}\" has more than 64 instruments"));
    }
    for i in list {
        if i.env.is_empty() || i.env.len() > 64 || i.env.iter().any(|a| *a > 15) {
            return err(format!(
                "An instrument in \"{cue}\" needs 1 to 64 volume steps between 0 and 15"
            ));
        }
        if i.arp.len() > 16 {
            return err(format!(
                "An instrument in \"{cue}\" has more than 16 arpeggio steps"
            ));
        }
        if let Some(v) = i.vibrato
            && !v.iter().all(|x| x.is_finite() && *x >= 0.0)
        {
            return err(format!(
                "An instrument in \"{cue}\" has a vibrato that is not valid"
            ));
        }
    }
    Ok(())
}

impl SongsFile {
    pub fn parse(bytes: &[u8]) -> Result<SongsFile, ContentError> {
        let f: SongsFile = parse_json(bytes, SONGS_MAX_BYTES, "songs")?;
        check_v(f.v, "songs")?;
        if f.songs.len() > 64 {
            return err("A songs file holds at most 64 songs.");
        }
        for (k, s) in &f.songs {
            if !valid_kind(k) {
                return err(format!("The song name \"{k}\" is not a valid name"));
            }
            if !(s.bpm.is_finite() && (30.0..=300.0).contains(&s.bpm)) {
                return err(format!("The song \"{k}\" needs a tempo from 30 to 300 bpm"));
            }
            if !(1..=16).contains(&s.beats_per_bar) || !(1..=16).contains(&s.steps_per_beat) {
                return err(format!(
                    "The song \"{k}\" needs 1 to 16 beats per bar and steps per beat"
                ));
            }
            if !(1..=64).contains(&s.bars) {
                return err(format!("The song \"{k}\" needs 1 to 64 bars"));
            }
            if s.voices
                .iter()
                .chain([&s.noise])
                .any(|v| v.len() > MAX_PATTERN_CHARS)
            {
                return err(format!("A part of the song \"{k}\" is too long"));
            }
            check_instruments(k, &s.instruments)?;
            check_instruments(k, &s.noise_instruments)?;
        }
        Ok(f)
    }
}

// ---- sfx ------------------------------------------------------------------

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SfxFile {
    pub v: u8,
    pub sfx: BTreeMap<String, Vec<Vec<FxOp>>>,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
#[serde(untagged)]
pub enum NoteOrHz {
    Hz(f32),
    Note(String),
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
#[serde(tag = "op", rename_all = "lowercase", deny_unknown_fields)]
pub enum FxOp {
    Tone {
        voice: u8,
        from: NoteOrHz,
        to: NoteOrHz,
        a0: u8,
        a1: u8,
        frames: u16,
    },
    Hit {
        voice: u8,
        note: NoteOrHz,
        frames: u16,
    },
    Slide {
        voice: u8,
        from: NoteOrHz,
        to: NoteOrHz,
        frames: u16,
    },
    Noise {
        mode: String,
        rate: String,
        a0: u8,
        a1: u8,
        frames: u16,
    },
    Rest {
        frames: u16,
    },
}

fn note_ok(n: &NoteOrHz) -> bool {
    match n {
        NoteOrHz::Hz(h) => h.is_finite() && *h > 0.0 && *h <= 20000.0,
        NoteOrHz::Note(s) => !s.is_empty() && s.len() <= 8,
    }
}

fn op_ok(op: &FxOp) -> bool {
    let frames_ok = |f: u16| (1..=600).contains(&f);
    match op {
        FxOp::Tone {
            voice,
            from,
            to,
            a0,
            a1,
            frames,
        } => {
            *voice <= 2
                && note_ok(from)
                && note_ok(to)
                && *a0 <= 15
                && *a1 <= 15
                && frames_ok(*frames)
        }
        FxOp::Hit {
            voice,
            note,
            frames,
        } => *voice <= 2 && note_ok(note) && frames_ok(*frames),
        FxOp::Slide {
            voice,
            from,
            to,
            frames,
        } => *voice <= 2 && note_ok(from) && note_ok(to) && frames_ok(*frames),
        FxOp::Noise {
            mode,
            rate,
            a0,
            a1,
            frames,
        } => {
            !mode.is_empty()
                && mode.len() <= 16
                && !rate.is_empty()
                && rate.len() <= 16
                && *a0 <= 15
                && *a1 <= 15
                && frames_ok(*frames)
        }
        FxOp::Rest { frames } => frames_ok(*frames),
    }
}

impl SfxFile {
    pub fn parse(bytes: &[u8]) -> Result<SfxFile, ContentError> {
        let f: SfxFile = parse_json(bytes, SFX_MAX_BYTES, "sound effects")?;
        check_v(f.v, "sound effects")?;
        if f.sfx.len() > 128 {
            return err("A sound effects file holds at most 128 effects.");
        }
        for (k, parts) in &f.sfx {
            if !valid_kind(k) {
                return err(format!("The sound effect name \"{k}\" is not a valid name"));
            }
            if parts.is_empty()
                || parts.len() > 32
                || parts.iter().any(|p| p.is_empty() || p.len() > 8)
            {
                return err(format!(
                    "The sound effect \"{k}\" needs 1 to 32 parts of 1 to 8 sounds each"
                ));
            }
            if !parts.iter().flatten().all(op_ok) {
                return err(format!(
                    "A sound in \"{k}\" has a voice above 2, a volume above 15, a pitch that is not valid or a length outside 1 to 600 frames"
                ));
            }
        }
        Ok(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lv(things: &str) -> Vec<u8> {
        format!(r#"{{"v":1,"things":{things}}}"#).into_bytes()
    }

    #[test]
    fn valid_level_parses() {
        let f = LevelFile::parse(&lv(
            r#"[{"id":"p1","kind":"Player","at":[1,2]},{"id":"g-2","kind":"Platform","at":[0,0],"size":[10,2]}]"#,
        ))
        .unwrap();
        assert_eq!(f.things.len(), 2);
        assert_eq!(f.things[1].size, Some([10.0, 2.0]));
    }

    #[test]
    fn level_rejects_duplicate_and_bad_ids() {
        let e = LevelFile::parse(&lv(
            r#"[{"id":"a","kind":"K","at":[0,0]},{"id":"a","kind":"K","at":[1,1]}]"#,
        ))
        .unwrap_err();
        assert!(e.0.contains("\"a\"") && e.0.ends_with('.'));
        for bad in ["", "A", "has space", &"x".repeat(33)] {
            let s = format!(r#"[{{"id":"{bad}","kind":"K","at":[0,0]}}]"#);
            assert!(LevelFile::parse(&lv(&s)).is_err(), "{bad}");
        }
    }

    #[test]
    fn level_rejects_bad_kinds_size_version_and_extras() {
        for bad in ["", "1a", "a b", &"k".repeat(33)] {
            let s = format!(r#"[{{"id":"a","kind":"{bad}","at":[0,0]}}]"#);
            assert!(LevelFile::parse(&lv(&s)).is_err(), "{bad}");
        }
        for sz in ["[0,1]", "[1,-1]"] {
            let s = format!(r#"[{{"id":"a","kind":"K","at":[0,0],"size":{sz}}}]"#);
            assert!(LevelFile::parse(&lv(&s)).is_err(), "{sz}");
        }
        assert!(LevelFile::parse(br#"{"v":2,"things":[]}"#).is_err());
        assert!(LevelFile::parse(br#"{"v":1,"things":[],"x":1}"#).is_err());
        assert!(LevelFile::parse(&lv(r#"[{"id":"a","kind":"K","at":[0,0],"z":1}]"#)).is_err());
        assert!(LevelFile::parse(b"not json").is_err());
    }

    #[test]
    fn level_limits() {
        let many: Vec<String> = (0..2001)
            .map(|i| format!(r#"{{"id":"t{i}","kind":"K","at":[0,0]}}"#))
            .collect();
        let e = LevelFile::parse(&lv(&format!("[{}]", many.join(",")))).unwrap_err();
        assert_eq!(e.0, "A level holds at most 2000 things.");
        let ok = &many[..2000];
        assert!(LevelFile::parse(&lv(&format!("[{}]", ok.join(",")))).is_ok());
        let big = vec![b' '; LEVEL_MAX_BYTES + 1];
        assert!(LevelFile::parse(&big).unwrap_err().0.contains("256 KB"));
    }

    #[test]
    fn tunables_value_shapes() {
        let ok = br#"{"v":1,"values":{"a.b":1.5,"c":true,"d":"x","e":[1,2,3]}}"#;
        assert!(TunablesFile::parse(ok).is_ok());
        for bad in [r#"null"#, r#"{"x":1}"#, r#"[1,"a"]"#, r#"[[1]]"#] {
            let s = format!(r#"{{"v":1,"values":{{"a":{bad}}}}}"#);
            assert!(TunablesFile::parse(s.as_bytes()).is_err(), "{bad}");
        }
        assert!(TunablesFile::parse(br#"{"v":0,"values":{}}"#).is_err());
        assert!(TunablesFile::parse(&vec![b' '; TUNABLES_MAX_BYTES + 1]).is_err());
    }

    #[test]
    fn sprites_rules() {
        let f = SpritesFile::parse(br#"{"v":1,"sprites":{"Player":{}}}"#).unwrap();
        let m = &f.sprites["Player"];
        assert_eq!((m.frames, m.fps, m.scale), (1, 8.0, 1.0));
        for bad in [
            r#"{"frames":0}"#,
            r#"{"scale":0}"#,
            r#"{"scale":-1}"#,
            r#"{"fps":0}"#,
            r#"{"anchor":[2,0]}"#,
            r#"{"x":1}"#,
        ] {
            let s = format!(r#"{{"v":1,"sprites":{{"P":{bad}}}}}"#);
            assert!(SpritesFile::parse(s.as_bytes()).is_err(), "{bad}");
        }
        assert!(SpritesFile::parse(br#"{"v":1,"sprites":{"1x":{}}}"#).is_err());
        assert!(SpritesFile::parse(&vec![b' '; SPRITES_MAX_BYTES + 1]).is_err());
    }

    fn song(extra: &str) -> Vec<u8> {
        format!(
            r#"{{"v":1,"songs":{{"Menu":{{"bpm":120,"beats_per_bar":4,"steps_per_beat":4,"bars":4,
            "voices":["a","b","c"],"instruments":[{{"env":[15,8,0]}}]{extra}}}}}}}"#
        )
        .into_bytes()
    }

    #[test]
    fn songs_rules() {
        assert!(SongsFile::parse(&song("")).is_ok());
        for (from, to) in [
            ("\"bpm\":120", "\"bpm\":29"),
            ("\"bpm\":120", "\"bpm\":301"),
            ("\"bars\":4", "\"bars\":0"),
            ("\"bars\":4", "\"bars\":65"),
            ("[\"a\",\"b\",\"c\"]", "[\"a\",\"b\"]"),
            ("[\"a\",\"b\",\"c\"]", "[\"a\",\"b\",\"c\",\"d\"]"),
            ("[15,8,0]", "[16]"),
            ("[15,8,0]", "[]"),
        ] {
            let s = String::from_utf8(song("")).unwrap().replace(from, to);
            assert!(SongsFile::parse(s.as_bytes()).is_err(), "{to}");
        }
        assert!(SongsFile::parse(&vec![b' '; SONGS_MAX_BYTES + 1]).is_err());
    }

    #[test]
    fn sfx_rules() {
        let ok = br#"{"v":1,"sfx":{"Jump":[[{"op":"tone","voice":0,"from":"C4","to":440.0,"a0":15,"a1":0,"frames":6}],
            [{"op":"rest","frames":2},{"op":"noise","mode":"white","rate":"low","a0":8,"a1":0,"frames":3}]]}}"#;
        assert!(SfxFile::parse(ok).is_ok());
        for op in [
            r#"{"op":"hit","voice":3,"note":"C4","frames":1}"#,
            r#"{"op":"hit","voice":0,"note":"C4","frames":0}"#,
            r#"{"op":"hit","voice":0,"note":"C4","frames":601}"#,
            r#"{"op":"tone","voice":0,"from":1,"to":2,"a0":16,"a1":0,"frames":1}"#,
            r#"{"op":"rest"}"#,
            r#"{"op":"boom","frames":1}"#,
            r#"{"op":"slide","voice":0,"from":0,"to":2,"frames":1}"#,
        ] {
            let s = format!(r#"{{"v":1,"sfx":{{"X":[[{op}]]}}}}"#);
            assert!(SfxFile::parse(s.as_bytes()).is_err(), "{op}");
        }
        assert!(SfxFile::parse(br#"{"v":1,"sfx":{"X":[]}}"#).is_err());
        assert!(SfxFile::parse(br#"{"v":1,"sfx":{"X":[[]]}}"#).is_err());
        assert!(SfxFile::parse(&vec![b' '; SFX_MAX_BYTES + 1]).is_err());
    }
}
