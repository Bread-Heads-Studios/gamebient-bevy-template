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

/// The 2D material retint (`retint_color_material`) follows the same rules
/// as the others: the pack on the spawn frame, again on `LookChanged`, and
/// nothing in between. It writes the shared material asset, so an untagged
/// entity drawing the same handle changes too.
#[test]
fn a_tagged_color_material_takes_the_pack_colour() {
    let mut app = app_with(Selection::Embedded("night-shift".into()));
    app.init_resource::<Assets<ColorMaterial>>();
    let shared = app
        .world_mut()
        .resource_mut::<Assets<ColorMaterial>>()
        .add(ColorMaterial::from(theme::PAPER));
    let own = app
        .world_mut()
        .resource_mut::<Assets<ColorMaterial>>()
        .add(ColorMaterial::from(theme::INK));
    app.world_mut()
        .spawn((MeshMaterial2d(shared.clone()), LookColour::new("paper")));
    app.world_mut().spawn(MeshMaterial2d(shared.clone()));
    app.world_mut().spawn((
        MeshMaterial2d(own.clone()),
        LookColour::new("ink").alpha(0.5),
    ));
    app.update();
    let colour = |app: &App, h: &Handle<ColorMaterial>| {
        app.world()
            .resource::<Assets<ColorMaterial>>()
            .get(h)
            .unwrap()
            .color
    };
    let night_paper = parse_colour("#f4f1e8").unwrap();
    assert_eq!(colour(&app, &shared), night_paper);
    assert_eq!(
        colour(&app, &own),
        parse_colour("#05070f").unwrap().with_alpha(0.5)
    );

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
    assert_eq!(colour(&app, &shared), theme::PAPER);
    assert_eq!(colour(&app, &own), theme::INK.with_alpha(0.5));
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

/// D1 for hosted looks: a pack that arrives through the hosted path in the
/// middle of the run (its files land when the sim reaches tick 10) leaves
/// the score, checksum, content hash and tick count as under the default.
#[test]
fn a_hosted_pack_arriving_mid_run_does_not_change_a_replay() {
    use gamebient_game::game::sim::SimTick;
    use gamebient_game::look::fetch::PendingFetch;
    use gamebient_game::look::validate::content_hash;

    let replay = record_scripted_run();
    let base = verify(&replay);
    assert!(base.matches, "{base:?}");

    let files = look::pack::embedded_files("night-shift").unwrap();
    let mut app = build_verify_app(&replay);
    app.insert_resource(Selection::Default);
    app.add_plugins(LookPlugin {
        contract: &theme::LOOK,
    });
    let (pending, arrival) = PendingFetch::new_hosted("night-shift", content_hash(&files));
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
        Selection::Hosted("night-shift".into()),
        "the hosted pack never landed"
    );
    assert_eq!(
        (v.score, v.checksum, v.content, v.ticks),
        (base.score, base.checksum, base.content, base.ticks),
        "a hosted pack changed the sim"
    );
    assert!(v.matches);
}

use bevy::camera::primitives::MeshAabb;
use gamebient_game::game::content::sprites::{ArtOf, SpriteArt};
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

/// A Player Thing as `content::level::spawn_level` leaves it: the sim
/// entity, and a box art child with the kind's unit cuboid and material.
fn spawn_player_thing(app: &mut App) -> (Entity, Entity) {
    let mesh = app
        .world_mut()
        .resource_mut::<Assets<Mesh>>()
        .add(Cuboid::new(1.0, 1.0, 1.0));
    let material = app
        .world_mut()
        .resource_mut::<Assets<StandardMaterial>>()
        .add(StandardMaterial::default());
    let player = app.world_mut().spawn((Player, Transform::default())).id();
    let child = app
        .world_mut()
        .spawn((
            Mesh3d(mesh),
            MeshMaterial3d(material),
            Transform::default(),
            ArtOf(0),
            ChildOf(player),
        ))
        .id();
    (player, child)
}

/// The render half of the player (`player::dress_player`, registered by
/// `AssetsPlugin`) sizes the art child's mesh from the pack, on the spawn
/// frame, and rebuilds it on `LookChanged`. The sim entity is never touched.
#[test]
fn the_player_is_dressed_from_the_pack_on_the_frame_it_spawns() {
    let mut app = app_with(Selection::Embedded("sample-sponsor".into()));
    app.init_resource::<Assets<Mesh>>();
    app.init_resource::<Assets<StandardMaterial>>();
    app.add_plugins(gamebient_game::assets::AssetsPlugin);
    let (player, art) = spawn_player_thing(&mut app);
    app.update();
    assert!((cuboid_edge(&app, art) - 1.4).abs() < 1e-5);
    assert_eq!(
        app.world().get::<LookColour>(art).map(|c| c.role),
        Some("player")
    );
    assert!(
        app.world().get::<Mesh3d>(player).is_none()
            && app.world().get::<LookColour>(player).is_none(),
        "dress_player must write to the art child, never the sim entity"
    );

    // Hot reload: a new look with no recipe and a LookChanged rebuild the mesh.
    let plain = ActiveLook::resolve(&theme::LOOK, Selection::Default);
    app.insert_resource(plain);
    app.world_mut().write_message(look::LookChanged);
    app.update();
    assert!((cuboid_edge(&app, art) - 1.0).abs() < 1e-5);
}

/// A player drawn by a sprite (`content/sprites/Player.png`) keeps it: the
/// pack does not swap the quad for its mesh, and the retint tag comes off.
#[test]
fn a_player_sprite_is_left_to_the_game() {
    let mut app = app_with(Selection::Embedded("sample-sponsor".into()));
    app.init_resource::<Assets<Mesh>>();
    app.init_resource::<Assets<StandardMaterial>>();
    app.add_plugins(gamebient_game::assets::AssetsPlugin);
    let (_, art) = spawn_player_thing(&mut app);
    app.update();
    assert!(app.world().get::<LookColour>(art).is_some());
    let quad = app
        .world_mut()
        .resource_mut::<Assets<Mesh>>()
        .add(Rectangle::new(1.0, 1.0));
    app.insert_resource(SpriteArt {
        quad: quad.clone(),
        materials: vec![None],
    });
    app.world_mut().entity_mut(art).insert(Mesh3d(quad.clone()));
    app.update();
    assert_eq!(app.world().get::<Mesh3d>(art).unwrap().0, quad);
    assert!(app.world().get::<LookColour>(art).is_none());
    app.world_mut().write_message(look::LookChanged);
    app.update();
    assert_eq!(
        app.world().get::<Mesh3d>(art).unwrap().0,
        quad,
        "a LookChanged must not replace the sprite"
    );
}

/// D1: the sim's spawn is sim-only. With the asset stores present the
/// headless app's Player entity still carries no mesh (its art is a child).
#[test]
fn the_sim_player_carries_no_mesh() {
    let mut app = gamebient_game::game::replay::build_builtin_app();
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
    let meshed = w
        .query_filtered::<Entity, (With<Player>, With<Mesh3d>)>()
        .iter(w)
        .count();
    assert_eq!(meshed, 0, "the Player sim entity must not be dressed");
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

    /// Every `AssetEvent::Modified` the mesh store emits, in order.
    #[derive(Resource, Default)]
    struct Modified(Vec<AssetId<Mesh>>);

    fn record_modified(mut events: MessageReader<AssetEvent<Mesh>>, mut seen: ResMut<Modified>) {
        for e in events.read() {
            if let AssetEvent::Modified { id } = e {
                seen.0.push(*id);
            }
        }
    }

    /// `setup`, but with a real `AssetPlugin` so the mesh store emits its
    /// `AssetEvent`s, and every `Modified` recorded: a test can then see a
    /// write, not just compare data after it.
    fn setup_watched(selection: Selection) -> (App, Handle<Mesh>, Handle<Mesh>) {
        let mut app = app_with(selection);
        app.add_plugins(AssetPlugin::default())
            .init_asset::<Mesh>()
            .init_resource::<Modified>()
            .add_systems(Last, record_modified);
        let mut meshes = app.world_mut().resource_mut::<Assets<Mesh>>();
        let target = meshes.add(Cuboid::new(1.0, 2.0, 3.0));
        let other = meshes.add(Cuboid::new(1.0, 2.0, 3.0));
        app.world_mut()
            .resource_mut::<ShapeTargets>()
            .register("player", target.clone());
        (app, target, other)
    }

    #[test]
    fn a_registered_mesh_is_bit_identical_under_the_default_look() {
        let (mut app, target, other) = setup_watched(Selection::Default);
        app.update();
        app.world_mut().write_message(look::LookChanged);
        app.update();
        app.update();
        assert_eq!(mesh_of(&app, &target), cuboid());
        assert_eq!(mesh_of(&app, &other), cuboid());
        assert!(
            app.world().resource::<Modified>().0.is_empty(),
            "the default look wrote a mesh: {:?}",
            app.world().resource::<Modified>().0
        );
    }

    #[test]
    fn the_modification_watch_sees_a_pack_write() {
        // The watch above is only worth something if it can see a write.
        let (mut app, target, _) = setup_watched(Selection::Embedded("sample-shapes".into()));
        app.update();
        assert_eq!(app.world().resource::<Modified>().0, vec![target.id()]);
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
        let (_, art) = spawn_player_thing(&mut app);
        app.update();
        let shape = pack_shape(app.world().resource::<ActiveLook>());
        let h = app.world().get::<Mesh3d>(art).unwrap().0.clone();
        let mesh = app.world().resource::<Assets<Mesh>>().get(&h).unwrap();
        assert_eq!(mesh.count_vertices(), vertex_count(&shape));
        assert_eq!(data(mesh), data(&build_mesh(&shape)));
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

    fn night_shift() -> Files {
        look::pack::embedded_files("night-shift").unwrap()
    }

    fn swap(app: &mut App, files: Files, hash: Option<String>) -> Result<(), Report> {
        app.world_mut()
            .run_system_once(move |mut swap: LookSwap| {
                swap.apply(
                    files.clone(),
                    Selection::Fetch("night-shift".into()),
                    Some("night-shift"),
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
        let mut app = app_with(Selection::Default);
        let files = night_shift();
        let hash = content_hash(&files);
        swap(&mut app, files, Some(hash.clone())).unwrap();
        let look = app.world().resource::<ActiveLook>();
        assert_eq!(look.id(), "night-shift");
        assert_eq!(look.content_hash.as_deref(), Some(hash.as_str()));
        assert_eq!(look.source, Selection::Fetch("night-shift".into()));
        assert_eq!(look.string("pause"), "LIGHTS LOW");
        assert_eq!(sent(&app), 1);
    }

    #[test]
    fn no_expected_hash_skips_the_check() {
        // Native hot reload: the folder is the author's, there is no hash.
        let mut app = app_with(Selection::Default);
        swap(&mut app, night_shift(), None).unwrap();
        assert_eq!(app.world().resource::<ActiveLook>().id(), "night-shift");
        assert_eq!(sent(&app), 1);
    }

    #[test]
    fn a_hash_mismatch_is_refused_and_the_default_kept() {
        let mut app = app_with(Selection::Default);
        let mut files = night_shift();
        let hash = content_hash(&files);
        // Same names, one byte different: a tampered or stale download.
        files.get_mut("look.json").unwrap().push(b' ');
        let err = swap(&mut app, files, Some(hash)).unwrap_err();
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
        let mut files = night_shift();
        files.insert("look.json".into(), b"{\"v\":2,".to_vec());
        // The hash matches: the bytes are what the build expected, but they
        // are still not a valid pack, so they never reach the screen.
        let hash = content_hash(&files);
        assert!(swap(&mut app, files, Some(hash)).is_err());
        assert_eq!(app.world().resource::<ActiveLook>().id(), "default");
        assert_eq!(sent(&app), 0);
    }

    #[test]
    fn a_pack_with_another_id_is_refused() {
        let mut app = app_with(Selection::Default);
        let files = night_shift();
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
        let app = app_with(Selection::Fetch("night-shift".into()));
        assert_eq!(app.world().resource::<ActiveLook>().id(), "default");
        assert_eq!(
            *app.world().resource::<Selection>(),
            Selection::Fetch("night-shift".into())
        );
    }

    #[test]
    fn native_builds_fetch_nothing() {
        assert_eq!(look::pack::fetchable_ids().count(), 0);
        assert!(look::pack::fetchable("night-shift").is_none());
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

    /// A `?look=night-shift` launch whose pack is "in flight", expecting
    /// `hash`.
    fn in_flight(hash: &str) -> (App, Arrival) {
        let mut app = app_with(Selection::Fetch("night-shift".into()));
        let (pending, arrival) = PendingFetch::new("night-shift", hash);
        app.insert_resource(pending)
            .init_resource::<Announced>()
            .add_systems(Update, count.after(arrive));
        (app, arrival)
    }

    #[test]
    fn the_fetched_pack_is_applied_once_and_polling_stops() {
        let files = look::pack::embedded_files("night-shift").unwrap();
        let (mut app, arrival) = in_flight(&content_hash(&files));
        app.update();
        app.update();
        // Still in flight: the default look, nothing announced.
        assert_eq!(app.world().resource::<ActiveLook>().id(), "default");
        assert!(app.world().contains_resource::<PendingFetch>());
        assert_eq!(app.world().resource::<Announced>().0, 0);

        *arrival.lock().unwrap() = Some(Ok(files.clone()));
        app.update();
        assert_eq!(app.world().resource::<ActiveLook>().id(), "night-shift");
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
        let files = look::pack::embedded_files("night-shift").unwrap();
        let (mut app, arrival) = in_flight(&content_hash(&files));
        *arrival.lock().unwrap() = Some(Err("assets/looks/night-shift/look.json: HTTP 404".into()));
        app.update();
        assert_eq!(app.world().resource::<ActiveLook>().id(), "default");
        assert!(!app.world().contains_resource::<PendingFetch>());
        assert_eq!(app.world().resource::<Announced>().0, 0);
    }

    #[test]
    fn a_fetched_pack_with_the_wrong_hash_is_never_applied() {
        let files = look::pack::embedded_files("night-shift").unwrap();
        let (mut app, arrival) = in_flight(&"0".repeat(64));
        *arrival.lock().unwrap() = Some(Ok(files));
        app.update();
        assert_eq!(app.world().resource::<ActiveLook>().id(), "default");
        assert!(!app.world().contains_resource::<PendingFetch>());
        assert_eq!(app.world().resource::<Announced>().0, 0);
    }
}

/// Editing shows the level dressed like the game: the shown Player (no
/// `Player` marker, no sim components) has its art child dressed from the
/// pack, through the presentation-only `EditArt` marker.
#[test]
fn the_shown_player_is_dressed_while_editing() {
    use bevy::ecs::system::RunSystemOnce;
    use gamebient_game::game::content::edit::{EditArt, EditThing};
    use gamebient_game::game::content::kit::{Kinds, Thing, ThingArt, setup_thing_art};
    use gamebient_game::game::content::level::set_level;
    // `app_with` has no `GameState`; the headless game app has the state,
    // `GamePlugin`'s `OnEnter(Editing)` and the kinds, and takes the pack the
    // same way.
    let mut app = gamebient_game::game::replay::build_headless_app();
    app.insert_resource(Selection::Embedded("sample-sponsor".into()));
    app.add_plugins(LookPlugin {
        contract: &theme::LOOK,
    });
    app.init_resource::<Assets<Mesh>>();
    app.init_resource::<Assets<StandardMaterial>>();
    app.add_plugins(gamebient_game::assets::AssetsPlugin);
    if !app.world().contains_resource::<ThingArt>() {
        app.world_mut().run_system_once(setup_thing_art).unwrap();
    }
    set_level(
        app.world_mut(),
        br#"{"v":1,"things":[{"id":"player","kind":"Player","at":[0,0]}]}"#,
    )
    .unwrap();
    app.update();
    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(GameState::Editing);
    app.update(); // OnEnter(Editing) shows the level; dress_player runs in Update
    app.update(); // its inserts have landed
    let player_kind = app.world().resource::<Kinds>().index("Player").unwrap();
    let mut q = app
        .world_mut()
        .query_filtered::<(Entity, &ArtOf, &ChildOf), With<EditArt>>();
    let (art, _, parent) = q
        .iter(app.world())
        .find(|(_, a, _)| a.0 == player_kind)
        .map(|(e, a, c)| (e, *a, c.parent()))
        .expect("the shown Player has an art child");
    assert_eq!(
        app.world().get::<LookColour>(art).map(|c| c.role),
        Some("player")
    );
    assert!((cuboid_edge(&app, art) - 1.4).abs() < 1e-5);
    // The shown Thing itself stays a picture: no sim marker, nothing written.
    assert!(app.world().get::<EditThing>(parent).is_some());
    assert!(app.world().get::<Thing>(parent).is_some());
    assert!(app.world().get::<Player>(parent).is_none());
    assert!(app.world().get::<LookColour>(parent).is_none());
}

/// D1 for the editor's path: a pack worn through `look.preview`, setting every
/// palette role and string key to a value unlike the defaults, replays the
/// selftest to the same score, checksum, content hash and tick count.
#[cfg(feature = "editor")]
#[test]
fn a_look_preview_pack_does_not_change_a_replay() {
    use gamebient_input::HostCommand;
    let replay = record_scripted_run();
    let base = verify(&replay);
    let mut palette = serde_json::Map::new();
    for (role, _) in theme::LOOK.palette {
        palette.insert((*role).into(), "#7f3fbf".into());
    }
    let mut strings = serde_json::Map::new();
    for s in theme::LOOK.strings {
        strings.insert(s.key.into(), "Z".into());
    }
    let manifest = serde_json::json!({
        "v": 1, "id": "d1-editor", "name": "D1", "kind": "skin",
        "palette": palette, "strings": strings,
    });
    let mut app = build_verify_app(&replay);
    app.insert_resource(Selection::Default);
    app.add_plugins(LookPlugin {
        contract: &theme::LOOK,
    });
    app.world_mut().write_message(HostCommand::Custom {
        name: "look.preview".into(),
        data: serde_json::json!({ "id": "d1-editor", "files": { "look.json": { "text": manifest.to_string() } } })
            .to_string(),
    });
    let v = run_verify_app(&mut app, &replay);
    assert_eq!(
        app.world().resource::<ActiveLook>().id(),
        "d1-editor",
        "the pack was worn"
    );
    assert_eq!(
        (v.score, v.checksum, v.content, v.ticks),
        (base.score, base.checksum, base.content, base.ticks),
        "a look.preview pack changed the sim"
    );
    assert!(v.matches);
}
