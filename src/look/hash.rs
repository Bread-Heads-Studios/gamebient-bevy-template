//! A pack's content hash. Shared verbatim with the build-time generator
//! (`embed.rs`, `#[path]` include), which hashes web-fetchable packs, so this file
//! uses `std` and `blake3` only. Re-exported as `validate::content_hash`.

use std::collections::BTreeMap;

/// BLAKE3 over every file, in name order: name length, name, byte length,
/// bytes. Domain-separated so it can never collide with another use.
pub fn content_hash(files: &BTreeMap<String, Vec<u8>>) -> String {
    let mut h = blake3::Hasher::new();
    h.update(b"gx-look-v1\0");
    for (name, bytes) in files {
        h.update(&(name.len() as u64).to_le_bytes());
        h.update(name.as_bytes());
        h.update(&(bytes.len() as u64).to_le_bytes());
        h.update(bytes);
    }
    h.finalize().to_hex().to_string()
}
