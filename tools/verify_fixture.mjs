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
// `checksum` arrives as a decimal string, so the u64 survives JSON.parse
// intact and prints verbatim. `matches` is still the Rust-side comparison.
const out = JSON.parse(verify(new Uint8Array(readFileSync(file))));
out.ms = Math.round(performance.now() - t0);
console.log(JSON.stringify(out));
process.exit(out.matches === true ? 0 : 1);
