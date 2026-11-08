//! The reflection attributes that make a resource field a tunable:
//! `#[reflect(@Tune::float("Bullet speed", 100.0, 1200.0).unit("px/s"))]`
//! on a field, `#[reflect(@Group::thing("bullet", "Bullet"))]` on the
//! resource. Opaque reflect values (stored by `TypeId`, read back with
//! `get_attribute::<Tune>()`); nothing here is sim state.

use bevy::prelude::*;

/// What kind of control a field takes and how its value is encoded.
#[derive(Reflect, Clone, Copy, Debug, PartialEq, Eq)]
#[reflect(opaque)]
pub enum Kind {
    Float,
    Int,
    Bool,
    Colour,
    Choice,
    Button,
    Sound,
    Place,
    Curve,
}

impl Kind {
    /// The `type` string the host protocol reports.
    pub fn name(self) -> &'static str {
        match self {
            Kind::Float => "f32",
            Kind::Int => "i32",
            Kind::Bool => "bool",
            Kind::Colour => "colour",
            Kind::Choice => "choice",
            Kind::Button => "button",
            Kind::Sound => "sound",
            Kind::Place => "place",
            Kind::Curve => "curve",
        }
    }
}

/// When a change takes effect: at the next sim tick, or only when the next
/// run starts (a starting count, read on `OnEnter(Playing)`).
#[derive(Reflect, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[reflect(opaque)]
pub enum Applies {
    #[default]
    Tick,
    NextRun,
}

/// Field attribute. Ranges are hard limits a card may narrow, never widen.
#[derive(Reflect, Clone, Debug, PartialEq)]
#[reflect(opaque)]
pub struct Tune {
    pub label: &'static str,
    pub kind: Kind,
    pub min: f64,
    pub max: f64,
    pub step: f64,
    pub unit: &'static str,
    pub applies: Applies,
}

impl Tune {
    const fn new(label: &'static str, kind: Kind, min: f64, max: f64, step: f64) -> Self {
        Self {
            label,
            kind,
            min,
            max,
            step,
            unit: "",
            applies: Applies::Tick,
        }
    }
    pub const fn float(label: &'static str, min: f64, max: f64) -> Self {
        Self::new(label, Kind::Float, min, max, 1.0)
    }
    pub const fn int(label: &'static str, min: i64, max: i64) -> Self {
        Self::new(label, Kind::Int, min as f64, max as f64, 1.0)
    }
    pub const fn toggle(label: &'static str) -> Self {
        Self::new(label, Kind::Bool, 0.0, 1.0, 1.0)
    }
    pub const fn colour(label: &'static str) -> Self {
        Self::new(label, Kind::Colour, 0.0, 1.0, 0.0)
    }
    pub const fn choice(label: &'static str) -> Self {
        Self::new(label, Kind::Choice, 0.0, 0.0, 0.0)
    }
    pub const fn button(label: &'static str) -> Self {
        Self::new(label, Kind::Button, 0.0, 0.0, 0.0)
    }
    pub const fn sound(label: &'static str) -> Self {
        Self::new(label, Kind::Sound, 0.0, 0.0, 0.0)
    }
    /// `min`/`max` bound both axes (game pixels, origin at the centre).
    pub const fn place(label: &'static str, min: f64, max: f64) -> Self {
        Self::new(label, Kind::Place, min, max, 1.0)
    }
    /// Four y-values in `min..=max` at x = 0, 1/3, 2/3, 1.
    pub const fn curve(label: &'static str, min: f64, max: f64) -> Self {
        Self::new(label, Kind::Curve, min, max, 0.01)
    }
    pub const fn step(mut self, step: f64) -> Self {
        self.step = step;
        self
    }
    pub const fn unit(mut self, unit: &'static str) -> Self {
        self.unit = unit;
        self
    }
    pub const fn next_run(mut self) -> Self {
        self.applies = Applies::NextRun;
        self
    }
}

/// What a group of tunables describes, for the inspector's tabs.
#[derive(Reflect, Clone, Copy, Debug, PartialEq, Eq)]
#[reflect(opaque)]
pub enum GroupKind {
    Thing,
    Counter,
    Rule,
}

impl GroupKind {
    pub fn name(self) -> &'static str {
        match self {
            GroupKind::Thing => "thing",
            GroupKind::Counter => "counter",
            GroupKind::Rule => "rule",
        }
    }
}

/// Resource attribute: the group every field of the resource belongs to.
/// `path` is the prefix of each field's tunable path (`ammo` → `ammo.start`).
#[derive(Reflect, Clone, Debug, PartialEq)]
#[reflect(opaque)]
pub struct Group {
    pub path: &'static str,
    pub label: &'static str,
    pub kind: GroupKind,
}

impl Group {
    pub const fn thing(path: &'static str, label: &'static str) -> Self {
        Self {
            path,
            label,
            kind: GroupKind::Thing,
        }
    }
    pub const fn counter(path: &'static str, label: &'static str) -> Self {
        Self {
            path,
            label,
            kind: GroupKind::Counter,
        }
    }
    pub const fn rule(path: &'static str, label: &'static str) -> Self {
        Self {
            path,
            label,
            kind: GroupKind::Rule,
        }
    }
}

/// The canon action buttons, as a tunable value (`Kind::Button`).
#[derive(Reflect, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Button {
    A,
    #[default]
    B,
}

impl Button {
    /// Whether this button was just pressed on this tick.
    pub fn just_pressed(self, input: &gamebient_input::GameInput) -> bool {
        match self {
            Button::A => input.primary_just_pressed,
            Button::B => input.secondary_just_pressed,
        }
    }
}
