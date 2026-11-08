//! Content hash: FNV-1a over the tune log, so a replay names which game it
//! was played on. Reuses `sim::Checksum`'s fold so the two agree bit for bit.

use crate::game::sim::Checksum;

use super::apply::TuneLog;
use super::value::Value;

/// Folds `(id, value)` as the checksum sees it: the id, the byte length,
/// then the bytes in 8-byte little-endian chunks (zero padded).
pub fn fold_value(sum: &mut Checksum, id: u16, value: &Value) {
    let bytes = value.to_bytes();
    sum.fold(u64::from(id));
    sum.fold(bytes.len() as u64);
    for chunk in bytes.chunks(8) {
        let mut word = [0u8; 8];
        word[..chunk.len()].copy_from_slice(chunk);
        sum.fold(u64::from_le_bytes(word));
    }
}

/// Hash of the start values and every mid-run change, in order.
pub fn content(log: &TuneLog) -> u64 {
    let mut sum = Checksum::default();
    for (id, v) in &log.starts {
        fold_value(&mut sum, *id, v);
    }
    for c in &log.changes {
        sum.fold(u64::from(c.tick));
        fold_value(&mut sum, c.id, &c.value);
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
}
