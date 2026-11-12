//! Where pack bytes come from: built into the binary at compile time
//! (`build.rs` embeds `assets/looks/*/`), or, natively, a folder on disk.

#[cfg(not(target_arch = "wasm32"))]
use std::path::Path;

use super::validate::Files;
#[cfg(not(target_arch = "wasm32"))]
use super::validate::limits;

mod generated {
    include!(concat!(env!("OUT_DIR"), "/looks_embedded.rs"));
}

pub fn embedded_ids() -> impl Iterator<Item = &'static str> {
    generated::EMBEDDED.iter().map(|(id, _)| *id)
}

pub fn embedded_files(id: &str) -> Option<Files> {
    generated::EMBEDDED
        .iter()
        .find(|(i, _)| *i == id)
        .map(|(_, files)| {
            files
                .iter()
                .map(|(name, bytes)| (name.to_string(), bytes.to_vec()))
                .collect()
        })
}

/// Reads a flat pack folder. Refuses subfolders and symlinks (an entry's
/// metadata does not follow links), and stops early on the file-count and
/// total-size limits so a huge folder is never read into memory.
#[cfg(not(target_arch = "wasm32"))]
pub fn read_dir(path: &Path) -> Result<Files, String> {
    let rd = std::fs::read_dir(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let mut files = Files::new();
    let mut total = 0usize;
    for entry in rd {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || name == "README.md" {
            continue;
        }
        let meta = entry.metadata().map_err(|e| format!("{name}: {e}"))?;
        if !meta.is_file() {
            return Err(format!(
                "{name}: packs are flat folders of files (no folders, no links)"
            ));
        }
        if files.len() >= limits::FILES_MAX {
            return Err(format!("more than {} files", limits::FILES_MAX));
        }
        total += meta.len() as usize;
        if total > limits::PACK_MAX {
            return Err(format!("larger than {} bytes", limits::PACK_MAX));
        }
        let bytes = std::fs::read(entry.path()).map_err(|e| format!("{name}: {e}"))?;
        files.insert(name, bytes);
    }
    Ok(files)
}

/// Changes whenever a file in the folder is added, removed, resized or
/// rewritten (name, length and modification time of every entry).
#[cfg(not(target_arch = "wasm32"))]
pub fn dir_fingerprint(path: &Path) -> u64 {
    let mut h = blake3::Hasher::new();
    if let Ok(rd) = std::fs::read_dir(path) {
        let mut entries: Vec<_> = rd.filter_map(Result::ok).collect();
        entries.sort_by_key(|e| e.file_name());
        for e in entries {
            let Ok(meta) = e.metadata() else { continue };
            h.update(e.file_name().to_string_lossy().as_bytes());
            h.update(&meta.len().to_le_bytes());
            if let Some(d) = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            {
                h.update(&d.as_nanos().to_le_bytes());
            }
        }
    }
    u64::from_le_bytes(h.finalize().as_bytes()[..8].try_into().unwrap())
}
