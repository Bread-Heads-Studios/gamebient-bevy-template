//! Scene colours `LookColour` does not reach: lights and the clear colour.
//!
//! - [`LookLight`] (a component, 3D only): the entity's `PointLight`,
//!   `SpotLight`, `DirectionalLight` and/or camera `AmbientLight` colour
//!   follows a palette role.
//! - [`LookAmbient`] (a resource, 3D only): `GlobalAmbientLight`'s colour
//!   follows a role.
//! - [`LookClearColour`] (a resource, 2D and 3D): `ClearColor` follows a
//!   role (the letterbox bars and any gap no geometry or UI covers).
//!
//! All three write **only the colour**: intensity, illuminance, brightness,
//! range and shadows stay the game's, so an intensity animation (a
//! flickering lamp, a pulsing spot) keeps running under every pack. They
//! write on every `LookChanged` (a clear included, which restores the role
//! default), and on the frame the tag spawns or the resource is inserted
//! **only under a pack**: under the default look nothing is written at
//! spawn, so tagging a light can never change the default look, even where
//! the game's light colour is not exactly its role's default. Each compares
//! before it writes.
//!
//! Don't tag a light whose **colour** a game system animates (a hue-cycling
//! party light): every `LookChanged` would reset it (pattern 1, AGENTS.md).
//! Have that system read `ActiveLook::colour` / `mix` instead.
//!
//! ```ignore
//! commands.spawn((PointLight { color: LAMP, intensity: 9e5, ..default() }, LookLight("lamp")));
//! app.insert_resource(ClearColor(VOID)).insert_resource(LookClearColour("void"));
//! ```
//!
//! The light half needs `bevy_pbr` / `bevy_light`: compiled out under
//! `gx_look_no_pbr` (see `apply.rs`). `LookClearColour` works in 2D.

use bevy::prelude::*;

use super::apply::LookChanged;
use super::runtime::ActiveLook;

/// `ClearColor` follows palette role `.0`. Insert it next to the game's
/// own `ClearColor` (`main.rs`), whose value should be the role default.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct LookClearColour(pub &'static str);

/// Whether a tag that just appeared should be painted: only under a pack,
/// so the default look keeps exactly what spawn code built.
fn paint_on_spawn(look: &ActiveLook) -> bool {
    look.manifest.is_some()
}

pub fn sync_clear_colour(
    look: Res<ActiveLook>,
    mut changed: MessageReader<LookChanged>,
    tag: Res<LookClearColour>,
    clear: Option<ResMut<ClearColor>>,
) {
    let all = changed.read().count() > 0;
    let Some(mut clear) = clear else {
        return;
    };
    if all || (tag.is_added() && paint_on_spawn(&look)) {
        let c = look.colour(tag.0);
        if clear.0 != c {
            clear.0 = c;
        }
    }
}

#[cfg(not(gx_look_no_pbr))]
pub use lights::{LookAmbient, LookLight, retint_lights, sync_ambient};

#[cfg(not(gx_look_no_pbr))]
mod lights {
    use bevy::light::GlobalAmbientLight;
    use bevy::prelude::*;

    use super::paint_on_spawn;
    use crate::look::apply::LookChanged;
    use crate::look::runtime::ActiveLook;

    /// The colour of the entity's `PointLight`, `SpotLight`,
    /// `DirectionalLight` and camera `AmbientLight` (whichever it has)
    /// follows palette role `.0`. Colour only (module docs).
    #[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
    pub struct LookLight(pub &'static str);

    /// `GlobalAmbientLight`'s colour follows palette role `.0`.
    #[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
    pub struct LookAmbient(pub &'static str);

    pub fn retint_lights(
        look: Res<ActiveLook>,
        mut changed: MessageReader<LookChanged>,
        mut q: Query<(
            Ref<LookLight>,
            Option<&mut PointLight>,
            Option<&mut SpotLight>,
            Option<&mut DirectionalLight>,
            Option<&mut AmbientLight>,
        )>,
    ) {
        let all = changed.read().count() > 0;
        let spawn = paint_on_spawn(&look);
        for (tag, point, spot, sun, ambient) in &mut q {
            if !(all || (tag.is_added() && spawn)) {
                continue;
            }
            let c = look.colour(tag.0);
            // Deref only to write: an untouched light is not marked changed.
            if let Some(mut l) = point
                && l.color != c
            {
                l.color = c;
            }
            if let Some(mut l) = spot
                && l.color != c
            {
                l.color = c;
            }
            if let Some(mut l) = sun
                && l.color != c
            {
                l.color = c;
            }
            if let Some(mut l) = ambient
                && l.color != c
            {
                l.color = c;
            }
        }
    }

    pub fn sync_ambient(
        look: Res<ActiveLook>,
        mut changed: MessageReader<LookChanged>,
        tag: Res<LookAmbient>,
        ambient: Option<ResMut<GlobalAmbientLight>>,
    ) {
        let all = changed.read().count() > 0;
        let Some(mut ambient) = ambient else {
            return;
        };
        if all || (tag.is_added() && paint_on_spawn(&look)) {
            let c = look.colour(tag.0);
            if ambient.color != c {
                ambient.color = c;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::look::Contract;
    use crate::look::manifest::MANIFEST_FILE;
    use crate::look::select::Selection;
    use std::collections::BTreeMap;

    const VOID: Color = Color::srgb(0.01, 0.02, 0.05);
    const LAMP: Color = Color::srgb(1.0, 0.8, 0.5);

    static C: Contract = Contract {
        palette: &[("void", VOID), ("lamp", LAMP)],
        strings: &[],
        slots: &[],
        audio: &[],
        fonts: &[],
        glow: &[],
    };

    fn pack() -> ActiveLook {
        let json = r##"{"v":1,"id":"p","name":"P","kind":"palette","palette":{"void":"#203040","lamp":"#33ccff"}}"##;
        let files = BTreeMap::from([(MANIFEST_FILE.to_string(), json.as_bytes().to_vec())]);
        ActiveLook::from_files(&C, files, Selection::Default, None).expect("valid pack")
    }

    fn app(look: ActiveLook) -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<LookChanged>()
            .insert_resource(look)
            .add_systems(
                Update,
                sync_clear_colour.run_if(resource_exists::<LookClearColour>),
            );
        #[cfg(not(gx_look_no_pbr))]
        app.add_systems(
            Update,
            (
                retint_lights,
                sync_ambient.run_if(resource_exists::<LookAmbient>),
            ),
        );
        app
    }

    fn clear(app: &App) -> Color {
        app.world().resource::<ClearColor>().0
    }

    /// A game colour that is not the role default, so a write would show.
    const OFF: Color = Color::srgb(0.5, 0.0, 0.5);

    #[test]
    fn the_clear_colour_is_left_alone_under_the_default_look() {
        let mut app = app(ActiveLook::default_for(&C));
        app.insert_resource(ClearColor(OFF))
            .insert_resource(LookClearColour("void"));
        app.update();
        app.update();
        assert_eq!(clear(&app), OFF);
    }

    #[test]
    fn the_clear_colour_follows_a_pack_and_a_clear_restores_the_default() {
        let mut app = app(pack());
        app.insert_resource(ClearColor(VOID))
            .insert_resource(LookClearColour("void"));
        app.update();
        let want = app.world().resource::<ActiveLook>().colour("void");
        assert_eq!(clear(&app), want);
        assert_ne!(want, VOID);

        app.insert_resource(ActiveLook::default_for(&C));
        app.world_mut().write_message(LookChanged);
        app.update();
        assert_eq!(clear(&app), VOID);

        // A later hot swap to a pack lands too.
        app.insert_resource(pack());
        app.world_mut().write_message(LookChanged);
        app.update();
        assert_eq!(clear(&app), want);
    }

    #[test]
    fn no_clear_colour_resource_is_not_an_error() {
        let mut app = app(pack());
        app.insert_resource(LookClearColour("void"));
        app.update();
    }

    #[cfg(not(gx_look_no_pbr))]
    mod lights {
        use super::*;
        use bevy::light::GlobalAmbientLight;

        fn spawn_lights(app: &mut App, colour: Color) -> [Entity; 4] {
            let w = app.world_mut();
            [
                w.spawn((
                    PointLight {
                        color: colour,
                        intensity: 12_345.0,
                        ..default()
                    },
                    LookLight("lamp"),
                ))
                .id(),
                w.spawn((
                    SpotLight {
                        color: colour,
                        intensity: 6_789.0,
                        ..default()
                    },
                    LookLight("lamp"),
                ))
                .id(),
                w.spawn((
                    DirectionalLight {
                        color: colour,
                        illuminance: 4_321.0,
                        ..default()
                    },
                    LookLight("lamp"),
                ))
                .id(),
                w.spawn((
                    AmbientLight {
                        color: colour,
                        brightness: 55.0,
                        ..default()
                    },
                    LookLight("lamp"),
                ))
                .id(),
            ]
        }

        fn colours(app: &App, e: [Entity; 4]) -> [Color; 4] {
            let w = app.world();
            [
                w.get::<PointLight>(e[0]).unwrap().color,
                w.get::<SpotLight>(e[1]).unwrap().color,
                w.get::<DirectionalLight>(e[2]).unwrap().color,
                w.get::<AmbientLight>(e[3]).unwrap().color,
            ]
        }

        fn intensities(app: &App, e: [Entity; 4]) -> [f32; 4] {
            let w = app.world();
            [
                w.get::<PointLight>(e[0]).unwrap().intensity,
                w.get::<SpotLight>(e[1]).unwrap().intensity,
                w.get::<DirectionalLight>(e[2]).unwrap().illuminance,
                w.get::<AmbientLight>(e[3]).unwrap().brightness,
            ]
        }

        const SET: [f32; 4] = [12_345.0, 6_789.0, 4_321.0, 55.0];

        #[test]
        fn lights_are_left_alone_under_the_default_look() {
            let mut app = app(ActiveLook::default_for(&C));
            app.insert_resource(GlobalAmbientLight {
                color: OFF,
                ..default()
            })
            .insert_resource(LookAmbient("lamp"));
            let e = spawn_lights(&mut app, OFF);
            app.update();
            app.update();
            assert_eq!(colours(&app, e), [OFF; 4]);
            assert_eq!(app.world().resource::<GlobalAmbientLight>().color, OFF);
        }

        #[test]
        fn lights_follow_a_pack_keep_their_intensity_and_restore_on_clear() {
            let mut app = app(pack());
            app.insert_resource(GlobalAmbientLight {
                color: LAMP,
                brightness: 77.0,
                ..default()
            })
            .insert_resource(LookAmbient("lamp"));
            let e = spawn_lights(&mut app, LAMP);
            app.update();
            let want = app.world().resource::<ActiveLook>().colour("lamp");
            assert_ne!(want, LAMP);
            assert_eq!(colours(&app, e), [want; 4]);
            assert_eq!(intensities(&app, e), SET);
            let global = app.world().resource::<GlobalAmbientLight>();
            assert_eq!((global.color, global.brightness), (want, 77.0));

            // A game system animating intensity keeps running under a pack.
            app.world_mut()
                .get_mut::<PointLight>(e[0])
                .unwrap()
                .intensity = 1.0;
            app.update();
            assert_eq!(app.world().get::<PointLight>(e[0]).unwrap().intensity, 1.0);

            app.insert_resource(ActiveLook::default_for(&C));
            app.world_mut().write_message(LookChanged);
            app.update();
            assert_eq!(colours(&app, e), [LAMP; 4]);
            assert_eq!(app.world().resource::<GlobalAmbientLight>().color, LAMP);
        }

        #[test]
        fn a_light_spawned_later_under_a_pack_is_painted() {
            let mut app = app(pack());
            app.update();
            let e = spawn_lights(&mut app, LAMP);
            app.update();
            let want = app.world().resource::<ActiveLook>().colour("lamp");
            assert_eq!(colours(&app, e), [want; 4]);
        }
    }
}
