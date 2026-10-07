// Vercel Routing Middleware: gates the full-game bundle under /full/ behind
// a ColecoVision GX entitlement token (docs: AGENTS.md "Demo and full
// bundles"). Runs before the edge cache. No npm dependencies: Web APIs only.
// Vercel runs this file on the Node.js runtime as CommonJS without bundling, so
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

export default async function middleware(request: Request): Promise<Response | undefined> {
  const { COOKIE, cookieHeader, readCookie, verifyToken } = await entitlement;

  const pubkey = process.env.GX_ENTITLEMENT_PUBKEY;
  if (!pubkey) return forbidden("unconfigured");
  const url = new URL(request.url);
  const ctx = { host: url.host, now: Math.floor(Date.now() / 1000) };

  // Entry: the website frames /full/?t=<token>. Verify, set the cookie, and
  // redirect to the clean URL so the bundle's relative asset loads carry it.
  const t = url.searchParams.get("t");
  if (t) {
    const r = await verifyToken(t, pubkey, ctx);
    if (!r.ok) return forbidden(r.reason);
    url.searchParams.delete("t");
    return new Response(null, {
      status: 302,
      headers: {
        location: url.pathname + url.search,
        "set-cookie": cookieHeader(t),
        "cache-control": "no-store",
        "referrer-policy": "no-referrer",
      },
    });
  }

  const cookie = readCookie(request.headers.get("cookie"), COOKIE);
  if (!cookie) return forbidden("missing");
  const r = await verifyToken(cookie, pubkey, ctx);
  if (!r.ok) return forbidden(r.reason);
  return undefined; // fall through to the static file
}
