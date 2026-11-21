//! The run's level: which things are placed where. Loaded from
//! `content/level.json` (embedded at build time) by `ContentPlugin`, or
//! replaced live with [`set_level`]; spawned by [`spawn_level`] on
//! `OnEnter(Playing)`, in file order.
//!
//! The level is run content: its bytes' FNV hash ([`LevelHash`]) is the
//! first thing the replay content hash folds (`tunables::hash::content`).
//! The selftest and named checks run on [`test_level`] instead
//! ([`LevelSource::Test`]), so editing `content/level.json` never
//! invalidates the fixtures.

use bevy::prelude::*;

use super::files::fnv1a64;
use super::kit::{KindProps, Kinds, Size, Thing, ThingArt, ThingId};
use super::model::{ContentError, LevelFile, PlacedThing};
use super::sprites::{ArtOf, SpriteArt, SpriteImages, sprite_for};
use crate::game::GameEntity;
use crate::game::sim::SpawnCounter;

/// The level the next run spawns.
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct Level(pub LevelFile);

/// FNV-1a 64 of the level file's exact bytes (0 for no file).
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LevelHash(pub u64);

/// Where the run's level comes from. `Test` is the built-in selftest layout.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LevelSource {
    #[default]
    Content,
    Test,
}

/// The level hash a [`LevelSource::Test`] run folds.
pub fn test_level_hash() -> u64 {
    fnv1a64(b"test")
}

/// The built-in level the replay selftest and the named checks run on,
/// built in Rust so a data save never changes them.
///
/// TEMPLATE NOTE: replace this with your game's own test layout. Keep it a
/// valid level for your kinds (`test_level_uses_registered_kinds` checks).
/// It need not match `content/level.json`: the template's file starts as
/// the same layout, but tests check only that the file is valid, so a data
/// save never fails them.
pub fn test_level() -> LevelFile {
    let thing = |id: &str, kind: &str, x: f32, y: f32| PlacedThing {
        id: id.into(),
        kind: kind.into(),
        at: [x, y],
        size: None,
    };
    LevelFile {
        v: 1,
        things: vec![
            thing("player", "Player", 0.0, 0.0),
            thing("coin-1", "Coin", 3.0, 0.0),
            thing("coin-2", "Coin", -3.0, 2.0),
            thing("coin-3", "Coin", 0.0, -3.0),
        ],
    }
}

/// The first kind `level` names that `kinds` does not have.
pub fn check_kinds(level: &LevelFile, kinds: &Kinds) -> Result<(), ContentError> {
    match level.things.iter().find(|t| kinds.index(&t.kind).is_none()) {
        Some(t) => Err(ContentError(format!(
            "There is no kind called \"{}\".",
            t.kind
        ))),
        None => Ok(()),
    }
}

/// Parses `bytes`, checks every kind exists, and replaces [`Level`] and
/// [`LevelHash`]. On error nothing changes. Takes effect at the next run.
pub fn set_level(world: &mut World, bytes: &[u8]) -> Result<(), ContentError> {
    let level = LevelFile::parse(bytes)?;
    world.init_resource::<Kinds>();
    check_kinds(&level, world.resource::<Kinds>())?;
    world.insert_resource(Level(level));
    world.insert_resource(LevelHash(fnv1a64(bytes)));
    Ok(())
}

/// `OnEnter(Playing)`: spawns the level's things in file order, each with
/// the next `SpawnOrder`. Art goes on a child so the parent's archetype is
/// the same in the windowed game and the verifier, apart from `Children`
/// (see `tests/archetype_order.rs`).
pub fn spawn_level(
    mut commands: Commands,
    level: Res<Level>,
    source: Option<Res<LevelSource>>,
    kinds: Res<Kinds>,
    mut spawn: ResMut<SpawnCounter>,
    art: Option<Res<ThingArt>>,
    sprite_art: Option<Res<SpriteArt>>,
    sprite_images: Option<Res<SpriteImages>>,
) {
    let test;
    let file = if source.is_some_and(|s| *s == LevelSource::Test) {
        test = test_level();
        &test
    } else {
        &level.0
    };
    for placed in &file.things {
        let Some(kind) = kinds.index(&placed.kind) else {
            continue;
        };
        let props = kinds.props(kind).expect("index came from kinds");
        let size = placed_size(placed, props);
        let mut e = commands.spawn((
            Thing { kind },
            ThingId(placed.id.clone()),
            Size(size),
            GameEntity,
            spawn.stamp(),
            Transform::from_xyz(placed.at[0], placed.at[1], 0.0),
            Visibility::default(),
        ));
        if let Some(on_spawn) = props.on_spawn {
            on_spawn(&mut e);
        }
        let sources = ArtSources {
            art: art.as_deref(),
            sprite_art: sprite_art.as_deref(),
            sprite_images: sprite_images.as_deref(),
        };
        spawn_art_child(&mut e, &sources, &kinds, kind, size, ());
    }
}

/// A placed Thing's size: its own `size`, else its kind's default.
pub fn placed_size(placed: &PlacedThing, props: &KindProps) -> Vec2 {
    placed.size.map_or(props.size, Vec2::from)
}

/// The art a Thing's child can draw from. Every field is optional: the
/// headless sim has none of them and spawns no art.
pub struct ArtSources<'a> {
    pub art: Option<&'a ThingArt>,
    pub sprite_art: Option<&'a SpriteArt>,
    pub sprite_images: Option<&'a SpriteImages>,
}

/// Puts a Thing's art on a child of `parent`: the kind's sprite when it has
/// one, else its box. `extra` goes on the child too (`()` for a run, the
/// level editor's `EditArt` for a shown Thing). Shared by [`spawn_level`]
/// and `edit::spawn_edit_level`, so edit mode draws exactly what a run does.
pub fn spawn_art_child(
    parent: &mut EntityCommands,
    sources: &ArtSources,
    kinds: &Kinds,
    kind: u16,
    size: Vec2,
    extra: impl Bundle,
) {
    if let Some((quad, material, transform)) =
        sprite_for(sources.sprite_art, sources.sprite_images, kinds, kind, size)
    {
        parent.with_child((
            Mesh3d(quad),
            MeshMaterial3d(material),
            transform,
            ArtOf(kind),
            extra,
        ));
    } else if let Some(art) = sources.art
        && let Some(material) = art.materials.get(usize::from(kind))
    {
        parent.with_child((
            Mesh3d(art.mesh.clone()),
            MeshMaterial3d(material.clone()),
            Transform::from_scale(size.extend(1.0)),
            ArtOf(kind),
            extra,
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::content::files::embedded;
    use crate::game::replay::build_headless_app;
    use crate::game::sim::{PendingSeed, SpawnOrder, seed_bytes};
    use crate::game::states::GameState;

    fn enter_playing(app: &mut App) {
        app.world_mut().resource_mut::<PendingSeed>().0 = Some(seed_bytes(1));
        app.update();
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Playing);
        app.update();
    }

    fn spawned(app: &mut App) -> Vec<(u64, String, u16, Vec2)> {
        let mut v: Vec<_> = app
            .world_mut()
            .query::<(&SpawnOrder, &ThingId, &Thing, &Transform)>()
            .iter(app.world())
            .map(|(o, id, t, tf)| (o.0, id.0.clone(), t.kind, tf.translation.truncate()))
            .collect();
        v.sort_by_key(|x| x.0);
        v
    }

    /// Spawned straight from `spawn_level`, before any sim tick, so the
    /// positions are the file's even for things that fall or move.
    #[test]
    fn the_template_level_spawns_in_file_order() {
        use bevy::ecs::system::RunSystemOnce;
        let mut app = build_headless_app();
        app.update();
        app.world_mut().run_system_once(spawn_level).unwrap();
        let file = LevelFile::parse(embedded("level.json").unwrap()).unwrap();
        let got = spawned(&mut app);
        assert_eq!(got.len(), file.things.len());
        let kinds = app.world().resource::<Kinds>();
        for (i, ((order, id, kind, at), placed)) in got.iter().zip(&file.things).enumerate() {
            assert_eq!(*order, i as u64);
            assert_eq!(id, &placed.id);
            assert_eq!(Some(*kind), kinds.index(&placed.kind));
            assert_eq!(*at, Vec2::from(placed.at));
        }
    }

    #[test]
    fn the_test_source_spawns_the_test_level() {
        let mut app = build_headless_app();
        app.insert_resource(Level(LevelFile {
            v: 1,
            things: vec![],
        }));
        app.insert_resource(LevelSource::Test);
        enter_playing(&mut app);
        let ids: Vec<String> = spawned(&mut app).into_iter().map(|x| x.1).collect();
        let want: Vec<String> = test_level().things.into_iter().map(|t| t.id).collect();
        assert_eq!(ids, want);
    }

    #[test]
    fn test_level_uses_registered_kinds_and_is_valid() {
        let app = build_headless_app();
        let level = test_level();
        check_kinds(&level, app.world().resource::<Kinds>()).unwrap();
        let bytes = serde_json::to_vec(&level).unwrap();
        assert_eq!(LevelFile::parse(&bytes).unwrap(), level);
    }

    /// The shipped level is valid, whatever it holds: it parses, uses only
    /// registered kinds and `set_level` accepts it. (Not equal to
    /// `test_level()`: a data save that moves a Thing must not fail tests.)
    #[test]
    fn the_content_level_file_is_valid() {
        let Some(bytes) = embedded("level.json") else {
            return;
        };
        let file = LevelFile::parse(bytes).unwrap();
        let mut app = build_headless_app();
        check_kinds(&file, app.world().resource::<Kinds>()).unwrap();
        set_level(app.world_mut(), bytes).unwrap();
        assert_eq!(app.world().resource::<Level>().0, file);
        assert_eq!(app.world().resource::<LevelHash>().0, fnv1a64(bytes));
    }

    #[test]
    fn an_unknown_kind_is_refused_and_nothing_changes() {
        let mut app = build_headless_app();
        let before = app.world().resource::<LevelHash>().0;
        let e = set_level(
            app.world_mut(),
            br#"{"v":1,"things":[{"id":"g","kind":"Ghost","at":[0,0]}]}"#,
        )
        .unwrap_err();
        assert_eq!(e.0, "There is no kind called \"Ghost\".");
        assert_eq!(app.world().resource::<LevelHash>().0, before);
    }

    #[test]
    fn the_level_hash_follows_the_bytes() {
        let mut app = build_headless_app();
        let a = br#"{"v":1,"things":[{"id":"p","kind":"Player","at":[0,0]}]}"#;
        let b = br#"{"v":1,"things":[{"id":"p","kind":"Player","at":[0,1]}]}"#;
        set_level(app.world_mut(), a).unwrap();
        let ha = app.world().resource::<LevelHash>().0;
        assert_eq!(ha, fnv1a64(a));
        set_level(app.world_mut(), b).unwrap();
        assert_ne!(app.world().resource::<LevelHash>().0, ha);
        assert_eq!(app.world().resource::<Level>().0.things[0].at, [0.0, 1.0]);
    }
}
