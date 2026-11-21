//! The level editor's game side. `GameState::Editing` shows the current
//! level, frozen, under a separate orthographic camera, and keeps
//! [`EditView`] (the camera and the canvas rectangle) for the editor's
//! `level.view` (src/game/tunables/protocol.rs). See docs/content-kit.md,
//! "The level editor".
//!
//! Nothing here is sim code. `SimSet` runs only in `Playing`, so Editing
//! ticks nothing: replays, `RunClock`, the selftest and the checks are
//! untouched. The Things shown here carry [`EditThing`] and no `on_spawn`
//! components and no `SpawnOrder`: they are a picture of the level, not a
//! run. They keep `GameEntity`, so the `OnExit(Editing)` cleanup removes them.

use bevy::camera::ScalingMode;
use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;

use super::kit::{Kinds, LevelGrid, Size, Thing, ThingArt, ThingId};
use super::level::{ArtSources, Level, placed_size, spawn_art_child};
use super::model::LevelFile;
use super::sprites::{SpriteArt, SpriteImages};
use crate::display::{GAME_HEIGHT, GAME_WIDTH}; // allow-display: the edit camera's default canvas, presentation only
use crate::game::GameEntity;
use crate::game::states::GameState;

/// The zoom range, in CSS pixels per world unit (the editor uses the same).
pub const ZOOM_MIN: f32 = 0.01;
pub const ZOOM_MAX: f32 = 100.0;
/// Room left around the level when the camera fits it (10 % each side).
const FIT_MARGIN: f32 = 1.2;
/// The edit camera's height above the level (the far plane is 1000).
const CAMERA_Z: f32 = 500.0;

/// A Thing shown in Editing (a picture of the level, not a run entity).
#[derive(Component)]
pub struct EditThing;

/// Presentation-only marker on a shown Thing's art child. Dressing
/// (`player::dress_player`) finds the shown Player's art by it and its
/// `ArtOf`, since the shown Thing has no `Player` marker.
#[derive(Component)]
pub struct EditArt;

/// The camera that draws the level while Editing.
#[derive(Component)]
pub struct EditCamera;

/// On a game camera that `spawn_edit_camera` switched off, so leaving
/// Editing switches back on only those (a camera that was already off
/// stays off).
#[derive(Component)]
struct DisabledForEdit;

/// The edit camera and the canvas it draws into, as `level.view` reports them.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct EditView {
    /// World point at the centre of the canvas.
    pub at: Vec2,
    /// CSS pixels per world unit.
    pub zoom: f32,
    /// The canvas's rectangle in the frame's CSS pixels: x, y, width, height.
    pub canvas: [f32; 4],
}

impl Default for EditView {
    fn default() -> Self {
        Self {
            at: Vec2::ZERO,
            zoom: 1.0,
            canvas: [0.0, 0.0, GAME_WIDTH as f32, GAME_HEIGHT as f32],
        }
    }
}

impl EditView {
    /// Moves the camera. A non-finite `at` keeps the current centre; a zoom
    /// goes through [`clamp_zoom`].
    pub fn set_camera(&mut self, at: Option<Vec2>, zoom: Option<f32>) {
        if let Some(at) = at.filter(|a| a.is_finite()) {
            self.at = at;
        }
        if let Some(z) = zoom {
            self.zoom = clamp_zoom(self.zoom, z);
        }
    }

    pub fn canvas_size(&self) -> Vec2 {
        Vec2::new(self.canvas[2], self.canvas[3])
    }
}

/// A non-finite zoom keeps `current`; anything else (zero and negatives
/// included) is clamped into `[ZOOM_MIN, ZOOM_MAX]`.
pub fn clamp_zoom(current: f32, zoom: f32) -> f32 {
    if zoom.is_finite() {
        zoom.clamp(ZOOM_MIN, ZOOM_MAX)
    } else {
        current
    }
}

/// The box around every Thing of a registered kind; `None` for an empty level.
pub fn level_bounds(level: &LevelFile, kinds: &Kinds) -> Option<Rect> {
    let mut out: Option<Rect> = None;
    for t in &level.things {
        let Some(kind) = kinds.index(&t.kind) else {
            continue;
        };
        let size = placed_size(t, kinds.props(kind).expect("index came from kinds"));
        let r = Rect::from_center_size(Vec2::from(t.at), size);
        out = Some(out.map_or(r, |o| o.union(r)));
    }
    out
}

/// The centre and zoom that show `bounds` (plus a margin) in a canvas of
/// `canvas` CSS pixels. An empty level shows ten grid steps each way.
pub fn fit_view(bounds: Option<Rect>, grid: f32, canvas: Vec2) -> (Vec2, f32) {
    let b = bounds.unwrap_or_else(|| {
        Rect::from_center_half_size(Vec2::ZERO, Vec2::splat(grid.max(f32::EPSILON) * 10.0))
    });
    let size = (b.size() * FIT_MARGIN).max(Vec2::splat(f32::EPSILON));
    let zoom = (canvas.x / size.x).min(canvas.y / size.y);
    (b.center(), clamp_zoom(1.0, zoom))
}

/// `OnEnter(Editing)`, after the run is despawned: shows the level's Things
/// in file order, art on a child exactly as `spawn_level` draws it (the same
/// `level::spawn_art_child`, plus [`EditArt`]).
pub fn spawn_edit_level(
    mut commands: Commands,
    level: Res<Level>,
    kinds: Res<Kinds>,
    art: Option<Res<ThingArt>>,
    sprite_art: Option<Res<SpriteArt>>,
    sprite_images: Option<Res<SpriteImages>>,
) {
    for placed in &level.0.things {
        let Some(kind) = kinds.index(&placed.kind) else {
            continue;
        };
        let props = kinds.props(kind).expect("index came from kinds");
        let size = placed_size(placed, props);
        let mut e = commands.spawn((
            EditThing,
            GameEntity,
            Thing { kind },
            ThingId(placed.id.clone()),
            Size(size),
            Transform::from_xyz(placed.at[0], placed.at[1], 0.0),
            Visibility::default(),
        ));
        let sources = ArtSources {
            art: art.as_deref(),
            sprite_art: sprite_art.as_deref(),
            sprite_images: sprite_images.as_deref(),
        };
        spawn_art_child(&mut e, &sources, &kinds, kind, size, EditArt);
    }
}

/// `OnEnter(Editing)`: fits the camera to the level. Marked changed even when
/// nothing moved, so `level.view` is sent on every entry.
pub fn fit_edit_view(
    level: Res<Level>,
    kinds: Res<Kinds>,
    grid: Res<LevelGrid>,
    mut view: ResMut<EditView>,
) {
    let (at, zoom) = fit_view(level_bounds(&level.0, &kinds), grid.0, view.canvas_size());
    view.at = at;
    view.zoom = zoom;
    view.set_changed();
}

/// A live `level.json` while Editing: the shown level is replaced in place
/// and the camera stays where it is.
pub fn respawn_edit_level(world: &mut World) {
    let shown: Vec<Entity> = world
        .query_filtered::<Entity, With<EditThing>>()
        .iter(world)
        .collect();
    for e in shown {
        world.despawn(e);
    }
    if let Err(e) = world.run_system_once(spawn_edit_level) {
        error!("level editor: {e}");
    }
}

/// The edit camera (windowed builds only; `GamePlugin` adds it when not
/// headless): spawned on entering Editing with the game's own cameras
/// switched off, kept in step with [`EditView`], removed on leaving.
pub struct EditCameraPlugin;

impl Plugin for EditCameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(GameState::Editing),
            spawn_edit_camera.after(fit_edit_view),
        )
        .add_systems(OnExit(GameState::Editing), remove_edit_camera)
        .add_systems(
            Update,
            (track_canvas_rect, sync_edit_camera)
                .chain()
                .run_if(in_state(GameState::Editing)),
        );
    }
}

/// An orthographic projection that shows `zoom` CSS pixels per world unit
/// across the whole canvas.
fn projection_for(view: &EditView) -> Projection {
    let size = (view.canvas_size() / view.zoom).max(Vec2::splat(f32::EPSILON));
    Projection::from(OrthographicProjection {
        scaling_mode: ScalingMode::Fixed {
            width: size.x,
            height: size.y,
        },
        ..OrthographicProjection::default_3d()
    })
}

fn camera_transform(at: Vec2) -> Transform {
    Transform::from_xyz(at.x, at.y, CAMERA_Z).looking_at(at.extend(0.0), Vec3::Y)
}

fn spawn_edit_camera(
    mut commands: Commands,
    view: Res<EditView>,
    mut game_cameras: Query<
        (Entity, &mut Camera),
        (
            With<Camera3d>,
            Without<EditCamera>,
            Without<crate::frame::FrameCamera>,
        ),
    >,
) {
    for (e, mut camera) in &mut game_cameras {
        if camera.is_active {
            camera.is_active = false;
            commands.entity(e).insert(DisabledForEdit);
        }
    }
    commands.spawn((
        EditCamera,
        Camera3d::default(),
        projection_for(&view),
        camera_transform(view.at),
    ));
}

fn remove_edit_camera(
    mut commands: Commands,
    edit: Query<Entity, With<EditCamera>>,
    mut disabled: Query<
        (Entity, &mut Camera),
        (With<DisabledForEdit>, Without<crate::frame::FrameCamera>),
    >,
) {
    for e in &edit {
        commands.entity(e).despawn();
    }
    for (e, mut camera) in &mut disabled {
        camera.is_active = true;
        commands.entity(e).remove::<DisabledForEdit>();
    }
}

fn sync_edit_camera(
    view: Res<EditView>,
    mut cameras: Query<(&mut Projection, &mut Transform), With<EditCamera>>,
) {
    if !view.is_changed() {
        return;
    }
    for (mut projection, mut transform) in &mut cameras {
        *projection = projection_for(&view);
        *transform = camera_transform(view.at);
    }
}

/// Keeps `EditView::canvas` on the `#game` canvas's on-screen rectangle (the
/// gamebient-input glue letterboxes it with a CSS transform, which
/// `getBoundingClientRect` includes). Written only when it differs, so
/// `level.view` follows a resize.
#[cfg(target_arch = "wasm32")]
fn track_canvas_rect(mut view: ResMut<EditView>) {
    let Some(canvas) = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.get_element_by_id("game"))
    else {
        return;
    };
    let r = canvas.get_bounding_client_rect();
    let rect = [
        r.x() as f32,
        r.y() as f32,
        r.width() as f32,
        r.height() as f32,
    ];
    if rect[2] > 0.0 && rect[3] > 0.0 && view.canvas != rect {
        view.canvas = rect;
    }
}

/// Native windows draw the game at its own size (the editor hosts web builds).
#[cfg(not(target_arch = "wasm32"))]
fn track_canvas_rect() {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::content::kit::{EditRules, KindProps};
    use crate::game::content::level::set_level;
    use crate::game::replay::build_headless_app;
    use crate::game::sim::{Checksum, PendingSeed, RunClock, SimTick, SpawnOrder, seed_bytes};
    use crate::game::states::GameState;

    /// Kinds every template-derived kit game has.
    const LEVEL: &[u8] = br#"{"v":1,"things":[{"id":"player","kind":"Player","at":[0,0]},{"id":"coin-1","kind":"Coin","at":[3,0]}]}"#;
    const ONE: &[u8] = br#"{"v":1,"things":[{"id":"coin-9","kind":"Coin","at":[5,5]}]}"#;

    fn go(app: &mut App, s: GameState) {
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(s);
        app.update();
    }

    fn shown(app: &mut App) -> usize {
        app.world_mut()
            .query_filtered::<(), With<EditThing>>()
            .iter(app.world())
            .count()
    }

    fn edit_art(app: &mut App) -> usize {
        app.world_mut()
            .query_filtered::<(), With<EditArt>>()
            .iter(app.world())
            .count()
    }

    fn sim_entities(app: &mut App) -> usize {
        app.world_mut()
            .query::<&SpawnOrder>()
            .iter(app.world())
            .count()
    }

    fn boxes() -> Kinds {
        Kinds(vec![(
            "Box".into(),
            KindProps {
                size: Vec2::splat(10.0),
                colour: Color::WHITE,
                on_spawn: None,
                edit: EditRules::default(),
            },
        )])
    }

    #[test]
    fn level_bounds_use_the_kind_size_and_skip_unknown_kinds() {
        let level = LevelFile::parse(
            br#"{"v":1,"things":[{"id":"a","kind":"Box","at":[0,0]},{"id":"b","kind":"Box","at":[100,50],"size":[20,40]},{"id":"g","kind":"Ghost","at":[999,999]}]}"#,
        )
        .unwrap();
        let b = level_bounds(&level, &boxes()).unwrap();
        assert_eq!(b, Rect::new(-5.0, -5.0, 110.0, 70.0));
        let empty = LevelFile {
            v: 1,
            things: vec![],
        };
        assert_eq!(level_bounds(&empty, &boxes()), None);
    }

    #[test]
    fn fit_view_centres_the_level_with_a_margin() {
        // 400 x 200 around (100, 50); with 10 % each side 480 x 240.
        let (at, zoom) = fit_view(
            Some(Rect::new(-100.0, -50.0, 300.0, 150.0)),
            20.0,
            Vec2::new(960.0, 720.0),
        );
        assert_eq!(at, Vec2::new(100.0, 50.0));
        assert!((zoom - 2.0).abs() < 1e-5, "{zoom}");
    }

    #[test]
    fn an_empty_level_fits_ten_grid_steps_each_way() {
        let (at, zoom) = fit_view(None, 20.0, Vec2::new(960.0, 720.0));
        assert_eq!(at, Vec2::ZERO);
        assert!((zoom - 1.5).abs() < 1e-5, "{zoom}");
    }

    #[test]
    fn zoom_keeps_non_finite_values_out_and_clamps_zero() {
        assert_eq!(clamp_zoom(2.0, f32::NAN), 2.0);
        assert_eq!(clamp_zoom(2.0, f32::INFINITY), 2.0);
        assert_eq!(clamp_zoom(2.0, 0.0), ZOOM_MIN);
        assert_eq!(clamp_zoom(2.0, -3.0), ZOOM_MIN);
        assert_eq!(clamp_zoom(2.0, 1e9), ZOOM_MAX);
        let mut v = EditView::default();
        v.set_camera(Some(Vec2::new(f32::INFINITY, 0.0)), Some(0.0));
        assert_eq!(v.at, Vec2::ZERO);
        assert_eq!(v.zoom, ZOOM_MIN);
        v.set_camera(Some(Vec2::new(4.0, -2.0)), None);
        assert_eq!((v.at, v.zoom), (Vec2::new(4.0, -2.0), ZOOM_MIN));
    }

    /// Entering Editing despawns the run and shows the level with no sim
    /// components; ten frames in Editing tick nothing; Play starts a fresh run.
    #[test]
    fn editing_runs_no_sim_system_and_despawns_the_run() {
        let mut app = build_headless_app();
        set_level(app.world_mut(), LEVEL).unwrap();
        app.world_mut().resource_mut::<PendingSeed>().0 = Some(seed_bytes(1));
        app.update();
        go(&mut app, GameState::Playing);
        for _ in 0..5 {
            app.update();
        }
        let ticked = app.world().resource::<SimTick>().0;
        assert!(ticked >= 5, "{ticked}");
        assert!(sim_entities(&mut app) > 0);

        go(&mut app, GameState::Editing);
        assert_eq!(sim_entities(&mut app), 0, "the run is despawned");
        assert_eq!(shown(&mut app), 2, "the level is shown");
        let tick = *app.world().resource::<SimTick>();
        let clock = *app.world().resource::<RunClock>();
        let sum = *app.world().resource::<Checksum>();
        for _ in 0..10 {
            app.update();
        }
        assert_eq!(*app.world().resource::<SimTick>(), tick);
        assert_eq!(*app.world().resource::<RunClock>(), clock);
        assert_eq!(*app.world().resource::<Checksum>(), sum);

        go(&mut app, GameState::Playing);
        assert_eq!(shown(&mut app), 0, "the shown level is gone");
        assert!(app.world().resource::<SimTick>().0 < ticked, "a fresh run");
        assert_eq!(sim_entities(&mut app), 2, "the run spawned the level");
    }

    #[test]
    fn entering_editing_fits_the_camera_to_the_level() {
        let mut app = build_headless_app();
        set_level(app.world_mut(), LEVEL).unwrap();
        app.update();
        go(&mut app, GameState::Editing);
        let view = *app.world().resource::<EditView>();
        let grid = app.world().resource::<LevelGrid>().0;
        let level = &app.world().resource::<Level>().0;
        let kinds = app.world().resource::<Kinds>();
        let (at, zoom) = fit_view(level_bounds(level, kinds), grid, view.canvas_size());
        assert_eq!((view.at, view.zoom), (at, zoom));
    }

    #[test]
    fn respawn_shows_the_new_level_and_keeps_the_camera() {
        let mut app = build_headless_app();
        // Asset stores, so `setup_thing_art` builds `ThingArt` at Startup
        // and the shown Things get art children.
        app.init_resource::<Assets<Mesh>>();
        app.init_resource::<Assets<StandardMaterial>>();
        set_level(app.world_mut(), LEVEL).unwrap();
        app.update();
        go(&mut app, GameState::Editing);
        assert_eq!(shown(&mut app), 2);
        assert_eq!(edit_art(&mut app), 2, "each shown Thing has an art child");
        app.world_mut().resource_mut::<EditView>().at = Vec2::new(7.0, 7.0);
        set_level(app.world_mut(), ONE).unwrap();
        respawn_edit_level(app.world_mut());
        assert_eq!(shown(&mut app), 1);
        assert_eq!(edit_art(&mut app), 1, "the old art children are gone");
        assert_eq!(app.world().resource::<EditView>().at, Vec2::new(7.0, 7.0));
    }
}
