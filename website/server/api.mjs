/**
 * Developer Portal — HTTP API.
 *
 * Every handler here follows the same order: method → configuration →
 * origin/CSRF → authentication → authorisation → validation → work. Nothing is
 * trusted from the request body except the two credential fields on sign-in.
 *
 * What is deliberately absent:
 * - no registration, invitation, or account-creation endpoint;
 * - no endpoint that reads a role, account id, or "isAdmin" flag from the
 *   client;
 * - no CORS headers, so a cross-origin document cannot read a response;
 * - no route that returns another user's data.
 */

import fs from "node:fs/promises";
import path from "node:path";

import {
  CSRF_HEADER,
  SESSION_COOKIE,
  dummyVerify,
  generateCsrfToken,
  generateSessionToken,
  hashToken,
  isValidEmailShape,
  isValidSessionToken,
  normalizeEmail,
  safeEqualHex,
  verifyPassword,
} from "./auth.mjs";
import {
  checkRequestOrigin,
  clientAddress,
  parseCookies,
  readJsonBody,
  sendError,
  sendJson,
  serializeCookie,
} from "./http.mjs";
import { StoreUnavailableError } from "./store.mjs";

/** One generic message for every credential failure — no account enumeration. */
const INVALID_CREDENTIALS = "Email or password is incorrect.";

/** One generic message for an expired, revoked, or never-issued session. */
const NOT_AUTHENTICATED = "Not authenticated.";

/**
 * @typedef {object} ApiContext
 * @property {import("./config.mjs").resolveConfig extends (...a: any[]) => infer R ? R : never} config
 * @property {import("./store.mjs").Store | null} store
 * @property {import("./rate-limit.mjs").RateLimiter} loginLimiter
 * @property {string | null} storeProblem
 */

/** Public, unauthenticated facts. Reveals no identity and no secret. */
function publicStatus(context) {
  return {
    service: "nexora-developer-portal",
    available: context.store !== null,
    ownerProvisioned: context.store?.provisioned === true,
    environment: context.config.environment,
  };
}

/**
 * Resolves the session cookie to a live session and its owner.
 *
 * Both expiries are enforced here on *every* call — the absolute lifetime and
 * the sliding idle window — because a stored `expiresAt` is only a fallback if
 * the request path forgets to check it.
 *
 * @returns {Promise<{ session: any, user: any } | { error: "missing" | "invalid" | "expired" | "forbidden" }>}
 */
async function resolveSession(req, context) {
  const cookies = parseCookies(req.headers.cookie);
  const raw = cookies[SESSION_COOKIE];
  if (!raw) return { error: "missing" };
  if (!isValidSessionToken(raw)) return { error: "invalid" };

  const store = context.store;
  if (!store) return { error: "invalid" };

  const session = store.getSessionByTokenHash(hashToken(raw));
  if (!session) return { error: "invalid" };

  const now = Date.now();
  const absolute = Date.parse(session.expiresAt);
  if (!Number.isFinite(absolute) || absolute <= now) {
    await store.dropSession(session.id);
    return { error: "expired" };
  }

  const user = store.getUserById(session.userId);
  if (!user || user.status !== "active") {
    await store.dropSession(session.id);
    return { error: "invalid" };
  }
  if (user.role !== "owner") {
    // A future non-owner account authenticates but is not authorised here.
    return { error: "forbidden" };
  }

  // Slide the idle window, but never past the hard cap recorded at creation.
  await store.touchSession(session.id, {
    now: new Date(now),
    idleSeconds: context.config.sessionIdleSeconds,
    absoluteExpiresAt: session.createdAt
      ? new Date(Date.parse(session.createdAt) + context.config.sessionTtlSeconds * 1000).toISOString()
      : session.expiresAt,
  });

  return { session, user };
}

/** Identity shape sent to the client. No hash, no email of anyone else. */
function publicUser(user) {
  return {
    id: user.id,
    email: user.email,
    displayName: user.displayName,
    role: user.role,
    lastSignInAt: user.lastSignInAt ?? null,
    createdAt: user.createdAt,
  };
}

function cookieOptions(config, maxAgeSeconds) {
  return {
    maxAgeSeconds,
    secure: config.secureCookies,
    httpOnly: true,
    sameSite: /** @type {const} */ ("Strict"),
    path: "/",
  };
}

/** Clears the session cookie (expired or signed out). */
function clearSessionCookie(config) {
  return serializeCookie(SESSION_COOKIE, "", { ...cookieOptions(config, 0), secure: config.secureCookies });
}

/**
 * `POST /api/portal/session` — sign in.
 *
 * The order matters: origin, then rate limit, then credentials. A request with
 * a foreign origin never reaches the credential check, so it cannot be used to
 * burn an account's rate-limit budget.
 */
async function handleSignIn(req, res, context) {
  const origin = checkRequestOrigin(req, context.config);
  if (!origin.allowed) {
    return sendError(res, 403, "origin_not_allowed", "Request origin is not allowed.", {
      Vary: "Origin",
    });
  }

  const body = await readJsonBody(req);
  if (!body.ok) return sendError(res, body.status, body.code, body.message);

  const emailRaw = typeof body.value.email === "string" ? body.value.email : "";
  const password = typeof body.value.password === "string" ? body.value.password : "";
  const email = normalizeEmail(emailRaw);

  if (email === "" || password === "") {
    return sendError(res, 400, "validation_failed", "Email and password are required.");
  }
  if (email.length > 254 || password.length > 256) {
    // Do not run the KDF on an absurd input; same generic response as a normal
    // failure so this cannot be used to probe.
    return sendError(res, 401, "invalid_credentials", INVALID_CREDENTIALS);
  }
  if (!isValidEmailShape(email)) {
    return sendError(res, 401, "invalid_credentials", INVALID_CREDENTIALS);
  }

  const ip = clientAddress(req, context.config);
  const ipKey = `ip:${ip}`;
  const accountKey = `account:${email}`;

  const limit = context.loginLimiter.check(ipKey, ["ip", "ipHourly"]);
  const accountLimit = context.loginLimiter.check(accountKey, ["account"]);
  if (!limit.allowed || !accountLimit.allowed) {
    const retryAfter = Math.max(limit.retryAfterSeconds, accountLimit.retryAfterSeconds, 1);
    await context.store?.recordAudit("sign_in_rate_limited", null);
    return sendError(res, 429, "rate_limited", "Too many sign-in attempts. Try again later.", {
      "Retry-After": String(retryAfter),
    });
  }

  const store = context.store;
  const user = store?.getUserByEmail(email) ?? null;

  // Unknown account: run a throwaway verification so timing does not separate
  // "no such account" from "wrong password", then fail identically.
  const passwordOk = user
    ? await verifyPassword(password, user.passwordHash)
    : await dummyVerify(password);

  if (!user || !passwordOk || user.status !== "active") {
    context.loginLimiter.record(ipKey, ["ip", "ipHourly"]);
    context.loginLimiter.record(accountKey, ["account"]);
    await store?.recordAudit("sign_in_failure", user?.id ?? null);
    return sendError(res, 401, "invalid_credentials", INVALID_CREDENTIALS);
  }

  // Authenticated. Rotate: any older session for this account is revoked, so a
  // stolen cookie stops working the moment the real owner signs in.
  await store.dropSessionsForUser(user.id);

  const rawToken = generateSessionToken();
  const csrfToken = generateCsrfToken();
  const expiresAt = new Date(Date.now() + context.config.sessionTtlSeconds * 1000).toISOString();

  await store.createSession({
    userId: user.id,
    tokenHash: hashToken(rawToken),
    csrfToken,
    expiresAt,
  });
  await store.recordSignIn(user.id);
  context.loginLimiter.reset(accountKey);

  return sendJson(
    res,
    200,
    {
      authenticated: true,
      user: publicUser(user),
      csrfToken,
      expiresAt,
    },
    {
      "Set-Cookie": serializeCookie(
        SESSION_COOKIE,
        rawToken,
        cookieOptions(context.config, context.config.sessionTtlSeconds),
      ),
    },
  );
}

/** `GET /api/portal/session` — is this cookie a live session? */
async function handleGetSession(req, res, context) {
  const resolved = await resolveSession(req, context);
  if ("error" in resolved) {
    const clearing = resolved.error === "expired" || resolved.error === "invalid";
    return sendJson(
      res,
      401,
      { authenticated: false, error: { code: "not_authenticated", message: NOT_AUTHENTICATED } },
      clearing ? { "Set-Cookie": clearSessionCookie(context.config) } : {},
    );
  }
  return sendJson(res, 200, {
    authenticated: true,
    user: publicUser(resolved.user),
    csrfToken: resolved.session.csrfToken,
    expiresAt: resolved.session.expiresAt,
  });
}

/** `DELETE /api/portal/session` — sign out, server-side. */
async function handleSignOut(req, res, context) {
  const origin = checkRequestOrigin(req, context.config);
  if (!origin.allowed) {
    return sendError(res, 403, "origin_not_allowed", "Request origin is not allowed.", { Vary: "Origin" });
  }

  const resolved = await resolveSession(req, context);
  if ("error" in resolved) {
    return sendJson(
      res,
      200,
      { authenticated: false },
      { "Set-Cookie": clearSessionCookie(context.config) },
    );
  }

  // CSRF: the double-submit token must match this session's token.
  const supplied = req.headers[CSRF_HEADER];
  const token = typeof supplied === "string" ? supplied : "";
  if (!safeEqualHex(hashToken(token), hashToken(resolved.session.csrfToken))) {
    return sendError(res, 403, "csrf_failed", "Missing or invalid CSRF token.");
  }

  await context.store?.dropSession(resolved.session.id);
  await context.store?.recordAudit("sign_out", resolved.user.id);
  return sendJson(
    res,
    200,
    { authenticated: false },
    { "Set-Cookie": clearSessionCookie(context.config) },
  );
}

/**
 * Wraps an owner-only read handler: resolves the session, requires the cookie
 * *and* the CSRF header, and only then runs the handler.
 *
 * The CSRF token is required on reads too. Nothing here mutates state, but a
 * dashboard response is private data; requiring the header means a cross-site
 * `<script src>` or `fetch` cannot read it even if a future change relaxed the
 * cookie's `SameSite` attribute.
 */
function ownerHandler(handler) {
  return async (req, res, context) => {
    const origin = checkRequestOrigin(req, context.config);
    if (!origin.allowed) {
      return sendError(res, 403, "origin_not_allowed", "Request origin is not allowed.", { Vary: "Origin" });
    }
    const resolved = await resolveSession(req, context);
    if ("error" in resolved) {
      const status = resolved.error === "forbidden" ? 403 : 401;
      const code = resolved.error === "forbidden" ? "forbidden" : "not_authenticated";
      return sendError(res, status, code, resolved.error === "forbidden" ? "Not authorised." : NOT_AUTHENTICATED);
    }
    const supplied = req.headers[CSRF_HEADER];
    const token = typeof supplied === "string" ? supplied : "";
    if (!safeEqualHex(hashToken(token), hashToken(resolved.session.csrfToken))) {
      return sendError(res, 403, "csrf_failed", "Missing or invalid CSRF token.");
    }
    return handler(req, res, context, resolved);
  };
}

/* ─────────────────────────── protected data ─────────────────────────── */

async function readFileIfPresent(filePath) {
  try {
    const stat = await fs.stat(filePath);
    return stat.isFile() ? stat : null;
  } catch {
    return null;
  }
}

/** Portal and environment facts. Presence, never values. */
async function overviewHandler(req, res, context, { user, session }) {
  const buildFile = await readFileIfPresent(path.join(context.config.staticDir, "index.html"));
  return sendJson(res, 200, {
    portal: {
      service: "Nexora Developer Portal",
      environment: context.config.environment,
      sessionExpiresAt: session.expiresAt,
      signedInAt: session.createdAt,
      recentEvents: context.store?.recentAudit(6) ?? [],
    },
    account: {
      ...publicUser(user),
      roleLabel: "Owner / Super Admin",
    },
    readiness: {
      // Everything below is a configuration fact read from the server, not a
      // measurement. Nothing here is invented.
      ownerProvisioned: context.store?.provisioned === true,
      portalOriginConfigured: context.config.origin !== null,
      secureCookies: context.config.secureCookies,
      staticBuildPresent: buildFile !== null,
      staticBuildBytes: buildFile?.size ?? null,
      analyticsConfigured: false,
      contentManagementConfigured: false,
      supportInboxConfigured: false,
    },
  });
}

/**
 * Website status. Every field is derived from the build on disk or from
 * configuration presence — never from a "deployment" claim we cannot verify.
 */
async function websiteHandler(req, res, context) {
  const staticDir = context.config.staticDir;
  const indexFile = await readFileIfPresent(path.join(staticDir, "index.html"));
  const robotsFile = await readFileIfPresent(path.join(staticDir, "robots.txt"));
  const sitemapFile = await readFileIfPresent(path.join(staticDir, "sitemap.xml"));

  return sendJson(res, 200, {
    build: {
      // The site is built (dist exists) — this says nothing about deployment.
      present: indexFile !== null,
      bytes: indexFile?.size ?? null,
      builtAt: indexFile ? indexFile.mtime.toISOString() : null,
    },
    indexing: {
      robotsPresent: robotsFile !== null,
      sitemapEmitted: sitemapFile !== null,
      productionOriginConfigured: context.config.isProduction && context.config.origin !== null,
    },
    release: {
      // Read from the filesystem, not from a promise: there is no installer.
      installerHosted: (await readFileIfPresent(path.join(staticDir, "downloads", "Nexora-setup.exe"))) !== null,
    },
    deployment: {
      // The portal cannot verify a deployment, so it does not claim one.
      verified: false,
      note: "No deployment integration is configured. A running local server is not a deployment.",
    },
  });
}

/** Content management: honestly unconfigured — there is no CMS backend. */
async function contentHandler(_req, res) {
  return sendJson(res, 200, {
    configured: false,
    reason:
      "The website is a static build with no content API. Editing prose from a browser would require a review and publish pipeline that does not exist.",
    editableSurfaces: [],
  });
}

/** Support: honestly unconfigured — there is no ticket store. */
async function supportHandler(_req, res) {
  return sendJson(res, 200, {
    configured: false,
    reason:
      "No support inbox or ticketing backend is connected to this portal. The public Support page explains what users can check themselves.",
    openRequests: null,
  });
}

/* ────────────────────────────── routing ─────────────────────────────── */

/**
 * Routes the API. Returns `true` when the request was handled.
 *
 * @param {import("node:http").IncomingMessage} req
 * @param {import("node:http").ServerResponse} res
 * @param {ApiContext} context
 * @returns {Promise<boolean>}
 */
export async function handleApi(req, res, context) {
  const url = new URL(req.url ?? "/", "http://portal.local");
  const pathname = url.pathname.replace(/\/+$/, "") || "/";
  const rawMethod = (req.method ?? "GET").toUpperCase();
  // HEAD is answered like GET: Node suppresses the body itself, and a deployment
  // health check or uptime probe that uses HEAD must not see a 404.
  const method = rawMethod === "HEAD" ? "GET" : rawMethod;

  // No CORS: a preflight is refused rather than answered, so no response ever
  // grants a cross-origin document access to this API.
  if (method === "OPTIONS") {
    sendError(res, 405, "method_not_allowed", "Cross-origin requests are not supported.");
    return true;
  }

  try {
    // A misconfiguration answers honestly instead of degrading into a portal
    // that looks configured but cannot authenticate anyone.
    if (context.config.fatal.length > 0) {
      sendError(res, 503, "portal_unavailable", "Portal configuration is incomplete.");
      return true;
    }

    if (pathname === "/api/portal/status" && method === "GET") {
      sendJson(res, 200, publicStatus(context));
      return true;
    }

    // Everything below needs storage. A missing or corrupt store is a hard 503
    // — never a fallback to "allow".
    if (context.store === null) {
      sendError(res, 503, "portal_unavailable", "Portal storage is not available.");
      return true;
    }

    if (pathname === "/api/portal/session") {
      if (method === "POST") {
        await handleSignIn(req, res, context);
        return true;
      }
      if (method === "GET") {
        await handleGetSession(req, res, context);
        return true;
      }
      if (method === "DELETE") {
        await handleSignOut(req, res, context);
        return true;
      }
      sendError(res, 405, "method_not_allowed", "Method not allowed.");
      return true;
    }

    /** @type {Record<string, (rq: any, rs: any, cx: any, session: any) => Promise<void>>} */
    const protectedRoutes = {
      "/api/portal/overview": overviewHandler,
      "/api/portal/website": websiteHandler,
      "/api/portal/content": contentHandler,
      "/api/portal/support": supportHandler,
    };

    const route = protectedRoutes[pathname];
    if (route) {
      if (method !== "GET") {
        sendError(res, 405, "method_not_allowed", "Method not allowed.");
        return true;
      }
      await ownerHandler(route)(req, res, context);
      return true;
    }

    sendError(res, 404, "not_found", "No such API route.");
    return true;
  } catch (error) {
    if (error instanceof StoreUnavailableError) {
      sendError(res, 503, "portal_unavailable", "Portal storage is not available.");
      return true;
    }
    // Never leak an internal message, path, or stack to the client.
    console.error("[portal] unhandled API error:", error?.name ?? "Error");
    sendError(res, 500, "internal_error", "The portal could not complete the request.");
    return true;
  }
}

export { publicStatus };
