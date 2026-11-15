//! The web fetch of a large built-in pack (`pack::FETCHABLE`): `?look=<id>`
//! boots on the default look, `start` fetches exactly the listed files from
//! `assets/looks/<id>/<file>` (a relative URL, so same-origin and resolved
//! like Bevy's own `assets/`), and `arrive` hands them to `swap::LookSwap`
//! with the build's content hash once they are all in.
//!
//! The hand-off is a one-shot slot shared with the async task. `arrive`
//! (presentation, `Update`) is target-independent so it tests natively with
//! a slot filled by hand; only `start` is wasm. The sim never sees any of
//! this.

use std::sync::{Arc, Mutex};

use bevy::prelude::*;

use super::select::Selection;
use super::swap::LookSwap;
use super::validate::{Files, Report};

/// Where the async fetch leaves its result: the files, or why not.
pub type Arrival = Arc<Mutex<Option<Result<Files, String>>>>;

/// A fetch in flight. Removed by `arrive` once the result is in, so a pack
/// is applied (or refused) exactly once.
#[derive(Resource)]
pub struct PendingFetch {
    pub id: String,
    /// The build's `content_hash` for this pack; nothing else is applied.
    pub content_hash: String,
    arrival: Arrival,
}

impl PendingFetch {
    /// A pending fetch and the slot its producer fills.
    pub fn new(id: impl Into<String>, content_hash: impl Into<String>) -> (Self, Arrival) {
        let arrival = Arrival::default();
        let pending = Self {
            id: id.into(),
            content_hash: content_hash.into(),
            arrival: arrival.clone(),
        };
        (pending, arrival)
    }
}

/// Polls the slot; when the result is in, swaps the pack in (hash and
/// validator first) or logs one warning and keeps the current look.
#[allow(clippy::result_large_err)] // Report is the full diagnosis
pub fn arrive(mut commands: Commands, pending: Option<Res<PendingFetch>>, mut swap: LookSwap) {
    let Some(pending) = pending else { return };
    let result = match pending.arrival.lock() {
        Ok(mut slot) => slot.take(),
        Err(_) => Some(Err("fetch task panicked".into())),
    };
    let Some(result) = result else { return };
    commands.remove_resource::<PendingFetch>();
    let id = pending.id.as_str();
    let applied = result.map_err(Report::read_error).and_then(|files| {
        swap.apply(
            files,
            Selection::Fetch(id.to_string()),
            Some(id),
            Some(&pending.content_hash),
        )
    });
    if let Err(report) = applied {
        let reasons: Vec<String> = report
            .errors()
            .map(|p| format!("{}: {}", p.at, p.message))
            .collect();
        warn!(
            "look pack \"{id}\" not applied ({}); keeping the default look",
            reasons.join("; ")
        );
    }
}

/// Starts fetching the built-in pack `id` if this build lists it as
/// fetchable; `None` (nothing started) otherwise.
#[cfg(target_arch = "wasm32")]
pub fn start(id: &str) -> Option<PendingFetch> {
    let fetchable = super::pack::fetchable(id)?;
    let (pending, arrival) = PendingFetch::new(fetchable.id, fetchable.content_hash);
    wasm_bindgen_futures::spawn_local(async move {
        let result = fetch_all(fetchable.id, fetchable.files).await;
        if let Ok(mut slot) = arrival.lock() {
            *slot = Some(result);
        }
    });
    info!("look: fetching {}", fetchable.id);
    Some(pending)
}

/// Every listed file, or the first failure. `id` and the names come from
/// the build-time table and passed the naming rules there (`embed.rs`), so
/// the URL stays under `assets/looks/<id>/`.
#[cfg(target_arch = "wasm32")]
async fn fetch_all(id: &str, names: &[&str]) -> Result<Files, String> {
    let mut files = Files::new();
    for name in names {
        let url = format!("assets/looks/{id}/{name}");
        let bytes = fetch_bytes(&url).await.map_err(|e| format!("{url}: {e}"))?;
        files.insert(name.to_string(), bytes);
    }
    Ok(files)
}

#[cfg(target_arch = "wasm32")]
async fn fetch_bytes(url: &str) -> Result<Vec<u8>, String> {
    use js_sys::wasm_bindgen::JsCast;
    use wasm_bindgen_futures::JsFuture;

    fn describe(e: js_sys::wasm_bindgen::JsValue) -> String {
        e.as_string().unwrap_or_else(|| format!("{e:?}"))
    }

    let window = web_sys::window().ok_or("no window")?;
    let response: web_sys::Response = JsFuture::from(window.fetch_with_str(url))
        .await
        .map_err(describe)?
        .dyn_into()
        .map_err(|_| "not a Response".to_string())?;
    if !response.ok() {
        return Err(format!("HTTP {}", response.status()));
    }
    let buffer = JsFuture::from(response.array_buffer().map_err(describe)?)
        .await
        .map_err(describe)?;
    Ok(js_sys::Uint8Array::new(&buffer).to_vec())
}
