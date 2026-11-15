//! The naming rules for pack ids and pack file names. Shared verbatim with
//! the build-time generator (`embed.rs`, `#[path]` include), so built-in
//! packs are held to the validator's rules before they are embedded or
//! listed as fetchable; this file uses `std` only.

/// The manifest's file name, the one file in a pack that is not a `.png`
/// or `.ogg`. Mirrored as `manifest::MANIFEST_FILE`.
pub const MANIFEST_FILE: &str = "look.json";
pub const ID_MAX: usize = 32;
pub const FILE_STEM_MAX: usize = 48;

/// `[a-z0-9-]{1,32}`.
pub fn is_id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= ID_MAX
        && s.bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
}

/// `[a-z0-9_-]{1,48}.png|.ogg`: no dots in the stem, so no `..`, no
/// hidden files, no second extension.
pub fn is_file_name(s: &str) -> bool {
    let Some((stem, ext)) = s.rsplit_once('.') else {
        return false;
    };
    matches!(ext, "png" | "ogg")
        && !stem.is_empty()
        && stem.len() <= FILE_STEM_MAX
        && stem
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'_' | b'-'))
}
