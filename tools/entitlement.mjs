// Verifies a ColecoVision GX full-game entitlement token. Web APIs only
// (TextEncoder, atob, crypto.subtle) so the same file runs in Vercel
// middleware and under `node --test`. Mirror of the website's
// src/lib/entitlement/payload.ts + sign.ts.

export const MAX_AGE = 43_200;
export const COOKIE = "gx_full";

const B64URL = /^[A-Za-z0-9_-]+$/;

function fromBase64Url(s) {
  const b64 = s.replace(/-/g, "+").replace(/_/g, "/") + "=".repeat((4 - (s.length % 4)) % 4);
  const bin = atob(b64);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}

function fromBase64(s) {
  const bin = atob(s);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}

function decodePayload(part) {
  let parsed;
  try {
    parsed = JSON.parse(new TextDecoder().decode(fromBase64Url(part)));
  } catch {
    return null;
  }
  if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) return null;
  const keys = Object.keys(parsed);
  if (keys.length !== 4) return null;
  const { h, w, g, exp } = parsed;
  if (typeof h !== "string" || typeof w !== "string" || typeof g !== "string") return null;
  if (typeof exp !== "number" || !Number.isFinite(exp)) return null;
  return { h, w, g, exp };
}

/**
 * @param {string} token  "<base64url payload>.<base64url signature>"
 * @param {string} publicKeyB64  base64 of the raw 32-byte Ed25519 public key
 * @param {{host: string, now: number}} ctx  request host and unix seconds
 */
export async function verifyToken(token, publicKeyB64, ctx) {
  if (typeof token !== "string") return { ok: false, reason: "format" };
  const parts = token.split(".");
  if (parts.length !== 2 || !B64URL.test(parts[0]) || !B64URL.test(parts[1])) return { ok: false, reason: "format" };
  const [part, sigB64] = parts;
  let sig;
  try {
    sig = fromBase64Url(sigB64);
  } catch {
    return { ok: false, reason: "format" };
  }
  if (sig.length !== 64) return { ok: false, reason: "format" };
  let key;
  try {
    key = await crypto.subtle.importKey("raw", fromBase64(publicKeyB64), { name: "Ed25519" }, false, ["verify"]);
  } catch {
    return { ok: false, reason: "key" };
  }
  const valid = await crypto.subtle.verify({ name: "Ed25519" }, key, sig, new TextEncoder().encode(part));
  if (!valid) return { ok: false, reason: "signature" };
  const payload = decodePayload(part);
  if (!payload) return { ok: false, reason: "payload" };
  if (payload.h.toLowerCase() !== ctx.host.toLowerCase()) return { ok: false, reason: "host" };
  if (!(ctx.now < payload.exp)) return { ok: false, reason: "expired" };
  return { ok: true, payload };
}

export function cookieHeader(token, maxAge = MAX_AGE) {
  return `${COOKIE}=${token}; Path=/full; Max-Age=${maxAge}; Secure; HttpOnly; SameSite=None`;
}

export function readCookie(cookieHeaderValue, name) {
  if (!cookieHeaderValue) return null;
  for (const pair of cookieHeaderValue.split(";")) {
    const [k, ...v] = pair.trim().split("=");
    if (k === name) return v.join("=");
  }
  return null;
}
