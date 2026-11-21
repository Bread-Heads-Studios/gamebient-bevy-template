//! The pack this run is wearing, and the queries game code asks of it.
//! Every query falls back to the game's own look (the contract defaults),
//! so code can ask unconditionally.

use bevy::prelude::*;

use super::contract::Contract;
use super::manifest::{Manifest, Param, Slot};
use super::pack;
use super::select::Selection;
use super::validate::{Files, Report, Severity, parse_colour, validate};

/// Drawn for a palette role the contract does not declare: a programming
/// error, made loud on screen.
pub const MISSING: Color = Color::srgb(1.0, 0.0, 1.0);

#[derive(Resource, Debug)]
pub struct ActiveLook {
    pub contract: &'static Contract,
    pub manifest: Option<Manifest>,
    pub files: Files,
    pub content_hash: Option<String>,
    pub source: Selection,
    colours: Vec<(String, Color)>,
}

pub fn log_report(report: &Report) {
    for p in &report.problems {
        match p.severity {
            Severity::Error => error!("look pack: {}: {}", p.at, p.message),
            Severity::Warning => warn!("look pack: {}: {}", p.at, p.message),
        }
    }
}

impl ActiveLook {
    pub fn default_for(contract: &'static Contract) -> Self {
        Self {
            contract,
            manifest: None,
            files: Files::new(),
            content_hash: None,
            source: Selection::Default,
            colours: Vec::new(),
        }
    }

    // Report is the rejection's full diagnosis; this runs once at startup.
    #[allow(clippy::result_large_err)]
    /// Validates `files`; an invalid pack is returned as its report.
    pub fn from_files(
        contract: &'static Contract,
        files: Files,
        source: Selection,
        expected_id: Option<&str>,
    ) -> Result<Self, Report> {
        let report = validate(&files, contract, expected_id);
        if !report.ok() {
            return Err(report);
        }
        log_report(&report); // warnings only at this point
        let Report {
            manifest,
            content_hash,
            ..
        } = report;
        let manifest = manifest.expect("ok() implies a manifest");
        let colours = manifest
            .palette
            .iter()
            .filter_map(|(role, hex)| parse_colour(hex).map(|c| (role.clone(), c)))
            .collect();
        Ok(Self {
            contract,
            manifest: Some(manifest),
            files,
            content_hash: Some(content_hash),
            source,
            colours,
        })
    }

    /// The look for `selection`, or the default (with the reasons logged).
    pub fn resolve(contract: &'static Contract, selection: Selection) -> Self {
        let attempt = match &selection {
            Selection::Default => return Self::default_for(contract),
            Selection::Embedded(id) => match pack::embedded_files(id) {
                Some(files) => Self::from_files(contract, files, selection.clone(), Some(id)),
                None => {
                    warn!("look pack \"{id}\" is not built into this game; using the default look");
                    return Self::default_for(contract);
                }
            },
            #[cfg(not(target_arch = "wasm32"))]
            Selection::Dir(path) => Self::load_dir(contract, std::path::Path::new(path)),
            #[cfg(target_arch = "wasm32")]
            Selection::Dir(_) => return Self::default_for(contract),
            // Boots on the default; the pack arrives later via `swap`.
            Selection::Fetch(_) => return Self::default_for(contract),
            // Only ever reached by a swap (`hosted.rs`), never at launch.
            Selection::Hosted(_) => return Self::default_for(contract),
        };
        attempt.unwrap_or_else(|report| {
            log_report(&report);
            warn!("look pack rejected; using the default look");
            Self::default_for(contract)
        })
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[allow(clippy::result_large_err)] // see from_files
    pub fn load_dir(contract: &'static Contract, path: &std::path::Path) -> Result<Self, Report> {
        let id = pack::dir_id(path);
        let files = pack::read_dir(path).map_err(Report::read_error)?;
        Self::from_files(
            contract,
            files,
            Selection::Dir(path.display().to_string()),
            id.as_deref(),
        )
    }

    pub fn id(&self) -> &str {
        self.manifest.as_ref().map_or("default", |m| m.id.as_str())
    }

    pub fn colour(&self, role: &str) -> Color {
        if let Some((_, c)) = self.colours.iter().find(|(r, _)| r == role) {
            return *c;
        }
        self.contract.default_colour(role).unwrap_or_else(|| {
            warn_once!("look: palette role \"{role}\" is not in the game's contract");
            MISSING
        })
    }

    /// How brightly `role` glows, as a share of the game's own: 1.0 unless
    /// the pack dims it (`glow`, schema v3). Always in `0.0..=1.0`, so a
    /// caller multiplies its own intensity by it and can only get darker.
    pub fn glow(&self, role: &str) -> f32 {
        self.manifest
            .as_ref()
            .and_then(|m| m.glow.get(role))
            .copied()
            .filter(|g| g.is_finite())
            .map_or(1.0, |g| g.clamp(0.0, 1.0))
    }

    pub fn string<'a>(&'a self, key: &'a str) -> &'a str {
        self.manifest
            .as_ref()
            .and_then(|m| m.strings.get(key))
            .map(String::as_str)
            .or_else(|| self.contract.default_string(key))
            .unwrap_or(key)
    }

    pub fn slot(&self, name: &str) -> Option<&Slot> {
        self.manifest.as_ref()?.slots.get(name)
    }

    pub fn recipe_param(&self, slot: &str, recipe: &str, param: &str) -> Option<&Param> {
        match self.slot(slot)? {
            Slot::Recipe(r) if r.recipe == recipe => r.params.get(param),
            _ => None,
        }
    }

    pub fn recipe_number(&self, slot: &str, recipe: &str, param: &str) -> Option<f64> {
        match self.recipe_param(slot, recipe, param)? {
            Param::Number(n) => Some(*n),
            _ => None,
        }
    }

    pub fn file(&self, name: &str) -> Option<&[u8]> {
        self.files.get(name).map(Vec::as_slice)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    static C: Contract = Contract {
        palette: &[
            ("ink", Color::srgb(0.0, 0.0, 0.0)),
            ("glow", Color::srgb(0.0, 1.0, 1.0)),
        ],
        strings: &[],
        slots: &[],
        audio: &[],
        fonts: &[],
        glow: &["glow"],
    };

    fn look(json: &str) -> ActiveLook {
        let files = BTreeMap::from([(
            super::super::manifest::MANIFEST_FILE.to_string(),
            json.as_bytes().to_vec(),
        )]);
        ActiveLook::from_files(&C, files, Selection::Default, None).expect("valid pack")
    }

    #[test]
    fn glow_is_full_unless_a_pack_dims_it() {
        assert_eq!(ActiveLook::default_for(&C).glow("glow"), 1.0);
        let plain = look(r#"{"v":1,"id":"p","name":"P","kind":"palette"}"#);
        assert_eq!(plain.glow("glow"), 1.0);
        let dim = look(r#"{"v":3,"id":"p","name":"P","kind":"palette","glow":{"glow":0.25}}"#);
        assert_eq!(dim.glow("glow"), 0.25);
        // A role the pack leaves alone keeps its glow.
        assert_eq!(dim.glow("ink"), 1.0);
    }
}
