//! Kinds of Thing: the named building blocks a level places. A game
//! registers each kind once, in a fixed order, with
//! [`KitAppExt::thing_kind`]; the level file names kinds by that name.
//!
//! A kind's index (its registration order) is what [`Thing`] carries, so
//! never reorder registrations between releases, exactly like tunables.

use bevy::ecs::system::EntityCommands;
use bevy::prelude::*;

/// What every Thing of one kind starts with.
#[derive(Clone)]
pub struct KindProps {
    /// Default size in world units, used when a placed thing gives none.
    pub size: Vec2,
    /// Placeholder colour of the kind's cuboid (presentation only).
    pub colour: Color,
    /// Game-specific components for a kind (markers, bodies). Runs inside the spawn, in the sim.
    pub on_spawn: Option<fn(&mut EntityCommands)>,
    /// What the level editor may do with this kind. `KindProps::size` is
    /// already the size a new Thing gets.
    pub edit: EditRules,
}

/// What the level editor may do with Things of one kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EditRules {
    /// The thing can be resized in the editor; its `size` is then written.
    pub resizable: bool,
    /// The most of this kind a level may hold (None = no limit).
    pub max: Option<u32>,
    /// The editor may add this kind to a level. False for kinds spawned
    /// only at runtime (a bullet): the catalog leaves them out, so the
    /// palette never offers them.
    pub placeable: bool,
}

impl Default for EditRules {
    fn default() -> Self {
        Self {
            resizable: false,
            max: None,
            placeable: true,
        }
    }
}

/// The level editor's grid step, in world units. A game whose world units
/// are not pixels inserts its own before `ContentPlugin` (the template's
/// units are about one player wide, so it uses 1.0).
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct LevelGrid(pub f32);

impl Default for LevelGrid {
    fn default() -> Self {
        Self(20.0)
    }
}

/// The game's kinds, in registration order.
#[derive(Resource, Default)]
pub struct Kinds(pub Vec<(String, KindProps)>);

impl Kinds {
    /// The index [`Thing::kind`] carries for `name`.
    pub fn index(&self, name: &str) -> Option<u16> {
        self.0
            .iter()
            .position(|(n, _)| n == name)
            .and_then(|i| u16::try_from(i).ok())
    }

    pub fn props(&self, kind: u16) -> Option<&KindProps> {
        self.0.get(usize::from(kind)).map(|(_, p)| p)
    }
}

/// A placed thing: its kind's index in [`Kinds`].
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Thing {
    pub kind: u16,
}

/// The id the level file gave this thing.
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub struct ThingId(pub String);

/// The thing's size in world units (the level's `size`, or its kind's).
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Size(pub Vec2);

pub trait KitAppExt {
    /// Registers a kind. Call in a fixed order; a name may be registered once.
    fn thing_kind(&mut self, name: &str, props: KindProps) -> &mut Self;
}

impl KitAppExt for App {
    fn thing_kind(&mut self, name: &str, props: KindProps) -> &mut Self {
        self.init_resource::<Kinds>();
        let mut kinds = self.world_mut().resource_mut::<Kinds>();
        assert!(
            kinds.index(name).is_none(),
            "the kind {name:?} is registered twice"
        );
        kinds.0.push((name.to_string(), props));
        self
    }
}

/// One mesh and one material per kind, made once (presentation). Inserted
/// by a `Startup` system only when both asset stores exist, so the headless
/// verifier never has it.
#[derive(Resource)]
pub struct ThingArt {
    pub mesh: Handle<Mesh>,
    /// Indexed by kind.
    pub materials: Vec<Handle<StandardMaterial>>,
}

/// `Startup`: builds [`ThingArt`] when the windowed asset stores exist.
pub fn setup_thing_art(
    mut commands: Commands,
    kinds: Res<Kinds>,
    meshes: Option<ResMut<Assets<Mesh>>>,
    materials: Option<ResMut<Assets<StandardMaterial>>>,
) {
    let (Some(mut meshes), Some(mut materials)) = (meshes, materials) else {
        return;
    };
    let mesh = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let materials = kinds
        .0
        .iter()
        .map(|(_, p)| {
            materials.add(StandardMaterial {
                base_color: p.colour,
                ..default()
            })
        })
        .collect();
    commands.insert_resource(ThingArt { mesh, materials });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edit_rules_default_to_fixed_size_no_limit_and_placeable() {
        assert_eq!(
            EditRules::default(),
            EditRules {
                resizable: false,
                max: None,
                placeable: true,
            }
        );
    }

    #[test]
    fn the_level_grid_defaults_to_twenty_units() {
        assert_eq!(LevelGrid::default(), LevelGrid(20.0));
    }
}
