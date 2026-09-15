//! Replay verifier. Native: `verify <file.gxr>` prints a verdict JSON and
//! exits 0 (matches) / 1 (mismatch) / 2 (decode error);
//! `verify --selftest [--write <file>]` records the scripted run, replays
//! it, and optionally writes the fixture. wasm (`--target nodejs`):
//! `verify(bytes)` returns the same JSON.

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
