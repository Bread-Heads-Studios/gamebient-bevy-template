//! Content hash: FNV-1a over the level hash and the tune log, so a replay
//! names which game (and level) it was played on. Reuses `sim::Checksum`'s fold so the two agree bit for bit.

use crate::game::sim::Checksum;

use super::apply::TuneLog;
use super::value::Value;

/// Folds `(id, value)` as the checksum sees it: the id, the byte length,
/// then the bytes in 8-byte little-endian chunks (zero padded).
pub fn fold_value(sum: &mut Checksum, id: u16, value: &Value) {
    fold_bytes(sum, id, &value.to_bytes());
}

/// [`fold_value`] over a value's encoded bytes (`Value::to_bytes`).
pub fn fold_bytes(sum: &mut Checksum, id: u16, bytes: &[u8]) {
    sum.fold(u64::from(id));
    sum.fold(bytes.len() as u64);
    for chunk in bytes.chunks(8) {
        let mut word = [0u8; 8];
        word[..chunk.len()].copy_from_slice(chunk);
        sum.fold(u64::from_le_bytes(word));
    }
}

/// Hash of the level, the start values and every mid-run change, in order.
pub fn content(log: &TuneLog) -> u64 {
    let starts: Vec<(u16, Vec<u8>)> = log
        .starts
        .iter()
        .map(|(id, v)| (*id, v.to_bytes()))
        .collect();
    let changes: Vec<(u32, u16, Vec<u8>)> = log
        .changes
        .iter()
        .map(|c| (c.tick, c.id, c.value.to_bytes()))
        .collect();
    content_of_bytes(
        log.level,
        starts.iter().map(|(id, b)| (*id, b.as_slice())),
        changes.iter().map(|(t, id, b)| (*t, *id, b.as_slice())),
    )
}

/// [`content`] over encoded values, as a replay carries them: `starts` as
/// `(id, bytes)`, `changes` as `(tick, id, bytes)`. The verifier uses it to
/// tell which level a replay names before re-simulating it.
pub fn content_of_bytes<'a>(
    level: u64,
    starts: impl IntoIterator<Item = (u16, &'a [u8])>,
    changes: impl IntoIterator<Item = (u32, u16, &'a [u8])>,
) -> u64 {
    let mut sum = Checksum::default();
    sum.fold(level);
    for (id, b) in starts {
        fold_bytes(&mut sum, id, b);
    }
    for (tick, id, b) in changes {
        sum.fold(u64::from(tick));
        fold_bytes(&mut sum, id, b);
    }
    sum.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::tunables::apply::TuneChange;

    #[test]
    fn content_depends_on_values_order_and_changes() {
        let a = TuneLog {
            level: 0,
            starts: vec![(0, Value::Int(1)), (1, Value::Float(2.0))],
            changes: vec![],
        };
        let mut b = a.clone();
        b.starts[0].1 = Value::Int(2);
        assert_ne!(content(&a), content(&b));
        let mut c = a.clone();
        c.changes.push(TuneChange {
            tick: 7,
            id: 0,
            value: Value::Int(1),
        });
        assert_ne!(
            content(&a),
            content(&c),
            "a change to the same value still counts"
        );
        assert_eq!(content(&a), content(&a.clone()));
    }

    #[test]
    fn content_depends_on_the_level_alone() {
        let a = TuneLog {
            level: 1,
            starts: vec![(0, Value::Int(1))],
            changes: vec![],
        };
        let mut b = a.clone();
        b.level = 2;
        assert_ne!(content(&a), content(&b));
    }

    #[test]
    fn content_of_bytes_agrees_with_content() {
        let log = TuneLog {
            level: 9,
            starts: vec![(0, Value::Int(1)), (1, Value::Float(2.5))],
            changes: vec![TuneChange {
                tick: 3,
                id: 1,
                value: Value::Float(4.0),
            }],
        };
        let s: Vec<(u16, Vec<u8>)> = log.starts.iter().map(|(i, v)| (*i, v.to_bytes())).collect();
        let c = [(3u32, 1u16, Value::Float(4.0).to_bytes())];
        assert_eq!(
            content(&log),
            content_of_bytes(
                9,
                s.iter().map(|(i, b)| (*i, b.as_slice())),
                c.iter().map(|(t, i, b)| (*t, *i, b.as_slice()))
            )
        );
    }
}
