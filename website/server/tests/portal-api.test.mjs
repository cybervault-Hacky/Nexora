/**
 * Integration tests for the Developer Portal.
 *
 * Each test starts a real portal server on an ephemeral port with its own
 * temporary data directory, provisions a test owner, and talks to it over HTTP
 * with `fetch`. Nothing is mocked: the cookie, the CSRF check, the rate limiter,
 * and the store are the production code paths.
 *
 * Run with `pnpm test:portal`.
 *
 * What these tests do **not** cover, and which is therefore not claimed:
 * a real identity provider (there is none), TLS termination, a reverse proxy,
 * multiple server instances sharing a store, or any browser behaviour (the
 * `puppeteer` UI pass is described in the portal report, not here).
 */

import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { test } from "node:test";

import { hashPassword } from "../auth.mjs";
import { resolveConfig } from "../config.mjs";
import { createPortalServer } from "../index.mjs";

const WEBSITE_DIR = path.resolve(import.meta.dirname, "../..");
const STATIC_DIR = path.join(WEBSITE_DIR, "dist");
const OWNER_EMAIL = "owner@example.com";
const OWNER_PASSWORD = "test-only-password-1234";

/* ─────────────────────────────── harness ─────────────────────────────── */

/**
 * Starts a portal server for one test.
 *
 * @param {{ owner?: boolean, env?: Record<string, string>, overrides?: Record<string, unknown> }} [options]
 */
async function startPortal(options = {}) {
  const dataDir = await fs.mkdtemp(path.join(os.tmpdir(), "nexora-portal-"));
  const base = resolveConfig(
    {
      NEXORA_PORTAL_DATA_DIR: dataDir,
      NEXORA_PORTAL_STATIC_DIR: STATIC_DIR,
      ...(options.env ?? {}),
    },
    { cwd: WEBSITE_DIR },
  );
  const config = { ...base, ...(options.overrides ?? {}) };

  const { server, context } = await createPortalServer(config);
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const baseUrl = `http://127.0.0.1:${server.address().port}`;

  if (options.owner === true) {
    await context.store.createOwner({
      email: OWNER_EMAIL,
      displayName: "Test Owner",
      passwordHash: await hashPassword(OWNER_PASSWORD),
    });
  }

  return {
    baseUrl,
    context,
    dataDir,
    storeFile: path.join(dataDir, "portal.json"),
    async close() {
      await new Promise((resolve) => server.close(resolve));
      await fs.rm(dataDir, { recursive: true, force: true });
    },
  };
}

/** The `name=value` half of the session cookie, or `null`. */
function sessionCookie(response) {
  const cookies = response.headers.getSetCookie?.() ?? [];
  const session = cookies.find((cookie) => cookie.startsWith("nexora_portal_session="));
  if (!session) return null;
  return {
    header: session.split(";")[0],
    raw: session,
  };
}

async function signIn(portal, password = OWNER_PASSWORD, extra = {}) {
  const response = await fetch(`${portal.baseUrl}/api/portal/session`, {
    method: "POST",
    headers: { "Content-Type": "application/json", Origin: portal.baseUrl },
    body: JSON.stringify({ email: OWNER_EMAIL, password, ...extra }),
  });
  const body = await response.json().catch(() => null);
  const cookie = sessionCookie(response);
  return { response, body, cookie };
}

/** Authenticated GET with both the cookie and the CSRF header. */
async function authedGet(portal, pathName, session, init = {}) {
  return await fetch(`${portal.baseUrl}${pathName}`, {
    headers: {
      Cookie: session.cookie.header,
      "X-Portal-CSRF": session.body.csrfToken,
      Origin: portal.baseUrl,
      ...(init.headers ?? {}),
    },
    ...init,
  });
}

/* ───────────────────────── public website surface ────────────────────── */

test("the nine public routes, the fallback, and the admin shell are all served", async () => {
  const portal = await startPortal();
  try {
    const routes = [
      "/",
      "/features",
      "/how-it-works",
      "/download",
      "/privacy",
      "/security",
      "/about",
      "/faq",
      "/support",
      "/no-such-page",
      "/admin",
      "/admin/website",
    ];
    for (const route of routes) {
      const response = await fetch(portal.baseUrl + route);
      assert.equal(response.status, 200, `${route} should be 200`);
      assert.match(response.headers.get("content-type") ?? "", /text\/html/);
      const html = await response.text();
      assert.ok(html.includes("<div id=\"root\">"), `${route} should serve the app shell`);
    }
  } finally {
    await portal.close();
  }
});

test("crawl files keep the Phase 16 behaviour: robots.txt, and an honest sitemap 404", async () => {
  const portal = await startPortal();
  try {
    const robots = await fetch(`${portal.baseUrl}/robots.txt`);
    assert.equal(robots.status, 200);
    assert.match(robots.headers.get("content-type") ?? "", /text\/plain/);
    const robotsBody = await robots.text();
    assert.match(robotsBody, /User-agent: \*/);
    assert.match(robotsBody, /Disallow: \//);
    assert.ok(!/^Sitemap:/m.test(robotsBody));

    const sitemap = await fetch(`${portal.baseUrl}/sitemap.xml`);
    assert.equal(sitemap.status, 404);
    assert.match(sitemap.headers.get("content-type") ?? "", /text\/plain/);
    assert.match(await sitemap.text(), /no confirmed production domain/i);
  } finally {
    await portal.close();
  }
});

test("the built site contains no sitemap and no portal route", async () => {
  // The portal cannot export a sitemap: this asserts the artefact the crawler
  // would read has no private path in it, in the default (unconfigured) build.
  const files = await fs.readdir(STATIC_DIR);
  assert.ok(!files.includes("sitemap.xml"), "no sitemap is emitted without a production origin");
  const indexHtml = await fs.readFile(path.join(STATIC_DIR, "index.html"), "utf8");
  assert.ok(!indexHtml.includes("/admin"), "the shell must not name a portal route");
});

/* ──────────────────────────── authenticated API ──────────────────────── */

test("status reports provisioning without revealing an account", async () => {
  const portal = await startPortal();
  try {
    const before = await (await fetch(`${portal.baseUrl}/api/portal/status`)).json();
    assert.equal(before.available, true);
    assert.equal(before.ownerProvisioned, false);
    assert.equal(JSON.stringify(before).includes("@"), false, "status must not contain an email");
  } finally {
    await portal.close();
  }

  const provisioned = await startPortal({ owner: true });
  try {
    const after = await (await fetch(`${provisioned.baseUrl}/api/portal/status`)).json();
    assert.equal(after.ownerProvisioned, true);
    assert.deepEqual(Object.keys(after).sort(), ["available", "environment", "ownerProvisioned", "service"]);
  } finally {
    await provisioned.close();
  }
});

test("sign-in succeeds with the provisioned credentials and issues an HttpOnly cookie", async () => {
  const portal = await startPortal({ owner: true });
  try {
    const { response, body, cookie } = await signIn(portal);
    assert.equal(response.status, 200);
    assert.equal(body.authenticated, true);
    assert.equal(body.user.email, OWNER_EMAIL);
    assert.equal(body.user.role, "owner");
    assert.ok(typeof body.csrfToken === "string" && body.csrfToken.length >= 32);

    assert.ok(cookie, "a session cookie must be set");
    assert.match(cookie.raw, /HttpOnly/);
    assert.match(cookie.raw, /SameSite=Strict/);
    assert.match(cookie.raw, /Path=\//);
    assert.ok(!/Secure/.test(cookie.raw), "development does not set Secure (the preview is plain http)");

    // The body must contain no credential material.
    const serialised = JSON.stringify(body);
    assert.ok(!serialised.includes(OWNER_PASSWORD));
    assert.ok(!serialised.includes("scrypt$"));
    assert.ok(!serialised.includes(cookie.header.split("=")[1]), "the raw session token stays in the cookie");
  } finally {
    await portal.close();
  }
});

test("invalid credentials are one generic failure, identical for an unknown account", async () => {
  const portal = await startPortal({ owner: true });
  try {
    const wrongPassword = await signIn(portal, "definitely-not-the-password");
    const unknownAccount = await fetch(`${portal.baseUrl}/api/portal/session`, {
      method: "POST",
      headers: { "Content-Type": "application/json", Origin: portal.baseUrl },
      body: JSON.stringify({ email: "nobody@example.com", password: "definitely-not-the-password" }),
    });

    assert.equal(wrongPassword.response.status, 401);
    assert.equal(unknownAccount.status, 401);
    assert.equal(wrongPassword.body.error.code, "invalid_credentials");
    assert.equal((await unknownAccount.json()).error.code, "invalid_credentials");

    // The two responses must be byte-identical: any difference is an oracle.
    const unknownBody = JSON.stringify({
      error: { code: "invalid_credentials", message: "Email or password is incorrect." },
    });
    assert.equal(JSON.stringify(wrongPassword.body), unknownBody);
    assert.equal(wrongPassword.cookie, null, "a failed sign-in must not set a cookie");
  } finally {
    await portal.close();
  }
});

test("protected data needs the cookie and the CSRF token, and refresh keeps the session", async () => {
  const portal = await startPortal({ owner: true });
  try {
    const anonymous = await fetch(`${portal.baseUrl}/api/portal/overview`);
    assert.equal(anonymous.status, 401);

    const session = await signIn(portal);

    // Cookie only: rejected, because a cross-site request could carry it.
    const withoutCsrf = await fetch(`${portal.baseUrl}/api/portal/overview`, {
      headers: { Cookie: session.cookie.header, Origin: portal.baseUrl },
    });
    assert.equal(withoutCsrf.status, 403);
    assert.equal((await withoutCsrf.json()).error.code, "csrf_failed");

    // Wrong CSRF token: rejected.
    const wrongCsrf = await fetch(`${portal.baseUrl}/api/portal/overview`, {
      headers: { Cookie: session.cookie.header, Origin: portal.baseUrl, "X-Portal-CSRF": "a".repeat(48) },
    });
    assert.equal(wrongCsrf.status, 403);

    // Cookie + token: the data.
    const allowed = await authedGet(portal, "/api/portal/overview", session);
    assert.equal(allowed.status, 200);
    const overview = await allowed.json();
    assert.equal(overview.account.email, OWNER_EMAIL);
    assert.equal(overview.account.role, "owner");
    assert.equal(overview.readiness.ownerProvisioned, true);
    assert.equal(overview.readiness.analyticsConfigured, false);
    assert.match(allowed.headers.get("cache-control") ?? "", /no-store/);
    assert.equal(allowed.headers.get("access-control-allow-origin"), null);

    // A "refresh" is just another request with the same cookie: the session is
    // server-held, so the client cannot invent one.
    const refreshed = await fetch(`${portal.baseUrl}/api/portal/session`, {
      headers: { Cookie: session.cookie.header, Origin: portal.baseUrl },
    });
    assert.equal(refreshed.status, 200);
    assert.equal((await refreshed.json()).user.email, OWNER_EMAIL);
  } finally {
    await portal.close();
  }
});

test("every privileged endpoint requires authorisation", async () => {
  const portal = await startPortal({ owner: true });
  try {
    const endpoints = ["/api/portal/overview", "/api/portal/website", "/api/portal/content", "/api/portal/support"];
    for (const endpoint of endpoints) {
      const anonymous = await fetch(portal.baseUrl + endpoint);
      assert.equal(anonymous.status, 401, `${endpoint} must reject an anonymous request`);
    }

    const session = await signIn(portal);
    for (const endpoint of endpoints) {
      const response = await authedGet(portal, endpoint, session);
      assert.equal(response.status, 200, `${endpoint} should answer the owner`);
    }

    const content = await (await authedGet(portal, "/api/portal/content", session)).json();
    assert.equal(content.configured, false);
    const support = await (await authedGet(portal, "/api/portal/support", session)).json();
    assert.equal(support.configured, false);
    assert.equal(support.openRequests, null);
  } finally {
    await portal.close();
  }
});

test("a forged identity, role, or cookie never grants access", async () => {
  const portal = await startPortal({ owner: true });
  try {
    // Claiming a role at sign-in is ignored: the account's role comes from the store.
    const forgedSignIn = await signIn(portal, OWNER_PASSWORD, {
      role: "owner",
      isAdmin: true,
      userId: "00000000-0000-0000-0000-000000000000",
    });
    assert.equal(forgedSignIn.response.status, 200);
    assert.equal(forgedSignIn.body.user.role, "owner");

    // Claiming a role without a session is rejected outright.
    const forgedRequest = await fetch(`${portal.baseUrl}/api/portal/overview`, {
      headers: {
        Origin: portal.baseUrl,
        "X-Portal-Role": "owner",
        "X-Portal-User-Id": "00000000-0000-0000-0000-000000000000",
        "X-Portal-CSRF": "b".repeat(48),
      },
    });
    assert.equal(forgedRequest.status, 401);

    // A made-up cookie value is not a session.
    const fakeCookie = await fetch(`${portal.baseUrl}/api/portal/overview`, {
      headers: {
        Cookie: `nexora_portal_session=${"c".repeat(64)}`,
        Origin: portal.baseUrl,
        "X-Portal-CSRF": "b".repeat(48),
      },
    });
    assert.equal(fakeCookie.status, 401);

    // Query parameters cannot change identity either.
    const queryAttempt = await authedGet(portal, "/api/portal/overview?role=owner&userId=1", forgedSignIn);
    assert.equal(queryAttempt.status, 200);
    assert.equal((await queryAttempt.json()).account.email, OWNER_EMAIL);
  } finally {
    await portal.close();
  }
});

test("sign-out revokes the session server-side and clears the cookie", async () => {
  const portal = await startPortal({ owner: true });
  try {
    const session = await signIn(portal);

    const withoutCsrf = await fetch(`${portal.baseUrl}/api/portal/session`, {
      method: "DELETE",
      headers: { Cookie: session.cookie.header, Origin: portal.baseUrl },
    });
    assert.equal(withoutCsrf.status, 403);

    const signedOut = await fetch(`${portal.baseUrl}/api/portal/session`, {
      method: "DELETE",
      headers: {
        Cookie: session.cookie.header,
        Origin: portal.baseUrl,
        "X-Portal-CSRF": session.body.csrfToken,
      },
    });
    assert.equal(signedOut.status, 200);
    const cleared = sessionCookie(signedOut);
    assert.match(cleared.raw, /Max-Age=0/);

    // The old cookie no longer works, even replayed manually.
    const replayed = await fetch(`${portal.baseUrl}/api/portal/overview`, {
      headers: {
        Cookie: session.cookie.header,
        Origin: portal.baseUrl,
        "X-Portal-CSRF": session.body.csrfToken,
      },
    });
    assert.equal(replayed.status, 401);
  } finally {
    await portal.close();
  }
});

test("a second sign-in rotates the session and invalidates the first cookie", async () => {
  const portal = await startPortal({ owner: true });
  try {
    const first = await signIn(portal);
    assert.equal((await authedGet(portal, "/api/portal/overview", first)).status, 200);

    const second = await signIn(portal);
    assert.equal(second.response.status, 200);
    assert.notEqual(first.body.csrfToken, second.body.csrfToken);

    const stale = await fetch(`${portal.baseUrl}/api/portal/overview`, {
      headers: {
        Cookie: first.cookie.header,
        Origin: portal.baseUrl,
        "X-Portal-CSRF": first.body.csrfToken,
      },
    });
    assert.equal(stale.status, 401, "the previous session must be revoked by rotation");

    assert.equal((await authedGet(portal, "/api/portal/overview", second)).status, 200);
  } finally {
    await portal.close();
  }
});

test("an expired session stops working, idle or absolute", async () => {
  const idle = await startPortal({ owner: true, overrides: { sessionIdleSeconds: 1 } });
  try {
    const session = await signIn(idle);
    assert.equal((await authedGet(idle, "/api/portal/overview", session)).status, 200);
    await new Promise((resolve) => setTimeout(resolve, 1200));
    const expired = await authedGet(idle, "/api/portal/overview", session);
    assert.equal(expired.status, 401, "the idle window must expire the session");
  } finally {
    await idle.close();
  }

  const absolute = await startPortal({ owner: true, overrides: { sessionTtlSeconds: 1, sessionIdleSeconds: 60 } });
  try {
    const session = await signIn(absolute);
    await new Promise((resolve) => setTimeout(resolve, 1200));
    assert.equal((await authedGet(absolute, "/api/portal/overview", session)).status, 401);
  } finally {
    await absolute.close();
  }
});

/* ───────────────────────────── abuse protection ──────────────────────── */

test("repeated failures are rate limited with a generic 429", async () => {
  const portal = await startPortal({ owner: true });
  try {
    for (let attempt = 0; attempt < 5; attempt += 1) {
      const failure = await signIn(portal, `wrong-password-${attempt}`);
      assert.equal(failure.response.status, 401);
    }

    const blocked = await signIn(portal, "wrong-password-again");
    assert.equal(blocked.response.status, 429);
    assert.equal(blocked.body.error.code, "rate_limited");
    assert.ok(Number.parseInt(blocked.response.headers.get("retry-after") ?? "0", 10) >= 1);

    // Even the correct password is refused while the window is closed: the
    // limiter is checked before the credential.
    const correctWhileBlocked = await signIn(portal);
    assert.equal(correctWhileBlocked.response.status, 429);
    assert.equal(correctWhileBlocked.cookie, null);
  } finally {
    await portal.close();
  }
});

test("cross-origin requests are refused before any credential is checked", async () => {
  const portal = await startPortal({ owner: true });
  try {
    const crossOriginSignIn = await fetch(`${portal.baseUrl}/api/portal/session`, {
      method: "POST",
      headers: { "Content-Type": "application/json", Origin: "https://attacker.example" },
      body: JSON.stringify({ email: OWNER_EMAIL, password: OWNER_PASSWORD }),
    });
    assert.equal(crossOriginSignIn.status, 403);
    assert.equal(sessionCookie(crossOriginSignIn), null);
    assert.equal((await crossOriginSignIn.json()).error.code, "origin_not_allowed");

    const session = await signIn(portal);
    const crossOriginRead = await fetch(`${portal.baseUrl}/api/portal/overview`, {
      headers: {
        Cookie: session.cookie.header,
        "X-Portal-CSRF": session.body.csrfToken,
        Origin: "https://attacker.example",
      },
    });
    assert.equal(crossOriginRead.status, 403);

    const crossOriginSignOut = await fetch(`${portal.baseUrl}/api/portal/session`, {
      method: "DELETE",
      headers: {
        Cookie: session.cookie.header,
        "X-Portal-CSRF": session.body.csrfToken,
        Origin: "https://attacker.example",
      },
    });
    assert.equal(crossOriginSignOut.status, 403);

    // The session survives the rejected attempts.
    assert.equal((await authedGet(portal, "/api/portal/overview", session)).status, 200);
  } finally {
    await portal.close();
  }
});

test("no CORS grant is ever issued, and preflights are refused", async () => {
  const portal = await startPortal({ owner: true });
  try {
    const preflight = await fetch(`${portal.baseUrl}/api/portal/session`, {
      method: "OPTIONS",
      headers: { Origin: "https://attacker.example", "Access-Control-Request-Method": "POST" },
    });
    assert.equal(preflight.status, 405);
    for (const header of [
      "access-control-allow-origin",
      "access-control-allow-credentials",
      "access-control-allow-methods",
      "access-control-allow-headers",
    ]) {
      assert.equal(preflight.headers.get(header), null, `${header} must never be sent`);
    }
  } finally {
    await portal.close();
  }
});

test("request bodies and methods are constrained", async () => {
  const portal = await startPortal({ owner: true });
  try {
    const wrongType = await fetch(`${portal.baseUrl}/api/portal/session`, {
      method: "POST",
      headers: { "Content-Type": "text/plain", Origin: portal.baseUrl },
      body: "email=owner@example.com",
    });
    assert.equal(wrongType.status, 415);

    const tooLarge = await fetch(`${portal.baseUrl}/api/portal/session`, {
      method: "POST",
      headers: { "Content-Type": "application/json", Origin: portal.baseUrl },
      body: JSON.stringify({ email: "a".repeat(20_000) }),
    });
    assert.equal(tooLarge.status, 413);

    const notJson = await fetch(`${portal.baseUrl}/api/portal/session`, {
      method: "POST",
      headers: { "Content-Type": "application/json", Origin: portal.baseUrl },
      body: "{not json",
    });
    assert.equal(notJson.status, 400);

    const missingFields = await fetch(`${portal.baseUrl}/api/portal/session`, {
      method: "POST",
      headers: { "Content-Type": "application/json", Origin: portal.baseUrl },
      body: JSON.stringify({}),
    });
    assert.equal(missingFields.status, 400);
    assert.equal((await missingFields.json()).error.code, "validation_failed");

    const session = await signIn(portal);
    const wrongMethod = await authedGet(portal, "/api/portal/overview", session, { method: "POST" });
    assert.equal(wrongMethod.status, 405);

    const unknownRoute = await fetch(`${portal.baseUrl}/api/portal/nope`);
    assert.equal(unknownRoute.status, 404);
  } finally {
    await portal.close();
  }
});

/* ─────────────────────── fail-closed / misconfiguration ──────────────── */

test("an unusable store fails closed with 503 rather than allowing access", async () => {
  const dataDir = await fs.mkdtemp(path.join(os.tmpdir(), "nexora-portal-corrupt-"));
  const brokenDataDir = path.join(dataDir, "portal.json");
  // A directory where the store expects its file: opening it fails with EISDIR.
  await fs.mkdir(brokenDataDir, { recursive: true });

  const portal = await startPortal({ env: { NEXORA_PORTAL_DATA_DIR: dataDir } });
  try {
    assert.equal(portal.context.store, null, "a corrupt store must not open");

    const status = await (await fetch(`${portal.baseUrl}/api/portal/status`)).json();
    assert.equal(status.available, false);
    assert.equal(status.ownerProvisioned, false);

    for (const endpoint of ["/api/portal/session", "/api/portal/overview"]) {
      const response = await fetch(`${portal.baseUrl}${endpoint}`);
      assert.equal(response.status, 503, `${endpoint} must be 503 without a store`);
      assert.equal((await response.json()).error.code, "portal_unavailable");
    }

    // The public website is unaffected by a broken portal store.
    assert.equal((await fetch(`${portal.baseUrl}/`)).status, 200);
  } finally {
    // `startPortal` owns its own temp dir; this one is separate.
    await portal.close();
    await fs.rm(dataDir, { recursive: true, force: true });
  }
});

test("a fatal configuration answers 503 instead of pretending to work", async () => {
  const portal = await startPortal({ owner: true, env: { NEXORA_PORTAL_ENV: "production" } });
  try {
    assert.equal(portal.context.config.usable, false);
    const response = await fetch(`${portal.baseUrl}/api/portal/session`, {
      method: "POST",
      headers: { "Content-Type": "application/json", Origin: portal.baseUrl },
      body: JSON.stringify({ email: OWNER_EMAIL, password: OWNER_PASSWORD }),
    });
    assert.equal(response.status, 503);
    assert.equal((await response.json()).error.code, "portal_unavailable");
    assert.equal(sessionCookie(response), null);
  } finally {
    await portal.close();
  }
});

test("in production the development origin fallback is gone", async () => {
  const portal = await startPortal({
    owner: true,
    env: {
      NEXORA_PORTAL_ENV: "production",
      NEXORA_PORTAL_ORIGIN: "https://portal.example.com",
    },
  });
  try {
    assert.equal(portal.context.config.usable, true);
    // Same host, different scheme: enough in development, refused in production.
    const response = await fetch(`${portal.baseUrl}/api/portal/session`, {
      method: "POST",
      headers: { "Content-Type": "application/json", Origin: portal.baseUrl },
      body: JSON.stringify({ email: OWNER_EMAIL, password: OWNER_PASSWORD }),
    });
    assert.equal(response.status, 403);

    const allowed = await fetch(`${portal.baseUrl}/api/portal/session`, {
      method: "POST",
      headers: { "Content-Type": "application/json", Origin: "https://portal.example.com" },
      body: JSON.stringify({ email: OWNER_EMAIL, password: OWNER_PASSWORD }),
    });
    assert.equal(allowed.status, 200);
    const cookie = sessionCookie(allowed);
    assert.match(cookie.raw, /Secure/, "production cookies must be Secure");
  } finally {
    await portal.close();
  }
});

/* ──────────────────────────── secrets at rest ────────────────────────── */

test("the data file stores only hashes, and never a password or a raw token", async () => {
  const portal = await startPortal({ owner: true });
  try {
    const session = await signIn(portal);
    const rawToken = session.cookie.header.split("=")[1];

    const file = await fs.readFile(portal.storeFile, "utf8");
    assert.ok(!file.includes(OWNER_PASSWORD), "no plaintext password");
    assert.ok(!file.includes(rawToken), "no raw session token");
    assert.match(file, /scrypt\$32768\$8\$1\$/);

    const parsed = JSON.parse(file);
    assert.equal(parsed.users.length, 1);
    assert.equal(parsed.sessions.length, 1);
    assert.ok(!("password" in parsed.users[0]));
    assert.match(parsed.sessions[0].tokenHash, /^[0-9a-f]{64}$/);

    // Audit entries carry an event and an account id, never an address or a token.
    const audit = JSON.stringify(parsed.audit);
    assert.ok(!audit.includes(OWNER_EMAIL));
    assert.ok(!audit.includes(rawToken));
  } finally {
    await portal.close();
  }
});

test("error responses never leak internals, paths, or stack traces", async () => {
  const portal = await startPortal({ owner: true });
  try {
    const failures = await Promise.all([
      fetch(`${portal.baseUrl}/api/portal/overview`),
      fetch(`${portal.baseUrl}/api/portal/nope`),
      fetch(`${portal.baseUrl}/api/portal/session`, {
        method: "POST",
        headers: { "Content-Type": "application/json", Origin: portal.baseUrl },
        body: "{bad",
      }),
    ]);
    for (const response of failures) {
      const body = await response.text();
      assert.ok(!/scrypt\$/.test(body));
      assert.ok(!/\/home\/|\\\\|node_modules/.test(body), `no filesystem path in: ${body}`);
      assert.ok(!/\bat .*\.mjs:\d+/.test(body), `no stack trace in: ${body}`);
      assert.ok(!/passwordHash|tokenHash|csrfToken/.test(body));
    }
  } finally {
    await portal.close();
  }
});

/* ───────────────────────────── open redirects ────────────────────────── */

test("the portal issues no redirects and accepts no destination parameter", async () => {
  const portal = await startPortal({ owner: true });
  try {
    const attempts = [
      "/api/portal/session?next=https://attacker.example",
      "/api/portal/session?redirect=//attacker.example",
      "/admin?next=https://attacker.example",
      "/admin/website?returnTo=https://attacker.example",
    ];
    for (const target of attempts) {
      const response = await fetch(portal.baseUrl + target, { redirect: "manual" });
      assert.ok(response.status < 300 || response.status >= 400, `${target} must not redirect`);
      assert.equal(response.headers.get("location"), null, `${target} must not send a Location header`);
    }
  } finally {
    await portal.close();
  }
});

/* ────────────────────────── provisioning CLI ─────────────────────────── */

test("the provisioning CLI creates exactly one owner and refuses a second", async () => {
  const dataDir = await fs.mkdtemp(path.join(os.tmpdir(), "nexora-portal-cli-"));
  const env = { ...process.env, NEXORA_PORTAL_DATA_DIR: dataDir };
  const cli = path.join(WEBSITE_DIR, "server/provision-owner.mjs");

  /** Runs the CLI with a password piped on stdin (never on argv). */
  const runCli = (args, stdin = "") =>
    new Promise((resolve) => {
      const child = spawn(process.execPath, [cli, ...args], { env, cwd: WEBSITE_DIR });
      let stdout = "";
      let stderr = "";
      child.stdout.on("data", (chunk) => {
        stdout += chunk;
      });
      child.stderr.on("data", (chunk) => {
        stderr += chunk;
      });
      child.on("close", (code) => resolve({ code, stdout, stderr }));
      child.stdin.end(stdin);
    });

  try {
    const provisioned = await runCli(["--email", "cli-owner@example.com"], "cli-test-password-1234\n");
    assert.equal(provisioned.code, 0, provisioned.stderr);
    assert.match(provisioned.stdout, /Owner account provisioned/);
    assert.ok(!provisioned.stdout.includes("cli-test-password-1234"), "the password is never echoed");

    const file = JSON.parse(await fs.readFile(path.join(dataDir, "portal.json"), "utf8"));
    assert.equal(file.users.length, 1);
    assert.equal(file.users[0].email, "cli-owner@example.com");
    assert.match(file.users[0].passwordHash, /^scrypt\$/);
    assert.ok(!JSON.stringify(file).includes("cli-test-password-1234"));

    // A second run must not create a second account.
    const refused = await runCli(["--email", "another@example.com"], "cli-test-password-1234\n");
    assert.notEqual(refused.code, 0, "a second provisioning must fail");
    assert.match(refused.stderr, /already exists/);
    const after = JSON.parse(await fs.readFile(path.join(dataDir, "portal.json"), "utf8"));
    assert.equal(after.users.length, 1);
    assert.ok(!JSON.stringify(after).includes("another@example.com"));

    // A weak password is rejected before anything is written.
    const weak = await runCli(["--email", "cli-owner@example.com", "--rotate-password"], "short\n");
    assert.notEqual(weak.code, 0);
    assert.match(weak.stderr, /at least 12 characters/);

    // Rotation succeeds and revokes sessions.
    const rotated = await runCli(
      ["--email", "cli-owner@example.com", "--rotate-password"],
      "another-long-password-5678\n",
    );
    assert.equal(rotated.code, 0, rotated.stderr);
    assert.match(rotated.stdout, /password rotated/i);
    const rotatedFile = JSON.parse(await fs.readFile(path.join(dataDir, "portal.json"), "utf8"));
    assert.notEqual(rotatedFile.users[0].passwordHash, file.users[0].passwordHash);
  } finally {
    await fs.rm(dataDir, { recursive: true, force: true });
  }
});
