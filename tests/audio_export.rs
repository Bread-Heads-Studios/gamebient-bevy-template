//! Renders every baked cue, layer and sound effect, times it, and (with
//! `GX_AUDIO_EXPORT=<dir>`) writes each to a WAV so the owner can listen to a
//! game's sound without running it:
//!
//!     GX_AUDIO_EXPORT=/tmp/gx-audio cargo test --test audio_export -- --nocapture
//!
//! The time budget is the spec's: everything baked renders in <= 150 ms in a
//! release build on a Pi 4. Tests run unoptimized, so the ceiling here is
//! loose; the printed timings are the useful number.

use gamebient_game::game::audio::{export_inventory, tracker::wav_secs};

#[test]
fn baked_audio_renders_within_budget_and_exports() {
    let t0 = std::time::Instant::now();
    let inventory = export_inventory();
    let secs = t0.elapsed().as_secs_f32();
    for (name, source) in &inventory {
        println!("{name}: {:.2}s", wav_secs(source));
    }
    println!("rendered {} cues in {secs:.3}s", inventory.len());
    assert!(!inventory.is_empty());
    assert!(
        secs < 2.0,
        "baked audio took {secs:.2}s to render (debug build; the release target is 150 ms on a Pi 4)"
    );
    if let Ok(dir) = std::env::var("GX_AUDIO_EXPORT") {
        let dir = std::path::Path::new(&dir);
        std::fs::create_dir_all(dir).expect("create export dir");
        for (name, source) in &inventory {
            let path = dir.join(format!("{}.wav", name.replace('/', "-")));
            std::fs::write(&path, &source.bytes[..]).expect("write wav");
            println!("wrote {}", path.display());
        }
    }
}
