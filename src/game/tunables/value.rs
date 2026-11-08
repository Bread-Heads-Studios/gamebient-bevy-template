//! A tunable's value: the closed set of types a control can bind to, with
//! a bit-exact byte encoding (what the checksum and the replay carry) and a
//! JSON form (what the host protocol carries).

use bevy::color::Srgba;
use bevy::prelude::*;
use serde_json::{Value as Json, json};

use super::attr::Kind;

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Float(f32),
    Int(i64),
    Bool(bool),
    Colour(Srgba),
    Place(Vec2),
    /// A unit variant's name (choice, button, sound).
    Choice(String),
    Curve([f32; 4]),
}

impl Value {
    pub fn kind(&self) -> Kind {
        match self {
            Value::Float(_) => Kind::Float,
            Value::Int(_) => Kind::Int,
            Value::Bool(_) => Kind::Bool,
            Value::Colour(_) => Kind::Colour,
            Value::Place(_) => Kind::Place,
            Value::Choice(_) => Kind::Choice,
            Value::Curve(_) => Kind::Curve,
        }
    }

    /// Whether this value can be written to a field of `kind`. Choice-like
    /// kinds (choice, button, sound) all carry a `Choice`.
    pub fn fits(&self, kind: Kind) -> bool {
        match self {
            Value::Choice(_) => matches!(kind, Kind::Choice | Kind::Button | Kind::Sound),
            other => other.kind() == kind,
        }
    }

    /// Little-endian, bit-exact. Floats via `to_bits`, so two values that
    /// print alike but differ in the last bit encode differently.
    pub fn to_bytes(&self) -> Vec<u8> {
        match self {
            Value::Float(f) => f.to_bits().to_le_bytes().to_vec(),
            Value::Int(i) => i.to_le_bytes().to_vec(),
            Value::Bool(b) => vec![u8::from(*b)],
            Value::Colour(c) => [c.red, c.green, c.blue, c.alpha]
                .iter()
                .flat_map(|f| f.to_bits().to_le_bytes())
                .collect(),
            Value::Place(p) => [p.x, p.y]
                .iter()
                .flat_map(|f| f.to_bits().to_le_bytes())
                .collect(),
            Value::Choice(s) => s.as_bytes().to_vec(),
            Value::Curve(c) => c.iter().flat_map(|f| f.to_bits().to_le_bytes()).collect(),
        }
    }

    pub fn from_bytes(kind: Kind, b: &[u8]) -> Option<Value> {
        fn f32s<const N: usize>(b: &[u8]) -> Option<[f32; N]> {
            if b.len() != N * 4 {
                return None;
            }
            let mut out = [0f32; N];
            // Indexed rather than `chunks_exact(4)`: newer clippy rejects a
            // constant chunk size in favour of `as_chunks`, which is not
            // stable on every toolchain the fleet builds with.
            for (i, slot) in out.iter_mut().enumerate() {
                let word: [u8; 4] = b[i * 4..i * 4 + 4].try_into().ok()?;
                *slot = f32::from_bits(u32::from_le_bytes(word));
            }
            Some(out)
        }
        Some(match kind {
            Kind::Float => Value::Float(f32s::<1>(b)?[0]),
            Kind::Int => Value::Int(i64::from_le_bytes(b.try_into().ok()?)),
            Kind::Bool => Value::Bool(*b.first()? != 0),
            Kind::Colour => {
                let [r, g, bl, a] = f32s::<4>(b)?;
                Value::Colour(Srgba::new(r, g, bl, a))
            }
            Kind::Place => {
                let [x, y] = f32s::<2>(b)?;
                Value::Place(Vec2::new(x, y))
            }
            Kind::Choice | Kind::Button | Kind::Sound => {
                Value::Choice(core::str::from_utf8(b).ok()?.to_string())
            }
            Kind::Curve => Value::Curve(f32s::<4>(b)?),
        })
    }

    pub fn to_json(&self) -> Json {
        match self {
            Value::Float(f) => json!(f),
            Value::Int(i) => json!(i),
            Value::Bool(b) => json!(b),
            Value::Colour(c) => json!(hex(c)),
            Value::Place(p) => json!([p.x, p.y]),
            Value::Choice(s) => json!(s),
            Value::Curve(c) => json!(c),
        }
    }

    pub fn from_json(kind: Kind, j: &Json) -> Option<Value> {
        Some(match kind {
            Kind::Float => Value::Float(j.as_f64()? as f32),
            Kind::Int => Value::Int(j.as_i64()?),
            Kind::Bool => Value::Bool(j.as_bool()?),
            Kind::Colour => Value::Colour(from_hex(j.as_str()?)?),
            Kind::Place => {
                let a = j.as_array()?;
                if a.len() != 2 {
                    return None;
                }
                Value::Place(Vec2::new(a[0].as_f64()? as f32, a[1].as_f64()? as f32))
            }
            Kind::Choice | Kind::Button | Kind::Sound => Value::Choice(j.as_str()?.to_string()),
            Kind::Curve => {
                let a = j.as_array()?;
                if a.len() != 4 {
                    return None;
                }
                let mut c = [0f32; 4];
                for (i, v) in a.iter().enumerate() {
                    c[i] = v.as_f64()? as f32;
                }
                Value::Curve(c)
            }
        })
    }
}

/// `#rrggbb` (alpha is always 1 for a tunable colour).
fn hex(c: &Srgba) -> String {
    let q = |f: f32| (f.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("#{:02x}{:02x}{:02x}", q(c.red), q(c.green), q(c.blue))
}

fn from_hex(s: &str) -> Option<Srgba> {
    let s = s.strip_prefix('#')?;
    if s.len() != 6 {
        return None;
    }
    let p = |i: usize| {
        u8::from_str_radix(&s[i..i + 2], 16)
            .ok()
            .map(|v| f32::from(v) / 255.0)
    };
    Some(Srgba::new(p(0)?, p(2)?, p(4)?, 1.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytes_round_trip_every_kind() {
        let all = [
            Value::Float(600.5),
            Value::Int(-3),
            Value::Bool(true),
            Value::Colour(Srgba::new(1.0, 0.85, 0.2, 1.0)),
            Value::Place(Vec2::new(-12.5, 300.0)),
            Value::Choice("B".into()),
            Value::Curve([0.0, 0.25, 0.75, 1.0]),
        ];
        for v in all {
            let b = v.to_bytes();
            assert_eq!(Value::from_bytes(v.kind(), &b), Some(v.clone()), "{v:?}");
        }
        assert_eq!(Value::from_bytes(Kind::Float, &[1, 2]), None);
        assert_eq!(
            Value::from_bytes(Kind::Button, b"A"),
            Some(Value::Choice("A".into()))
        );
    }

    #[test]
    fn json_round_trip_and_rejects_wrong_shape() {
        let v = Value::Colour(Srgba::new(1.0, 0.0, f32::from(0x80u8) / 255.0, 1.0));
        assert_eq!(v.to_json(), serde_json::json!("#ff0080"));
        assert_eq!(Value::from_json(Kind::Colour, &v.to_json()), Some(v));
        assert_eq!(
            Value::from_json(Kind::Int, &serde_json::json!(7)),
            Some(Value::Int(7))
        );
        assert_eq!(Value::from_json(Kind::Int, &serde_json::json!(7.5)), None);
        assert_eq!(
            Value::from_json(Kind::Place, &serde_json::json!([1, 2])),
            Some(Value::Place(Vec2::new(1.0, 2.0)))
        );
        assert_eq!(
            Value::from_json(Kind::Bool, &serde_json::json!("yes")),
            None
        );
        assert_eq!(
            Value::from_json(Kind::Sound, &serde_json::json!("pickup")),
            Some(Value::Choice("pickup".into()))
        );
    }

    #[test]
    fn float_bytes_are_bit_exact() {
        // Adjacent floats print the same to six places; the bytes differ.
        let a = Value::Float(0.3).to_bytes();
        let b = Value::Float(f32::from_bits(0.3f32.to_bits() + 1)).to_bytes();
        assert_ne!(a, b, "the checksum must see the last bit");
        assert_eq!(
            Value::from_bytes(Kind::Float, &b),
            Some(Value::Float(f32::from_bits(0.3f32.to_bits() + 1)))
        );
    }

    #[test]
    fn choice_fits_every_choice_like_kind() {
        let c = Value::Choice("A".into());
        assert!(c.fits(Kind::Button) && c.fits(Kind::Sound) && c.fits(Kind::Choice));
        assert!(!c.fits(Kind::Int));
        assert!(Value::Int(1).fits(Kind::Int) && !Value::Int(1).fits(Kind::Float));
    }
}
