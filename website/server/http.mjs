/**
 * Developer Portal — HTTP primitives.
 *
 * Small, explicit helpers rather than a framework: request bodies are capped,
 * cookies are parsed and serialised here (so the attributes live in one place),
 * and the static-file resolver is the only code that turns a URL into a path.
 */

import fs from "node:fs/promises";
import path from "node:path";

/** Defence in depth: no portal request needs a larger body. */
export const MAX_BODY_BYTES = 8 * 1024;

const MIME_TYPES = new Map([
  [".html", "text/html; charset=utf-8"],
  [".js", "text/javascript; charset=utf-8"],
  [".mjs", "text/javascript; charset=utf-8"],
  [".css", "text/css; charset=utf-8"],
  [".json", "application/json; charset=utf-8"],
  [".svg", "image/svg+xml"],
  [".png", "image/png"],
  [".jpg", "image/jpeg"],
  [".webp", "image/webp"],
  [".ico", "image/x-icon"],
  [".txt", "text/plain; charset=utf-8"],
  [".xml", "application/xml; charset=utf-8"],
  [".woff2", "font/woff2"],
  [".map", "application/json; charset=utf-8"],
]);

/**
 * Headers that are safe to send on every response.
 *
 * `X-Robots-Tag` is deliberately **not** here. Phase 17 found it being sent on
 * every response, including the public pages and their assets: a launch build
 * with indexing enabled would then have had `robots.txt: Allow: /` while every
 * response told crawlers `noindex, nofollow`. It is now applied only where it
 * belongs — portal paths and API responses (`NOINDEX_HEADERS`).
 *
 * HSTS and framing headers are also not here: they are production-only and are
 * applied once per request in `server/index.mjs` via `productionSecurityHeaders`,
 * so the local preview stays embeddable while a real deployment gets them.
 */
export const BASE_HEADERS = Object.freeze({
  "X-Content-Type-Options": "nosniff",
  "Referrer-Policy": "strict-origin-when-cross-origin",
});

/** Applied to portal pages and every API response. Never to public pages. */
export const NOINDEX_HEADERS = Object.freeze({
  "X-Robots-Tag": "noindex, nofollow",
});

/**
 * Headers only a real HTTPS deployment should send.
 *
 * Gated on `isProduction` (and, for HSTS, on secure cookies): sending HSTS from
 * a local http:// development server would pin `localhost` to HTTPS in that
 * browser for a year, which is a self-inflicted outage.
 *
 * `frame-ancestors 'none'` is a standalone CSP directive — it restricts framing
 * only and cannot break any resource on the page. A full resource CSP is
 * deliberately not invented here; it needs a review pass against the particle
 * canvas, the custom cursor, and Vite's asset URLs (DEVELOPER_PORTAL.md §10).
 */
export function productionSecurityHeaders(config) {
  if (!config.isProduction) return {};
  /** @type {Record<string, string>} */
  const headers = {
    "X-Frame-Options": "DENY",
    "Content-Security-Policy": "frame-ancestors 'none'",
  };
  if (config.secureCookies) {
    headers["Strict-Transport-Security"] = "max-age=31536000; includeSubDomains";
  }
  return headers;
}

/** Headers for anything that must never be cached by a shared cache. */
export const NO_STORE_HEADERS = Object.freeze({
  "Cache-Control": "no-store, no-cache, must-revalidate, private",
  Pragma: "no-cache",
  Expires: "0",
});

/**
 * @param {import("node:http").ServerResponse} res
 * @param {number} status
 * @param {unknown} body
 * @param {Record<string, string>} [headers]
 */
export function sendJson(res, status, body, headers = {}) {
  const payload = JSON.stringify(body);
  res.writeHead(status, {
    ...BASE_HEADERS,
    ...NOINDEX_HEADERS,
    ...NO_STORE_HEADERS,
    "Content-Type": "application/json; charset=utf-8",
    "Content-Length": Buffer.byteLength(payload),
    Vary: "Cookie",
    ...headers,
  });
  res.end(payload);
}

/**
 * @param {import("node:http").ServerResponse} res
 * @param {number} status
 * @param {string} text
 * @param {Record<string, string>} [headers]
 */
export function sendText(res, status, text, headers = {}) {
  res.writeHead(status, {
    ...BASE_HEADERS,
    ...NOINDEX_HEADERS,
    "Content-Type": "text/plain; charset=utf-8",
    "Content-Length": Buffer.byteLength(text),
    ...headers,
  });
  res.end(text);
}

/** The normal error envelope: a stable code and a short human message. */
export function sendError(res, status, code, message, headers = {}) {
  sendJson(res, status, { error: { code, message } }, headers);
}

/**
 * Parses a `Cookie` header into a plain object. First occurrence of a name wins;
 * duplicates are ignored rather than concatenated.
 *
 * @param {string | undefined} header
 * @returns {Record<string, string>}
 */
export function parseCookies(header) {
  /** @type {Record<string, string>} */
  const out = {};
  if (typeof header !== "string" || header === "") return out;
  for (const part of header.split(";")) {
    const index = part.indexOf("=");
    if (index < 1) continue;
    const name = part.slice(0, index).trim();
    const value = part.slice(index + 1).trim();
    if (name === "" || Object.hasOwn(out, name)) continue;
    try {
      out[name] = decodeURIComponent(value);
    } catch {
      out[name] = value;
    }
  }
  return out;
}

/**
 * Serialises a cookie. `sameSite` defaults to `Strict`, which is what stops a
 * cross-site request from carrying the session cookie at all.
 *
 * @param {string} name
 * @param {string} value
 * @param {{ maxAgeSeconds?: number, secure?: boolean, httpOnly?: boolean, sameSite?: "Strict" | "Lax", path?: string }} [options]
 */
export function serializeCookie(name, value, options = {}) {
  const parts = [`${name}=${encodeURIComponent(value)}`];
  parts.push(`Path=${options.path ?? "/"}`);
  if (options.maxAgeSeconds !== undefined) {
    parts.push(`Max-Age=${Math.max(0, Math.floor(options.maxAgeSeconds))}`);
  }
  parts.push(`SameSite=${options.sameSite ?? "Strict"}`);
  if (options.httpOnly !== false) parts.push("HttpOnly");
  if (options.secure) parts.push("Secure");
  return parts.join("; ");
}

/**
 * Reads and parses a JSON request body.
 *
 * Returns `{ ok: true, value }` or `{ ok: false, status, code, message }` so the
 * caller never has to distinguish "too large", "not JSON" and "not an object".
 *
 * @param {import("node:http").IncomingMessage} req
 * @returns {Promise<{ ok: true, value: Record<string, unknown> } | { ok: false, status: number, code: string, message: string }>}
 */
export async function readJsonBody(req) {
  const contentType = String(req.headers["content-type"] ?? "");
  if (!contentType.toLowerCase().startsWith("application/json")) {
    return { ok: false, status: 415, code: "unsupported_media_type", message: "Expected application/json." };
  }

  const declared = Number.parseInt(String(req.headers["content-length"] ?? ""), 10);
  if (Number.isFinite(declared) && declared > MAX_BODY_BYTES) {
    return { ok: false, status: 413, code: "payload_too_large", message: "Request body is too large." };
  }

  /** @type {Buffer[]} */
  const chunks = [];
  let total = 0;
  for await (const chunk of req) {
    total += chunk.length;
    if (total > MAX_BODY_BYTES) {
      return { ok: false, status: 413, code: "payload_too_large", message: "Request body is too large." };
    }
    chunks.push(Buffer.from(chunk));
  }

  const text = Buffer.concat(chunks).toString("utf8");
  if (text.trim() === "") {
    return { ok: false, status: 400, code: "invalid_request", message: "Request body is required." };
  }

  let parsed;
  try {
    parsed = JSON.parse(text);
  } catch {
    return { ok: false, status: 400, code: "invalid_json", message: "Request body is not valid JSON." };
  }
  if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) {
    return { ok: false, status: 400, code: "invalid_request", message: "Request body must be a JSON object." };
  }
  return { ok: true, value: parsed };
}

/**
 * The client address, for rate limiting only.
 *
 * `X-Forwarded-For` is believed **only** when `NEXORA_PORTAL_TRUST_PROXY=true`,
 * because otherwise any caller could set it and evade the limiter by rotating a
 * header.
 *
 * @param {import("node:http").IncomingMessage} req
 * @param {{ trustProxy: boolean }} config
 */
export function clientAddress(req, config) {
  if (config.trustProxy) {
    const forwarded = req.headers["x-forwarded-for"];
    const raw = Array.isArray(forwarded) ? forwarded[0] : forwarded;
    if (typeof raw === "string" && raw.trim() !== "") {
      const first = raw.split(",")[0]?.trim();
      if (first) return first;
    }
  }
  return req.socket?.remoteAddress ?? "unknown";
}

/**
 * Origin / same-site enforcement for state-changing requests.
 *
 * Accepts when either:
 * - `Origin` matches the configured origin exactly, or
 * - there is no `Origin` (some same-origin form posts) **and** the browser says
 *   `Sec-Fetch-Site: same-origin` / `none`, or the `Host` header matches the
 *   configured origin's host.
 *
 * Everything else is rejected. In development without `NEXORA_PORTAL_ORIGIN`,
 * the comparison host is derived from `Host` — which is why production requires
 * the variable (§10 of DEVELOPER_PORTAL.md).
 *
 * @param {import("node:http").IncomingMessage} req
 * @param {{ origin: string | null, isProduction: boolean }} config
 * @returns {{ allowed: boolean, reason: string }}
 */
export function checkRequestOrigin(req, config) {
  const host = String(req.headers.host ?? "");
  const expected = config.origin ?? (host ? `http://${host}` : null);
  const origin = req.headers.origin;

  if (typeof origin === "string" && origin !== "") {
    if (expected !== null && origin === expected) return { allowed: true, reason: "origin" };
    // Local development may be reached on several loopback spellings while the
    // configured origin names one of them; allow only a same-host match.
    if (!config.isProduction && expected !== null) {
      try {
        if (new URL(origin).host === new URL(expected).host) return { allowed: true, reason: "origin-host" };
      } catch {
        /* fall through to rejection */
      }
    }
    return { allowed: false, reason: "origin-mismatch" };
  }

  const site = req.headers["sec-fetch-site"];
  if (site === "same-origin" || site === "none") return { allowed: true, reason: "sec-fetch-site" };
  if (site === "cross-site" || site === "same-site") return { allowed: false, reason: "sec-fetch-site" };

  // No Origin and no Sec-Fetch-Site: a non-browser client. Requiring a Host that
  // matches the configured origin keeps this from being a universal bypass.
  if (expected !== null && host !== "" && new URL(expected).host === host) {
    return { allowed: true, reason: "host" };
  }
  return { allowed: false, reason: "no-origin" };
}

/**
 * Resolves a URL path to a file inside `root`, or `null`.
 *
 * Rejects traversal (`..`), encoded traversal, null bytes and absolute paths;
 * every candidate is resolved and then required to stay inside `root`.
 *
 * @param {string} root
 * @param {string} urlPath
 * @returns {string | null}
 */
export function resolveStaticPath(root, urlPath) {
  let decoded;
  try {
    decoded = decodeURIComponent(urlPath);
  } catch {
    return null;
  }
  if (decoded.includes("\0")) return null;
  if (!decoded.startsWith("/")) return null;

  const relative = path.posix.normalize(decoded).replace(/^\/+/, "");
  if (relative === "" || relative.startsWith("..")) return null;

  const full = path.resolve(root, relative);
  const rootWithSep = path.resolve(root) + path.sep;
  if (full !== path.resolve(root) && !full.startsWith(rootWithSep)) return null;
  return full;
}

/**
 * Serves a file with a sensible content type and cache policy.
 *
 * Hashed build assets are immutable. Phase 17 corrected the detection: Vite
 * emits `index-BbfPWENj.js`, i.e. `name-HASH.ext`, and the previous pattern
 * expected a dot before the hash, so every asset was sent `no-cache` and would
 * have revalidated on every request behind a CDN.
 *
 * @param {import("node:http").ServerResponse} res
 * @param {string} filePath
 * @param {Record<string, string>} [extraHeaders]
 * @returns {Promise<boolean>} whether the file was served
 */
export async function serveFile(res, filePath, extraHeaders = {}) {
  let handle;
  try {
    handle = await fs.open(filePath, "r");
  } catch {
    return false;
  }
  try {
    const stat = await handle.stat();
    if (!stat.isFile()) {
      await handle.close();
      return false;
    }
    const type = MIME_TYPES.get(path.extname(filePath).toLowerCase()) ?? "application/octet-stream";
    // `assets/<name>-<hash>.<ext>` — the hash is at least six characters and is
    // separated from the name by `-` or `.`, which covers Vite's output and the
    // plain `name.hash.ext` form.
    const immutable =
      /(^|\/)assets\//.test(filePath) && /[-.][0-9A-Za-z_-]{6,}\.(js|css|woff2|png|svg|webp)$/.test(filePath);
    res.writeHead(200, {
      ...BASE_HEADERS,
      ...extraHeaders,
      "Content-Type": type,
      "Content-Length": stat.size,
      "Cache-Control": immutable ? "public, max-age=31536000, immutable" : "no-cache",
    });
    const stream = handle.createReadStream();
    await new Promise((resolve, reject) => {
      stream.on("error", reject);
      res.on("close", resolve);
      stream.pipe(res);
    });
    return true;
  } catch {
    return false;
  } finally {
    await handle.close().catch(() => {});
  }
}
