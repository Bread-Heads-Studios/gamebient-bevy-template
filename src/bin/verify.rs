//! Replay verifier. Native: `verify <file.gxr>` prints a verdict JSON and
//! exits 0 (matches) / 1 (mismatch) / 2 (decode error);
//! `verify --selftest [--write <file>]` records the scripted run, replays
//! it, and optionally writes the fixture. wasm (`--target nodejs`):
//! `verify(bytes)` returns the same JSON, and `selftest_record()` returns
//! the encoded bytes of the same scripted run recorded *by the wasm build*
//! — that is what `node tools/verify_fixture.mjs --record <out.gxr>` writes
//! as `tests/fixtures/selftest-wasm.gxr`, so CI's fixture gate is
//! wasm-vs-wasm rather than native-vs-wasm. See docs/replay-verification.md.

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    use gamebient_game::game::replay::selftest::record_scripted_run;
    use gamebient_game::game::replay::{Replay, verify};
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("--selftest") => {
            let replay = record_scripted_run();
            let v = verify(&replay);
            println!("{}", v.to_json());
            if let Some(i) = args.iter().position(|a| a == "--write") {
                let path = &args[i + 1];
                if let Some(parent) = std::path::Path::new(path).parent()
                    && !parent.as_os_str().is_empty()
                {
                    std::fs::create_dir_all(parent).expect("create fixture dir");
                }
                std::fs::write(path, replay.encode()).expect("write fixture");
                eprintln!("wrote {path} ({} bytes)", replay.encode().len());
            }
            std::process::exit(if v.matches { 0 } else { 1 });
        }
        Some(path) => {
            let bytes = std::fs::read(path).expect("read replay");
            match Replay::decode(&bytes) {
                Ok(r) => {
                    let v = verify(&r);
                    println!("{}", v.to_json());
                    std::process::exit(if v.matches { 0 } else { 1 });
                }
                Err(e) => {
                    eprintln!("decode error: {e:?}");
                    std::process::exit(2);
                }
            }
        }
        None => {
            eprintln!("usage: verify <file.gxr> | verify --selftest [--write <file>]");
            std::process::exit(2);
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn main() {}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn verify(bytes: &[u8]) -> String {
    use gamebient_game::game::replay::{Replay, verify};
    match Replay::decode(bytes) {
        Ok(r) => verify(&r).to_json(),
        Err(e) => format!("{{\"error\":\"{e:?}\"}}"),
    }
}

/// Records the scripted selftest run inside this wasm module and returns the
/// encoded `GXR1` bytes. Native code cannot produce this file: the whole
/// point is that the recording side is the same wasm arithmetic the
/// verifying side uses, so `verify(selftest_record())` compares like with
/// like even for a sim that calls `sin`/`cos`/`powf`.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn selftest_record() -> Vec<u8> {
    use gamebient_game::game::replay::selftest::record_scripted_run;
    record_scripted_run().encode()
}
