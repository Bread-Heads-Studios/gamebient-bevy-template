//! Validates an untrusted pack against the limits and the game's
//! contract. Pure: no Bevy app, no decoding. `Severity::Error` blocks the
//! pack; a warning loads it and logs.

use std::collections::BTreeMap;

use bevy::color::{Color, Srgba};

use super::contract::{Contract, ParamKind, ShapeSpec};
use super::manifest::{self, MANIFEST_FILE, Manifest, Param, Primitive, Slot};
// The id and file-name rules, shared with the build-time generator.
use super::names::{is_file_name, is_id};
use super::shape;
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
    pub const ID_MAX: usize = super::super::names::ID_MAX;
    pub const KEY_MAX: usize = 48;
    pub const FILE_STEM_MAX: usize = super::super::names::FILE_STEM_MAX;
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
    /// `(slot, vertices)` for every shape slot that passed its checks.
    pub shapes: Vec<(String, usize)>,
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
    /// A pack refused before validation: unreadable, too large, or not the
    /// bytes the build expected (content hash mismatch).
    pub fn read_error(message: String) -> Self {
        Self {
            manifest: None,
            problems: vec![Problem {
                severity: Severity::Error,
                at: "pack".into(),
                message,
            }],
            content_hash: String::new(),
            shapes: Vec::new(),
        }
    }
}

/// Lives in `hash.rs` so `src/look/embed.rs` hashes fetchable packs with the same code.
pub use super::hash::content_hash;

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

fn is_key(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= limits::KEY_MAX
        && s.bytes().all(|c| {
            c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'.' | b'_' | b'-')
        })
}

const AXES: [&str; 3] = ["x", "y", "z"];

/// The largest absolute coordinate of any vertex `shape::build_mesh`
/// produces on each axis: the half-extents of the smallest centred box
/// holding the shape. `[NAN; 3]` if any coordinate is NaN.
fn shape_reach(prims: &[Primitive]) -> [f32; 3] {
    use bevy::mesh::{Mesh, VertexAttributeValues};
    let mut reach = [0.0_f32; 3];
    if let Some(VertexAttributeValues::Float32x3(pos)) =
        shape::build_mesh(prims).attribute(Mesh::ATTRIBUTE_POSITION)
    {
        for v in pos {
            for (r, c) in reach.iter_mut().zip(v) {
                *r = r.max(c.abs());
            }
        }
        // `f32::max` drops NaN, so check for it explicitly.
        if pos.iter().flatten().any(|c| c.is_nan()) {
            return [f32::NAN; 3];
        }
    }
    reach
}

struct Check<'a> {
    files: &'a Files,
    problems: Vec<Problem>,
    used: Vec<String>,
}

impl Check<'_> {
    /// Numeric rules for one shape slot. Returns the vertex count when the
    /// shape is valid (and so counted); `None` after any error.
    fn shape(&mut self, at: &str, prims: &[Primitive], limit: &ShapeSpec) -> Option<usize> {
        let before = self.problems.len();
        let ext = limit.extent;
        // Cheap prefilter only: sizes are full edges/lengths for most
        // primitives (half-sizes for sphere/torus), so allow twice the
        // largest half-extent. The built-mesh reach check is the real bound.
        let max_size = 2.0 * ext.iter().copied().fold(0.0_f32, f32::max);
        if prims.len() > limit.max_prims {
            self.error(
                format!("{at}.shape"),
                format!("{} primitives; at most {}", prims.len(), limit.max_prims),
            );
        }
        for (i, p) in prims.iter().enumerate() {
            let pat = format!("{at}.shape[{i}]");
            let want = shape::arity(p.prim);
            if p.size.len() != want {
                self.error(
                    pat.as_str(),
                    format!("size needs {want} number(s), got {}", p.size.len()),
                );
            } else if !p
                .size
                .iter()
                .all(|v| v.is_finite() && (0.001..=max_size).contains(v))
            {
                self.error(
                    pat.as_str(),
                    format!("size components are 0.001..={max_size}"),
                );
            }
            if let Some(v) = p.at {
                for (i, c) in v.iter().enumerate() {
                    let e = ext[i];
                    if !(c.is_finite() && (-e..=e).contains(c)) {
                        self.error(
                            pat.as_str(),
                            format!("at {} is {c}; must be -{e}..={e}", AXES[i]),
                        );
                    }
                }
            }
            if let Some(v) = p.rot
                && !v.iter().all(|c| c.is_finite())
            {
                self.error(pat.as_str(), "rot components must be finite numbers");
            }
            if let Some(v) = p.scale
                && !v.iter().all(|c| c.is_finite() && (0.01..=4.0).contains(c))
            {
                self.error(pat.as_str(), "scale components are 0.01..=4");
            }
            if let Some(d) = p.detail
                && !shape::DETAIL_RANGE.contains(&d)
            {
                self.error(
                    pat.as_str(),
                    format!(
                        "detail is {}..={}",
                        shape::DETAIL_RANGE.start(),
                        shape::DETAIL_RANGE.end()
                    ),
                );
            }
        }
        if self.problems.len() != before {
            return None;
        }
        let n = shape::vertex_count(prims);
        if n > limit.max_vertices {
            self.error(
                format!("{at}.shape"),
                format!("{n} vertices; budget {}", limit.max_vertices),
            );
            return None;
        }
        // Last, and only now that every rule above bounds the work: the
        // built geometry itself must sit inside the extent cube.
        let reach = shape_reach(prims);
        for i in 0..3 {
            if !(reach[i].is_finite() && reach[i] <= ext[i] * (1.0 + 1e-5)) {
                self.error(
                    format!("{at}.shape"),
                    format!(
                        "reaches {:.3} on {}; extent is {}",
                        reach[i], AXES[i], ext[i]
                    ),
                );
                return None;
            }
        }
        Some(n)
    }

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
            shapes: Vec::new(),
        };
    };
    // The version gate runs before the strict parse: a newer pack carries
    // fields and primitives this build rejects, and must say it is newer.
    if let Some(v) = manifest::version(raw)
        && let Err(e) = manifest::check_version(v)
    {
        c.error("v", e);
        return Report {
            manifest: None,
            problems: c.problems,
            content_hash,
            shapes: Vec::new(),
        };
    }
    let m = match manifest::parse(raw) {
        Ok(m) => m,
        Err(e) => {
            c.error(MANIFEST_FILE, e);
            return Report {
                manifest: None,
                problems: c.problems,
                content_hash,
                shapes: Vec::new(),
            };
        }
    };
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

    let mut shapes = Vec::new();
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
            Slot::Shape(sh) => {
                if m.v < 2 {
                    c.error(at.as_str(), r#"shape slots need "v": 2"#);
                }
                if spec.shape.is_none() {
                    c.error(at.as_str(), "this slot does not take a shape");
                }
                if let Some(limit) = &spec.shape
                    && let Some(n) = c.shape(&at, &sh.shape, limit)
                {
                    shapes.push((name.clone(), n));
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
        shapes,
    }
}

#[cfg(test)]
mod tests {
    use super::super::contract::{ParamSpec, RecipeSpec, ShapeSpec, SlotSpec, StringSpec};
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
        slots: &[
            SlotSpec {
                name: "prop",
                image: false,
                recipes: &[],
                shape: None,
            },
            SlotSpec {
                name: "wall",
                image: false,
                recipes: &[],
                shape: Some(ShapeSpec {
                    max_prims: 4,
                    max_vertices: 3000,
                    extent: [7.5, 0.5, 0.25],
                }),
            },
            SlotSpec {
                name: "player",
                image: true,
                shape: Some(ShapeSpec::cube(12, 3000, 1.5)),
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
            },
        ],
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
                &pack(r#"{"v":3,"id":"demo","name":"D","kind":"palette"}"#),
                &TEST,
                None
            )
            .ok()
        );
    }

    const SHAPE: &str = r#""shape":[{"prim":"sphere","size":[0.3]},{"prim":"cone","size":[0.1,0.3],"at":[0,0.4,0],"rot":[0,0,15],"scale":[1,1,1],"detail":8}]"#;

    fn shape_pack(v: u32, slot: &str, body: &str) -> Files {
        pack(&format!(
            r#"{{"v":{v},"id":"demo","name":"D","kind":"skin","slots":{{"{slot}":{{{body}}}}}}}"#
        ))
    }

    #[test]
    fn a_v1_pack_still_validates_and_v2_is_accepted() {
        assert!(validate(&pack(MIN), &TEST, Some("demo")).ok());
        let v2 = MIN.replace("\"v\":1", "\"v\":2");
        assert!(validate(&pack(&v2), &TEST, Some("demo")).ok());
    }

    #[test]
    fn a_newer_schema_is_refused_cleanly() {
        // A real v3 pack: a top-level field and a primitive this build has
        // never seen. The version gate must answer, not the strict parse.
        let v3 = r#"{"v":3,"id":"demo","name":"D","kind":"skin","sparkle":true,
            "slots":{"player":{"shape":[{"prim":"blob","size":[1],"wobble":2}]}}}"#;
        let r = validate(&pack(v3), &TEST, Some("demo"));
        assert!(!r.ok() && r.manifest.is_none());
        assert_eq!(
            errors(&r),
            vec!["v: made for a newer version of this game (v3)".to_string()]
        );
        let r = validate(&pack(&MIN.replace("\"v\":1", "\"v\":99")), &TEST, None);
        assert_eq!(
            errors(&r),
            vec!["v: made for a newer version of this game (v99)".to_string()]
        );
    }

    #[test]
    fn schema_zero_is_unknown() {
        let r = validate(
            &pack(r#"{"v":0,"junk":[1,2],"slots":{"x":{"what":1}}}"#),
            &TEST,
            None,
        );
        assert!(!r.ok());
        assert_eq!(
            errors(&r),
            vec!["v: unknown schema version (0)".to_string()]
        );
        let r = validate(&pack(&MIN.replace("\"v\":1", "\"v\":0")), &TEST, None);
        assert_eq!(
            errors(&r),
            vec!["v: unknown schema version (0)".to_string()]
        );
    }

    #[test]
    fn a_current_schema_with_an_unknown_field_is_a_parse_error() {
        let r = validate(
            &pack(r#"{"v":2,"id":"demo","name":"D","kind":"skin","sparkle":true}"#),
            &TEST,
            None,
        );
        assert!(!r.ok());
        let e = errors(&r);
        assert!(
            e.len() == 1 && e[0].starts_with("look.json: unknown field `sparkle`"),
            "{e:?}"
        );
    }

    #[test]
    fn a_v2_pack_with_a_shape_validates() {
        let r = validate(&shape_pack(2, "player", SHAPE), &TEST, Some("demo"));
        assert!(r.ok(), "{:?}", errors(&r));
        let m = r.manifest.unwrap();
        let Slot::Shape(s) = &m.slots["player"] else {
            panic!("a shape slot")
        };
        assert_eq!(s.shape.len(), 2);
    }

    #[test]
    fn a_shape_needs_schema_2() {
        let r = validate(&shape_pack(1, "player", SHAPE), &TEST, None);
        assert!(
            errors(&r)
                .iter()
                .any(|e| e.contains(r#"shape slots need "v": 2"#)),
            "{:?}",
            errors(&r)
        );
    }

    #[test]
    fn a_shape_in_a_slot_that_takes_none_is_an_error() {
        let r = validate(&shape_pack(2, "prop", SHAPE), &TEST, None);
        assert!(
            errors(&r)
                .iter()
                .any(|e| e.contains("this slot does not take a shape")),
            "{:?}",
            errors(&r)
        );
    }

    #[test]
    fn unknown_primitive_fields_and_kinds_are_refused() {
        let bad = r#""shape":[{"prim":"sphere","size":[1],"colour":"red"}]"#;
        let r = validate(&shape_pack(2, "player", bad), &TEST, None);
        assert!(!r.ok());
        let e = errors(&r);
        assert!(
            has(&e, "shape[0]: unknown field `colour`"),
            "names the primitive and field: {e:?}"
        );
        let bad = r#""shape":[{"prim":"sphere","size":[1]},{"prim":"pyramid","size":[1]}]"#;
        let r = validate(&shape_pack(2, "player", bad), &TEST, None);
        assert!(!r.ok());
        let e = errors(&r);
        assert!(
            has(&e, "shape[1]: unknown variant `pyramid`"),
            "names the primitive and variant: {e:?}"
        );
    }

    #[test]
    fn an_empty_shape_is_valid() {
        let r = validate(&shape_pack(2, "player", r#""shape":[]"#), &TEST, None);
        assert!(r.ok(), "{:?}", errors(&r));
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

    fn shape_errors(body: &str) -> Vec<String> {
        let r = validate(&shape_pack(2, "player", body), &TEST, None);
        errors(&r)
    }

    fn has(errs: &[String], needle: &str) -> bool {
        errs.iter().any(|e| e.contains(needle))
    }

    #[test]
    fn shape_size_arity_is_checked() {
        let e = shape_errors(
            r#""shape":[{"prim":"sphere","size":[0.3]},{"prim":"cuboid","size":[0.3,0.3]}]"#,
        );
        assert!(has(&e, "slots.player.shape[1]"), "{e:?}");
        assert!(has(&e, "size needs 3"), "{e:?}");
    }

    #[test]
    fn shape_numbers_out_of_range_are_errors() {
        // Stand-ins for NaN: JSON cannot encode it, so use out-of-range values.
        let e = shape_errors(r#""shape":[{"prim":"sphere","size":[0.0]}]"#);
        assert!(has(&e, "slots.player.shape[0]") && has(&e, "size"), "{e:?}");
        let e = shape_errors(r#""shape":[{"prim":"sphere","size":[-1]}]"#);
        assert!(has(&e, "slots.player.shape[0]"), "{e:?}");
        let e = shape_errors(r#""shape":[{"prim":"sphere","size":[1e39]}]"#);
        assert!(has(&e, "slots.player.shape[0]"), "{e:?}");
    }

    #[test]
    fn shape_size_over_the_extent_is_an_error() {
        let e = shape_errors(
            r#""shape":[{"prim":"sphere","size":[0.3]},{"prim":"sphere","size":[3.1]}]"#,
        );
        assert!(has(&e, "slots.player.shape[1]") && has(&e, "size"), "{e:?}");
        assert!(!has(&e, "shape[0]"), "{e:?}");
        let ok = shape_errors(r#""shape":[{"prim":"sphere","size":[1.5]}]"#);
        assert!(ok.is_empty(), "{ok:?}");
    }

    #[test]
    fn shape_size_prefilter_is_twice_the_largest_extent() {
        // 3.1 > 2 * 1.5.
        let e = shape_errors(r#""shape":[{"prim":"cuboid","size":[3.1,0.1,0.1]}]"#);
        assert!(has(&e, "size components are 0.001..=3"), "{e:?}");
    }

    #[test]
    fn shape_at_outside_the_extent_is_an_error() {
        let e = shape_errors(r#""shape":[{"prim":"sphere","size":[0.3],"at":[0,1.6,0]}]"#);
        assert!(has(&e, "slots.player.shape[0]") && has(&e, "at"), "{e:?}");
        let e = shape_errors(r#""shape":[{"prim":"sphere","size":[0.3],"at":[0,-1.6,0]}]"#);
        assert!(has(&e, "at"), "{e:?}");
    }

    fn wall_errors(body: &str) -> Vec<String> {
        errors(&validate(&shape_pack(2, "wall", body), &TEST, None))
    }

    #[test]
    fn non_cubic_extent_accepts_wide_flat_and_rejects_tall_or_deep() {
        // 15 wide, 1 tall, 0.5 deep: exactly the box.
        let ok = wall_errors(r#""shape":[{"prim":"cuboid","size":[7.5,0.5,0.25]}]"#);
        assert!(ok.is_empty(), "{ok:?}");
        // Full-edge cuboid sizes: the default 14 x 1 x 0.3 wall fits.
        let ok = wall_errors(r#""shape":[{"prim":"cuboid","size":[14,1,0.3]}]"#);
        assert!(ok.is_empty(), "{ok:?}");
        // Too long on x: 15 wide passes the size prefilter (<= 15) but
        // scaled by 1.1 it reaches 8.25 > 7.5, caught by the reach check.
        let e = wall_errors(r#""shape":[{"prim":"cuboid","size":[15,1,0.3],"scale":[1.1,1,1]}]"#);
        assert_eq!(e.len(), 1, "{e:?}");
        assert!(e[0].contains(" on x; extent is 7.5"), "{e:?}");
        // Too tall on y: the 0.4 edge scaled by 4 is 1.6 tall, reaching 0.8.
        let e = wall_errors(r#""shape":[{"prim":"cuboid","size":[7.0,0.4,0.2],"scale":[1,4,1]}]"#);
        assert_eq!(e.len(), 1, "{e:?}");
        assert_eq!(
            e[0], "slots.wall.shape: reaches 0.800 on y; extent is 0.5",
            "{e:?}"
        );
        // Too deep on z: the 0.4 edge scaled by 4 reaches 0.8.
        let e = wall_errors(r#""shape":[{"prim":"cuboid","size":[1.0,0.1,0.4],"scale":[1,1,4]}]"#);
        assert_eq!(e.len(), 1, "{e:?}");
        assert_eq!(
            e[0], "slots.wall.shape: reaches 0.800 on z; extent is 0.25",
            "{e:?}"
        );
        // A size over twice the largest extent is still a size error.
        let e = wall_errors(r#""shape":[{"prim":"sphere","size":[15.1]}]"#);
        assert!(has(&e, "size components are 0.001..=15"), "{e:?}");
    }

    #[test]
    fn shape_at_is_bounded_per_axis() {
        let ok = wall_errors(r#""shape":[{"prim":"sphere","size":[0.1],"at":[7.0,0.3,0.1]}]"#);
        assert!(ok.is_empty(), "{ok:?}");
        let e = wall_errors(r#""shape":[{"prim":"sphere","size":[0.1],"at":[0,0.6,0]}]"#);
        assert!(has(&e, "at y is 0.6"), "{e:?}");
        let e = wall_errors(r#""shape":[{"prim":"sphere","size":[0.1],"at":[0,0,0.3]}]"#);
        assert!(has(&e, "at z is 0.3"), "{e:?}");
        assert!(!has(&e, "at x"), "{e:?}");
    }

    #[test]
    fn built_geometry_past_the_extent_is_an_error() {
        // Every number passes on its own (at 1.4, size 0.5, scale 3) but the
        // sphere reaches 1.4 + 0.5 * 3 = 2.9 from the centre.
        let body = r#""shape":[{"prim":"sphere","size":[0.5],"at":[1.4,0,0],"scale":[3,1,1]}]"#;
        let e = shape_errors(body);
        assert_eq!(e.len(), 1, "{e:?}");
        assert!(
            e[0].starts_with("slots.player.shape: reaches 2.9")
                && e[0].ends_with("on x; extent is 1.5"),
            "{e:?}"
        );
        // A rotated cuboid's corner, not its size, is what reaches out.
        let e = shape_errors(
            r#""shape":[{"prim":"cuboid","size":[1.5,1.5,1.5],"at":[0.5,0,0],"rot":[0,45,0]}]"#,
        );
        assert!(has(&e, "slots.player.shape: reaches"), "{e:?}");
        let r = validate(&shape_pack(2, "player", body), &TEST, None);
        assert!(r.shapes.is_empty());
        let ok = shape_errors(r#""shape":[{"prim":"sphere","size":[0.5],"at":[1.0,0,0]}]"#);
        assert!(ok.is_empty(), "{ok:?}");
    }

    #[test]
    fn shape_scale_bounds_are_checked() {
        let e = shape_errors(r#""shape":[{"prim":"sphere","size":[0.3],"scale":[1,0.005,1]}]"#);
        assert!(
            has(&e, "slots.player.shape[0]") && has(&e, "scale"),
            "{e:?}"
        );
        let e = shape_errors(r#""shape":[{"prim":"sphere","size":[0.3],"scale":[4.5,1,1]}]"#);
        assert!(has(&e, "scale"), "{e:?}");
        let ok = shape_errors(r#""shape":[{"prim":"sphere","size":[0.3],"scale":[0.01,4,1]}]"#);
        assert!(ok.is_empty(), "{ok:?}");
    }

    #[test]
    fn too_many_primitives_is_an_error() {
        let one = r#"{"prim":"cuboid","size":[0.1,0.1,0.1]}"#;
        let body = format!(r#""shape":[{}]"#, [one; 13].join(","));
        let e = shape_errors(&body);
        assert!(
            has(&e, "slots.player.shape") && has(&e, "13 primitives; at most 12"),
            "{e:?}"
        );
        let body = format!(r#""shape":[{}]"#, [one; 12].join(","));
        assert!(shape_errors(&body).is_empty());
    }

    #[test]
    fn the_vertex_budget_is_enforced() {
        // A detail-32 sphere is 33 * 17 = 561 vertices; six of them is 3366.
        let one = r#"{"prim":"sphere","size":[0.3],"detail":32}"#;
        let body = format!(r#""shape":[{}]"#, [one; 6].join(","));
        let e = shape_errors(&body);
        assert!(
            has(&e, "slots.player.shape") && has(&e, "3366 vertices; budget 3000"),
            "{e:?}"
        );
        let body = format!(r#""shape":[{}]"#, [one; 5].join(","));
        assert!(shape_errors(&body).is_empty());
    }

    #[test]
    fn detail_out_of_range_is_an_error_not_a_clamp() {
        let e = shape_errors(r#""shape":[{"prim":"sphere","size":[0.3],"detail":2}]"#);
        assert!(
            has(&e, "slots.player.shape[0]") && has(&e, "detail is 3..=32"),
            "{e:?}"
        );
        let e = shape_errors(r#""shape":[{"prim":"sphere","size":[0.3],"detail":33}]"#);
        assert!(has(&e, "detail is 3..=32"), "{e:?}");
    }

    #[test]
    fn a_valid_shape_reports_its_vertex_count() {
        let r = validate(&shape_pack(2, "player", SHAPE), &TEST, Some("demo"));
        assert!(r.ok(), "{:?}", errors(&r));
        // sphere d12: 13 * 7 = 91; cone d8: 17.
        assert_eq!(r.shapes, vec![("player".to_string(), 108)]);
        let r = validate(&shape_pack(2, "player", r#""shape":[]"#), &TEST, None);
        assert_eq!(r.shapes, vec![("player".to_string(), 0)]);
    }

    #[test]
    fn an_invalid_shape_reports_no_vertex_count() {
        let r = validate(
            &shape_pack(2, "player", r#""shape":[{"prim":"sphere","size":[9]}]"#),
            &TEST,
            None,
        );
        assert!(r.shapes.is_empty());
    }
}
