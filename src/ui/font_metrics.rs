//! Holds every embedded face's fitting tables (`advance_em` aside) to the
//! real font, measured as Bevy draws it: each string is **shaped** with
//! cosmic-text (the version and features `bevy_text` uses, `Shaping::Advanced`,
//! default OpenType features, no hinting), so kerning and ligatures count.
//!
//! Test-only (`#[cfg(test)] mod font_metrics;` in `ui/mod.rs`). Copy this
//! file into a game verbatim, with the dev-dependency
//! `cosmic-text = { version = "=0.16.0", features = ["shape-run-cache"] }`
//! (already in every game's lockfile through Bevy). It names only
//! `ui::fonts::ALL`, `ui::fit` and `ui::theme::STRINGS`.
//!
//! When a game adds or changes a face, run
//! `cargo test print_glyph_tables -- --ignored --nocapture` and paste the
//! printed `advances` rows (into a `const NAME_ADVANCES: Advances` with
//! `#[allow(clippy::approx_constant)]`: a measured 0.3010 reads as log10
//! 2), its `widest_em`, and its `kerns` list (`kerns: &[]` when it prints
//! none, as for every template face) into `fonts.rs`.

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
        let widest = refit::widest_em(face.advances);
        assert!(
            face.widest_em >= widest && face.widest_em - widest < 0.01,
            "{}: widest_em {} against the table's {widest}",
            face.name,
            face.widest_em
        );
        assert!(face.widest_em >= face.advance_em, "{}", face.name);
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

/// Stands in for Bevy's UI text layout (which needs a window and a
/// renderer): whenever a `LookFit` line's text or font changes, it writes
/// the line's width as Bevy measures it, the cosmic-text shaped width at
/// that size rounded up to a whole pixel, into `TextLayoutInfo`. Runs
/// where Bevy's `text_system` does: after the look's refit, before the
/// clamp.
fn stand_in_layout(
    mut q: Query<
        (&Text, Ref<TextFont>, &mut TextLayoutInfo),
        bevy::prelude::With<crate::look::LookFit>,
    >,
) {
    for (text, font, mut layout) in &mut q {
        if font.is_changed() || layout.size.x <= 0.0 {
            let face = ALL
                .iter()
                .find(|f| f.handle == font.font)
                .expect("an embedded face");
            layout.size.x = (real_advance_em(face, &text.0) * font.font_size).ceil();
        }
    }
}

use bevy::prelude::{App, DetectChanges, MinimalPlugins, Query, Ref, Text, TextFont};
use bevy::text::TextLayoutInfo;

/// A `LookFit` line under `pack` (string `title` set to `text`), in a box
/// `box_px` wide, laid out by `stand_in_layout` and, if `clamp`, clamped.
/// Returns (the estimate's size, the size and laid-out width after a few
/// frames).
fn laid_out(face: &'static Face, text: Option<&str>, box_px: f32, clamp: bool) -> (f32, f32, f32) {
    use crate::look::{self, ActiveLook, LookSet, LookText, Selection};
    use bevy::prelude::{IntoScheduleConfigs, PostUpdate};
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_message::<look::LookChanged>()
        .insert_resource(match text {
            Some(t) => {
                let json = format!(
                    r#"{{"v":1,"id":"p","name":"P","kind":"palette","strings":{{"title":"{t}"}}}}"#
                );
                let files = std::collections::BTreeMap::from([(
                    look::manifest::MANIFEST_FILE.to_string(),
                    json.into_bytes(),
                )]);
                ActiveLook::from_files(&crate::ui::theme::LOOK, files, Selection::Default, None)
                    .expect("valid pack")
            }
            None => ActiveLook::default_for(&crate::ui::theme::LOOK),
        })
        .configure_sets(PostUpdate, LookSet)
        .add_systems(
            PostUpdate,
            (look::apply::retext, look::refit::refit_text).in_set(LookSet),
        )
        .add_systems(PostUpdate, stand_in_layout.after(LookSet));
    if clamp {
        app.add_systems(PostUpdate, look::refit::clamp_fit.after(stand_in_layout));
    }
    let default = crate::ui::menu::TITLE;
    let fit = face.look_fit_in("title", 88.0, box_px);
    let e = app
        .world_mut()
        .spawn((
            Text::new(default),
            face.font(10.0),
            LookText("title"),
            fit,
            TextLayoutInfo::default(),
        ))
        .id();
    app.update();
    let estimate = app.world().get::<TextFont>(e).unwrap().font_size;
    for _ in 0..6 {
        app.update();
    }
    let w = app.world();
    (
        estimate,
        w.get::<TextFont>(e).unwrap().font_size,
        w.get::<TextLayoutInfo>(e).unwrap().size.x,
    )
}

/// The clamp is the fit guarantee. Strings whose shaping rules reach past
/// the tables' three-character contexts (Inter's `<---` arrow ligature,
/// 0.6 em wider than its parts; the five-character `$*[jU` alternate
/// chain) overflow the estimate; after a few frames of layout and clamp
/// every one fits its box. Without the clamp the arrow overflows (asserted
/// here, so this test fails if the tables ever change in a way that hides
/// the case it exists for).
#[test]
fn the_clamp_brings_estimate_overflows_inside_the_box() {
    let body = &crate::ui::fonts::BODY;
    let mut overflowed_without_clamp = false;
    for text in ["PRESS START <---", "GO <--- HERE", "$*[jU", "<----->"] {
        for box_px in (60..=900).step_by(29).map(|b| b as f32) {
            let (_, size, width) = laid_out(body, Some(text), box_px, true);
            assert!(
                size == 1.0 || width <= box_px,
                "{text:?} in {box_px}: {width} px at {size} after the clamp"
            );
            let (estimate, _, unclamped) = laid_out(body, Some(text), box_px, false);
            assert!(size <= estimate, "{text:?}: the clamp grew a line");
            overflowed_without_clamp |= unclamped > box_px && text.contains("<---");
        }
    }
    assert!(
        overflowed_without_clamp,
        "the arrow ligature no longer overflows the estimate: this test proves nothing"
    );
}

/// The game's own string keeps exactly its `fitted` size: under the
/// default look the clamp never runs, even where the default overflows a
/// narrow box (a default-look bug the clamp must not paper over).
#[test]
fn the_clamp_never_touches_the_default() {
    for face in ALL {
        for box_px in [80.0, 300.0, 900.0] {
            let (estimate, size, _) = laid_out(face, None, box_px, true);
            let fitted = face.fitted(crate::ui::menu::TITLE, 88.0).font_size;
            assert_eq!(
                (estimate, size),
                (fitted, fitted),
                "{} in {box_px}",
                face.name
            );
            // A pack that sets the key to the default string: the same.
            let (_, size, _) = laid_out(face, Some(crate::ui::menu::TITLE), box_px, true);
            assert_eq!(size, fitted, "{} in {box_px}", face.name);
        }
    }
}
