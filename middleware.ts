// Vercel Routing Middleware: serves the full-game bundle under
// /full/<token>/... only when <token> is a valid ColecoVision GX entitlement
// for this host (AGENTS.md "Demo and full bundles"). No cookies: the game is
// framed cross-site from colecovisiongx.com, where third-party cookies are
// blocked by Safari. Runs before the edge cache. Web APIs only, no npm deps.
// Vercel runs this file on the Node runtime as CommonJS without bundling, so
// the ESM verifier must be loaded with import(), not a static import.
const entitlement = import("./tools/entitlement.mjs");

export const config = {
  matcher: ["/full/:path*"],
};

const FORBIDDEN_HTML = `<!doctype html><meta charset="utf-8"><title>Full game</title>
<style>body{margin:0;min-height:100vh;display:grid;place-items:center;background:#0a0a0f;color:#e8e4df;font:16px/1.5 system-ui,sans-serif;text-align:center}a{color:#00d4ff}</style>
<div><p>This is the full game.</p><p>Launch it from <a href="https://colecovisiongx.com/play">colecovisiongx.com/play</a>.</p></div>`;

function forbidden(reason: string): Response {
  return new Response(FORBIDDEN_HTML, {
    status: 403,
    headers: {
      "content-type": "text/html; charset=utf-8",
      "cache-control": "no-store",
      "x-gx-entitlement": reason,
    },
  });
}

export default async function middleware(request: Request): Promise<Response> {
  const { verifyToken, splitFullPath } = await entitlement;
  const pubkey = process.env.GX_ENTITLEMENT_PUBKEY;
  if (!pubkey) return forbidden("unconfigured");
  const url = new URL(request.url);
  const split = splitFullPath(url.pathname);
  if (!split) return forbidden("missing");
  const r = await verifyToken(split.token, pubkey, { host: url.host, now: Math.floor(Date.now() / 1000) });
  if (!r.ok) return forbidden(r.reason);
  // Serve the static file behind the token segment. Vercel reads
  // x-middleware-rewrite and serves that path from this deployment.
  const target = new URL(`/full/${split.rest || "index.html"}`, request.url);
  target.search = url.search;
  return new Response(null, {
    headers: {
      "x-middleware-rewrite": target.toString(),
      "cache-control": "no-store",
      "referrer-policy": "no-referrer",
    },
  });
}
