// node --test tools/test_entitlement.mjs
import { test } from "node:test";
import assert from "node:assert/strict";
import { webcrypto } from "node:crypto";
import { splitFullPath, verifyToken, cacheControlFor } from "./entitlement.mjs";

const subtle = webcrypto.subtle;
const enc = new TextEncoder();
const b64url = (bytes) =>
  Buffer.from(bytes).toString("base64").replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");

async function keypair() {
  const kp = await subtle.generateKey({ name: "Ed25519" }, true, ["sign", "verify"]);
  const raw = new Uint8Array(await subtle.exportKey("raw", kp.publicKey));
  return { priv: kp.privateKey, pubB64: Buffer.from(raw).toString("base64") };
}

async function mint(priv, payload) {
  const part = b64url(enc.encode(JSON.stringify(payload)));
  const sig = new Uint8Array(await subtle.sign({ name: "Ed25519" }, priv, enc.encode(part)));
  return `${part}.${b64url(sig)}`;
}

const HOST = "voidrunner-theta.vercel.app";
const NOW = 1_800_000_000;

test("a good token verifies for its host before expiry", async () => {
  const { priv, pubB64 } = await keypair();
  const token = await mint(priv, { h: HOST, w: "W", g: "G", exp: NOW + 60 });
  const r = await verifyToken(token, pubB64, { host: HOST.toUpperCase(), now: NOW });
  assert.equal(r.ok, true);
  assert.deepEqual(r.payload, { h: HOST, w: "W", g: "G", exp: NOW + 60 });
});

test("expired, wrong host, wrong key, tampered and malformed tokens fail", async () => {
  const { priv, pubB64 } = await keypair();
  const other = await keypair();
  const good = await mint(priv, { h: HOST, w: "W", g: "G", exp: NOW + 60 });
  assert.equal((await verifyToken(good, pubB64, { host: HOST, now: NOW + 60 })).reason, "expired");
  assert.equal((await verifyToken(good, pubB64, { host: "x.vercel.app", now: NOW })).reason, "host");
  assert.equal((await verifyToken(good, other.pubB64, { host: HOST, now: NOW })).reason, "signature");
  const [part, sig] = good.split(".");
  const tampered = `${b64url(enc.encode(JSON.stringify({ h: "x.vercel.app", w: "W", g: "G", exp: NOW + 60 })))}.${sig}`;
  assert.equal((await verifyToken(tampered, pubB64, { host: "x.vercel.app", now: NOW })).reason, "signature");
  assert.equal((await verifyToken("nodot", pubB64, { host: HOST, now: NOW })).reason, "format");
  assert.equal((await verifyToken(`${part}.!!!`, pubB64, { host: HOST, now: NOW })).reason, "format");
  assert.equal((await verifyToken(`${part}.A`, pubB64, { host: HOST, now: NOW })).reason, "format");
  const badShape = await mint(priv, { h: HOST, w: "W", exp: NOW + 60 });
  assert.equal((await verifyToken(badShape, pubB64, { host: HOST, now: NOW })).reason, "payload");
});

test("splitFullPath extracts the token segment and the rest", () => {
  assert.deepEqual(splitFullPath("/full/abc.def/"), { token: "abc.def", rest: "" });
  assert.deepEqual(splitFullPath("/full/abc.def"), { token: "abc.def", rest: "" });
  assert.deepEqual(splitFullPath("/full/abc.def/assets/x.png"), { token: "abc.def", rest: "assets/x.png" });
  for (const p of ["/full/", "/full", "/full/nodot/x", "/full/abc.def.ghi/x", "/other/abc.def/"]) {
    assert.equal(splitFullPath(p), null, p);
  }
});

test("cacheControlFor returns immutable for hashed bundle files, no-store otherwise", () => {
  assert.equal(cacheControlFor("gamebient-game_bg.44888a2b.wasm"), "private, max-age=31536000, immutable");
  assert.equal(cacheControlFor("gamebient-game_bg.44888a2b.wasm.br"), "private, max-age=31536000, immutable");
  assert.equal(cacheControlFor("gamebient-game.1ea2da30.js"), "private, max-age=31536000, immutable");
  assert.equal(cacheControlFor(""), "no-store");
  assert.equal(cacheControlFor("index.html"), "no-store");
  assert.equal(cacheControlFor("assets/info.json"), "no-store");
  assert.equal(cacheControlFor("gamebient-game_bg.wasm"), "no-store");
});
