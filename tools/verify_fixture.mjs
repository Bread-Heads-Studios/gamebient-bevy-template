#!/usr/bin/env node
// Runs the wasm verifier on a .gxr file. Exit 0 only when matches === true.
// Usage: node tools/verify_fixture.mjs tests/fixtures/selftest.gxr
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { resolve } from "node:path";

const require = createRequire(import.meta.url);
const file = process.argv[2];
if (!file) {
  console.error("usage: verify_fixture.mjs <file.gxr>");
  process.exit(2);
}
const { verify } = require(resolve("dist-verify/verify.js"));
const t0 = performance.now();
// NOTE: the checksum is a u64 computed and compared in Rust; JSON.parse turns
// it into a JS double, so a printed value like 15390901594743612000 is a
// display artifact of JSON.parse/JSON.stringify precision loss, not a
// mismatch. `matches` below reflects the real Rust-side comparison.
const out = JSON.parse(verify(new Uint8Array(readFileSync(file))));
out.ms = Math.round(performance.now() - t0);
console.log(JSON.stringify(out));
process.exit(out.matches === true ? 0 : 1);
