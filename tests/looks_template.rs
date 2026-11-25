//! Look Packs tests that only the template can run: its built-in packs by
//! name, the `player` slot (recipe params, `dress_player`, sprites, shapes),
//! the level editor's shown player, the editor's `look.preview`, and the
//! replay's content hash (GXR2 only).
//!
//! **A game does not copy this file.** The copyable tests are in
//! `tests/looks.rs`; a game copies that one unchanged.

use bevy::prelude::*;
use gamebient_game::look::{self, ActiveLook, LookColour, LookPlugin, Selection};
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

#[test]
fn the_templates_packs_are_embedded() {
    let ids: Vec<_> = look::pack::embedded_ids().collect();
    assert!(
        ids.contains(&"night-shift")
            && ids.contains(&"sample-sponsor")
            && ids.contains(&"sample-shapes"),
        "build.rs did not embed assets/looks: {ids:?}"
    );
}

/// night-shift sets `pause` but not `pause.quit`, so one key of each kind.
#[test]
fn night_shift_overrides_only_what_it_sets() {
    use gamebient_game::look::validate::parse_colour;
    let app = app_with(Selection::Embedded("night-shift".into()));
    let look = app.world().resource::<ActiveLook>();
    assert_eq!(look.colour("paper"), parse_colour("#f4f1e8").unwrap());
    assert_eq!(look.string("pause"), "LIGHTS LOW");
    assert_eq!(
        look.string("pause.quit"),
        theme::PAUSE_QUIT,
        "unset keys keep the game's text"
    );
}

#[test]
fn recipe_params_are_readable() {
    let app = app_with(Selection::Embedded("sample-sponsor".into()));
    let look = app.world().resource::<ActiveLook>();
    assert_eq!(look.recipe_number("player", "cuboid", "size"), Some(1.4));
    assert_eq!(look.recipe_number("player", "sphere", "size"), None);
}

use gamebient_game::game::replay::selftest::record_scripted_run;
use gamebient_game::game::replay::{build_verify_app, run_verify_app, verify};

/// D1, the GXR2 half: the content hash a replay re-simulates to is the
/// same under every built-in pack. `tests/looks.rs` checks score,
/// checksum, end and ticks; only GXR2 games have `Verdict.content`.
#[test]
fn no_built_in_pack_changes_a_replays_content_hash() {
    let replay = record_scripted_run();
    let base = verify(&replay);
    assert!(base.matches, "{base:?}");
    for id in look::pack::embedded_ids() {
        let mut app = build_verify_app(&replay);
        app.insert_resource(Selection::Embedded(id.to_string()));
        app.add_plugins(LookPlugin {
            contract: &theme::LOOK,
        });
        let v = run_verify_app(&mut app, &replay);
        assert_eq!(
            v.content, base.content,
            "pack {id} changed the content hash"
        );
    }
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

/// The template's own cards carry `card_look` tags (`theme::*_ROLES`). Under
/// the default look every part is exactly the style's colour, bit for bit
/// (the tags change nothing); under a pack that sets the card roles each
/// part follows its role, at the alpha its tint names (else the pack's own,
/// which a 6-digit hex colour makes opaque).
#[test]
fn the_templates_cards_are_unchanged_by_the_default_look_and_follow_a_pack() {
    use gamebient_game::look::validate::parse_colour;
    use gamebient_game::ui::card::{CardLookExt, CardRoles, CardStyle, card_with};
    let cards: [(&CardStyle, &CardRoles); 4] = [
        (&theme::CARD, &theme::CARD_ROLES),
        (&theme::RAIL, &theme::RAIL_ROLES),
        (&theme::CHIP, &theme::CHIP_ROLES),
        (&theme::BADGE, &theme::BADGE_ROLES),
    ];
    let mut app = app_with(Selection::Default);
    let ids: Vec<Entity> = cards
        .iter()
        .map(|(style, roles)| {
            app.world_mut()
                .spawn(card_with(style, Node::default()))
                .card_look(style, roles)
                .id()
        })
        .collect();
    let parts = |app: &App, id: Entity| {
        let w = app.world();
        let stops: Vec<Color> = match w.get::<BackgroundGradient>(id).unwrap().0.first() {
            Some(Gradient::Linear(l)) => l.stops.iter().map(|s| s.color).collect(),
            _ => vec![],
        };
        (
            w.get::<BackgroundColor>(id).unwrap().0,
            stops,
            *w.get::<BorderColor>(id).unwrap(),
            w.get::<BoxShadow>(id)
                .unwrap()
                .0
                .iter()
                .map(|s| s.color)
                .collect::<Vec<_>>(),
        )
    };
    let want = |style: &CardStyle| {
        (
            style.fill,
            style.gradient.map_or(vec![], |g| g.to_vec()),
            BorderColor::all(style.border),
            style.shadow.into_iter().collect::<Vec<_>>(),
        )
    };
    app.update();
    for ((style, _), id) in cards.iter().zip(&ids) {
        assert_eq!(parts(&app, *id), want(style), "default look, first frame");
    }
    // A clear / reload to the same look changes nothing either.
    app.world_mut().write_message(look::LookChanged);
    app.update();
    for ((style, _), id) in cards.iter().zip(&ids) {
        assert_eq!(parts(&app, *id), want(style), "after a LookChanged");
    }
    // Parts the style lacks carry no tag at all.
    assert!(app.world().get::<look::LookGradient>(ids[1]).is_none());
    assert!(app.world().get::<look::LookShadow>(ids[2]).is_none());

    // A pack that sets every card role.
    let json = r##"{"v":1,"id":"cards","name":"C","kind":"palette","palette":{
        "card":"#112233","cardtop":"#223344","cardbottom":"#334455",
        "cardshadow":"#445566","signal":"#556677","ink":"#667788","coral":"#778899"}}"##;
    let files = std::collections::BTreeMap::from([(
        look::manifest::MANIFEST_FILE.to_string(),
        json.as_bytes().to_vec(),
    )]);
    app.insert_resource(
        ActiveLook::from_files(&theme::LOOK, files, Selection::Default, None).expect("valid pack"),
    );
    app.world_mut().write_message(look::LookChanged);
    app.update();
    let c = |hex: &str, a: f32| parse_colour(hex).unwrap().with_alpha(a);
    let card = parts(&app, ids[0]);
    assert_eq!(card.0, c("#112233", 1.0));
    assert_eq!(card.1, vec![c("#223344", 1.0), c("#334455", 1.0)]);
    assert_eq!(card.2, BorderColor::all(c("#556677", 0.85)));
    assert_eq!(card.3, vec![c("#445566", 1.0)]);
    let rail = parts(&app, ids[1]);
    assert_eq!(rail.0, c("#667788", 0.78));
    assert_eq!(rail.3, vec![c("#445566", 0.45)]);
    let badge = parts(&app, ids[3]);
    assert_eq!(badge.0, c("#778899", 0.18));
    assert_eq!(badge.2, BorderColor::all(c("#778899", 1.0)));
}

/// A prompt pulse writes its `TextColor` alpha every frame, and a look
/// change retints the same `TextColor` in `LookSet`: `UiPlugin` runs
/// `pulse_text` in `PostUpdate` after `LookSet`, so the pulse's alpha is the
/// last write. In `Update` the retint would repaint the role's own alpha for
/// a frame on every `LookChanged`.
#[test]
fn a_look_change_mid_pulse_keeps_the_pulse_alpha() {
    use bevy::asset::AssetPlugin;
    use bevy::input::InputPlugin;
    use bevy::state::app::StatesPlugin;
    use bevy::text::TextPlugin;
    use gamebient_game::ui::transition::Pulse;
    use gamebient_game::{assets, display, frame, game::GamePlugin, ui};

    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin::default(),
        StatesPlugin,
        InputPlugin,
        WindowPlugin {
            primary_window: None,
            exit_condition: bevy::window::ExitCondition::DontExit,
            ..default()
        },
        TextPlugin,
    ));
    app.init_asset::<Mesh>()
        .init_asset::<Image>()
        .init_asset::<Shader>()
        .init_asset::<StandardMaterial>()
        .init_asset::<ColorMaterial>()
        .init_asset::<AudioSource>()
        .init_resource::<UiScale>()
        .init_resource::<ClearColor>()
        .insert_resource(Selection::Default)
        .add_plugins((
            display::DisplayPlugin,
            LookPlugin {
                contract: &theme::LOOK,
            },
            GamePlugin::default(),
            assets::AssetsPlugin,
            ui::UiPlugin,
            frame::FramePlugin,
        ));
    app.finish();
    app.cleanup();
    for _ in 0..3 {
        app.update();
    }
    let e = app
        .world_mut()
        .spawn((
            Text::new("PRESS START"),
            TextColor(theme::PAPER),
            LookColour::new("paper"),
            Pulse {
                speed: 3.0,
                min: 0.3,
                max: 0.6,
            },
        ))
        .id();
    let alpha = |app: &App| app.world().get::<TextColor>(e).unwrap().0.alpha();
    for frame in 0..3 {
        app.world_mut().write_message(look::LookChanged);
        app.update();
        let a = alpha(&app);
        assert!(
            (0.3..=0.6).contains(&a),
            "frame {frame}: alpha {a} is outside the pulse's 0.3..=0.6: a look change repainted it"
        );
    }
}
