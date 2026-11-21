//! The web fetch of a large built-in pack (`pack::FETCHABLE`): `?look=<id>`
//! boots on the default look, `start` fetches exactly the listed files from
//! `assets/looks/<id>/<file>?h=<hash prefix>` (relative, so same-origin and
//! resolved like Bevy's own `assets/`; the query busts stale caches), and
//! `arrive` hands them to `swap::LookSwap` with the build's content hash once
//! they are all in.
//!
//! The hand-off is a one-shot slot shared with the async task. `arrive`
//! (presentation, `Update`) is target-independent so it tests natively with
//! a slot filled by hand; only `start` is wasm. The sim never sees any of
//! this.

use std::sync::{Arc, Mutex};

use bevy::prelude::*;

use gamebient_input::HostEvent;

use super::select::Selection;
use super::swap::LookSwap;
use super::validate::{Files, Report, content_hash};

/// Where the async fetch leaves its result: the files, or why not.
pub type Arrival = Arc<Mutex<Option<Result<Files, String>>>>;

/// Prefix of a fetch error for a pack over `limits::PACK_MAX`; `arrive`
/// maps it to the `caps` reason.
pub const CAPS_PREFIX: &str = "caps:";

/// A fetch in flight. Removed by `arrive` once the result is in, so a pack
/// is applied (or refused) exactly once.
#[derive(Resource)]
pub struct PendingFetch {
    pub id: String,
    /// The expected `content_hash`; nothing else is applied.
    pub content_hash: String,
    /// Picked by the host page (`look.set`): answered with `look.applied`
    /// and worn as `Selection::Hosted`. Built-in fetches stay silent.
    pub hosted: bool,
    arrival: Arrival,
}

impl PendingFetch {
    /// A pending built-in fetch and the slot its producer fills.
    pub fn new(id: impl Into<String>, content_hash: impl Into<String>) -> (Self, Arrival) {
        Self::make(id.into(), content_hash.into(), false)
    }

    /// A pending hosted fetch (`hosted.rs`) and its slot.
    pub fn new_hosted(id: impl Into<String>, content_hash: impl Into<String>) -> (Self, Arrival) {
        Self::make(id.into(), content_hash.into(), true)
    }

    fn make(id: String, content_hash: String, hosted: bool) -> (Self, Arrival) {
        let arrival = Arrival::default();
        let pending = Self {
            id,
            content_hash,
            hosted,
            arrival: arrival.clone(),
        };
        (pending, arrival)
    }
}

/// Polls the slot; when the result is in, swaps the pack in (hash and
/// validator first) or logs one warning and keeps the current look. A
/// hosted fetch is answered with `look.applied` either way.
pub fn arrive(
    mut commands: Commands,
    pending: Option<Res<PendingFetch>>,
    mut swap: LookSwap,
    mut answer: MessageWriter<HostEvent>,
) {
    let Some(pending) = pending else { return };
    let result = match pending.arrival.lock() {
        Ok(mut slot) => slot.take(),
        Err(_) => Some(Err("fetch task panicked".into())),
    };
    let Some(result) = result else { return };
    commands.remove_resource::<PendingFetch>();
    let id = pending.id.as_str();
    let source = if pending.hosted {
        Selection::Hosted(id.to_string())
    } else {
        Selection::Fetch(id.to_string())
    };
    let outcome: Result<(), (String, Report)> = match result {
        Err(e) => {
            let reason = if e.starts_with(CAPS_PREFIX) {
                "caps"
            } else {
                "fetch"
            };
            Err((reason.into(), Report::read_error(e)))
        }
        Ok(files) => {
            let got = content_hash(&files);
            if got != pending.content_hash {
                Err((
                    "hash".into(),
                    Report::read_error(format!(
                        "content hash {got} does not match {}",
                        pending.content_hash
                    )),
                ))
            } else {
                // None: the hash was just checked above.
                swap.apply(files, source, Some(id), None).map_err(|report| {
                    let first = report.errors().next().map_or_else(
                        || "rejected".to_string(),
                        |p| format!("{}: {}", p.at, p.message),
                    );
                    (format!("invalid: {}", truncate(&first, 120)), report)
                })
            }
        }
    };
    if let Err((_, report)) = &outcome {
        let reasons: Vec<String> = report
            .errors()
            .map(|p| format!("{}: {}", p.at, p.message))
            .collect();
        warn!(
            "look pack \"{id}\" not applied ({}); keeping the current look",
            reasons.join("; ")
        );
    }
    if pending.hosted {
        answer.write(super::hosted::applied(
            id,
            outcome.map_err(|(reason, _)| reason),
        ));
    }
}

/// At most `max` characters, on a char boundary.
fn truncate(s: &str, max: usize) -> &str {
    match s.char_indices().nth(max) {
        Some((i, _)) => &s[..i],
        None => s,
    }
}

/// Starts fetching the built-in pack `id` if this build lists it as
/// fetchable; `None` (nothing started) otherwise.
#[cfg(target_arch = "wasm32")]
pub fn start(id: &str) -> Option<PendingFetch> {
    let fetchable = super::pack::fetchable(id)?;
    let (pending, arrival) = PendingFetch::new(fetchable.id, fetchable.content_hash);
    wasm_bindgen_futures::spawn_local(async move {
        let result = fetch_all(fetchable).await;
        if let Ok(mut slot) = arrival.lock() {
            *slot = Some(result);
        }
    });
    info!("look: fetching {}", fetchable.id);
    Some(pending)
}

/// A built-in pack's files from `assets/looks/<id>/`. `id` and the names
/// come from the build-time table and passed the naming rules there
/// (`embed.rs`), so the URL stays under `assets/looks/<id>/`. The query is
/// the first 16 hex of the pack's build-time content hash: files sit at
/// stable unhashed URLs, so after a redeploy it keeps a browser's cached
/// old file from failing the hash check.
#[cfg(target_arch = "wasm32")]
async fn fetch_all(fetchable: super::pack::Fetchable) -> Result<Files, String> {
    let tag = &fetchable.content_hash[..fetchable.content_hash.len().min(16)];
    let urls = fetchable
        .files
        .iter()
        .map(|name| {
            (
                name.to_string(),
                format!("assets/looks/{}/{name}?h={tag}", fetchable.id),
            )
        })
        .collect();
    fetch_files(urls).await
}

/// Every `(name, url)`, fetched concurrently, or the first failure. Over
/// `limits::PACK_MAX` bytes in all is an error starting `CAPS_PREFIX`.
#[cfg(target_arch = "wasm32")]
async fn fetch_files(urls: Vec<(String, String)>) -> Result<Files, String> {
    use js_sys::wasm_bindgen::JsCast;
    use wasm_bindgen_futures::JsFuture;

    use super::validate::limits;

    fn describe(e: js_sys::wasm_bindgen::JsValue) -> String {
        e.as_string().unwrap_or_else(|| format!("{e:?}"))
    }

    let window = web_sys::window().ok_or("no window")?;
    let requests = js_sys::Array::new();
    for (_, url) in &urls {
        requests.push(&window.fetch_with_str(url));
    }
    let responses = JsFuture::from(js_sys::Promise::all(&requests))
        .await
        .map_err(describe)?;

    let bodies = js_sys::Array::new();
    for ((_, url), response) in urls.iter().zip(js_sys::Array::from(&responses).iter()) {
        let response: web_sys::Response = response
            .dyn_into()
            .map_err(|_| format!("{url}: not a Response"))?;
        if !response.ok() {
            return Err(format!("{url}: HTTP {}", response.status()));
        }
        bodies.push(&response.array_buffer().map_err(describe)?.into());
    }
    let buffers = JsFuture::from(js_sys::Promise::all(&bodies))
        .await
        .map_err(describe)?;

    let mut files = Files::new();
    let mut total = 0usize;
    for ((name, _), buffer) in urls.iter().zip(js_sys::Array::from(&buffers).iter()) {
        let bytes = js_sys::Uint8Array::new(&buffer).to_vec();
        total += bytes.len();
        if total > limits::PACK_MAX {
            return Err(format!("{CAPS_PREFIX} over {} bytes", limits::PACK_MAX));
        }
        files.insert(name.clone(), bytes);
    }
    Ok(files)
}

/// Starts fetching a hosted pack (`hosted.rs` already checked the origin,
/// names and caps).
#[cfg(target_arch = "wasm32")]
pub fn start_hosted(look: &super::hosted::HostedLook) -> PendingFetch {
    let (pending, arrival) = PendingFetch::new_hosted(look.id.clone(), look.hash.clone());
    let urls = look.urls();
    wasm_bindgen_futures::spawn_local(async move {
        let result = fetch_files(urls).await;
        if let Ok(mut slot) = arrival.lock() {
            *slot = Some(result);
        }
    });
    info!("look: fetching hosted {}", look.id);
    pending
}
