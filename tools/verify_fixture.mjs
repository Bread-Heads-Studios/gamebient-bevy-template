#!/usr/bin/env node
// Runs the wasm verifier on a .gxr file. Exit 0 only when matches === true.
//   node tools/verify_fixture.mjs tests/fixtures/selftest-wasm.gxr
//
// --record writes a fixture instead of checking one: it calls the module's
// selftest_record(), which records the scripted selftest run *inside the
// wasm module*, and writes the encoded bytes. That is the only way to get a
// fixture whose recording side is the same arithmetic as the verifying side,
// which is what makes CI's gate wasm-vs-wasm:
//   tools/build_verify.sh
//   node tools/verify_fixture.mjs --record tests/fixtures/selftest-wasm.gxr
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, resolve } from "node:path";

const require = createRequire(import.meta.url);
const args = process.argv.slice(2);
const recordAt = args.indexOf("--record");
const file = recordAt >= 0 ? args[recordAt + 1] : args[0];
if (!file) {
  console.error(
    "usage: verify_fixture.mjs <file.gxr> | verify_fixture.mjs --record <out.gxr>",
  );
  process.exit(2);
}
const mod = require(resolve("dist-verify/verify.js"));

if (recordAt >= 0) {
  if (typeof mod.selftest_record !== "function") {
    console.error(
      "dist-verify/verify.js has no selftest_record export; rebuild with tools/build_verify.sh",
    );
    process.exit(2);
  }
  const bytes = Buffer.from(mod.selftest_record());
  mkdirSync(dirname(resolve(file)), { recursive: true });
  writeFileSync(file, bytes);
  console.error(`wrote ${file} (${bytes.length} bytes)`);
  process.exit(0);
}

const t0 = performance.now();
// `checksum` arrives as a decimal string, so the u64 survives JSON.parse
// intact and prints verbatim. `matches` is still the Rust-side comparison.
const out = JSON.parse(mod.verify(new Uint8Array(readFileSync(file))));
out.ms = Math.round(performance.now() - t0);
console.log(JSON.stringify(out));
process.exit(out.matches === true ? 0 : 1);
