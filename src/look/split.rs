//! Which built-in packs a web build embeds. Shared verbatim with `build.rs`
//! (`#[path]` include), so this file uses `std` only.
//!
//! On wasm, a pack whose files total at most `WEB_EMBED_MAX_BYTES` is built
//! into the bundle; a larger one is left out and listed in
//! `pack::FETCHABLE`, fetched from the game's own site when `?look=` asks
//! for it. Native builds embed every pack.

/// Packs up to this size (all files, `look.json` included) stay in the web
/// bundle. A palette-and-strings pack is a few KiB; audio or art is not.
pub const WEB_EMBED_MAX_BYTES: u64 = 64 * 1024;

/// Whether a built-in pack of `total_bytes` is embedded in a web build.
pub fn web_embeds(total_bytes: u64) -> bool {
    total_bytes <= WEB_EMBED_MAX_BYTES
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_packs_embed_and_large_ones_are_fetched() {
        assert!(web_embeds(0));
        assert!(web_embeds(WEB_EMBED_MAX_BYTES - 1));
        assert!(web_embeds(WEB_EMBED_MAX_BYTES));
        assert!(!web_embeds(WEB_EMBED_MAX_BYTES + 1));
        assert!(!web_embeds(1_650_000));
    }
}
