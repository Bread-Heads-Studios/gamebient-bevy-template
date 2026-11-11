//! Look Packs end to end: built-in packs validate against the game's
//! contract, tagged entities pick up a pack, no pack changes a replay, and
//! a pack folder reloads live.

use gamebient_game::look::{self, validate::validate};
use gamebient_game::ui::theme;

#[test]
fn every_built_in_pack_validates_against_the_contract() {
    let ids: Vec<_> = look::pack::embedded_ids().collect();
    assert!(
        ids.contains(&"night-shift") && ids.contains(&"sample-sponsor"),
        "build.rs did not embed assets/looks: {ids:?}"
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

use bevy::prelude::*;
use gamebient_game::look::validate::parse_colour;
use gamebient_game::look::{ActiveLook, LookColour, LookPlugin, LookText, Selection};

fn app_with(selection: Selection) -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.insert_resource(selection);
    app.add_plugins(LookPlugin {
        contract: &theme::LOOK,
    });
    app
}

#[test]
fn no_selection_is_the_games_own_look() {
    let app = app_with(Selection::Default);
    let look = app.world().resource::<ActiveLook>();
    assert_eq!(look.id(), "default");
    assert_eq!(look.colour("paper"), theme::PAPER);
    assert_eq!(look.string("pause"), theme::PAUSE_WORD);
}

#[test]
fn a_built_in_pack_overrides_only_what_it_sets() {
    let app = app_with(Selection::Embedded("night-shift".into()));
    let look = app.world().resource::<ActiveLook>();
    assert_eq!(look.id(), "night-shift");
    assert_eq!(look.colour("paper"), parse_colour("#f4f1e8").unwrap());
    assert_eq!(look.string("pause"), "LIGHTS LOW");
    assert_eq!(
        look.string("pause.quit"),
        theme::PAUSE_QUIT,
        "unset keys keep the game's text"
    );
    assert!(look.content_hash.as_deref().is_some_and(|h| h.len() == 64));
}

#[test]
fn an_unknown_pack_falls_back_to_the_default() {
    let app = app_with(Selection::Embedded("no-such-pack".into()));
    assert_eq!(app.world().resource::<ActiveLook>().id(), "default");
}

#[test]
fn recipe_params_are_readable() {
    let app = app_with(Selection::Embedded("sample-sponsor".into()));
    let look = app.world().resource::<ActiveLook>();
    assert_eq!(look.recipe_number("player", "cuboid", "size"), Some(1.4));
    assert_eq!(look.recipe_number("player", "sphere", "size"), None);
}

#[test]
fn tagged_entities_take_the_pack_on_the_frame_they_spawn() {
    let mut app = app_with(Selection::Embedded("night-shift".into()));
    let text = app
        .world_mut()
        .spawn((
            Text::new(theme::PAUSE_WORD),
            TextColor(theme::PAPER),
            LookColour::new("paper"),
            LookText("pause"),
        ))
        .id();
    let veil = app
        .world_mut()
        .spawn((
            BackgroundColor(theme::INK),
            LookColour::new("ink").alpha(0.62),
        ))
        .id();
    app.update();
    let w = app.world();
    assert_eq!(
        w.get::<TextColor>(text).unwrap().0,
        parse_colour("#f4f1e8").unwrap()
    );
    assert_eq!(w.get::<Text>(text).unwrap().0, "LIGHTS LOW");
    assert_eq!(
        w.get::<BackgroundColor>(veil).unwrap().0,
        parse_colour("#05070f").unwrap().with_alpha(0.62)
    );
}

#[test]
fn a_tagged_sprite_takes_the_pack_colour() {
    let mut app = app_with(Selection::Embedded("night-shift".into()));
    let sprite = app
        .world_mut()
        .spawn((
            Sprite::from_color(theme::PAPER, Vec2::ONE),
            LookColour::new("paper"),
        ))
        .id();
    app.update();
    assert_eq!(
        app.world().get::<Sprite>(sprite).unwrap().color,
        parse_colour("#f4f1e8").unwrap()
    );
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn a_pack_folder_reloads_when_it_changes_and_survives_a_bad_edit() {
    use bevy::time::TimeUpdateStrategy;
    use std::time::Duration;

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
    let write = |pause: &str| {
        std::fs::write(
            pack.join("look.json"),
            format!(r#"{{"v":1,"id":"hot","name":"Hot","kind":"palette","strings":{{"pause":"{pause}"}}}}"#),
        )
        .unwrap();
    };
    write("ONE");

    let mut app = app_with(Selection::Dir(pack.display().to_string()));
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
        300,
    )));
    let text = app
        .world_mut()
        .spawn((Text::new("?"), LookText("pause")))
        .id();
    for _ in 0..4 {
        app.update();
    }
    assert_eq!(app.world().get::<Text>(text).unwrap().0, "ONE");

    write("TWO PLUS"); // a different length, so the fingerprint changes on any filesystem
    for _ in 0..4 {
        app.update();
    }
    assert_eq!(
        app.world().resource::<ActiveLook>().string("pause"),
        "TWO PLUS"
    );
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
        app.world().resource::<ActiveLook>().string("pause"),
        "TWO PLUS",
        "a broken edit keeps the last good look"
    );

    write("THREE"); // a new length again: proves polling survived the bad edit
    for _ in 0..4 {
        app.update();
    }
    assert_eq!(
        app.world().resource::<ActiveLook>().string("pause"),
        "THREE"
    );
    assert_eq!(app.world().get::<Text>(text).unwrap().0, "THREE");
}
