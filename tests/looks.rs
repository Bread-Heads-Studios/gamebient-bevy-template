//! Look Packs end to end, for any game: built-in packs validate against the
//! game's contract, tagged entities (`LookColour`, `LookText`, `LookBorder`,
//! `LookGradient`, `LookFit`) pick up a pack, no pack changes a replay, and
//! a pack folder reloads live. `LookGlow` needs `bevy_pbr`, so its tests are
//! unit tests in `src/look/glow.rs`, not here.
//!
//! **Copy this file into a game unchanged** (only the crate name in the
//! `use` lines differs). Every test is generic over the game's own contract
//! (`ui::theme::{LOOK, PALETTE, STRINGS}`) and its built-in packs in
//! `assets/looks/`, and uses only `look::*`, `ui::theme` and the replay API
//! every template game has (`game::replay::{build_verify_app, run_verify_app,
//! verify, selftest::record_scripted_run}`, `game::sim::SimTick`). Template-
//! only tests (the player slot, recipes, shapes, sprites, the editor) live in
//! `tests/looks_template.rs`, which a game does not copy.
//!
//! It needs one built-in pack that sets at least one palette role and one
//! string key to something other than the game's own (a port adds one that
//! sets them all).

use bevy::prelude::*;
use gamebient_game::look::validate::{parse_colour, validate};
use gamebient_game::look::{
    self, ActiveLook, LookBorder, LookColour, LookFit, LookGradient, LookPlugin, LookText,
    LookTint, Selection,
};
use gamebient_game::ui::theme;

fn app_with(selection: Selection) -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.insert_resource(selection);
    app.add_plugins(LookPlugin {
        contract: &theme::LOOK,
    });
    app
}

/// The game's own colour for `role`.
fn default_colour(role: &str) -> Color {
    theme::PALETTE
        .iter()
        .find(|(r, _)| *r == role)
        .map(|(_, c)| *c)
        .unwrap()
}

/// A built-in pack, and what it changes, for the tests that need a pack to
/// make a visible difference.
struct Probe {
    id: &'static str,
    /// A role the pack sets to a colour unlike the game's own.
    role: &'static str,
    /// A second such role (the same one when the pack changes only one).
    role2: &'static str,
    /// A string key the pack sets to text unlike the game's own.
    key: &'static str,
}

impl Probe {
    fn look(&self) -> ActiveLook {
        ActiveLook::resolve(&theme::LOOK, Selection::Embedded(self.id.into()))
    }
    fn colour(&self) -> Color {
        self.look().colour(self.role)
    }
    fn colour2(&self) -> Color {
        self.look().colour(self.role2)
    }
    fn text(&self) -> String {
        self.look().string(self.key).to_string()
    }
}

/// The first built-in pack (in id order) that changes a palette role and a
/// string key.
fn probe() -> Probe {
    for id in look::pack::embedded_ids() {
        let look = ActiveLook::resolve(&theme::LOOK, Selection::Embedded(id.into()));
        let roles: Vec<&'static str> = theme::PALETTE
            .iter()
            .filter(|(r, d)| look.colour(r) != *d)
            .map(|(r, _)| *r)
            .collect();
        let key = theme::STRINGS
            .iter()
            .find(|s| look.string(s.key) != s.default)
            .map(|s| s.key);
        if let (Some(&role), Some(key)) = (roles.first(), key) {
            return Probe {
                id,
                role,
                role2: roles.get(1).copied().unwrap_or(role),
                key,
            };
        }
    }
    panic!(
        "no built-in pack in assets/looks/ changes both a palette role and a \
         string key; a port adds one that sets every role and key"
    );
}

#[test]
fn every_built_in_pack_validates_against_the_contract() {
    let ids: Vec<_> = look::pack::embedded_ids().collect();
    assert!(
        !ids.is_empty(),
        "build.rs did not embed assets/looks (a port adds at least one pack)"
    );
    for id in ids {
        let files = look::pack::embedded_files(id).unwrap();
        let r = validate(&files, &theme::LOOK, Some(id));
        assert!(r.ok(), "{id}: {:#?}", r.problems);
        assert!(
            r.problems.is_empty(),
            "{id} has warnings; built-in packs ship clean: {:#?}",
            r.problems
        );
    }
}

#[test]
fn no_selection_is_the_games_own_look() {
    let app = app_with(Selection::Default);
    let look = app.world().resource::<ActiveLook>();
    assert_eq!(look.id(), "default");
    for (role, colour) in theme::PALETTE {
        assert_eq!(look.colour(role), *colour, "palette role {role}");
    }
    for s in theme::STRINGS {
        assert_eq!(look.string(s.key), s.default, "string {}", s.key);
    }
}

/// Every built-in pack: a role or key it sets takes the pack's value, and
/// one it leaves out keeps the game's own.
#[test]
fn a_built_in_pack_overrides_only_what_it_sets() {
    for id in look::pack::embedded_ids() {
        let app = app_with(Selection::Embedded(id.into()));
        let look = app.world().resource::<ActiveLook>();
        assert_eq!(look.id(), id);
        let m = look.manifest.as_ref().unwrap();
        for (role, default) in theme::PALETTE {
            let want = m
                .palette
                .get(*role)
                .map_or(*default, |v| parse_colour(v).unwrap());
            assert_eq!(look.colour(role), want, "{id}: palette role {role}");
        }
        for s in theme::STRINGS {
            let want = m.strings.get(s.key).map_or(s.default, String::as_str);
            assert_eq!(look.string(s.key), want, "{id}: string {}", s.key);
        }
        assert!(look.content_hash.as_deref().is_some_and(|h| h.len() == 64));
    }
    // The walk above must have had something to compare.
    let p = probe();
    assert_ne!(p.colour(), default_colour(p.role));
}

#[test]
fn an_unknown_pack_falls_back_to_the_default() {
    let app = app_with(Selection::Embedded("no-such-pack".into()));
    assert_eq!(app.world().resource::<ActiveLook>().id(), "default");
}

#[test]
fn tagged_entities_take_the_pack_on_the_frame_they_spawn() {
    let p = probe();
    let key_default = theme::STRINGS
        .iter()
        .find(|s| s.key == p.key)
        .unwrap()
        .default;
    let mut app = app_with(Selection::Embedded(p.id.into()));
    let text = app
        .world_mut()
        .spawn((
            Text::new(key_default),
            TextColor(default_colour(p.role)),
            LookColour::new(p.role),
            LookText(p.key),
        ))
        .id();
    let veil = app
        .world_mut()
        .spawn((
            BackgroundColor(default_colour(p.role2)),
            LookColour::new(p.role2).alpha(0.62),
        ))
        .id();
    app.update();
    let w = app.world();
    assert_eq!(w.get::<TextColor>(text).unwrap().0, p.colour());
    assert_eq!(w.get::<Text>(text).unwrap().0, p.text());
    assert_eq!(
        w.get::<BackgroundColor>(veil).unwrap().0,
        p.colour2().with_alpha(0.62)
    );
}

#[test]
fn a_tagged_text_colours_the_glyphs_and_leaves_its_background_clear() {
    // Bevy 0.18 gives every UI `Text` a required `BackgroundColor`. The tag
    // is for the text colour; it must not paint the text node's box too.
    let p = probe();
    let mut app = app_with(Selection::Embedded(p.id.into()));
    let text = app
        .world_mut()
        .spawn((
            Text::new("X"),
            TextColor(default_colour(p.role)),
            BackgroundColor(Color::NONE),
            LookColour::new(p.role),
        ))
        .id();
    app.update();
    let w = app.world();
    assert_eq!(w.get::<TextColor>(text).unwrap().0, p.colour());
    assert_eq!(w.get::<BackgroundColor>(text).unwrap().0.alpha(), 0.0);
}

#[test]
fn a_tagged_sprite_takes_the_pack_colour() {
    let p = probe();
    let mut app = app_with(Selection::Embedded(p.id.into()));
    let sprite = app
        .world_mut()
        .spawn((
            Sprite::from_color(default_colour(p.role), Vec2::ONE),
            LookColour::new(p.role),
        ))
        .id();
    app.update();
    assert_eq!(app.world().get::<Sprite>(sprite).unwrap().color, p.colour());
}

/// The 2D material retint (`retint_color_material`) follows the same rules
/// as the others: the pack on the spawn frame, again on `LookChanged`, and
/// nothing in between. It writes the shared material asset, so an untagged
/// entity drawing the same handle changes too.
#[test]
fn a_tagged_color_material_takes_the_pack_colour() {
    let p = probe();
    let mut app = app_with(Selection::Embedded(p.id.into()));
    app.init_resource::<Assets<ColorMaterial>>();
    let shared = app
        .world_mut()
        .resource_mut::<Assets<ColorMaterial>>()
        .add(ColorMaterial::from(default_colour(p.role)));
    let own = app
        .world_mut()
        .resource_mut::<Assets<ColorMaterial>>()
        .add(ColorMaterial::from(default_colour(p.role2)));
    app.world_mut()
        .spawn((MeshMaterial2d(shared.clone()), LookColour::new(p.role)));
    app.world_mut().spawn(MeshMaterial2d(shared.clone()));
    app.world_mut().spawn((
        MeshMaterial2d(own.clone()),
        LookColour::new(p.role2).alpha(0.5),
    ));
    app.update();
    let colour = |app: &App, h: &Handle<ColorMaterial>| {
        app.world()
            .resource::<Assets<ColorMaterial>>()
            .get(h)
            .unwrap()
            .color
    };
    assert_eq!(colour(&app, &shared), p.colour());
    assert_eq!(colour(&app, &own), p.colour2().with_alpha(0.5));

    // No LookChanged, no rewrite: the game's own later edit stands.
    let red = Color::srgb(1.0, 0.0, 0.0);
    app.world_mut()
        .resource_mut::<Assets<ColorMaterial>>()
        .get_mut(&shared)
        .unwrap()
        .color = red;
    app.update();
    assert_eq!(colour(&app, &shared), red);

    // Hot reload back to the default look recolours the shared material.
    app.insert_resource(ActiveLook::resolve(&theme::LOOK, Selection::Default));
    app.world_mut().write_message(look::LookChanged);
    app.update();
    assert_eq!(colour(&app, &shared), default_colour(p.role));
    assert_eq!(colour(&app, &own), default_colour(p.role2).with_alpha(0.5));
}

/// `LookBorder` and `LookGradient` colour a node's border and gradient
/// stops on spawn, follow a look change, and go back to the role defaults
/// on a clear. A `None` stop keeps its spawned colour throughout.
#[test]
fn border_and_gradient_tags_follow_the_look_and_restore_on_clear() {
    let p = probe();
    let mut app = app_with(Selection::Embedded(p.id.into()));
    let structural = Color::srgb(0.01, 0.02, 0.03);
    let node = app
        .world_mut()
        .spawn((
            BorderColor::all(default_colour(p.role)),
            BackgroundGradient::from(LinearGradient::to_bottom(vec![
                ColorStop::new(default_colour(p.role), Val::Percent(0.0)),
                ColorStop::new(structural, Val::Percent(50.0)),
                ColorStop::new(default_colour(p.role2), Val::Percent(100.0)),
            ])),
            LookBorder::new(p.role).alpha(0.5),
            LookGradient::new([
                Some(LookTint::new(p.role)),
                None,
                Some(LookTint::new(p.role2).alpha(0.25)),
            ]),
        ))
        .id();
    let stops = |app: &App| -> Vec<Color> {
        match &app.world().get::<BackgroundGradient>(node).unwrap().0[0] {
            Gradient::Linear(l) => l.stops.iter().map(|s| s.color).collect(),
            _ => unreachable!(),
        }
    };
    app.update();
    assert_eq!(
        *app.world().get::<BorderColor>(node).unwrap(),
        BorderColor::all(p.colour().with_alpha(0.5))
    );
    assert_eq!(
        stops(&app),
        vec![p.colour(), structural, p.colour2().with_alpha(0.25)]
    );

    // Back to the game's own look (what a clear does): the role defaults.
    app.insert_resource(ActiveLook::resolve(&theme::LOOK, Selection::Default));
    app.world_mut().write_message(look::LookChanged);
    app.update();
    assert_eq!(
        *app.world().get::<BorderColor>(node).unwrap(),
        BorderColor::all(default_colour(p.role).with_alpha(0.5))
    );
    assert_eq!(
        stops(&app),
        vec![
            default_colour(p.role),
            structural,
            default_colour(p.role2).with_alpha(0.25)
        ]
    );
}

/// A stand-in face for `look_fit_keeps_the_default_size_and_fits_any_pack_string`:
/// every glyph 0.55 em but 'W' (1.1), the widest.
static STAND_IN: look::refit::Advances = {
    let mut a = [0.55; look::refit::PRINTABLE];
    a['W' as usize - 0x20] = 1.1;
    a
};

/// `LookFit` sizes every string key: the game's own string keeps its
/// average-advance size exactly, a string a pack may set is sized by its
/// real width (never above the default's size) so ordinary words stay
/// close to the default and the worst case (`max_chars` of the widest
/// glyph) still fits the line, and the size follows a look change and a
/// clear. The metrics are a stand-in face, so this holds the arithmetic
/// for any game; `fonts::tests` holds each real face.
#[test]
fn look_fit_keeps_the_default_size_and_fits_any_pack_string() {
    let advance = 0.62;
    let widest = look::refit::widest_em(&STAND_IN);
    for s in theme::STRINGS {
        for line_px in [200.0, 648.0, 864.0] {
            let fit = LookFit::new(s.key, 72.0, advance, &STAND_IN, &[], line_px);
            let own = look::refit::size_for_em(advance, s.default, 72.0, line_px);
            assert_eq!(fit.size_for(s.default, s.default), own, "{}", s.key);
            let worst = "W".repeat(s.max_chars);
            let size = fit.size_for(&worst, s.default);
            assert!(size <= own, "{}", s.key);
            assert!(
                size == 1.0 || s.max_chars as f32 * widest * size <= line_px,
                "{}: {} glyphs at {size} overflow {line_px}",
                s.key,
                s.max_chars
            );
            // Ordinary letters as long as the default: at least the size
            // the widest-glyph rule would give, and they fit.
            let plain = "A".repeat(s.default.chars().count().min(s.max_chars));
            let size = fit.size_for(&plain, s.default);
            assert!(size <= own, "{}", s.key);
            assert!(
                size >= look::refit::size_for_em(widest, &plain, own, line_px),
                "{}",
                s.key
            );
            assert!(
                size == 1.0 || look::refit::text_em(&STAND_IN, &[], &plain) * size <= line_px,
                "{}",
                s.key
            );
        }
    }

    let p = probe();
    let spec = theme::STRINGS.iter().find(|s| s.key == p.key).unwrap();
    let mut app = app_with(Selection::Default);
    let fit = LookFit::new(p.key, 72.0, advance, &STAND_IN, &[], 300.0);
    let line = app
        .world_mut()
        .spawn((
            Text::new(spec.default),
            TextFont::from_font_size(5.0),
            LookText(p.key),
            fit,
        ))
        .id();
    let size = |app: &App| app.world().get::<TextFont>(line).unwrap().font_size;
    app.update();
    assert_eq!(size(&app), fit.size_for(spec.default, spec.default));

    app.insert_resource(p.look());
    app.world_mut().write_message(look::LookChanged);
    app.update();
    assert_eq!(size(&app), fit.size_for(&p.text(), spec.default));

    app.insert_resource(ActiveLook::resolve(&theme::LOOK, Selection::Default));
    app.world_mut().write_message(look::LookChanged);
    app.update();
    assert_eq!(size(&app), fit.size_for(spec.default, spec.default));
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn a_pack_folder_reloads_when_it_changes_and_survives_a_bad_edit() {
    use bevy::time::TimeUpdateStrategy;
    use std::time::Duration;

    // A key that fits the longest text below.
    let key = theme::STRINGS
        .iter()
        .find(|s| s.max_chars >= "TWO PLUS".len())
        .expect("a string key of at least 8 characters")
        .key;
    let dir = std::env::temp_dir().join(format!("gx-look-reload-{}", std::process::id()));
    let pack = dir.join("hot");
    std::fs::create_dir_all(&pack).unwrap();
    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).ok();
        }
    }
    let _cleanup = Cleanup(dir.clone());
    let write = |text: &str| {
        std::fs::write(
            pack.join("look.json"),
            format!(r#"{{"v":1,"id":"hot","name":"Hot","kind":"palette","strings":{{"{key}":"{text}"}}}}"#),
        )
        .unwrap();
    };
    write("ONE");

    let mut app = app_with(Selection::Dir(pack.display().to_string()));
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
        300,
    )));
    let text = app.world_mut().spawn((Text::new("?"), LookText(key))).id();
    for _ in 0..4 {
        app.update();
    }
    assert_eq!(app.world().get::<Text>(text).unwrap().0, "ONE");

    write("TWO PLUS"); // a different length, so the fingerprint changes on any filesystem
    for _ in 0..4 {
        app.update();
    }
    assert_eq!(app.world().resource::<ActiveLook>().string(key), "TWO PLUS");
    assert_eq!(
        app.world().get::<Text>(text).unwrap().0,
        "TWO PLUS",
        "LookChanged retexts"
    );

    std::fs::write(pack.join("look.json"), "{ not json").unwrap();
    for _ in 0..4 {
        app.update();
    }
    assert_eq!(
        app.world().resource::<ActiveLook>().string(key),
        "TWO PLUS",
        "a broken edit keeps the last good look"
    );

    write("THREE"); // a new length again: proves polling survived the bad edit
    for _ in 0..4 {
        app.update();
    }
    assert_eq!(app.world().resource::<ActiveLook>().string(key), "THREE");
    assert_eq!(app.world().get::<Text>(text).unwrap().0, "THREE");
}

use gamebient_game::game::replay::selftest::record_scripted_run;
use gamebient_game::game::replay::{build_verify_app, run_verify_app, verify};

/// D1: every pack replays to the same score, checksum, end and tick count.
/// In the template nothing in the sim reads a look, so this guards the
/// plugin; in a game it fails the moment a sim system starts reading one
/// whose value some built-in pack changes.
#[test]
fn no_built_in_pack_changes_a_replay() {
    let replay = record_scripted_run();
    let base = verify(&replay);
    assert!(
        base.matches,
        "the selftest replay must verify plainly first: {base:?}"
    );
    for id in look::pack::embedded_ids() {
        let mut app = build_verify_app(&replay);
        app.insert_resource(Selection::Embedded(id.to_string()));
        app.add_plugins(LookPlugin {
            contract: &theme::LOOK,
        });
        assert_eq!(
            app.world().resource::<ActiveLook>().id(),
            id,
            "pack {id} did not load"
        );
        let v = run_verify_app(&mut app, &replay);
        assert_eq!(
            (v.score, v.checksum, v.ended, v.ticks),
            (base.score, base.checksum, base.ended, base.ticks),
            "pack {id} changed the sim"
        );
        assert!(v.matches);
    }
}

/// D1 for hosted looks: a pack that arrives through the hosted path in the
/// middle of the run (its files land when the sim reaches tick 10) leaves
/// the score, checksum, end and tick count as under the default.
#[test]
fn a_hosted_pack_arriving_mid_run_does_not_change_a_replay() {
    use gamebient_game::game::sim::SimTick;
    use gamebient_game::look::fetch::PendingFetch;
    use gamebient_game::look::validate::content_hash;

    let replay = record_scripted_run();
    let base = verify(&replay);
    assert!(base.matches, "{base:?}");

    let id = probe().id;
    let files = look::pack::embedded_files(id).unwrap();
    let mut app = build_verify_app(&replay);
    app.insert_resource(Selection::Default);
    app.add_plugins(LookPlugin {
        contract: &theme::LOOK,
    });
    let (pending, arrival) = PendingFetch::new_hosted(id, content_hash(&files));
    app.insert_resource(pending);
    let landing = files.clone();
    app.add_systems(Update, move |tick: Option<Res<SimTick>>| {
        if tick.is_some_and(|t| t.0 >= 10) {
            let mut slot = arrival.lock().unwrap();
            if slot.is_none() {
                *slot = Some(Ok(landing.clone()));
            }
        }
    });
    let v = run_verify_app(&mut app, &replay);
    assert_eq!(
        app.world().resource::<ActiveLook>().source,
        Selection::Hosted(id.into()),
        "the hosted pack never landed"
    );
    assert_eq!(
        (v.score, v.checksum, v.ended, v.ticks),
        (base.score, base.checksum, base.ended, base.ticks),
        "a hosted pack changed the sim"
    );
    assert!(v.matches);
}

/// `pack::read_dir` is the gate for every folder pack (`--look`, the CLI).
#[cfg(not(target_arch = "wasm32"))]
mod read_dir {
    use gamebient_game::look::pack::read_dir;
    use gamebient_game::look::validate::limits;
    use std::path::PathBuf;

    struct TempDir(PathBuf);
    impl TempDir {
        fn new(tag: &str) -> Self {
            let p = std::env::temp_dir().join(format!("gx-look-rd-{tag}-{}", std::process::id()));
            std::fs::remove_dir_all(&p).ok();
            std::fs::create_dir_all(&p).unwrap();
            Self(p)
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).ok();
        }
    }

    #[test]
    fn a_subfolder_is_rejected() {
        let d = TempDir::new("sub");
        std::fs::write(d.0.join("look.json"), "{}").unwrap();
        std::fs::create_dir(d.0.join("nested")).unwrap();
        let e = read_dir(&d.0).unwrap_err();
        assert!(e.starts_with("nested:") && e.contains("no folders"), "{e}");
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_is_rejected() {
        let d = TempDir::new("link");
        let target = d.0.join(".real");
        std::fs::write(&target, "x").unwrap();
        std::os::unix::fs::symlink(&target, d.0.join("logo.png")).unwrap();
        let e = read_dir(&d.0).unwrap_err();
        assert!(e.starts_with("logo.png:") && e.contains("no links"), "{e}");
    }

    #[test]
    fn more_than_files_max_is_rejected() {
        let d = TempDir::new("many");
        for i in 0..=limits::FILES_MAX {
            std::fs::write(d.0.join(format!("f{i}.png")), "x").unwrap();
        }
        let e = read_dir(&d.0).unwrap_err();
        assert_eq!(e, format!("more than {} files", limits::FILES_MAX));
    }

    #[test]
    fn exactly_files_max_is_read() {
        let d = TempDir::new("max");
        for i in 0..limits::FILES_MAX {
            std::fs::write(d.0.join(format!("f{i}.png")), "x").unwrap();
        }
        assert_eq!(read_dir(&d.0).unwrap().len(), limits::FILES_MAX);
    }

    #[test]
    fn readme_and_dotfiles_are_skipped() {
        let d = TempDir::new("skip");
        std::fs::write(d.0.join("look.json"), "{}").unwrap();
        std::fs::write(d.0.join("README.md"), "notes").unwrap();
        std::fs::write(d.0.join(".DS_Store"), "junk").unwrap();
        std::fs::create_dir(d.0.join(".git")).unwrap();
        let files = read_dir(&d.0).unwrap();
        let names: Vec<_> = files.keys().cloned().collect();
        assert_eq!(names, vec!["look.json".to_string()]);
    }
}

/// The step native hot reload and the web fetch share: files in, checked,
/// then `ActiveLook` replaced, `LookAssets` rebuilt and `LookChanged` sent.
#[allow(clippy::result_large_err)] // Report is the full diagnosis
mod swap {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use gamebient_game::look::swap::LookSwap;
    use gamebient_game::look::validate::content_hash;
    use gamebient_game::look::{Files, LookChanged, Report};

    fn pack_files() -> (&'static str, Files) {
        let id = probe().id;
        (id, look::pack::embedded_files(id).unwrap())
    }

    fn swap(
        app: &mut App,
        id: &'static str,
        files: Files,
        hash: Option<String>,
    ) -> Result<(), Report> {
        app.world_mut()
            .run_system_once(move |mut swap: LookSwap| {
                swap.apply(
                    files.clone(),
                    Selection::Fetch(id.into()),
                    Some(id),
                    hash.as_deref(),
                )
            })
            .unwrap()
    }

    fn sent(app: &App) -> usize {
        app.world().resource::<Messages<LookChanged>>().len()
    }

    #[test]
    fn a_valid_pack_with_its_hash_is_applied_and_announced() {
        let p = probe();
        let mut app = app_with(Selection::Default);
        let (id, files) = pack_files();
        let hash = content_hash(&files);
        swap(&mut app, id, files, Some(hash.clone())).unwrap();
        let look = app.world().resource::<ActiveLook>();
        assert_eq!(look.id(), id);
        assert_eq!(look.content_hash.as_deref(), Some(hash.as_str()));
        assert_eq!(look.source, Selection::Fetch(id.into()));
        assert_eq!(look.colour(p.role), p.colour());
        assert_eq!(look.string(p.key), p.text());
        assert_eq!(sent(&app), 1);
    }

    #[test]
    fn no_expected_hash_skips_the_check() {
        // Native hot reload: the folder is the author's, there is no hash.
        let mut app = app_with(Selection::Default);
        let (id, files) = pack_files();
        swap(&mut app, id, files, None).unwrap();
        assert_eq!(app.world().resource::<ActiveLook>().id(), id);
        assert_eq!(sent(&app), 1);
    }

    #[test]
    fn a_hash_mismatch_is_refused_and_the_default_kept() {
        let mut app = app_with(Selection::Default);
        let (id, mut files) = pack_files();
        let hash = content_hash(&files);
        // Same names, one byte different: a tampered or stale download.
        files.get_mut("look.json").unwrap().push(b' ');
        let err = swap(&mut app, id, files, Some(hash)).unwrap_err();
        assert!(!err.ok());
        assert!(
            err.errors().any(|p| p.message.contains("content hash")),
            "{:#?}",
            err.problems
        );
        assert_eq!(app.world().resource::<ActiveLook>().id(), "default");
        assert_eq!(sent(&app), 0);
    }

    #[test]
    fn a_pack_that_fails_validation_is_refused_and_the_default_kept() {
        let mut app = app_with(Selection::Default);
        let (id, mut files) = pack_files();
        files.insert("look.json".into(), b"{\"v\":2,".to_vec());
        // The hash matches: the bytes are what the build expected, but they
        // are still not a valid pack, so they never reach the screen.
        let hash = content_hash(&files);
        assert!(swap(&mut app, id, files, Some(hash)).is_err());
        assert_eq!(app.world().resource::<ActiveLook>().id(), "default");
        assert_eq!(sent(&app), 0);
    }

    #[test]
    fn a_pack_with_another_id_is_refused() {
        let mut app = app_with(Selection::Default);
        let (_, files) = pack_files();
        let hash = content_hash(&files);
        let result = app
            .world_mut()
            .run_system_once(move |mut swap: LookSwap| {
                swap.apply(
                    files.clone(),
                    Selection::Fetch("other".into()),
                    Some("other"),
                    Some(&hash),
                )
            })
            .unwrap();
        assert!(result.is_err());
        assert_eq!(app.world().resource::<ActiveLook>().id(), "default");
        assert_eq!(sent(&app), 0);
    }

    #[test]
    fn a_fetch_selection_boots_on_the_default_look() {
        // The pack arrives later through the swap; startup never blocks.
        let id = probe().id;
        let app = app_with(Selection::Fetch(id.into()));
        assert_eq!(app.world().resource::<ActiveLook>().id(), "default");
        assert_eq!(
            *app.world().resource::<Selection>(),
            Selection::Fetch(id.into())
        );
    }

    #[test]
    fn native_builds_fetch_nothing() {
        assert_eq!(look::pack::fetchable_ids().count(), 0);
        for id in look::pack::embedded_ids() {
            assert!(look::pack::fetchable(id).is_none(), "{id}");
        }
    }
}

/// The web fetch's hand-off, native-compiled: a slot filled by hand stands
/// in for the async fetch, and `fetch::arrive` swaps the pack in once.
mod fetch_arrival {
    use super::*;
    use gamebient_game::look::LookChanged;
    use gamebient_game::look::fetch::{Arrival, PendingFetch, arrive};
    use gamebient_game::look::validate::content_hash;

    #[derive(Resource, Default)]
    struct Announced(usize);

    fn count(mut seen: MessageReader<LookChanged>, mut n: ResMut<Announced>) {
        n.0 += seen.read().count();
    }

    /// A `?look=<id>` launch whose pack is "in flight", expecting `hash`.
    fn in_flight(id: &str, hash: &str) -> (App, Arrival) {
        let mut app = app_with(Selection::Fetch(id.into()));
        let (pending, arrival) = PendingFetch::new(id, hash);
        app.insert_resource(pending)
            .init_resource::<Announced>()
            .add_systems(Update, count.after(arrive));
        (app, arrival)
    }

    #[test]
    fn the_fetched_pack_is_applied_once_and_polling_stops() {
        let id = probe().id;
        let files = look::pack::embedded_files(id).unwrap();
        let (mut app, arrival) = in_flight(id, &content_hash(&files));
        app.update();
        app.update();
        // Still in flight: the default look, nothing announced.
        assert_eq!(app.world().resource::<ActiveLook>().id(), "default");
        assert!(app.world().contains_resource::<PendingFetch>());
        assert_eq!(app.world().resource::<Announced>().0, 0);

        *arrival.lock().unwrap() = Some(Ok(files.clone()));
        app.update();
        assert_eq!(app.world().resource::<ActiveLook>().id(), id);
        assert!(!app.world().contains_resource::<PendingFetch>());
        assert_eq!(app.world().resource::<Announced>().0, 1);

        // A late second delivery into the old slot is never read.
        *arrival.lock().unwrap() = Some(Ok(files));
        for _ in 0..3 {
            app.update();
        }
        assert_eq!(app.world().resource::<Announced>().0, 1);
        assert!(arrival.lock().unwrap().is_some());
    }

    #[test]
    fn a_failed_fetch_keeps_the_default_and_stops() {
        let id = probe().id;
        let files = look::pack::embedded_files(id).unwrap();
        let (mut app, arrival) = in_flight(id, &content_hash(&files));
        *arrival.lock().unwrap() = Some(Err(format!("assets/looks/{id}/look.json: HTTP 404")));
        app.update();
        assert_eq!(app.world().resource::<ActiveLook>().id(), "default");
        assert!(!app.world().contains_resource::<PendingFetch>());
        assert_eq!(app.world().resource::<Announced>().0, 0);
    }

    #[test]
    fn a_fetched_pack_with_the_wrong_hash_is_never_applied() {
        let id = probe().id;
        let files = look::pack::embedded_files(id).unwrap();
        let (mut app, arrival) = in_flight(id, &"0".repeat(64));
        *arrival.lock().unwrap() = Some(Ok(files));
        app.update();
        assert_eq!(app.world().resource::<ActiveLook>().id(), "default");
        assert!(!app.world().contains_resource::<PendingFetch>());
        assert_eq!(app.world().resource::<Announced>().0, 0);
    }
}
