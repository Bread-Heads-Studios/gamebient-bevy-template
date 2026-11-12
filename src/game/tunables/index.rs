//! The ordered list of every tunable the game exposes, built from
//! reflection at registration time. **Order is registration order**, never
//! `TypeRegistry` iteration order (a hash map): the index position is the
//! `id` the replay and the checksum use, so it must be the same in every
//! build of the same source.

use core::any::TypeId;

use bevy::ecs::reflect::ReflectResource;
use bevy::prelude::*;
use bevy::reflect::{
    DynamicEnum, DynamicVariant, GetPath, GetTypeRegistration, ReflectRef, TypeInfo, Typed,
};

use super::attr::{Group, GroupKind, Kind, Tune};
use super::value::Value;

#[derive(Clone, Debug)]
pub struct Entry {
    pub id: u16,
    pub path: String,
    /// Index into `TunableIndex::groups`.
    pub group: usize,
    pub field: &'static str,
    pub type_id: TypeId,
    pub tune: Tune,
    /// Variant names for choice-like kinds; empty otherwise.
    pub options: Vec<String>,
    pub default: Value,
}

#[derive(Clone, Debug)]
pub struct GroupEntry {
    pub path: &'static str,
    pub label: &'static str,
    pub kind: GroupKind,
}

#[derive(Resource, Default, Clone, Debug)]
pub struct TunableIndex {
    pub entries: Vec<Entry>,
    pub groups: Vec<GroupEntry>,
}

impl TunableIndex {
    pub fn by_path(&self, path: &str) -> Option<&Entry> {
        self.entries.iter().find(|e| e.path == path)
    }
    pub fn by_id(&self, id: u16) -> Option<&Entry> {
        self.entries.get(usize::from(id))
    }
}

/// Registers `T` as a tunable resource: reflection registration, the
/// resource itself (`Default`), and one index entry per `Tune` field. Call
/// it from the game's plugin in a fixed order.
pub fn register<T: Resource + Reflect + Typed + GetTypeRegistration + Default>(app: &mut App) {
    app.register_type::<T>();
    app.init_resource::<T>();
    let TypeInfo::Struct(info) = T::type_info() else {
        panic!("tunable resources are structs: {}", T::type_path());
    };
    let group = info
        .get_attribute::<Group>()
        .unwrap_or_else(|| panic!("{} has no #[reflect(@Group…)]", T::type_path()))
        .clone();
    let world = app.world_mut();
    let group_ix = {
        let mut idx = world.resource_mut::<TunableIndex>();
        idx.groups.push(GroupEntry {
            path: group.path,
            label: group.label,
            kind: group.kind,
        });
        idx.groups.len() - 1
    };
    let mut pending = Vec::new();
    for field in info.iter() {
        let Some(tune) = field.get_attribute::<Tune>() else {
            continue;
        };
        pending.push((field.name(), tune.clone(), field.type_info()));
    }
    for (name, tune, field_info) in pending {
        let options = match (&tune.kind, field_info) {
            (Kind::Choice | Kind::Button | Kind::Sound, Some(TypeInfo::Enum(e))) => {
                e.variant_names().iter().map(|s| s.to_string()).collect()
            }
            _ => Vec::new(),
        };
        let id = u16::try_from(world.resource::<TunableIndex>().entries.len())
            .expect("fewer than 65536 tunables");
        let entry = Entry {
            id,
            path: format!("{}.{}", group.path, name),
            group: group_ix,
            field: name,
            type_id: TypeId::of::<T>(),
            tune,
            options,
            default: Value::Bool(false), // replaced below
        };
        let default = read(world, &entry)
            .unwrap_or_else(|| panic!("{}: field type does not match Tune kind", entry.path));
        world
            .resource_mut::<TunableIndex>()
            .entries
            .push(Entry { default, ..entry });
    }
}

fn with_field<R>(
    world: &World,
    entry: &Entry,
    f: impl FnOnce(&dyn PartialReflect) -> Option<R>,
) -> Option<R> {
    let registry = world.resource::<AppTypeRegistry>().clone();
    let registry = registry.read();
    let rr = registry.get(entry.type_id)?.data::<ReflectResource>()?;
    let res = rr.reflect(world).ok()?;
    let field = res.reflect_path(entry.field).ok()?;
    f(field)
}

/// The field's current value, or `None` when its Rust type does not fit
/// the attribute's kind.
pub fn read(world: &World, entry: &Entry) -> Option<Value> {
    with_field(world, entry, |field| {
        Some(match entry.tune.kind {
            Kind::Float => Value::Float(*field.try_downcast_ref::<f32>()?),
            Kind::Int => {
                if let Some(v) = field.try_downcast_ref::<u32>() {
                    Value::Int(i64::from(*v))
                } else {
                    Value::Int(i64::from(*field.try_downcast_ref::<i32>()?))
                }
            }
            Kind::Bool => Value::Bool(*field.try_downcast_ref::<bool>()?),
            Kind::Colour => Value::Colour(*field.try_downcast_ref::<bevy::color::Srgba>()?),
            Kind::Place => Value::Place(*field.try_downcast_ref::<Vec2>()?),
            Kind::Curve => Value::Curve(*field.try_downcast_ref::<[f32; 4]>()?),
            Kind::Choice | Kind::Button | Kind::Sound => {
                let ReflectRef::Enum(e) = field.reflect_ref() else {
                    return None;
                };
                Value::Choice(e.variant_name().to_string())
            }
        })
    })
}

/// Writes `value` into the field. Returns false, changing nothing, when the
/// kind does not match, an option is unknown, or a number is out of the
/// attribute's range.
pub fn write(world: &mut World, entry: &Entry, value: &Value) -> bool {
    if !value.fits(entry.tune.kind) {
        return false;
    }
    let (min, max) = (entry.tune.min, entry.tune.max);
    match value {
        Value::Float(f) if f64::from(*f) < min || f64::from(*f) > max => return false,
        Value::Int(i) if (*i as f64) < min || (*i as f64) > max => return false,
        Value::Choice(s) if !entry.options.iter().any(|o| o == s) => return false,
        _ => {}
    }
    let registry = world.resource::<AppTypeRegistry>().clone();
    let registry = registry.read();
    let Some(rr) = registry
        .get(entry.type_id)
        .and_then(|r| r.data::<ReflectResource>())
    else {
        return false;
    };
    let Ok(mut res) = rr.reflect_mut(world) else {
        return false;
    };
    let Ok(field) = res.reflect_path_mut(entry.field) else {
        return false;
    };
    match value {
        Value::Float(f) => set(field, *f),
        Value::Int(i) => {
            if let Some(slot) = field.try_downcast_mut::<u32>() {
                let Ok(v) = u32::try_from(*i) else {
                    return false;
                };
                *slot = v;
                true
            } else {
                let Ok(v) = i32::try_from(*i) else {
                    return false;
                };
                set(field, v)
            }
        }
        Value::Bool(b) => set(field, *b),
        Value::Colour(c) => set(field, *c),
        Value::Place(p) => set(field, *p),
        Value::Curve(c) => set(field, *c),
        Value::Choice(name) => field
            .try_apply(&DynamicEnum::new(name.clone(), DynamicVariant::Unit))
            .is_ok(),
    }
}

fn set<T: Reflect>(field: &mut dyn PartialReflect, v: T) -> bool {
    match field.try_downcast_mut::<T>() {
        Some(slot) => {
            *slot = v;
            true
        }
        None => false,
    }
}

/// Every tunable's current value, in index order.
pub fn snapshot(world: &World) -> Vec<(u16, Value)> {
    let idx = world.resource::<TunableIndex>().clone();
    idx.entries
        .iter()
        .filter_map(|e| read(world, e).map(|v| (e.id, v)))
        .collect()
}

/// The start values a run records: every tunable except cosmetic ones
/// (`Tune::cosmetic`), which must never reach a replay or its hash.
/// `snapshot` stays the full set (the editor lists cosmetic values too).
pub fn run_starts(world: &World) -> Vec<(u16, Value)> {
    let idx = world.resource::<TunableIndex>().clone();
    idx.entries
        .iter()
        .filter(|e| !e.tune.cosmetic)
        .filter_map(|e| read(world, e).map(|v| (e.id, v)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::tunables::attr::{Button, Group, Tune};

    #[derive(Resource, Reflect, Default)]
    #[reflect(Resource, @Group::counter("ammo", "Ammo"))]
    struct Ammo {
        #[reflect(@Tune::int("Starting ammo", 0, 50).next_run())]
        start: u32,
        #[reflect(@Tune::button("Fire button"))]
        fire: Button,
        /// Not a tunable: no attribute.
        #[allow(dead_code)]
        secret: f32,
    }

    #[derive(Resource, Reflect)]
    #[reflect(Resource, @Group::thing("bullet", "Bullet"))]
    struct Bullet {
        #[reflect(@Tune::float("Bullet speed", 100.0, 1200.0).unit("px/s"))]
        speed: f32,
        #[reflect(@Tune::colour("Colour"))]
        #[allow(dead_code)]
        colour: bevy::color::Srgba,
    }
    impl Default for Bullet {
        fn default() -> Self {
            Self {
                speed: 600.0,
                colour: bevy::color::Srgba::WHITE,
            }
        }
    }

    fn app() -> App {
        let mut app = App::new();
        app.init_resource::<TunableIndex>();
        register::<Ammo>(&mut app);
        register::<Bullet>(&mut app);
        app
    }

    #[test]
    fn index_is_in_registration_order_with_paths_and_options() {
        let app = app();
        let idx = app.world().resource::<TunableIndex>();
        let paths: Vec<&str> = idx.entries.iter().map(|e| e.path.as_str()).collect();
        assert_eq!(
            paths,
            ["ammo.start", "ammo.fire", "bullet.speed", "bullet.colour"]
        );
        assert_eq!(idx.entries[1].options, ["A", "B"]);
        assert_eq!(idx.entries[0].default, Value::Int(0));
        assert_eq!(idx.entries[1].default, Value::Choice("B".into()));
        assert_eq!(idx.entries[2].tune.unit, "px/s");
        assert_eq!(idx.groups.len(), 2);
        assert_eq!(idx.by_path("bullet.speed").unwrap().id, 2);
    }

    #[test]
    fn read_and_write_through_reflection() {
        let mut app = app();
        let idx = app.world().resource::<TunableIndex>().clone();
        let speed = idx.by_path("bullet.speed").unwrap();
        assert_eq!(read(app.world(), speed), Some(Value::Float(600.0)));
        assert!(write(app.world_mut(), speed, &Value::Float(900.0)));
        assert_eq!(app.world().resource::<Bullet>().speed, 900.0);
        let fire = idx.by_path("ammo.fire").unwrap();
        assert!(write(app.world_mut(), fire, &Value::Choice("A".into())));
        assert_eq!(app.world().resource::<Ammo>().fire, Button::A);
        assert!(
            !write(app.world_mut(), fire, &Value::Choice("C".into())),
            "unknown option"
        );
        assert!(!write(app.world_mut(), speed, &Value::Int(1)), "wrong kind");
        assert!(
            !write(app.world_mut(), speed, &Value::Float(5000.0)),
            "out of range"
        );
        assert_eq!(
            app.world().resource::<Bullet>().speed,
            900.0,
            "untouched after a bad write"
        );
        let snap = snapshot(app.world());
        assert_eq!(snap[2], (2, Value::Float(900.0)));
        assert_eq!(snap.len(), 4);
    }
}
