//! Sprites per kind (presentation): decoding limits, nearest sampling, the
//! quad-or-box choice, and in-place replacement. Runs in an app with the
//! windowed build's asset stores, built on the headless app like
//! `windowed_shape.rs`.
//!
//! Everything runs on built-in content (the test level, Rust default
//! tunables, the embedded sprites cleared, fixture PNGs for the sprite under
//! test), so a save to `content/` never breaks these tests. What is shipped
//! in `content/sprites/` is checked for validity only.

use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::render::render_resource::FilterMode;

use gamebient_game::game::content::files::EMBEDDED;
use gamebient_game::game::content::kit::{Kinds, Thing, ThingArt};
use gamebient_game::game::content::level::LevelSource;
use gamebient_game::game::content::model::SpritesFile;
use gamebient_game::game::content::saved::reset_to_defaults;
use gamebient_game::game::content::sprites::{
    ArtOf, SpriteArt, SpriteImages, animate_sprites, clear_sprites, decode_png, set_sprite,
    set_sprites_meta,
};
use gamebient_game::game::replay::build_headless_app;
use gamebient_game::game::sim::{PendingSeed, seed_bytes};
use gamebient_game::game::states::GameState;

const A: &[u8] = include_bytes!("fixtures/sprite-a.png");
const B: &[u8] = include_bytes!("fixtures/sprite-b.png");
const WIDE: &[u8] = include_bytes!("fixtures/sprite-wide.png");

fn windowed() -> App {
    let mut app = build_headless_app();
    app.add_plugins(bevy::asset::AssetPlugin::default());
    app.init_asset::<Mesh>();
    app.init_asset::<StandardMaterial>();
    app.init_asset::<Image>();
    app.update(); // Startup: ThingArt, then the embedded sprites
    clear_sprites(app.world_mut());
    app.insert_resource(LevelSource::Test);
    reset_to_defaults(app.world_mut());
    app
}

fn play(app: &mut App) {
    app.world_mut().resource_mut::<PendingSeed>().0 = Some(seed_bytes(1));
    app.update();
    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(GameState::Playing);
    app.update();
}

/// (kind name, art child entity, its mesh, its material) for every thing.
fn art(app: &mut App) -> Vec<(String, Entity, Handle<Mesh>, Handle<StandardMaterial>)> {
    let mut q = app
        .world_mut()
        .query::<(&ArtOf, &ChildOf, &Mesh3d, &MeshMaterial3d<StandardMaterial>)>();
    let mut v: Vec<_> = q
        .iter(app.world())
        .map(|(a, c, m, mat)| (a.0, c.parent(), m.0.clone(), mat.0.clone()))
        .collect();
    v.sort_by_key(|x| x.1);
    let kinds = app.world().resource::<Kinds>();
    v.into_iter()
        .map(|(k, e, m, mat)| (kinds.0[usize::from(k)].0.clone(), e, m, mat))
        .collect()
}

#[test]
fn limits_are_enforced() {
    assert!(decode_png(A).is_ok());
    assert!(decode_png(WIDE).is_err(), "300 px wide");
    let mut big = A.to_vec();
    big.resize(64 * 1024 + 1, 0);
    assert!(decode_png(&big).is_err(), "over 64 KB");
    assert!(decode_png(b"not a png").is_err());
    let mut app = windowed();
    assert!(set_sprite(app.world_mut(), "Player", WIDE).is_err());
    assert!(set_sprite(app.world_mut(), "Ghost", A).is_err());
    assert!(app.world().resource::<SpriteImages>().0.is_empty());
}

/// The shipped sprites are valid, whatever they are: each
/// `content/sprites/<Kind>.png` names a registered kind and decodes within
/// the limits, and `content/sprites.json` (if any) parses.
#[test]
fn the_shipped_sprites_are_valid() {
    let app = windowed();
    let kinds = app.world().resource::<Kinds>();
    for (path, bytes) in EMBEDDED {
        if *path == "sprites.json" {
            SpritesFile::parse(bytes).unwrap_or_else(|e| panic!("{path}: {e:?}"));
            continue;
        }
        let Some(kind) = path
            .strip_prefix("sprites/")
            .and_then(|f| f.strip_suffix(".png"))
        else {
            continue;
        };
        assert!(kinds.index(kind).is_some(), "{path}: no kind called {kind}");
        decode_png(bytes).unwrap_or_else(|e| panic!("{path}: {e:?}"));
    }
}

#[test]
fn a_valid_png_decodes_with_nearest_sampling() {
    let mut app = windowed();
    set_sprite(app.world_mut(), "Player", A).unwrap();
    let images = app.world().resource::<SpriteImages>();
    assert_eq!(images.0.len(), 1);
    let handle = images.0[0].1.clone();
    let img = app
        .world()
        .resource::<Assets<Image>>()
        .get(&handle)
        .unwrap();
    assert_eq!((img.width(), img.height()), (4, 4));
    match &img.sampler {
        ImageSampler::Descriptor(d) => assert_eq!(d.mag_filter, FilterMode::Nearest.into()),
        other => panic!("expected a nearest descriptor, got {other:?}"),
    }
}

#[test]
fn a_sprite_kind_gets_a_quad_and_others_keep_the_box() {
    let mut app = windowed();
    set_sprite(app.world_mut(), "Player", A).unwrap();
    play(&mut app);
    let box_mesh = app.world().resource::<ThingArt>().mesh.clone();
    let quad = app.world().resource::<SpriteArt>().quad.clone();
    let got = art(&mut app);
    assert_eq!(got.len(), 4, "the test level's things");
    for (kind, _, mesh, _) in &got {
        if kind == "Player" {
            assert_eq!(mesh, &quad);
        } else {
            assert_eq!(mesh, &box_mesh);
        }
    }
}

#[test]
fn replacing_a_sprite_changes_the_texture_in_place() {
    let mut app = windowed();
    set_sprite(app.world_mut(), "Player", A).unwrap();
    play(&mut app);
    let before = art(&mut app);
    let things_before = app.world_mut().query::<&Thing>().iter(app.world()).count();
    let parents: Vec<_> = before.iter().map(|b| b.1).collect();
    let children_before: Vec<Entity> = {
        let mut q = app.world_mut().query_filtered::<Entity, With<ArtOf>>();
        q.iter(app.world()).collect()
    };
    let (_, _, _, mat) = before.iter().find(|b| b.0 == "Player").unwrap().clone();
    let tex_before = app
        .world()
        .resource::<Assets<StandardMaterial>>()
        .get(&mat)
        .unwrap()
        .base_color_texture
        .clone()
        .unwrap();
    set_sprite(app.world_mut(), "Player", B).unwrap();
    let tex_after = app
        .world()
        .resource::<Assets<StandardMaterial>>()
        .get(&mat)
        .unwrap()
        .base_color_texture
        .clone()
        .unwrap();
    assert_ne!(tex_before.id(), tex_after.id());
    let after = art(&mut app);
    assert_eq!(parents, after.iter().map(|b| b.1).collect::<Vec<_>>());
    let children_after: Vec<Entity> = {
        let mut q = app.world_mut().query_filtered::<Entity, With<ArtOf>>();
        q.iter(app.world()).collect()
    };
    assert_eq!(children_before, children_after);
    assert_eq!(
        things_before,
        app.world_mut().query::<&Thing>().iter(app.world()).count()
    );
}

#[test]
fn a_sprite_added_mid_run_upgrades_the_existing_box_in_place() {
    let mut app = windowed();
    play(&mut app);
    let before = art(&mut app);
    let box_mesh = app.world().resource::<ThingArt>().mesh.clone();
    assert!(before.iter().all(|b| b.2 == box_mesh));
    set_sprite(app.world_mut(), "Coin", A).unwrap();
    let after = art(&mut app);
    let quad = app.world().resource::<SpriteArt>().quad.clone();
    assert_eq!(
        before.iter().map(|b| b.1).collect::<Vec<_>>(),
        after.iter().map(|b| b.1).collect::<Vec<_>>()
    );
    for (kind, _, mesh, _) in &after {
        assert_eq!(mesh == &quad, kind == "Coin");
    }
}

#[test]
fn frames_split_the_image_and_time_advances_them() {
    let mut app = windowed();
    app.add_systems(Update, animate_sprites);
    set_sprite(app.world_mut(), "Player", B).unwrap();
    set_sprites_meta(
        app.world_mut(),
        br#"{"v":1,"sprites":{"Player":{"frames":2,"fps":10}}}"#,
    )
    .unwrap();
    let mat = app.world().resource::<SpriteArt>().materials[0]
        .clone()
        .unwrap();
    let uv = |app: &App| {
        app.world()
            .resource::<Assets<StandardMaterial>>()
            .get(&mat)
            .unwrap()
            .uv_transform
    };
    assert_eq!(uv(&app).matrix2.x_axis.x, 0.5);
    app.world_mut()
        .resource_mut::<Time<Virtual>>()
        .advance_by(std::time::Duration::from_millis(150));
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(std::time::Duration::from_millis(150));
    app.update();
    let t = app.world().resource::<Time>().elapsed_secs();
    assert!(t >= 0.15, "time advanced");
    let want = ((t * 10.0) as u32 % 2) as f32 * 0.5;
    assert_eq!(uv(&app).translation.x, want);
    assert_eq!(want, 0.5, "1.5 frames in is the second frame");
}

#[test]
fn meta_moves_and_scales_existing_children() {
    let mut app = windowed();
    set_sprite(app.world_mut(), "Player", A).unwrap();
    play(&mut app);
    set_sprites_meta(
        app.world_mut(),
        br#"{"v":1,"sprites":{"Player":{"scale":2,"anchor":[0.5,0]}}}"#,
    )
    .unwrap();
    let player = art(&mut app)
        .into_iter()
        .find(|a| a.0 == "Player")
        .unwrap()
        .1;
    let child = app.world().get::<Children>(player).unwrap()[0];
    let t = app.world().get::<Transform>(child).unwrap();
    assert_eq!(t.scale, Vec3::new(2.0, 2.0, 1.0));
    assert_eq!(t.translation.y, 1.0);
    assert!(
        set_sprites_meta(
            app.world_mut(),
            br#"{"v":1,"sprites":{"Player":{"frames":0}}}"#
        )
        .is_err()
    );
}
