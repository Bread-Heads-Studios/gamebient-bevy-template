# Bevy Version Reference

| Field | Value |
|-------|-------|
| Pinned version | `bevy 0.18.0` (`Cargo.lock`; `Cargo.toml` says `"0.18"`) |
| Rust edition | 2024 |
| API docs | <https://docs.rs/bevy/0.18.0> (never `latest`) |
| Upstream | 0.19 and 0.20 exist (migration guides published); not used here |
| Sources | Official Bevy migration guides, bevy-website @ 2026-11-01 |

## Knowledge gap warning

Model training data is weighted toward Bevy 0.14-0.16 and writes stale APIs
(`EventWriter`, bundles, `despawn_recursive`, `Style`, `weak_handle!`). This
template is on **0.18**. Before writing a Bevy call, check this page and grep
`src/` for how the template already does it.

The opposite mistake matters too: **do not use 0.19 or 0.20 APIs**, and do
not follow docs.rs "latest" pages or 0.19+ release notes. They describe APIs
this crate does not have (0.19 alone moves resources onto entities). Use
`docs.rs/bevy/0.18.0`.

Every row below cites the migration guide section that documents it
(PR numbers from the guide's `heading_metadata`, or its `prs` list for
0.15/0.16). Rows marked "in `src/`" were also checked against this repo.

## Most common stale patterns

| Stale (what agents write) | 0.18 | Since |
|---------------------------|------|-------|
| `EventWriter<E>` / `EventReader<E>` / `Events<E>` for buffered events | `MessageWriter<M>` / `MessageReader<M>` / `Messages<M>`, `#[derive(Message)]`, `app.add_message::<M>()` | 0.17 |
| `writer.send(e)` / `send_batch` / `send_default` | `writer.write(e)` / `write_batch` / `write_default` | 0.16 |
| `world.send_event(e)`, `commands.send_event(e)` | `world.write_message(e)`, `commands.write_message(e)` | 0.17 |
| `Trigger<OnAdd, T>`, `trigger.target()` / `.entity()` | `On<Add, T>`, `add.entity` (event field) | 0.17 |
| `world.trigger_targets(E, entity)` | `#[derive(EntityEvent)] struct E { entity: Entity }`, `world.trigger(E { entity })` | 0.17 |
| `app.observe(f)` | `app.add_observer(f)`; `entity.observe(f)` on one entity | 0.15 |
| `query.single()` panics; `get_single()` | `single()` / `single_mut()` return `Result`: `let Ok(x) = q.single() else { return };` | 0.16 |
| `despawn_recursive()` | `despawn()` (now despawns children) | 0.16 |
| `despawn_descendants()` | `despawn_related::<Children>()` | 0.16 |
| `Parent`, `set_parent(p)`, `*parent` | `ChildOf(p)`, `child_of.parent()` | 0.16 |
| `clear_children()` / `remove_children()` | `detach_all_children()` / `detach_children()` | 0.18 |
| `SpriteBundle`, `Camera2dBundle`, `NodeBundle`, `TextBundle`, `AudioBundle`, `SpatialBundle`, `PbrBundle`, `MaterialMesh2dBundle` | Spawn the component (`Sprite`, `Camera2d`, `Node`, `Text`, `AudioPlayer`, `Transform` + `Visibility`, `Mesh3d` + `MeshMaterial3d`); required components fill the rest | 0.15 |
| `#[require(A(\|\| A(10)))]` | `#[require(A(10))]` or `#[require(A = make_a())]` | 0.16 |
| `Style { .. }` inside `NodeBundle` | `Node { .. }` (layout fields); old `Node` is `ComputedNode` | 0.15 |
| `Text::from_section`, `TextStyle { font, font_size, color }` | `Text::new(s)` + `TextFont { .. }` + `TextColor(c)`; extra sections are `TextSpan` children | 0.15 |
| `TextFont { line_height, .. }` | `LineHeight` component (required by `Text`/`Text2d`) | 0.18 |
| `JustifyText::Center` | `Justify::Center` | 0.17 |
| `Transform` on a UI node | `UiTransform` (`Val2` translation, `Rot2` rotation, `Vec2` scale) | 0.17 |
| `BorderRadius::all(..)` as a component | `Node { border_radius: BorderRadius::all(..), .. }` | 0.18 |
| `BorderColor(c)` | `BorderColor::all(c)` | 0.17 |
| `UiImage` | `ImageNode` | 0.15 |
| `Camera { target: .., .. }` | `RenderTarget` component on the camera entity | 0.18 |
| `Camera { hdr: true, .. }` | `Hdr` marker component | 0.17 |
| `Msaa` resource | `Msaa` component on each camera | 0.15 |
| `bevy::render::camera::*`, `bevy::render::view::Visibility`, `RenderLayers` | `bevy::camera::*`, `bevy::camera::visibility::*` | 0.17 |
| `Sprite { anchor: Anchor::Center, .. }` | `Anchor::CENTER` as a separate component | 0.17 |
| `AudioBundle { source, settings }` | `(AudioPlayer(handle), PlaybackSettings::DESPAWN)` | 0.15 |
| `Volume(0.5)`, `Volume::ZERO` | `Volume::Linear(0.5)`, `Volume::SILENT` | 0.16 |
| `time.elapsed_seconds()` | `time.elapsed_secs()` | 0.15 |
| `timer.finished()` / `timer.paused()` | `timer.is_finished()` / `timer.is_paused()` | 0.17 |
| `bevy::utils::HashMap` / `HashSet` | `bevy::platform::collections::HashMap` / `HashSet` | 0.16 |
| `Handle::weak_from_u128(..)`, `weak_handle!("..")` | `uuid_handle!("..")` | 0.17 |
| `handle.clone_weak()` | `handle.clone()` | 0.17 |
| `StateScoped(S::X)`, `enable_state_scoped_entities` | `DespawnOnExit(S::X)`; scoping is always on | 0.17 |
| `next.set(S::X)` is a no-op when already in `X` | It now re-runs `OnExit`/`OnEnter`; use `set_if_neq` for the old behaviour | 0.18 |
| `WindowResolution::new(960.0, 720.0)` | `WindowResolution::new(960, 720)` (`u32`) | 0.17 |
| `UiSystem`, `CameraUpdateSystem`, `TransformSystem`, `RunFixedMainLoopSystem` | `UiSystems`, `CameraUpdateSystems`, `TransformSystems`, `RunFixedMainLoopSystems` | 0.17 |
| `Condition` trait | `SystemCondition` | 0.17 |

## 0.14 → 0.15

Guide: `release-content/0.15/migration-guides/` in bevy-website.

| Area | Change | PR |
|------|--------|----|
| ECS | Bundles deprecated for required components: cameras, sprites, meshes, UI, text, audio, transform, visibility | 15641, 15489, 15524, 15898, 15887, 15573, 14964, 15474 |
| ECS | `SpatialBundle` deprecated: insert `Transform` + `Visibility` | 15830 |
| ECS | `App/World/Commands::observe` → `add_observer`; `EntityWorldMut::observe_entity` → `observe` | 15754 |
| Assets | `Handle<T>` is no longer a `Component`; wrap it (`Mesh2d`, `Mesh3d`, `MeshMaterial2d`, `MeshMaterial3d`) | 15796, 15524 |
| UI | `Style` → `Node`; old `Node` → `ComputedNode`; `NodeBundle` → `Node` | 15898 |
| UI | `UiImage` → `ImageNode`, `UiImageSize` → `ImageNodeSize` | 16271 |
| UI | `BackgroundColor` / `BorderColor` default is transparent; `BackgroundColor` no longer tints images | 14017 |
| Text | `TextBundle`/`Text2dBundle` removed; `Text`/`Text2d` require `TextLayout`, `TextFont`, `TextColor`; sections are `TextSpan` children (`TextUiWriter`, `Text2dWriter` to index them) | 15591, 15887 |
| Text | `TextStyle` → `TextFont`; its `color` → `TextColor` | 15857 |
| Camera | `Camera2dBundle` → `Camera2d` | 15641 |
| Camera | `world_to_viewport`, `viewport_to_world(_2d)` return `Result` | 14989 |
| Camera | `OrthographicProjection { .., ..default() }` → `..OrthographicProjection::default_2d()` / `default_3d()` | 15073 |
| Render | `Msaa` is a per-camera component, not a resource | 14273 |
| Render | `ScreenshotManager` removed: spawn `Screenshot` and observe `ScreenshotCaptured` | 14833 |
| Audio | `AudioBundle` / `AudioSourceBundle` → `AudioPlayer` (+ optional `PlaybackSettings`) | 15573 |
| Time | `Time::elapsed_seconds` → `elapsed_secs` (and `_f64`, `_wrapped`); `Stopwatch::paused` → `is_paused` | 15962 |
| Time | `run_fixed_main_schedule` private; order with the `RunFixedMainLoopSystem` set (renamed again in 0.17) | 14881 |

## 0.15 → 0.16

Guide: `release-content/0.16/migration-guides/` in bevy-website.

| Area | Change | PR |
|------|--------|----|
| ECS | `Query::single` / `single_mut` return `Result`; `get_single` deprecated. Systems may return `Result` and use `?` | 18082 |
| ECS | `EventWriter::send*` → `write*` | 17977 |
| ECS | Hierarchy is a relationship: `despawn()` is recursive, `despawn_descendants()` → `despawn_related::<Children>()`, `with_children` passes `ChildSpawnerCommands`, `set_parent(p)` → `insert(ChildOf(p))` | 17398 |
| ECS | `Parent` → `ChildOf`; parent via `child_of.parent()` (no `Deref`) | 17427 |
| ECS | `#[require]` syntax: `A(10)` inline value or `A = expr()` | 18555 |
| ECS | `Trigger::entity()` → `Trigger::target()` (both replaced in 0.17) | 16716 |
| ECS | `bevy_core` removed: `Name` in `bevy_ecs::name`, `FrameCount` in `bevy_diagnostic`, `TaskPoolPlugin` in `bevy_app` | 16894, 16897 |
| Platform | `HashMap`, `HashSet` → `bevy_platform::collections`; `Instant` → `bevy_platform::time`; `hashbrown` re-export removed | 15801 + 9 more |
| Assets | `Handle::weak_from_u128` deprecated for `weak_handle!` (replaced in 0.17) | 17695 |
| Audio | `Volume` is an enum: `Volume::Linear(f)` / `Volume::Decibels(db)`; `ZERO` → `SILENT`; sinks and `GlobalVolume` take `Volume` | 17605 |
| Audio | `AudioSinkPlayback::toggle()` → `toggle_playback()`; mute added | 16837, 16813 |
| UI | `TargetCamera` → `UiTargetCamera` | 17403 |
| Text | `TextFont` gained `line_height` (moved to `LineHeight` in 0.18) | 16614 |
| Window | `WindowMode::SizedFullscreen(m)` / `Fullscreen(m)` → `Fullscreen(m, VideoModeSelection)`; `VideoModeSelection::Current` recommended | 17525 |
| Build | Bevy itself moves to Rust 2024; some RPIT lifetimes changed | 17967 |

## 0.16 → 0.17

Guide: `content/learn/migration-guides/0.16-to-0.17.md` in bevy-website.

| Area | Change | PR |
|------|--------|----|
| ECS | Buffered events are messages: `EventWriter`/`EventReader`/`Events` → `MessageWriter`/`MessageReader`/`Messages`, `Message` trait. `Event` now means observer events only | 19647 |
| ECS | `World`/`DeferredWorld`/`Commands::send_event*` → `write_message*`; `Events::send` → `Messages::write` | 20017, 20953 |
| ECS | `RemovedComponents::events` → `messages` | 20953, 20954 |
| ECS | Observers: `Trigger` → `On`; `OnAdd`/`OnInsert`/`OnReplace`/`OnRemove`/`OnDespawn` → `Add`/`Insert`/`Replace`/`Remove`/`Despawn`; entity-targeted events derive `EntityEvent` with an entity field; `trigger_targets` removed for `trigger`; propagation is `#[entity_event(propagate)]` | 20731, 19440, 19596 |
| ECS | `Condition` → `SystemCondition` | 19328 |
| ECS | System sets use a `*Systems` suffix (`UiSystems`, `CameraUpdateSystems`, `TransformSystems`, `TimeSystems`, `InputSystems`, `RunFixedMainLoopSystems`, ...) | 18900 |
| ECS | `Option<Single<..>>` is `None` (not skipped) when several entities match | 18766 |
| ECS | `GLOBAL_ERROR_HANDLER` → `App::set_error_handler`; `configurable_error_handler` feature gone | 18810 |
| ECS | `insert_or_spawn_batch` removed | 18148 |
| State | `StateScoped` → `DespawnOnExit` (new `DespawnOnEnter`); `add_state_scoped_event` → `clear_events_on_exit` | 18818, 19435, 20872 |
| State | State-scoped entities always on; `enable_state_scoped_entities` and `#[states(scoped_entities)]` are no-ops / removed | 19354, 20883 |
| Assets | `Handle::Weak` → `Handle::Uuid`: `weak_handle!` → `uuid_handle!`, `clone_weak()` → `clone()` (in `src/`: `src/ui/fonts.rs`) | 19896 |
| Assets | `Assets::insert` / `get_or_insert_with` return `Result` | 20439 |
| Crates | Camera, visibility, `RenderLayers`, `ClearColor`, `Projection` → `bevy::camera`; lights → `bevy::light`; `Mesh`, `Mesh2d`, `Mesh3d` → `bevy::mesh`; `Image`, `ImagePlugin` → `bevy::image`; shaders → `bevy::shader`; `MeshMaterial2d`, `Material2d` → `bevy::sprite_render`; UI materials → `bevy::ui_render`; `Bloom` etc. → `bevy::post_process`; FXAA/SMAA/TAA → `bevy::anti_alias`; `RenderAssetUsages` → `bevy::asset` | `bevy_render` reorganization (20485 et al.) |
| Features | With `default-features = false`, `bevy_ui` needs `bevy_ui_render` and `bevy_sprite` needs `bevy_sprite_render` to draw; `UiPlugin` has no fields (toggle `UiRenderPlugin`) | same section |
| Features | `zstd` → `zstd_rust` or `zstd_c`; one is required with `tonemapping_luts` and no defaults (template: `zstd_rust`) | 19793 |
| Features | `gles` wgpu backend not default; add `bevy_render/gles` for native OpenGL | 20793 |
| Features | Wayland is a default feature | 19232 |
| Features | `minimp3` no longer exposed; `mp3` remains | 20183 |
| Camera | `Camera.hdr` → `Hdr` marker component | 18873 |
| Sprite | `Sprite.anchor` → required `Anchor` component; `Anchor::Center` → `Anchor::CENTER`, `Custom(v)` → `Anchor(v)` | 18393 |
| Text | `Text2d`, `Text2dShadow` moved to `bevy::sprite` | 20594 |
| Text | `JustifyText` → `Justify` | 19522 |
| Text | `TextFont::from_font(h)` / `from_line_height(l)` → `TextFont::from(..)` | 20335, 20450 |
| UI | Nodes use `UiTransform` / `UiGlobalTransform`, not `Transform`; `Node` requires `UiTransform` | 16615 |
| UI | `BorderColor` has per-edge fields: `BorderColor(c)` → `BorderColor::all(c)` | 18682 |
| UI | `ComputedNodeTarget` and `update_ui_context_system` renamed | 20519, 20532 |
| Audio | `Volume` lost `Add`/`Sub`; use `increase_by_percentage` | 19423 |
| Time | `Timer::finished` / `paused` → `is_finished` / `is_paused` | 19386 |
| Window | `CursorOptions` is its own component; `WindowPlugin { primary_cursor_options, .. }` | 19668 |
| Window | `WindowResolution::new` takes `u32`; `WindowResolution::from(uvec2)` | 20582 |
| Window | `CursorIcon`, `CustomCursor` moved to `bevy::window` | 20427 |
| wasm | New `getrandom` needs `features = ["wasm_js"]` plus the `getrandom_backend` cfg (template: `Cargo.toml`, `.cargo/config.toml`) | 18047 |

## 0.17 → 0.18

Guide: `content/learn/migration-guides/0.17-to-0.18.md` in bevy-website.

| Area | Change | PR |
|------|--------|----|
| ECS | `clear_children` → `detach_all_children`, `remove_children` → `detach_children`, `remove_child` → `detach_child`, `clear_related` → `detach_all_related` (they never despawn) | 21470, 21537 |
| ECS | `EntityEvent` is immutable; `from` / `event_target_mut` live on `SetEntityEventTarget` | 21408 |
| ECS | Run-condition combinators (`and`, `or`, ...) treat a failing system as `false` instead of erroring | 20671 |
| ECS | `Internal` component removed; observer and one-shot system entities are visible to queries | 21623 |
| ECS | `Tick`, `ComponentTicks` moved to `change_detection` | 21562, 21613 |
| ECS | `Entities` alloc/reserve/flush APIs reworked (`spawn` / `despawn` unchanged) | 19350, 19433, 19451 |
| State | `NextState::set` always runs `OnExit` / `OnEnter`, even for the current state; `set_if_neq` keeps the old behaviour | 19363, 21792 |
| Camera | `Camera.target` → `RenderTarget` required component (in `src/`: `src/display.rs`) | 20917 |
| Light | `AmbientLight` resource → `GlobalAmbientLight`; `AmbientLight` is now a per-camera component | 21585 |
| UI | `BorderRadius` is a `Node` field, not a component (in `src/`: `src/ui/card.rs`) | 21781 |
| UI | `BorderRect` fields → `min_inset` / `max_inset` (`Vec2`) | 21581 |
| UI | Only the text runs of a UI `Text` node are pickable | 22047 |
| Text | `TextFont.line_height` → `LineHeight` component, required by `Text`, `Text2d`, `TextSpan` | 21180 |
| Text | `FontAtlasSets` removed; `FontAtlasSet` is a resource | 21345 |
| Render | `Aabb` of sprites and meshes updates automatically when they change; `NoAutoAabb` opts out | 18742 |
| Platform | `bevy_platform` `HashMap::get_many_*` → `get_disjoint_*` | 21898 |
| Window | Winit user events removed: `EventLoopProxyWrapper` is not generic; send `WinitUserEvent::WakeUp` | 22088 |
| Features | `animation` → `gltf_animation`; picking backends → `sprite_picking`, `ui_picking`, `mesh_picking` | 21388, 21435, 21436, 21437 |
| Features | Input sources are features (`mouse`, `keyboard`, `gamepad`, `touch`, `gestures`); `bevy_window` / `bevy_gilrs` enable what they need, so the template's list does not name them | 21447 |
| Features | Feature collections (`2d`, `3d`, `ui`, `default_app`, ...) exist; this template keeps its curated list on purpose (wasm size) | 21472 |

## Features in this template's `Cargo.toml`

Several entries in the `default-features = false` list are 0.17 crate splits
(`bevy_render` reorganization): `bevy_camera`, `bevy_light`, `bevy_mesh`,
`bevy_image`, `bevy_shader`, `bevy_post_process`, `bevy_sprite_render`,
`bevy_ui_render`. Removing one because "the old docs don't list it" breaks
the build or silently stops a plugin rendering. `zstd_rust` is the 0.17
`zstd` rename (19793). Add features deliberately; see `AGENTS.md`.

## Where `src/` already does it right

Copy these rather than writing from memory:

| Pattern | File |
|---------|------|
| `#[derive(Message)]`, `add_message`, `MessageWriter` / `MessageReader` | `src/game/mod.rs`, `src/frame/highlight.rs`, `src/ui/menu.rs` |
| `Node { .. }`, `Text::new` + `TextFont` + `TextColor`, `UiTransform`, `border_radius` | `src/ui/card.rs`, `src/ui/menu.rs`, `src/ui/studio_logo.rs` |
| `Camera2d`, `Msaa::Off`, `RenderLayers` | `src/frame/camera.rs` |
| `Camera3d`, `Mesh3d` + `MeshMaterial3d`, `DirectionalLight` | `src/game/mod.rs`, `src/game/player.rs` |
| `RenderTarget`, `Viewport`, `CameraUpdateSystems`, `WindowResolution::new(u32, u32)` | `src/display.rs` |
| `Text2d`, `Anchor::CENTER` | `src/frame/marquee.rs` |
| `AudioPlayer` + `PlaybackSettings::DESPAWN`, `Volume::Linear`, `AudioSink` | `src/game/audio/mod.rs`, `src/game/host.rs` |
| `FixedUpdate`, `Time::<Fixed>::from_duration` | `src/game/mod.rs`, `src/game/sim.rs` |
| `uuid_handle!` | `src/ui/fonts.rs` |
| `Screenshot::primary_window()` + `.observe(save_to_disk(path))` / `On<ScreenshotCaptured>` | `src/game/autopilot.rs`, `src/game/record/mod.rs` |
| `q.single()` as `Result` (`let Ok(..) = .. else`) | `src/game/player.rs`, `src/display.rs` |
| `OnEnter` / `OnExit`, `NextState::set` | `src/ui/mod.rs`, `src/ui/transition.rs` (state enum: `src/game/states.rs`) |

## How to check

When unsure, grep the 0.18 guide (`0.17-to-0.18.md`, then `0.16-to-0.17.md`)
in bevy-website and the `bevy*-0.18.0` crate source in
`~/.cargo/registry/src/`, or read `docs.rs/bevy/0.18.0`. Do not trust memory.
