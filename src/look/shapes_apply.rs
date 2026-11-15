//! Pack shapes onto the screen: a game registers the mesh handles a shape
//! slot dresses (`ShapeTargets::register`), and `apply_shapes` rewrites the
//! mesh **asset** behind each handle. Entities and components are never
//! touched: whatever already draws that handle draws the new shape.
//!
//! On first sight of a target its current mesh is cloned as the baseline.
//! Then, and on every `LookChanged`, the target is applied: the active
//! pack's `Slot::Shape` for its slot is built (`shape::build_mesh`) into
//! the asset, or, with no shape for that slot, the baseline is written back
//! if the asset differs. Under the default look nothing is written, so
//! every mesh stays bit-identical.
//!
//! A target registered before its asset exists waits for it (retried each
//! frame) and stays listed if it never arrives. Targets must keep
//! `RenderAssetUsages::MAIN_WORLD` (the `Mesh` default): the baseline is
//! cloned from main-world data, which a render-world-only mesh does not have.
//!
//! Targets hold `AssetId`s, not strong handles, so registering never keeps
//! a mesh alive; a baselined target whose asset is gone is dropped. Headless
//! apps (the replay verifier) have no `Assets<Mesh>` and skip this.

use bevy::platform::collections::HashMap;
use bevy::prelude::*;

use super::apply::LookChanged;
use super::manifest::Slot;
use super::runtime::ActiveLook;
use super::shape::build_mesh;

/// Mesh handles that pack shapes replace, by slot name.
#[derive(Resource, Default, Debug)]
pub struct ShapeTargets {
    targets: Vec<(&'static str, AssetId<Mesh>)>,
    baselines: HashMap<AssetId<Mesh>, Mesh>,
}

impl ShapeTargets {
    /// Dresses `mesh` from slot `slot`. Registering a handle again is a
    /// no-op. Valid at any time, including for meshes spawned after startup
    /// (there is no separate register_spawned). Register from presentation
    /// code only, never from the sim.
    pub fn register(&mut self, slot: &'static str, mesh: Handle<Mesh>) {
        let id = mesh.id();
        if !self.targets.iter().any(|(_, t)| *t == id) {
            self.targets.push((slot, id));
        }
    }

    pub fn len(&self) -> usize {
        self.targets.len()
    }

    pub fn is_empty(&self) -> bool {
        self.targets.is_empty()
    }
}

fn apply_one(look: &ActiveLook, slot: &str, baseline: &Mesh, mesh: &mut Mesh) {
    match look.slot(slot) {
        Some(Slot::Shape(s)) => *mesh = build_mesh(&s.shape),
        _ => {
            if mesh != baseline {
                *mesh = baseline.clone();
            }
        }
    }
}

/// See the module docs. In `PostUpdate` (`LookSet`), before asset events,
/// so a changed mesh's bounds are recomputed on the same frame.
pub fn apply_shapes(
    look: Res<ActiveLook>,
    mut changed: MessageReader<LookChanged>,
    mut targets: ResMut<ShapeTargets>,
    meshes: Option<ResMut<Assets<Mesh>>>,
) {
    let all = changed.read().count() > 0;
    let Some(mut meshes) = meshes else {
        return;
    };
    if targets.targets.is_empty() {
        return;
    }
    let ShapeTargets { targets, baselines } = &mut *targets;
    // A baselined target whose asset is gone was removed: forget it.
    targets.retain(|(_, id)| !baselines.contains_key(id) || meshes.contains(*id));
    baselines.retain(|id, _| meshes.contains(*id));
    for (slot, id) in targets.iter() {
        let new = !baselines.contains_key(id);
        if !(new || all) {
            continue;
        }
        if new {
            // Not loaded yet: try again next frame.
            let Some(current) = meshes.get(*id) else {
                continue;
            };
            baselines.insert(*id, current.clone());
        }
        let baseline = &baselines[id];
        // Read first: `get_mut` marks the asset modified, which the default
        // look must not do.
        let wants_write = match look.slot(slot) {
            Some(Slot::Shape(_)) => true,
            _ => meshes.get(*id).is_some_and(|m| m != baseline),
        };
        if wants_write && let Some(mesh) = meshes.get_mut(*id) {
            apply_one(&look, slot, baseline, mesh);
        }
    }
}
