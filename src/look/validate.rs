//! Validates an untrusted pack against the limits and the game's
//! contract. Pure: no Bevy app, no decoding. `Severity::Error` blocks the
//! pack; a warning loads it and logs.

use std::collections::BTreeMap;

use bevy::color::{Color, Srgba};

use super::contract::{Contract, ParamKind};
use super::manifest::{self, MANIFEST_FILE, Manifest, Param, SCHEMA_VERSION, Slot};
use super::sniff;

/// A pack's files by name, `look.json` included.
pub type Files = BTreeMap<String, Vec<u8>>;

pub mod limits {
    pub const MANIFEST_MAX: usize = 64 * 1024;
    /// Including `look.json`.
    pub const FILES_MAX: usize = 48;
    pub const PACK_MAX: usize = 16 * 1024 * 1024;
    pub const PNG_MAX: usize = 2 * 1024 * 1024;
    pub const PNG_DIM_MAX: u32 = 2048;
    pub const OGG_MAX: usize = 4 * 1024 * 1024;
    pub const OGG_SECONDS_MAX: f32 = 180.0;
    pub const OGG_CHANNELS_MAX: u8 = 2;
    pub const OGG_RATE_MAX: u32 = 48_000;
    pub const STRING_MAX: usize = 64;
    pub const NAME_MAX: usize = 32;
    pub const CREATOR_MAX: usize = 48;
    pub const ID_MAX: usize = 32;
    pub const KEY_MAX: usize = 48;
    pub const FILE_STEM_MAX: usize = 48;
    pub const FRAMES_MAX: u32 = 64;
    pub const FPS_MAX: f32 = 60.0;
    pub const URL_MAX: usize = 200;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Problem {
    pub severity: Severity,
    /// Where: `look.json`, `palette.ink`, `hero.png`, ...
    pub at: String,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct Report {
    pub manifest: Option<Manifest>,
    pub problems: Vec<Problem>,
    pub content_hash: String,
}

impl Report {
    pub fn ok(&self) -> bool {
        self.manifest.is_some() && self.errors().next().is_none()
    }
    pub fn errors(&self) -> impl Iterator<Item = &Problem> {
        self.problems
            .iter()
            .filter(|p| p.severity == Severity::Error)
    }
    /// A pack that could not even be read (missing folder, too large).
    pub fn read_error(message: String) -> Self {
        Self {
            manifest: None,
            problems: vec![Problem {
                severity: Severity::Error,
                at: "pack".into(),
                message,
            }],
            content_hash: String::new(),
        }
    }
}

/// BLAKE3 over every file, in name order: name length, name, byte length,
/// bytes. Domain-separated so it can never collide with another use.
pub fn content_hash(files: &Files) -> String {
    let mut h = blake3::Hasher::new();
    h.update(b"gx-look-v1\0");
    for (name, bytes) in files {
        h.update(&(name.len() as u64).to_le_bytes());
        h.update(name.as_bytes());
        h.update(&(bytes.len() as u64).to_le_bytes());
        h.update(bytes);
    }
    h.finalize().to_hex().to_string()
}

/// `#rrggbb` or `#rrggbbaa`, nothing looser.
pub fn parse_colour(s: &str) -> Option<Color> {
    if !s.starts_with('#')
        || !(s.len() == 7 || s.len() == 9)
        || !s[1..].bytes().all(|c| c.is_ascii_hexdigit())
    {
        return None;
    }
    Srgba::hex(&s[1..]).ok().map(Color::from)
}

fn is_text(s: &str, max: usize) -> bool {
    s.len() <= max && s.bytes().all(|c| (0x20..=0x7e).contains(&c))
}

fn is_id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= limits::ID_MAX
        && s.bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
}

fn is_key(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= limits::KEY_MAX
        && s.bytes().all(|c| {
            c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'.' | b'_' | b'-')
        })
}

/// `[a-z0-9_-]{1,48}.png|.ogg`: no dots in the stem, so no `..`, no
/// hidden files, no second extension.
fn is_file_name(s: &str) -> bool {
    let Some((stem, ext)) = s.rsplit_once('.') else {
        return false;
    };
    matches!(ext, "png" | "ogg")
        && !stem.is_empty()
        && stem.len() <= limits::FILE_STEM_MAX
        && stem
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'_' | b'-'))
}

struct Check<'a> {
    files: &'a Files,
    problems: Vec<Problem>,
    used: Vec<String>,
}

impl Check<'_> {
    fn error(&mut self, at: impl Into<String>, message: impl Into<String>) {
        self.problems.push(Problem {
            severity: Severity::Error,
            at: at.into(),
            message: message.into(),
        });
    }
    fn warn(&mut self, at: impl Into<String>, message: impl Into<String>) {
        self.problems.push(Problem {
            severity: Severity::Warning,
            at: at.into(),
            message: message.into(),
        });
    }
    /// `at` names `file`, which must be in the pack with extension `ext`.
    fn file_ref(&mut self, at: &str, file: &str, ext: &str) {
        if !file.ends_with(&format!(".{ext}")) {
            self.error(at, format!("expects a .{ext} file, got \"{file}\""));
        } else if !self.files.contains_key(file) {
            self.error(at, format!("\"{file}\" is not in the pack"));
        }
        self.used.push(file.to_string());
    }
    fn text(&mut self, at: &str, s: &str, max: usize) {
        if !is_text(s, max) {
            self.error(
                at,
                format!("must be printable ASCII, at most {max} characters"),
            );
        }
    }
}

pub fn validate(files: &Files, contract: &Contract, expected_id: Option<&str>) -> Report {
    let mut c = Check {
        files,
        problems: Vec::new(),
        used: Vec::new(),
    };
    let content_hash = content_hash(files);

    // --- The pack as a whole, and every file in it -------------------------
    if files.len() > limits::FILES_MAX {
        c.error(
            "pack",
            format!("{} files; at most {}", files.len(), limits::FILES_MAX),
        );
    }
    let total: usize = files.values().map(Vec::len).sum();
    if total > limits::PACK_MAX {
        c.error(
            "pack",
            format!("{total} bytes; at most {}", limits::PACK_MAX),
        );
    }
    for (name, bytes) in files {
        if name == MANIFEST_FILE {
            if bytes.len() > limits::MANIFEST_MAX {
                c.error(
                    name.as_str(),
                    format!("at most {} bytes", limits::MANIFEST_MAX),
                );
            }
            continue;
        }
        if !is_file_name(name) {
            c.error(
                name.as_str(),
                "file names are lowercase letters, digits, - and _, ending .png or .ogg (no fonts, no folders)",
            );
            continue;
        }
        if name.ends_with(".png") {
            if bytes.len() > limits::PNG_MAX {
                c.error(name.as_str(), format!("at most {} bytes", limits::PNG_MAX));
            }
            match sniff::png_size(bytes) {
                None => c.error(name.as_str(), "is not a PNG"),
                Some((w, h))
                    if w == 0 || h == 0 || w > limits::PNG_DIM_MAX || h > limits::PNG_DIM_MAX =>
                {
                    c.error(
                        name.as_str(),
                        format!("{w}x{h}; at most {0}x{0}", limits::PNG_DIM_MAX),
                    )
                }
                Some(_) => {}
            }
        } else {
            if bytes.len() > limits::OGG_MAX {
                c.error(name.as_str(), format!("at most {} bytes", limits::OGG_MAX));
            }
            match sniff::ogg_vorbis_info(bytes) {
                None => c.error(name.as_str(), "is not an Ogg Vorbis file"),
                Some(i) => {
                    if i.channels > limits::OGG_CHANNELS_MAX {
                        c.error(
                            name.as_str(),
                            format!(
                                "{} channels; at most {}",
                                i.channels,
                                limits::OGG_CHANNELS_MAX
                            ),
                        );
                    }
                    if i.rate > limits::OGG_RATE_MAX {
                        c.error(
                            name.as_str(),
                            format!("{} Hz; at most {}", i.rate, limits::OGG_RATE_MAX),
                        );
                    }
                    if !(i.seconds.is_finite() && i.seconds <= limits::OGG_SECONDS_MAX) {
                        c.error(
                            name.as_str(),
                            format!("{:.1} s; at most {} s", i.seconds, limits::OGG_SECONDS_MAX),
                        );
                    }
                }
            }
        }
    }

    // --- The manifest ------------------------------------------------------
    let Some(raw) = files.get(MANIFEST_FILE) else {
        c.error(MANIFEST_FILE, "missing");
        return Report {
            manifest: None,
            problems: c.problems,
            content_hash,
        };
    };
    let m = match manifest::parse(raw) {
        Ok(m) => m,
        Err(e) => {
            c.error(MANIFEST_FILE, e);
            return Report {
                manifest: None,
                problems: c.problems,
                content_hash,
            };
        }
    };
    if m.v != SCHEMA_VERSION {
        c.error(
            "v",
            format!(
                "this game reads schema {SCHEMA_VERSION}, the pack is {}",
                m.v
            ),
        );
    }
    if !is_id(&m.id) {
        c.error("id", "lowercase letters, digits and -, at most 32");
    }
    if let Some(expected) = expected_id
        && expected != m.id
    {
        c.error(
            "id",
            format!("\"{}\" must match the folder name \"{expected}\"", m.id),
        );
    }
    c.text("name", &m.name, limits::NAME_MAX);
    if let Some(creator) = &m.creator {
        c.text("creator", creator, limits::CREATOR_MAX);
    }

    for (role, hex) in &m.palette {
        let at = format!("palette.{role}");
        if contract.default_colour(role).is_none() {
            let known: Vec<_> = contract.palette.iter().map(|(r, _)| *r).collect();
            c.error(
                at.as_str(),
                format!("unknown role; this game has {known:?}"),
            );
        }
        if parse_colour(hex).is_none() {
            c.error(at, "colours are #rrggbb or #rrggbbaa");
        }
    }

    for (key, text) in &m.strings {
        let at = format!("strings.{key}");
        match contract.string_spec(key) {
            None => {
                let known: Vec<_> = contract.strings.iter().map(|s| s.key).collect();
                c.error(at, format!("unknown string; this game has {known:?}"));
            }
            Some(spec) => c.text(&at, text, spec.max_chars.min(limits::STRING_MAX)),
        }
    }

    for (name, slot) in &m.slots {
        let at = format!("slots.{name}");
        let Some(spec) = contract.slot(name) else {
            let known: Vec<_> = contract.slots.iter().map(|s| s.name).collect();
            c.error(at, format!("unknown slot; this game has {known:?}"));
            continue;
        };
        match slot {
            Slot::Image(i) => {
                if !spec.image {
                    c.error(at.as_str(), "this slot is drawn by a recipe, not an image");
                }
                c.file_ref(&at, &i.image, "png");
                if i.frames == 0 || i.frames > limits::FRAMES_MAX {
                    c.error(at.as_str(), format!("frames is 1..={}", limits::FRAMES_MAX));
                }
                if !(i.fps.is_finite() && (0.0..=limits::FPS_MAX).contains(&i.fps)) {
                    c.error(at.as_str(), format!("fps is 0..={}", limits::FPS_MAX));
                }
            }
            Slot::Recipe(r) => {
                let Some(recipe) = spec.recipe(&r.recipe) else {
                    let known: Vec<_> = spec.recipes.iter().map(|r| r.name).collect();
                    c.error(
                        at,
                        format!("unknown recipe \"{}\"; this slot has {known:?}", r.recipe),
                    );
                    continue;
                };
                for (pname, value) in &r.params {
                    let pat = format!("{at}.{pname}");
                    let Some(pspec) = recipe.params.iter().find(|p| p.name == pname.as_str())
                    else {
                        c.error(pat, "unknown parameter");
                        continue;
                    };
                    match (&pspec.kind, value) {
                        (ParamKind::Number { min, max }, Param::Number(n)) => {
                            if !(n.is_finite() && *n >= *min && *n <= *max) {
                                c.error(pat, format!("{n} is outside {min}..={max}"));
                            }
                        }
                        (ParamKind::Bool, Param::Bool(_)) => {}
                        (ParamKind::Text { max_chars }, Param::Text(s)) => {
                            c.text(&pat, s, *max_chars)
                        }
                        (ParamKind::Colour, Param::Text(s)) => {
                            if parse_colour(s).is_none() && contract.default_colour(s).is_none() {
                                c.error(pat, "a #rrggbb colour or a palette role");
                            }
                        }
                        (kind, _) => c.error(pat, format!("expects {kind:?}")),
                    }
                }
            }
        }
    }

    for (cue, file) in &m.audio {
        let at = format!("audio.{cue}");
        if !is_key(cue) || !contract.audio.contains(&cue.as_str()) {
            c.error(
                at.as_str(),
                format!("unknown cue; this game has {:?}", contract.audio),
            );
        }
        c.file_ref(&at, file, "ogg");
    }

    for (role, face) in &m.fonts {
        let at = format!("fonts.{role}");
        if !matches!(role.as_str(), "display" | "body") {
            c.error(at.as_str(), "font roles are display and body");
        }
        if !contract.fonts.contains(&face.as_str()) {
            c.error(
                at,
                format!("not a bundled face; this game has {:?}", contract.fonts),
            );
        }
    }

    for (part, file) in &m.frame {
        let at = format!("frame.{part}");
        if !matches!(part.as_str(), "marquee" | "bezel") {
            c.error(at.as_str(), "frame parts are marquee and bezel");
        }
        c.file_ref(&at, file, "png");
    }

    if let Some(card) = &m.end_card {
        c.text("end_card.headline", &card.headline, limits::STRING_MAX);
        if let Some(body) = &card.body {
            c.text("end_card.body", body, limits::STRING_MAX * 2);
        }
        if let Some(url) = &card.url
            && (!url.starts_with("https://") || !is_text(url, limits::URL_MAX))
        {
            c.error(
                "end_card.url",
                format!("an https:// URL, at most {} characters", limits::URL_MAX),
            );
        }
        if let Some(image) = &card.image {
            c.file_ref("end_card.image", image, "png");
        }
    }

    if m.protagonist.is_some() {
        c.warn("protagonist", "reserved for a later schema; ignored");
    }

    let unused: Vec<String> = files
        .keys()
        .filter(|n| n.as_str() != MANIFEST_FILE && is_file_name(n) && !c.used.contains(n))
        .cloned()
        .collect();
    for name in unused {
        c.warn(name, "not referenced by look.json");
    }

    Report {
        manifest: Some(m),
        problems: c.problems,
        content_hash,
    }
}

#[cfg(test)]
mod tests {
    use super::super::contract::{ParamSpec, RecipeSpec, SlotSpec, StringSpec};
    use super::super::sniff::fixtures::{ogg, png};
    use super::*;

    const TEST: Contract = Contract {
        palette: &[
            ("ink", Color::srgb(0.0, 0.0, 0.0)),
            ("signal", Color::srgb(0.0, 1.0, 1.0)),
        ],
        strings: &[StringSpec {
            key: "pause",
            default: "TAKE FIVE",
            max_chars: 12,
        }],
        slots: &[SlotSpec {
            name: "player",
            image: true,
            recipes: &[RecipeSpec {
                name: "cuboid",
                params: &[
                    ParamSpec {
                        name: "size",
                        kind: ParamKind::Number {
                            min: 0.25,
                            max: 3.0,
                        },
                    },
                    ParamSpec {
                        name: "tint",
                        kind: ParamKind::Colour,
                    },
                ],
            }],
        }],
        audio: &["sfx.confirm", "music.menu"],
        fonts: &["Inter SemiBold"],
    };

    fn pack(json: &str) -> Files {
        BTreeMap::from([(MANIFEST_FILE.to_string(), json.as_bytes().to_vec())])
    }

    fn with(mut files: Files, name: &str, bytes: Vec<u8>) -> Files {
        files.insert(name.to_string(), bytes);
        files
    }

    fn errors(r: &Report) -> Vec<String> {
        r.errors()
            .map(|p| format!("{}: {}", p.at, p.message))
            .collect()
    }

    const MIN: &str = r#"{"v":1,"id":"demo","name":"Demo","kind":"palette"}"#;

    #[test]
    fn a_minimal_pack_is_ok() {
        let r = validate(&pack(MIN), &TEST, Some("demo"));
        assert!(r.ok(), "{:?}", errors(&r));
        assert_eq!(r.content_hash.len(), 64);
    }

    #[test]
    fn missing_or_broken_manifest_is_an_error() {
        assert!(!validate(&Files::new(), &TEST, None).ok());
        assert!(!validate(&pack("{"), &TEST, None).ok());
        assert!(
            !validate(
                &pack(r#"{"v":2,"id":"demo","name":"D","kind":"palette"}"#),
                &TEST,
                None
            )
            .ok()
        );
    }

    #[test]
    fn the_id_must_be_well_formed_and_match_the_folder() {
        assert!(!validate(&pack(MIN), &TEST, Some("other")).ok());
        let bad = r#"{"v":1,"id":"Demo Pack","name":"D","kind":"palette"}"#;
        assert!(!validate(&pack(bad), &TEST, None).ok());
    }

    #[test]
    fn palette_roles_must_exist_and_colours_must_be_hex() {
        let ok = r##"{"v":1,"id":"demo","name":"D","kind":"palette","palette":{"ink":"#102030","signal":"#10203040"}}"##;
        assert!(validate(&pack(ok), &TEST, None).ok());
        let typo =
            r##"{"v":1,"id":"demo","name":"D","kind":"palette","palette":{"inc":"#102030"}}"##;
        assert!(errors(&validate(&pack(typo), &TEST, None))[0].contains("palette.inc"));
        let bad = r#"{"v":1,"id":"demo","name":"D","kind":"palette","palette":{"ink":"red"}}"#;
        assert!(!validate(&pack(bad), &TEST, None).ok());
    }

    #[test]
    fn strings_respect_keys_ascii_and_length() {
        let ok =
            r#"{"v":1,"id":"demo","name":"D","kind":"palette","strings":{"pause":"SNACK BREAK"}}"#;
        assert!(validate(&pack(ok), &TEST, None).ok());
        let long = r#"{"v":1,"id":"demo","name":"D","kind":"palette","strings":{"pause":"THIS IS FAR TOO LONG"}}"#;
        assert!(!validate(&pack(long), &TEST, None).ok());
        let unicode =
            r#"{"v":1,"id":"demo","name":"D","kind":"palette","strings":{"pause":"CAFÉ"}}"#;
        assert!(!validate(&pack(unicode), &TEST, None).ok());
        let unknown = r#"{"v":1,"id":"demo","name":"D","kind":"palette","strings":{"title":"X"}}"#;
        assert!(!validate(&pack(unknown), &TEST, None).ok());
    }

    #[test]
    fn recipes_and_params_are_checked_against_the_contract() {
        let ok = r##"{"v":1,"id":"demo","name":"D","kind":"skin","slots":{"player":{"recipe":"cuboid","params":{"size":1.5,"tint":"signal"}}}}"##;
        assert!(
            validate(&pack(ok), &TEST, None).ok(),
            "{:?}",
            errors(&validate(&pack(ok), &TEST, None))
        );
        for bad in [
            r#"{"v":1,"id":"demo","name":"D","kind":"skin","slots":{"enemy":{"recipe":"cuboid"}}}"#,
            r#"{"v":1,"id":"demo","name":"D","kind":"skin","slots":{"player":{"recipe":"sphere"}}}"#,
            r#"{"v":1,"id":"demo","name":"D","kind":"skin","slots":{"player":{"recipe":"cuboid","params":{"size":9}}}}"#,
            r#"{"v":1,"id":"demo","name":"D","kind":"skin","slots":{"player":{"recipe":"cuboid","params":{"spin":1}}}}"#,
            r#"{"v":1,"id":"demo","name":"D","kind":"skin","slots":{"player":{"recipe":"cuboid","params":{"tint":"mauve"}}}}"#,
        ] {
            assert!(!validate(&pack(bad), &TEST, None).ok(), "{bad}");
        }
    }

    #[test]
    fn image_files_must_exist_be_png_and_be_bounded() {
        let json = r#"{"v":1,"id":"demo","name":"D","kind":"skin","slots":{"player":{"image":"hero.png"}}}"#;
        assert!(!validate(&pack(json), &TEST, None).ok(), "missing file");
        assert!(validate(&with(pack(json), "hero.png", png(64, 64)), &TEST, None).ok());
        assert!(
            !validate(&with(pack(json), "hero.png", png(4096, 64)), &TEST, None).ok(),
            "too wide"
        );
        assert!(
            !validate(
                &with(pack(json), "hero.png", b"not a png at all........".to_vec()),
                &TEST,
                None
            )
            .ok()
        );
    }

    #[test]
    fn audio_must_be_short_ogg_vorbis_for_a_known_cue() {
        let json =
            r#"{"v":1,"id":"demo","name":"D","kind":"skin","audio":{"sfx.confirm":"ding.ogg"}}"#;
        assert!(
            validate(
                &with(pack(json), "ding.ogg", ogg(1, 44_100, 22_050)),
                &TEST,
                None
            )
            .ok()
        );
        assert!(
            !validate(
                &with(pack(json), "ding.ogg", ogg(1, 44_100, 44_100 * 600)),
                &TEST,
                None
            )
            .ok(),
            "too long"
        );
        assert!(
            !validate(
                &with(pack(json), "ding.ogg", ogg(6, 44_100, 100)),
                &TEST,
                None
            )
            .ok(),
            "5.1"
        );
        let unknown =
            r#"{"v":1,"id":"demo","name":"D","kind":"skin","audio":{"sfx.boom":"ding.ogg"}}"#;
        assert!(
            !validate(
                &with(pack(unknown), "ding.ogg", ogg(1, 44_100, 100)),
                &TEST,
                None
            )
            .ok()
        );
    }

    #[test]
    fn a_forged_ogg_tail_fails_the_pack() {
        let json =
            r#"{"v":1,"id":"demo","name":"D","kind":"skin","audio":{"sfx.confirm":"ding.ogg"}}"#;
        let mut forged = ogg(1, 44_100, 44_100 * 600);
        forged.extend_from_slice(b"OggS\x00\x04");
        forged.extend_from_slice(&100u64.to_le_bytes());
        forged.extend_from_slice(&[0; 13]);
        assert!(!validate(&with(pack(json), "ding.ogg", forged), &TEST, None).ok());
    }

    #[test]
    fn parse_colour_is_strictly_hex() {
        for bad in [
            "##1234",
            "##123456",
            "#+12345",
            "#+1234567",
            "#12345",
            "red",
        ] {
            assert!(parse_colour(bad).is_none(), "{bad}");
        }
        assert!(parse_colour("#a1b2c3").is_some());
        assert!(parse_colour("#a1b2c3d4").is_some());
    }

    #[test]
    fn file_names_cannot_escape_or_smuggle() {
        for name in [
            "../x.png",
            "sub/x.png",
            "X.PNG",
            "font.ttf",
            "script.js",
            ".hidden.png",
        ] {
            let files = with(pack(MIN), name, png(8, 8));
            assert!(!validate(&files, &TEST, None).ok(), "{name}");
        }
    }

    #[test]
    fn fonts_are_bundled_names_only() {
        let ok =
            r#"{"v":1,"id":"demo","name":"D","kind":"genre","fonts":{"body":"Inter SemiBold"}}"#;
        assert!(validate(&pack(ok), &TEST, None).ok());
        let bad = r#"{"v":1,"id":"demo","name":"D","kind":"genre","fonts":{"body":"Comic Sans"}}"#;
        assert!(!validate(&pack(bad), &TEST, None).ok());
    }

    #[test]
    fn end_card_url_must_be_https() {
        let ok = r#"{"v":1,"id":"demo","name":"D","kind":"sponsor","end_card":{"headline":"THANKS","url":"https://example.com"}}"#;
        assert!(validate(&pack(ok), &TEST, None).ok());
        let bad = r#"{"v":1,"id":"demo","name":"D","kind":"sponsor","end_card":{"headline":"THANKS","url":"javascript:alert(1)"}}"#;
        assert!(!validate(&pack(bad), &TEST, None).ok());
    }

    #[test]
    fn protagonist_and_unused_files_warn_but_load() {
        let json = r#"{"v":1,"id":"demo","name":"D","kind":"skin","protagonist":{"avatar":"x"}}"#;
        let r = validate(&with(pack(json), "spare.png", png(8, 8)), &TEST, None);
        assert!(r.ok());
        assert_eq!(
            r.problems
                .iter()
                .filter(|p| p.severity == Severity::Warning)
                .count(),
            2
        );
    }

    #[test]
    fn too_many_files_is_an_error() {
        let mut files = pack(MIN);
        for i in 0..limits::FILES_MAX {
            files.insert(format!("f{i}.png"), png(1, 1));
        }
        assert!(!validate(&files, &TEST, None).ok());
    }

    #[test]
    fn the_content_hash_covers_names_and_bytes() {
        let a = with(pack(MIN), "a.png", png(8, 8));
        let b = with(pack(MIN), "b.png", png(8, 8));
        let c = with(pack(MIN), "a.png", png(8, 9));
        assert_eq!(content_hash(&a), content_hash(&a.clone()));
        assert_ne!(content_hash(&a), content_hash(&b));
        assert_ne!(content_hash(&a), content_hash(&c));
    }
}
