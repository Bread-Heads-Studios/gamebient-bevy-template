//! Holds every embedded face's fitting tables (`advance_em` aside) to the
//! real font, measured as Bevy draws it: each string is **shaped** with
//! cosmic-text (the version and features `bevy_text` uses, `Shaping::Advanced`,
//! default OpenType features, no hinting), so kerning and ligatures count.
//!
//! Test-only (`#[cfg(test)] mod font_metrics;` in `ui/mod.rs`). Copy this
//! file into a game **unchanged except the config block just below**, with
//! the dev-dependency
//! `cosmic-text = { version = "=0.16.0", features = ["shape-run-cache"] }`
//! (already in every game's lockfile through Bevy).
//!
//! **What a game sets in the config block** (two constants, nothing else):
//!
//! - `DEFAULT_TITLE`: the game's own string for the look key below, usually
//!   `crate::ui::menu::TITLE` (or wherever the game keeps it). The layout
//!   tests fit it and check the clamp never touches it.
//! - `TITLE_KEY`: the `theme::LOOK` string key that string is the default of
//!   (`"title"` in the template). It must be one of `theme::STRINGS`; the
//!   pack strings the tests build are sized from that key's `max_chars`.
//!
//! The clamp test lays the title line out in **`ui::fonts::BODY`**, the
//! face the template's body text uses (and whose `<---` ligature it was
//! written to catch), so the game's `fonts.rs` needs a `Face` named `BODY`
//! (every template game has one). It is not part of the config block: the
//! face-dependent checks adapt to whatever `BODY` measures.
//!
//! Beyond the block the file names only `ui::fonts::{ALL, BODY}`, `ui::fit`,
//! `ui::theme` (`LOOK`, `STRINGS`) and `look::refit`. Do not edit the test
//! bodies: the face-dependent checks adapt themselves. A **monospace** face
//! (every printable advance equal, measured from its own table) has no
//! narrow glyphs for the real-width estimate to win back, so the "close to
//! the default size" test skips it; a face with **no widening ligature**
//! (measured with the shaper) is not required to defeat the estimate with
//! `<---`. The clamp assertions run for every face either way.
//!
//! The clamp tests run Bevy's real UI layout headless, which needs bevy's
//! `bevy_picking` feature: on in native builds through Bevy's defaults. A
//! game with `default-features = false` and no picking adds it as a
//! test-only dev-dependency feature (`[dev-dependencies] bevy = { version =
//! "0.18", default-features = false, features = ["bevy_picking"] }`, as
//! Gravestone did).
//!
//! When a game adds or changes a face, run
//! `cargo test print_glyph_tables -- --ignored --nocapture` and paste the
//! printed `advances` rows (into a `const NAME_ADVANCES: Advances` with
//! `#[allow(clippy::approx_constant)]`: a measured 0.3010 reads as log10
//! 2), its `widest_em`, and its `kerns` list (`kerns: &[]` when it prints
//! none, as for every template face) into `fonts.rs`.

// ---------------------------------------------------------------------
// GAME CONFIG: the only part of this file a game edits.
// ---------------------------------------------------------------------

/// The game's own string for `TITLE_KEY` (its default look).
const DEFAULT_TITLE: &str = crate::ui::menu::TITLE;
/// The `theme::LOOK` string key whose default is `DEFAULT_TITLE`.
const TITLE_KEY: &str = "title";

// ---------------------------------------------------------------------
// End of the config block.
// ---------------------------------------------------------------------

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use cosmic_text::{
    Attrs, Buffer, Family, FontSystem, Metrics, Shaping, Stretch, Style, Weight, Wrap,
};

use crate::look::refit::{self, PRINTABLE};
use crate::ui::fit::Face;
use crate::ui::fonts::ALL;

/// Printable ASCII, every character the Look Pack validator accepts.
pub(crate) const ASCII: std::ops::RangeInclusive<char> = ' '..='~';

struct Shaper {
    fonts: FontSystem,
    buffer: Buffer,
    family: String,
    style: Style,
    stretch: Stretch,
    weight: Weight,
    upem: f32,
}

impl Shaper {
    fn new(face: &Face) -> Self {
        let mut db = cosmic_text::fontdb::Database::new();
        let ids = db.load_font_source(cosmic_text::fontdb::Source::Binary(Arc::new(face.bytes)));
        let info = db.face(*ids.last().expect("the face loads")).unwrap();
        let (family, style, stretch) = (info.families[0].0.clone(), info.style, info.stretch);
        let upem = f32::from(
            ttf_parser::Face::parse(face.bytes, 0)
                .unwrap()
                .units_per_em(),
        );
        let mut fonts = FontSystem::new_with_locale_and_db("en-US".into(), db);
        // Sized at units-per-em, so widths come out in font units.
        let mut buffer = Buffer::new(&mut fonts, Metrics::new(upem, upem));
        buffer.set_wrap(&mut fonts, Wrap::None);
        Self {
            fonts,
            buffer,
            family,
            style,
            stretch,
            weight: Weight(face.weight.0),
            upem,
        }
    }

    /// Shapes `text`; returns each glyph's byte range, glyph id and advance
    /// (kerning included), in ems.
    fn glyphs(&mut self, text: &str) -> Vec<(usize, usize, u16, f32)> {
        self.shape(text);
        self.buffer
            .layout_runs()
            .flat_map(|run| {
                run.glyphs
                    .iter()
                    .map(|g| (g.start, g.end, g.glyph_id, g.w / self.upem))
            })
            .collect()
    }

    fn shape(&mut self, text: &str) {
        let attrs = Attrs::new()
            .family(Family::Name(&self.family))
            .style(self.style)
            .stretch(self.stretch)
            .weight(self.weight);
        self.buffer
            .set_text(&mut self.fonts, text, &attrs, Shaping::Advanced, None);
        self.buffer.shape_until_scroll(&mut self.fonts, false);
    }

    /// The shaped width of `text`, in ems: the right edge of its last glyph
    /// (trailing spaces included).
    fn em(&mut self, text: &str) -> f32 {
        self.shape(text);
        let right = self
            .buffer
            .layout_runs()
            .flat_map(|run| run.glyphs.iter().map(|g| g.x + g.w))
            .fold(0.0, f32::max);
        right / self.upem
    }
}

thread_local! {
    static SHAPERS: RefCell<HashMap<&'static str, Shaper>> = RefCell::new(HashMap::new());
    static FORMS: RefCell<HashMap<&'static str, Vec<f32>>> = RefCell::new(HashMap::new());
}

fn with_shaper<T>(face: &Face, f: impl FnOnce(&mut Shaper) -> T) -> T {
    SHAPERS.with(|s| {
        f(s.borrow_mut()
            .entry(face.name)
            .or_insert_with(|| Shaper::new(face)))
    })
}

/// The advance `Face::advances` declares for `c` in `face`, in ems: the
/// widest advance `c`'s glyph takes when shaped alone or anywhere in any
/// three printable characters, **kerning included** (cosmic-text folds a pair's
/// kern into the left glyph's advance).
///
/// Why per glyph and not a plain advance plus a sparse table of widening
/// pairs: these faces kern in context. Inter swaps in case-sensitive
/// alternates beside capitals and digits (`*` is 0.12 em wider, `x`
/// between digits becomes a wider `×`), the swap reaches through a run
/// (`"**I"` widens both asterisks) and the alternates carry their own
/// kerns (`"1)a"` widens though `")a"` does not), so no table of pairs
/// measured alone bounds a string. The per-glyph maximum does, at a small
/// cost: about 0.015 em on Inter's letters, up to 0.04 em on a few of
/// Fraunces Italic's. Ligatures (one glyph for several characters) are
/// left to the residual pair table, `Face::kerns`.
pub(crate) fn widest_form_em(face: &Face, c: char) -> f32 {
    FORMS.with(|f| {
        f.borrow_mut().entry(face.name).or_insert_with(|| {
            with_shaper(face, |s| {
                // Every printable character alone, then every glyph of every
                // three-character string: each character is seen with up to
                // two neighbours on either side.
                let mut widest: Vec<f32> = ASCII.map(|c| s.em(&c.to_string())).collect();
                let mut text = String::with_capacity(3);
                for x in ASCII {
                    for y in ASCII {
                        for z in ASCII {
                            text.clear();
                            text.extend([x, y, z]);
                            for (start, end, _, w) in s.glyphs(&text) {
                                if end == start + 1 {
                                    let i = text.as_bytes()[start] as usize - 0x20;
                                    widest[i] = widest[i].max(w);
                                }
                            }
                        }
                    }
                }
                widest
            })
        })[c as usize - 0x20]
    })
}

pub(crate) fn real_advance_em(face: &Face, text: &str) -> f32 {
    with_shaper(face, |s| s.em(text))
}

/// How much the pair `a` `b` widens (positive) or tightens (negative) a
/// line in `face` beyond the two glyphs' widest forms, in ems.
pub(crate) fn real_kern_em(face: &Face, a: char, b: char) -> f32 {
    let pair: String = [a, b].iter().collect();
    real_advance_em(face, &pair) - widest_form_em(face, a) - widest_form_em(face, b)
}

/// Below this a pair counts as not widening (shaping noise, in ems).
const NOISE: f32 = 1e-5;

/// A table value: four decimals, never under the measured f32.
fn table_value(real: f32) -> f32 {
    let mut v = (f64::from(real) * 1e4).round() / 1e4;
    if (v as f32) < real {
        v += 1e-4;
    }
    v as f32
}

/// Every widening pair of `face` over printable ASCII, measured.
fn measured_kerns(face: &Face) -> Vec<(char, char, f32)> {
    let mut out = Vec::new();
    for a in ASCII {
        for b in ASCII {
            let k = real_kern_em(face, a, b);
            if k > NOISE {
                out.push((a, b, k));
            }
        }
    }
    out
}

/// Largest spread between printable advances still called one width.
const MONO_TOLERANCE: f32 = 0.005;

/// Whether a table of advances is monospace: every entry equal within
/// [`MONO_TOLERANCE`].
fn advances_are_monospace(advances: &[f32]) -> bool {
    let lo = advances.iter().copied().fold(f32::INFINITY, f32::min);
    let hi = advances.iter().copied().fold(0.0, f32::max);
    !advances.is_empty() && hi - lo <= MONO_TOLERANCE
}

/// A face is monospace by its own declared `advances` table (which
/// `declared_advances_match_every_printable_ascii_glyph` holds to the font).
fn is_monospace(face: &Face) -> bool {
    advances_are_monospace(face.advances)
}

/// A shaped run is a widening ligature when it is wider than its parts
/// shaped alone by more than this many ems.
const LIGATURE_MARGIN: f32 = 0.05;

fn widens(whole_em: f32, parts_em: f32) -> bool {
    whole_em > parts_em + LIGATURE_MARGIN
}

/// Whether the face shapes `<---` wider than its four characters alone
/// (Inter's arrow ligature does; Plex Mono and Courier Prime do not),
/// measured with the shaper.
fn has_widening_ligature(face: &Face) -> bool {
    let parts: f32 = "<---"
        .chars()
        .map(|c| real_advance_em(face, &c.to_string()))
        .sum();
    widens(real_advance_em(face, "<---"), parts)
}

/// Deterministic pseudo-random strings (xorshift) over printable ASCII.
fn sample_strings(n: usize, len: usize, seed: u64) -> Vec<String> {
    let mut x = seed;
    let mut next = move || {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        x
    };
    (0..n)
        .map(|_| {
            (0..len)
                .map(|_| char::from(b' ' + (next() % PRINTABLE as u64) as u8))
                .collect()
        })
        .collect()
}

/// The face's most widening pair, chained to `len` characters
/// (`"*I*I*I…"` in Inter): the adversarial kerned string.
fn kerned_worst(face: &Face, len: usize) -> String {
    let &(a, b, _) = face
        .kerns
        .iter()
        .max_by(|x, y| x.2.total_cmp(&y.2))
        .unwrap_or(&('W', 'W', 0.0));
    (0..len).map(|i| if i % 2 == 0 { a } else { b }).collect()
}

/// Asserts `text` in a `LookFit` against `box_px` really fits, shaped.
fn assert_fits(face: &Face, text: &str, default: &str, max: f32, box_px: f32) -> f32 {
    let fit = face.look_fit_in("k", max, box_px);
    let size = fit.size_for(text, default);
    let shaped = real_advance_em(face, text) * size;
    assert!(
        size == 1.0 || shaped <= box_px,
        "{}: {text:?} at {size} is {shaped:.1} px shaped > {box_px} px",
        face.name
    );
    size
}

#[test]
fn declared_advances_match_every_printable_ascii_glyph() {
    for face in ALL {
        for (i, c) in ASCII.enumerate() {
            let real = widest_form_em(face, c);
            let declared = face.advances[i];
            assert!(
                declared >= real && declared - real < 2e-4,
                "{}: {c:?} declared {declared} em, its widest form shapes to {real:.5}",
                face.name
            );
        }
        // `widest_em` is the worst-case glyph the box fit sizes against: it
        // must never undersize against the measured table (that is what
        // keeps a `max_chars` string of widest glyphs inside its box), and
        // it must stay close to it (a loose value shrinks every box fit).
        // It is *not* required to be at least `advance_em`: `advance_em` is
        // the declared average that default fitted sizes come from (and so
        // must not change), and a monospace face may declare it a hair
        // above its one real advance (Plex Mono: measured 0.600, declared
        // 0.61) with nothing wrong. The two only need to be the same order
        // (an `advance_em` far above `widest_em` is a typo).
        let widest = refit::widest_em(face.advances);
        assert!(
            face.widest_em >= widest && face.widest_em - widest < 0.01,
            "{}: widest_em {} against the table's {widest}",
            face.name,
            face.widest_em
        );
        assert!(
            face.advance_em <= face.widest_em + 0.05,
            "{}: advance_em {} is far above widest_em {}",
            face.name,
            face.advance_em,
            face.widest_em
        );
    }
}

/// Each face's `kerns` is exactly its widening pairs, sorted, each value
/// at least the shaped kern and within 2e-4 of it.
#[test]
fn declared_kerns_match_every_widening_pair() {
    for face in ALL {
        assert!(
            face.kerns
                .windows(2)
                .all(|w| (w[0].0, w[0].1) < (w[1].0, w[1].1)),
            "{}: kerns are not sorted by (left, right) without repeats",
            face.name
        );
        let measured = measured_kerns(face);
        for &(a, b, k) in &measured {
            let declared = refit::pair_kern_em(face.kerns, a, b);
            assert!(
                declared >= k && declared - k < 2e-4,
                "{}: pair {a:?}{b:?} widens {k:.5} em, declared {declared}",
                face.name
            );
        }
        assert_eq!(
            face.kerns.len(),
            measured.len(),
            "{}: kerns lists a pair that does not widen",
            face.name
        );
    }
}

/// Prints each face's tables in the form `fonts.rs` declares them.
#[test]
#[ignore = "a measuring tool, not a check"]
fn print_glyph_tables() {
    for face in ALL {
        println!("// {}: advances", face.name);
        let rows: Vec<String> = ASCII
            .map(|c| format!("{:.4}", table_value(widest_form_em(face, c))))
            .collect();
        for row in rows.chunks(8) {
            println!("    {},", row.join(", "));
        }
        let widest = (0..PRINTABLE)
            .map(|i| table_value(widest_form_em(face, char::from(b' ' + i as u8))))
            .fold(0.0, f32::max);
        println!(
            "// {}: widest_em {:.3}",
            face.name,
            (widest * 1000.0).ceil() / 1000.0
        );
        let kerns = measured_kerns(face);
        println!("// {}: kerns ({})", face.name, kerns.len());
        for row in kerns.chunks(4) {
            let row: Vec<String> = row
                .iter()
                .map(|&(a, b, k)| format!("({a:?}, {b:?}, {:.4})", table_value(k)))
                .collect();
            println!("    {},", row.join(", "));
        }
    }
}

/// The worst string a pack may set for any key (its `max_chars` of the
/// face's widest glyph, and the face's most widening pair chained) fits
/// the line a `LookFit` gives it, shaped, on the view and in a narrow box;
/// the default string keeps exactly its `fitted` size.
#[test]
fn a_worst_case_pack_string_fits_its_look_fit_line() {
    for face in ALL {
        let widest = ASCII
            .max_by(|a, b| {
                real_advance_em(face, &a.to_string())
                    .total_cmp(&real_advance_em(face, &b.to_string()))
            })
            .unwrap();
        for spec in crate::ui::theme::STRINGS {
            let worst = [
                widest.to_string().repeat(spec.max_chars),
                kerned_worst(face, spec.max_chars),
            ];
            for text in &worst {
                for box_px in [
                    crate::ui::fit::ui_width() * crate::ui::fit::MAX_LINE_SHARE,
                    300.0,
                ] {
                    assert_fits(face, text, spec.default, 88.0, box_px);
                }
            }
            assert_eq!(
                face.look_fit(spec.key, 88.0)
                    .size_for(spec.default, spec.default),
                face.fitted(spec.default, 88.0).font_size,
                "{} {}",
                face.name,
                spec.key
            );
        }
    }
}

/// Strings that overflowed when kerning was left out (the round-2 review:
/// `"*LIMITED*"` in Inter by 21 px), plus the adversarial chained pair,
/// all fit their box shaped, in every face, across box widths.
#[test]
fn kerned_pack_strings_fit_their_box() {
    for face in ALL {
        let adversarial = kerned_worst(face, 24);
        for text in [
            "*LIMITED*",
            "SPONSORED BY FIZZIX*",
            "LITTLE ITTY",
            "giant TTT",
            adversarial.as_str(),
        ] {
            for box_px in [120.0, 200.0, 300.0, 450.0, 487.0, 600.0, 862.0] {
                assert_fits(face, text, "GAMEBIENT GAME", 88.0, box_px);
            }
        }
    }
}

/// The estimate bounds the shaped width of every printable three-character
/// string (857 375 per face: every context a glyph or pair sees), and of
/// 400 pseudo-random 24-character ones.
#[test]
fn the_tables_bound_the_shaped_width_of_every_triple_and_random_strings() {
    for face in ALL {
        for x in ASCII {
            for c in ASCII {
                for y in ASCII {
                    let text: String = [x, c, y].iter().collect();
                    let bound = refit::text_em(face.advances, face.kerns, &text);
                    let shaped = real_advance_em(face, &text);
                    assert!(
                        shaped <= bound + 1e-4,
                        "{}: {text:?} shapes to {shaped:.4} em > bound {bound:.4}",
                        face.name
                    );
                }
            }
        }
        for text in sample_strings(400, 24, 0x9e37_79b9_7f4a_7c15) {
            let bound = refit::text_em(face.advances, face.kerns, &text);
            let shaped = real_advance_em(face, &text);
            assert!(
                shaped <= bound + 1e-4,
                "{}: {text:?} shapes to {shaped:.4} em > bound {bound:.4}",
                face.name
            );
        }
    }
}

/// A pack's ordinary words come out close to the default's size: they are
/// fitted by their real width, not as if every glyph were the widest.
/// Attic Excavator's tagline box was the case that shrank a pack tagline
/// to about 14 px against the default 21.
#[test]
fn a_realistic_pack_string_gets_close_to_the_default_size() {
    let pack = "CHILL IT. CHUCK IT.";
    for face in ALL {
        // A monospace face has nothing to win back: every glyph is as wide
        // as the widest, so the widest-glyph rule already equals the real
        // width and there is no larger honest size to get close to. The
        // proportional faces are all still held to it.
        if is_monospace(face) {
            continue;
        }
        for spec in crate::ui::theme::STRINGS {
            for max in [21.0, 88.0] {
                // A box the widest-glyph rule would have bound in.
                let box_px = pack.len() as f32 * face.widest_em * max * 0.7;
                let fit = face.look_fit_in(spec.key, max, box_px);
                let own = fit.default_size(spec.default);
                let old = refit::size_for_em(face.widest_em, pack, own, box_px);
                let size = assert_fits(face, pack, spec.default, max, box_px);
                if old < own - 1.0 {
                    assert!(
                        size >= own - 1.0,
                        "{}: {pack:?} at {size} against the default's {own} (old rule {old})",
                        face.name
                    );
                }
            }
        }
    }
}

use bevy::prelude::{App, MinimalPlugins, Text, TextFont};
use bevy::text::TextLayoutInfo;

/// A real, headless Bevy UI app: text shaping, layout and the clamp as a
/// game runs them, with no window surface and no renderer. A `box_px`-wide
/// parent holds one `LookFit` line under a pack setting `title` to `text`
/// (or the default look for `None`). Returns (the size, the laid-out width,
/// the number of visual lines) after `frames` frames.
fn real_layout(
    face: &'static Face,
    text: Option<&str>,
    box_px: f32,
    frames: usize,
) -> (f32, f32, usize) {
    use crate::look::{self, ActiveLook, LookPlugin, LookText, Selection};
    use bevy::asset::AssetApp;
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin::default(),
        bevy::state::app::StatesPlugin,
        bevy::input::InputPlugin,
        bevy::window::WindowPlugin::default(),
        bevy::transform::TransformPlugin,
        bevy::camera::CameraPlugin,
        bevy::text::TextPlugin,
        bevy::ui::UiPlugin,
    ))
    .init_asset::<bevy::image::Image>()
    .init_asset::<bevy::image::TextureAtlasLayout>()
    .init_asset::<bevy::mesh::Mesh>()
    .init_asset::<bevy::sprite_render::ColorMaterial>()
    // bevy_ui's picking backend and `viewport_picking` (always on with
    // bevy_ui's `bevy_picking` feature) need picking's own plugins.
    .add_plugins(bevy::picking::DefaultPickingPlugins)
    .insert_resource(Selection::Default)
    .add_plugins(LookPlugin {
        contract: &crate::ui::theme::LOOK,
    });
    {
        let mut fonts = app
            .world_mut()
            .resource_mut::<bevy::asset::Assets<bevy::text::Font>>();
        for f in ALL {
            fonts
                .insert(
                    f.handle.id(),
                    bevy::text::Font::try_from_bytes(f.bytes.to_vec()).unwrap(),
                )
                .unwrap();
        }
    }
    if let Some(t) = text {
        let json = format!(
            r#"{{"v":1,"id":"p","name":"P","kind":"palette","strings":{{"{TITLE_KEY}":"{t}"}}}}"#
        );
        let files = std::collections::BTreeMap::from([(
            look::manifest::MANIFEST_FILE.to_string(),
            json.into_bytes(),
        )]);
        app.insert_resource(
            ActiveLook::from_files(&crate::ui::theme::LOOK, files, Selection::Default, None)
                .expect("valid pack"),
        );
    }
    app.world_mut().spawn(bevy::prelude::Camera2d);
    let line = app
        .world_mut()
        .spawn((
            Text::new(DEFAULT_TITLE),
            face.font(10.0),
            LookText(TITLE_KEY),
            face.look_fit_in(TITLE_KEY, 88.0, box_px),
        ))
        .id();
    app.world_mut()
        .spawn((bevy::prelude::Node {
            width: bevy::prelude::Val::Px(box_px),
            ..Default::default()
        },))
        .add_child(line);
    for _ in 0..frames {
        app.update();
    }
    let w = app.world();
    let layout = w.get::<TextLayoutInfo>(line).unwrap();
    let mut tops: Vec<f32> = layout.run_geometry.iter().map(|r| r.bounds.min.y).collect();
    tops.dedup_by(|a, b| (*a - *b).abs() <= 0.5);
    (
        w.get::<TextFont>(line).unwrap().font_size,
        layout.size.x,
        tops.len(),
    )
}

/// `max_chars` of the key the layout tests set (`TITLE_KEY`): the longest
/// pack string the validator accepts there.
fn title_max_chars() -> usize {
    crate::ui::theme::LOOK
        .strings
        .iter()
        .find(|s| s.key == TITLE_KEY)
        .unwrap_or_else(|| panic!("TITLE_KEY {TITLE_KEY:?} is not in theme::STRINGS"))
        .max_chars
}

/// The pack strings the clamp test sets, none longer than the key's
/// `max_chars`: the fixed adversarial ones that fit, plus ones built to
/// that length (the ligature repeated, a widest-glyph run ending in it).
fn clamp_cases(max_chars: usize) -> Vec<(String, Vec<f32>)> {
    let fixed: [(&str, &[f32]); 5] = [
        ("GO <--- HERE", &[300.0, 250.0, 420.0]),
        ("PRESS START <---", &[400.0, 330.0, 520.0]),
        ("<---", &[120.0, 200.0]),
        ("<----->", &[150.0, 260.0]),
        ("$*[jU", &[90.0, 140.0, 230.0]),
    ];
    let mut cases: Vec<(String, Vec<f32>)> = fixed
        .iter()
        .filter(|(t, _)| t.len() <= max_chars)
        .map(|(t, b)| (t.to_string(), b.to_vec()))
        .collect();
    if max_chars >= 8 {
        cases.push(("<---".repeat(max_chars / 4), vec![160.0, 300.0]));
        let tail = format!("{}<---", "W".repeat(max_chars - 4));
        cases.push((tail, vec![200.0, 360.0, 520.0]));
    }
    assert!(
        !cases.is_empty(),
        "{TITLE_KEY:?} allows only {max_chars} characters: too short to test the clamp"
    );
    cases
}

/// The clamp is the fit guarantee, in Bevy's real UI layout. Strings
/// whose shaping rules reach past the tables' three-character contexts
/// (Inter's `<---` arrow ligature, 0.6 em wider than its parts; the
/// five-character `$*[jU` alternate chain) overflow the estimate: a single
/// word runs past its box, a multi-word line wraps. After a few frames
/// every one sits on one line within its box (**for every face**). Where
/// the face really has a widening `<---` ligature (measured with the
/// shaper) the estimate alone (frame 1) is asserted to wrap or overflow for
/// the arrow lines, so this test fails if the tables ever change in a way
/// that hides the case it exists for; a face without one (a monospace
/// body) has no such premise and only the clamp assertions run.
#[test]
fn the_clamp_brings_estimate_overflows_inside_the_box() {
    let body = &crate::ui::fonts::BODY;
    let mut estimate_failed = false;
    for (text, boxes) in clamp_cases(title_max_chars()) {
        for box_px in boxes {
            let (estimate, first_width, first_lines) = real_layout(body, Some(&text), box_px, 1);
            let (size, width, lines) = real_layout(body, Some(&text), box_px, 8);
            assert!(
                lines == 1 && width <= box_px,
                "{text:?} in {box_px}: {lines} line(s), {width} px at {size} after the clamp"
            );
            assert!(size <= estimate, "{text:?}: the clamp grew a line");
            if text.contains("<---") && (first_lines > 1 || first_width > box_px) {
                estimate_failed = true;
            }
        }
    }
    if has_widening_ligature(body) {
        assert!(
            estimate_failed,
            "the arrow ligature no longer defeats the estimate: this test proves nothing"
        );
    }
}

/// The detection the face-dependent tests rely on, on synthetic data.
#[test]
fn monospace_and_ligature_detection() {
    assert!(advances_are_monospace(&[0.6; 95]));
    assert!(advances_are_monospace(&[0.600, 0.602, 0.598]));
    let mut one_wide = [0.55; 95];
    one_wide[b'W' as usize - 0x20] = 1.1;
    assert!(!advances_are_monospace(&one_wide));
    assert!(!advances_are_monospace(&[]));
    // Four parts of 0.5 em: a 2.0 em run is just the parts, a 2.6 em one
    // is a ligature.
    assert!(!widens(2.0, 2.0));
    assert!(!widens(2.04, 2.0));
    assert!(widens(2.6, 2.0));
    // The faces agree with themselves: a monospace face's advances are
    // all one width, a proportional one's are not.
    for face in ALL {
        let mono = is_monospace(face);
        let (lo, hi) = face
            .advances
            .iter()
            .fold((f32::INFINITY, 0.0f32), |(l, h), &a| (l.min(a), h.max(a)));
        assert_eq!(mono, hi - lo <= MONO_TOLERANCE, "{}", face.name);
    }
}

/// The key's `max_chars` bounds every string the clamp test builds, so a
/// game with a short title key never makes the test set an invalid pack.
#[test]
fn clamp_cases_respect_the_keys_max_chars() {
    for max in [5, 8, 12, 16, 20, 40] {
        for (text, _) in clamp_cases(max) {
            assert!(text.len() <= max, "{text:?} over {max}");
        }
    }
    for (text, _) in clamp_cases(title_max_chars()) {
        assert!(text.len() <= title_max_chars());
    }
}

/// The game's own string keeps exactly its `fitted` size: under the
/// default look the clamp never runs, even where the default overflows a
/// narrow box (a default-look bug the clamp must not paper over), and a
/// pack that sets the key to the default string changes nothing either.
#[test]
fn the_clamp_never_touches_the_default() {
    for face in ALL {
        let fitted = face.fitted(DEFAULT_TITLE, 88.0).font_size;
        for box_px in [80.0, 300.0, 900.0] {
            let (size, _, _) = real_layout(face, None, box_px, 6);
            assert_eq!(size, fitted, "{} in {box_px}", face.name);
            let (size, _, _) = real_layout(face, Some(DEFAULT_TITLE), box_px, 6);
            assert_eq!(
                size, fitted,
                "{} in {box_px}, a pack's default string",
                face.name
            );
        }
    }
}
