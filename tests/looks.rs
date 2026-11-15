//! Look Packs end to end: built-in packs validate against the game's
//! contract, tagged entities pick up a pack, no pack changes a replay, and
//! a pack folder reloads live.

use gamebient_game::look::{self, validate::validate};
use gamebient_game::ui::theme;

#[test]
fn every_built_in_pack_validates_against_the_contract() {
    let ids: Vec<_> = look::pack::embedded_ids().collect();
    assert!(
        ids.contains(&"night-shift")
            && ids.contains(&"sample-sponsor")
            && ids.contains(&"sample-shapes"),
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
fn a_tagged_text_colours_the_glyphs_and_leaves_its_background_clear() {
    // Bevy 0.18 gives every UI `Text` a required `BackgroundColor`. The tag
    // is for the text colour; it must not paint the text node's box too.
    let mut app = app_with(Selection::Embedded("night-shift".into()));
    let text = app
        .world_mut()
        .spawn((
            Text::new("X"),
            TextColor(theme::PAPER),
            BackgroundColor(Color::NONE),
            LookColour::new("paper"),
        ))
        .id();
    app.update();
    let w = app.world();
    assert_eq!(
        w.get::<TextColor>(text).unwrap().0,
        parse_colour("#f4f1e8").unwrap()
    );
    assert_eq!(w.get::<BackgroundColor>(text).unwrap().0.alpha(), 0.0);
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

use gamebient_game::game::replay::selftest::record_scripted_run;
use gamebient_game::game::replay::{build_verify_app, run_verify_app, verify};

/// D1: every pack replays to the same score, checksum and content hash.
/// In the template nothing in the sim reads a look, so this guards the
/// plugin; in a game it fails the moment a sim system starts reading one.
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
            (v.score, v.checksum, v.content, v.ticks),
            (base.score, base.checksum, base.content, base.ticks),
            "pack {id} changed the sim"
        );
        assert!(v.matches);
    }
}

use bevy::camera::primitives::MeshAabb;
use gamebient_game::game::player::Player;
use gamebient_game::game::states::GameState;

/// The cuboid edge of `entity`'s mesh, read back from the mesh asset.
fn cuboid_edge(app: &App, entity: Entity) -> f32 {
    let handle = app
        .world()
        .get::<Mesh3d>(entity)
        .expect("player has a Mesh3d")
        .0
        .clone();
    let aabb = app
        .world()
        .resource::<Assets<Mesh>>()
        .get(&handle)
        .expect("mesh asset exists")
        .compute_aabb()
        .expect("mesh has positions");
    let e = Vec3::from(aabb.half_extents) * 2.0;
    assert!(
        (e.x - e.y).abs() < 1e-5 && (e.y - e.z).abs() < 1e-5,
        "not a cube: {e:?}"
    );
    e.x
}

/// The render half of the player (`player::dress_player`, registered by
/// `AssetsPlugin`) sizes the mesh from the pack, on the spawn frame, and
/// rebuilds it on `LookChanged`.
#[test]
fn the_player_is_dressed_from_the_pack_on_the_frame_it_spawns() {
    let mut app = app_with(Selection::Embedded("sample-sponsor".into()));
    app.init_resource::<Assets<Mesh>>();
    app.init_resource::<Assets<StandardMaterial>>();
    app.add_plugins(gamebient_game::assets::AssetsPlugin);
    let player = app.world_mut().spawn((Player, Transform::default())).id();
    app.update();
    assert!((cuboid_edge(&app, player) - 1.4).abs() < 1e-5);
    assert!(
        app.world()
            .get::<MeshMaterial3d<StandardMaterial>>(player)
            .is_some()
    );
    assert_eq!(
        app.world().get::<LookColour>(player).map(|c| c.role),
        Some("player")
    );

    // Hot reload: a new look with no recipe and a LookChanged rebuild the mesh.
    let plain = ActiveLook::resolve(&theme::LOOK, Selection::Default);
    app.insert_resource(plain);
    app.world_mut().write_message(look::LookChanged);
    app.update();
    assert!((cuboid_edge(&app, player) - 1.0).abs() < 1e-5);
}

/// D1: the sim's spawn is sim-only. With the asset stores present (so the
/// old in-spawn dressing would have fired) the headless app still gives the
/// player no mesh.
#[test]
fn spawn_player_alone_inserts_no_mesh() {
    let mut app = gamebient_game::game::replay::build_headless_app();
    app.init_resource::<Assets<Mesh>>();
    app.init_resource::<Assets<StandardMaterial>>();
    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(GameState::Playing);
    app.update();
    app.update();
    let w = app.world_mut();
    let players = w.query_filtered::<Entity, With<Player>>().iter(w).count();
    assert_eq!(players, 1, "the headless app entered Playing and spawned");
    let meshed = w.query_filtered::<Entity, With<Mesh3d>>().iter(w).count();
    assert_eq!(meshed, 0, "spawn_player must not dress the player");
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

mod shapes {
    use super::*;
    use bevy::mesh::{Indices, VertexAttributeValues};
    use gamebient_game::look::{Files, ShapeTargets, Slot, build_mesh, vertex_count};

    type MeshData = (Vec<[f32; 3]>, Vec<[f32; 3]>, Vec<[f32; 2]>, Vec<u32>);

    /// Positions, normals, UVs and indices: what "bit-identical" compares.
    fn data(m: &Mesh) -> MeshData {
        let f3 = |id| match m.attribute(id) {
            Some(VertexAttributeValues::Float32x3(v)) => v.clone(),
            other => panic!("{other:?}"),
        };
        let uv = match m.attribute(Mesh::ATTRIBUTE_UV_0) {
            Some(VertexAttributeValues::Float32x2(v)) => v.clone(),
            other => panic!("{other:?}"),
        };
        let idx = match m.indices() {
            Some(Indices::U32(v)) => v.clone(),
            Some(Indices::U16(v)) => v.iter().map(|&i| u32::from(i)).collect(),
            None => Vec::new(),
        };
        (
            f3(Mesh::ATTRIBUTE_POSITION),
            f3(Mesh::ATTRIBUTE_NORMAL),
            uv,
            idx,
        )
    }

    fn mesh_of(app: &App, h: &Handle<Mesh>) -> MeshData {
        data(app.world().resource::<Assets<Mesh>>().get(h).unwrap())
    }

    fn pack_shape(look: &ActiveLook) -> Vec<gamebient_game::look::Primitive> {
        match look.slot("player") {
            Some(Slot::Shape(s)) => s.shape.clone(),
            other => panic!("no player shape: {other:?}"),
        }
    }

    /// An app with a mesh store, a registered cuboid and an unregistered one.
    fn setup(selection: Selection) -> (App, Handle<Mesh>, Handle<Mesh>) {
        let mut app = app_with(selection);
        app.init_resource::<Assets<Mesh>>();
        let mut meshes = app.world_mut().resource_mut::<Assets<Mesh>>();
        let target = meshes.add(Cuboid::new(1.0, 2.0, 3.0));
        let other = meshes.add(Cuboid::new(1.0, 2.0, 3.0));
        app.world_mut()
            .resource_mut::<ShapeTargets>()
            .register("player", target.clone());
        (app, target, other)
    }

    fn cuboid() -> MeshData {
        data(&Mesh::from(Cuboid::new(1.0, 2.0, 3.0)))
    }

    fn shape_look(json: &str) -> ActiveLook {
        let mut files = Files::new();
        files.insert("look.json".into(), json.as_bytes().to_vec());
        ActiveLook::from_files(&theme::LOOK, files, Selection::Default, None)
            .unwrap_or_else(|r| panic!("{:#?}", r.problems))
    }

    #[test]
    fn registering_the_same_handle_twice_is_a_no_op() {
        let mut t = ShapeTargets::default();
        let h = Handle::<Mesh>::default();
        t.register("player", h.clone());
        t.register("player", h);
        assert_eq!(t.len(), 1);
    }

    #[test]
    fn a_registered_mesh_is_bit_identical_under_the_default_look() {
        let (mut app, target, other) = setup(Selection::Default);
        app.update();
        app.update();
        assert_eq!(mesh_of(&app, &target), cuboid());
        assert_eq!(mesh_of(&app, &other), cuboid());
    }

    #[test]
    fn a_pack_shape_replaces_only_registered_meshes() {
        let (mut app, target, other) = setup(Selection::Embedded("sample-shapes".into()));
        app.update();
        let shape = pack_shape(app.world().resource::<ActiveLook>());
        assert_eq!(mesh_of(&app, &target), data(&build_mesh(&shape)));
        assert_eq!(mesh_of(&app, &other), cuboid(), "unregistered is untouched");

        // Switching back to the default restores the baseline exactly.
        app.insert_resource(ActiveLook::resolve(&theme::LOOK, Selection::Default));
        app.world_mut().write_message(look::LookChanged);
        app.update();
        assert_eq!(mesh_of(&app, &target), cuboid());
        assert_eq!(mesh_of(&app, &other), cuboid());
    }

    #[test]
    fn a_hot_reload_to_a_different_shape_applies_it() {
        let (mut app, target, _) = setup(Selection::Embedded("sample-shapes".into()));
        app.update();
        let sphere = shape_look(
            r#"{"v":2,"id":"ball","name":"Ball","kind":"skin","slots":{"player":{"shape":[{"prim":"sphere","size":[0.5]}]}}}"#,
        );
        let shape = pack_shape(&sphere);
        app.insert_resource(sphere);
        app.world_mut().write_message(look::LookChanged);
        app.update();
        assert_eq!(mesh_of(&app, &target), data(&build_mesh(&shape)));
    }

    #[test]
    fn a_target_registered_later_is_baselined_then_shaped() {
        let (mut app, _, _) = setup(Selection::Embedded("sample-shapes".into()));
        app.update();
        let late = app
            .world_mut()
            .resource_mut::<Assets<Mesh>>()
            .add(Cuboid::new(1.0, 2.0, 3.0));
        app.world_mut()
            .resource_mut::<ShapeTargets>()
            .register("player", late.clone());
        app.update();
        let shape = pack_shape(app.world().resource::<ActiveLook>());
        assert_eq!(mesh_of(&app, &late), data(&build_mesh(&shape)));
        app.insert_resource(ActiveLook::resolve(&theme::LOOK, Selection::Default));
        app.world_mut().write_message(look::LookChanged);
        app.update();
        assert_eq!(mesh_of(&app, &late), cuboid());
    }

    #[test]
    fn a_removed_mesh_drops_its_target() {
        let (mut app, target, _) = setup(Selection::Embedded("sample-shapes".into()));
        app.update();
        app.world_mut()
            .resource_mut::<Assets<Mesh>>()
            .remove(&target);
        app.world_mut().write_message(look::LookChanged);
        app.update();
        assert!(app.world().resource::<ShapeTargets>().is_empty());
    }

    #[test]
    fn sample_shapes_dresses_the_player_with_its_shape() {
        let mut app = app_with(Selection::Embedded("sample-shapes".into()));
        app.init_resource::<Assets<Mesh>>();
        app.init_resource::<Assets<StandardMaterial>>();
        app.add_plugins(gamebient_game::assets::AssetsPlugin);
        let player = app.world_mut().spawn((Player, Transform::default())).id();
        app.update();
        let shape = pack_shape(app.world().resource::<ActiveLook>());
        let h = app.world().get::<Mesh3d>(player).unwrap().0.clone();
        let mesh = app.world().resource::<Assets<Mesh>>().get(&h).unwrap();
        assert_eq!(mesh.count_vertices(), vertex_count(&shape));
        assert_eq!(data(mesh), data(&build_mesh(&shape)));
    }
}
