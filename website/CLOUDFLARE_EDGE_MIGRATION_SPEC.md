# Cloudflare edge migration — detailed specification

**Status: specification for approval. No code has been written, no dependency
added, no account created, nothing deployed.**

Constraint you set: **₹0 and no payment method anywhere.** That rules out a VM
(Oracle needs a card for identity verification), Workers Paid ($5/month), Cloud
Run (billing account), and Deno Deploy (announced shutdown). It leaves exactly one
credible stack: **Cloudflare Workers Free + Static Assets + D1, with `/admin`
protected by Cloudflare Access (Zero Trust Free, ≤ 50 users).**

Because the free plan allows **10 ms of CPU per request**, a password hash cannot
live in the request path — so this design **retires the portal's password login**
and derives identity from Access. That is an authentication-architecture change,
which is why it needs your explicit approval before any of it is built.

Read with: `SERVERLESS_SCOPING.md` (why the alternatives fail),
`DEVELOPER_PORTAL.md` (what exists today), `LAUNCH_PLAN_ZERO_COST.md` (provider
comparison).

---

## 1. What changes, in one table

| Concern | Today (self-hosted Node) | Proposed (Cloudflare edge) |
| --- | --- | --- |
| Site | `node server/index.mjs` serves `dist/` | Static Assets (free, unlimited requests); SPA fallback via `not_found_handling` |
| Portal auth | Our scrypt password + signed cookie + CSRF token | **Cloudflare Access** (email one-time-PIN / Google / GitHub) in front of `/admin*`; the Worker verifies the Access JWT |
| Sessions | sha256 token digests in `portal.json` | Access session (Cloudflare-managed, configurable duration); no session table |
| Accounts | `portal.json` users table, CLI provisioning | D1 `portal_owner` row (email allow-list), seeded by SQL |
| Audit | Last 200 events in `portal.json` | D1 `audit` table (same 200-row cap), written once per Access session |
| Rate limiting | In-process sliding window | Removed; Access + Cloudflare edge handle sign-in abuse (documented trade-off) |
| Secrets | `/etc/nexora-portal.env` (mode 600) | `wrangler secret` / `vars` (Access AUD + team domain are not secrets; nothing else is needed) |
| Password storage | scrypt hash | **Nothing** — no password exists to store |

What is *gained*: no password to leak, phish, rotate, or brute-force; MFA/passkeys
available through the identity provider; zero servers to patch; static asset
requests are free and unlimited; no idle-reclamation risk.
What is *lost*: our own sign-in UI, our own rate limiter, our own session expiry
knobs, and the "single file I can copy" store (`portal.json`).

---

## 2. Target architecture

```
Request
  │
  ├─ /admin, /admin/*            ── Access policy: allow only <owner email>
  │     └─ Static Assets          → index.html (SPA), X-Robots-Tag: noindex via _headers
  │        └─ page calls /api/portal/*  (same origin, Access session cookie present)
  │
  ├─ /api/portal/*               ── Access policy: same application
  │     └─ Worker (this spec)     → verify Cf-Access-Jwt-Assertion, then D1
  │
  ├─ /, /features, /assets/*     ── Static Assets (free, cached, immutable for hashed names)
  ├─ /robots.txt, /sitemap.xml   ── Static Assets (built file, or 404 when unindexed)
  └─ anything else               ── Static Assets SPA fallback (200), as today
```

Routing config (one Worker, one deployment):

```jsonc
{
  "name": "nexora-site",
  "main": "edge/worker.mjs",
  "assets": {
    "directory": "./dist",
    "binding": "ASSETS",
    "not_found_handling": "single-page-application",
    "run_worker_first": ["/api/*", "/sitemap.xml"]
  },
  "d1_databases": [{ "binding": "DB", "database_name": "nexora-portal" }],
  "vars": { "TEAM_DOMAIN": "https://<team>.cloudflareaccess.com", "POLICY_AUD": "<AUD tag>" },
  "triggers": { "crons": ["17 3 * * *"] },
  "compatibility_date": "2026-10-01",
  "compatibility_flags": ["nodejs_compat"]
}
```

Why exactly those two patterns: only API paths and `/sitemap.xml` consume the
100,000/day Worker budget, so pages, assets, `robots.txt` and the SPA fallback are
served as free static assets. `/sitemap.xml` must be routed to the Worker because
with `not_found_handling: single-page-application` an absent sitemap would fall back
to the shell with `200` — the current server deliberately answers `404 text/plain`
("no confirmed production domain"), and that behaviour is preserved: the Worker
serves the built file when it exists and the honest `404` when it does not. (Free-plan
`run_worker_first` patterns return `429`, not a bill, once the daily budget is spent —
keeping the list this short keeps that blast radius tiny.)

---

## 3. File plan (new code, all inside `website/`)

```
website/edge/
  worker.mjs            fetch handler: routing, auth, JSON responses, errors
  access.mjs            Access JWT verification (zero-dependency RS256 via WebCrypto)
  jwks.mjs              JWKS fetch + cache (module scope + caches.default, 6-week rotation)
  db.mjs                D1 access: owner, audit, first-seen-session bookkeeping
  http.mjs              response helpers: JSON, headers, no-store, CSP, error shapes
  routes/
    status.mjs          GET /api/portal/status   (public, no Access requirement)
    session.mjs         GET /api/portal/session  (identity + capability summary)
    overview.mjs        GET /api/portal/overview
    website.mjs         GET /api/portal/website
    content.mjs         GET /api/portal/content
    support.mjs         GET /api/portal/support
  schema/0001_init.sql  D1 schema (idempotent)
  tests/*.test.mjs      node --test, driving worker.fetch() with real Request/Response
website/wrangler.jsonc  the config above
website/_headers        static header rules (immutable assets, noindex on /admin)
```

The React portal in `src/portal/` is **kept**; only the sign-in surface and the
session bootstrap change (§6). The existing Node server in `server/` is **kept** as
the self-hosted path and remains the reference implementation and test oracle for
the API contract — with the cost that two implementations must stay in sync (§10).

---

## 4. D1 schema

```sql
CREATE TABLE IF NOT EXISTS portal_owner (
  email        TEXT PRIMARY KEY,          -- normalised, from the Access identity
  display_name TEXT NOT NULL,
  role         TEXT NOT NULL DEFAULT 'owner',
  created_at   TEXT NOT NULL,
  last_seen_at TEXT,
  last_jti     TEXT                       -- dedupes audit writes per Access session
);

CREATE TABLE IF NOT EXISTS audit (
  id      INTEGER PRIMARY KEY AUTOINCREMENT,
  at      TEXT NOT NULL,
  event   TEXT NOT NULL,                  -- portal_opened | api_denied | forbidden_identity
  email   TEXT,
  detail  TEXT                            -- never a secret, never a token
);
CREATE INDEX IF NOT EXISTS audit_at ON audit(at);

CREATE TABLE IF NOT EXISTS rate_limit (
  key        TEXT PRIMARY KEY,
  window_at  INTEGER NOT NULL,
  hits       INTEGER NOT NULL
);
```

Seeding (once, by you or me with your approval):

```bash
wrangler d1 execute nexora-portal --remote \
  --command "INSERT INTO portal_owner (email, display_name, role, created_at)
             VALUES ('<owner-email>', '<display name>', 'owner', datetime('now'))"
```

Audit pruning runs in the daily cron (`DELETE FROM audit WHERE id NOT IN (SELECT id FROM audit ORDER BY id DESC LIMIT 200)`).
No passwords, hashes, tokens, or session rows exist anywhere.

---

## 5. Authentication and authorization design

**a. Access in front, Worker verifies.** Access is configured with one application
covering `/admin*` **and** `/api/portal/*`, policy = allow only the owner email
(one-time PIN or an identity provider you choose). Every request then carries
`Cf-Access-Jwt-Assertion`. The Worker:

1. reads the header — missing ⇒ `401 unauthenticated` (never falls back to "allow");
2. verifies the RS256 signature against the team JWKS
   (`https://<team>.cloudflareaccess.com/cdn-cgi/access/certs`), checking `iss`,
   `aud` (the application's AUD tag) and `exp`;
3. takes `email` from the verified claims and requires an **active `portal_owner`
   row** for it — a valid Access token for a *different* email is `403`, not
   "owner".

Boundary: the public site never depends on Access. The Access application covers
`/admin*` **and** `/api/portal/*` in full — so `status` is no longer reachable from
the internet, and the Worker additionally treats it as non-sensitive (booleans only)
as defence in depth. If you later want a public health endpoint, it can be excluded
from the Access app explicitly; that is a decision, not a default.

**b. Zero dependencies.** RS256 verification is ~40 lines with WebCrypto
(`crypto.subtle.importKey("jwk", …)` + `crypto.subtle.verify`), which costs well
under a millisecond of CPU. `jose` (the package Cloudflare's own sample uses) would
be an extra dependency; I would only add it if you prefer the sample code to hand
rolled verification.

**c. JWKS rotation.** Access rotates signing keys every 6 weeks and keeps the
previous key valid for 7 days. The Worker caches the key set in module scope and in
`caches.default` with a 1-hour TTL, and refetches on an unknown `kid`, so rotation is
transparent. A JWKS fetch failure fails **closed** (`503`), never open.

**d. CSRF.** We no longer set a cookie, but Access does (`CF_Authorization`), so
mutations still need protection. The design keeps both controls we have today:
`Origin` must equal the deployment origin (else `403`), **and** the response for
`GET /api/portal/session` carries a per-session CSRF token (HMAC of the JWT `jti`
with a Worker secret) that must be echoed in `X-Portal-CSRF` on every non-GET
request. Today's API has no non-GET routes beyond sign-in/out, so this is
future-proofing rather than a live surface — documented as such.

**e. Sign-out** is Cloudflare's: the portal links to the team's
`/cdn-cgi/access/logout` endpoint. No custom sign-out endpoint exists, and nothing
pretends one does.

**f. Email change / lost access.** If your Access identity email changes, the D1 row
must be updated (one SQL statement) or you are locked out of the dashboard while the
public site is unaffected. This is the honest operational shape of delegating
identity, and it goes in the limitations list.

---

## 6. Frontend changes (`src/portal/`)

Kept as-is: `PortalDashboard`, all five panels, `PortalPrimitives`, `portal-api.ts`
(shape unchanged), styling, responsive behaviour, keyboard handling.

Changed:

| File | Change | Reason |
| --- | --- | --- |
| `PortalSignIn.tsx` + `PortalAuth.css` | **Removed from the deployed build** | No password exists; Access shows its own sign-in page before our app is reached |
| `PortalSession.tsx` | Identity comes from `GET /api/portal/session`; `signIn()`/sign-out logic removed; a `signedOut` state becomes "not authorized for this account" | The browser cannot mint an Access session |
| `PortalPage.tsx` | Renders the dashboard when `/api/portal/session` returns an owner; renders an explanatory "no access" panel on `403`; never renders a login form | Honesty: a login form that cannot work is exactly what the brief forbids |
| `Panels` — Settings | "Sign out" becomes a link to the Access logout URL; "session expiry" text becomes "managed by Cloudflare Access" | Truthful UI |
| `Panels` — Overview | "Recent events" keeps reading the audit table; a note explains that sign-in events are now Access-side and not visible to the app | Avoid implying data we do not have |

The `/admin` route keeps its `noindex` metadata (already in `metadata.ts`) and gains
a static `X-Robots-Tag` via `_headers` to match today's header behaviour.

---

## 7. API parity contract

The Worker must return what the React portal already consumes, so the dashboard
needs no data-shape changes:

| Route | Method | Auth | Response |
| --- | --- | --- | --- |
| `/api/portal/status` | GET | Access JWT (gated by the same app) | `{ available, ownerProvisioned, environment }` |
| `/api/portal/session` | GET | Access JWT | `{ authenticated: true, user: { id, email, displayName, role, lastSignInAt, createdAt }, csrfToken, expiresAt }` |
| `/api/portal/overview` | GET | Access JWT + owner row | same JSON as today (readiness, recent audit, counts) |
| `/api/portal/website` | GET | ″ | routes/build/crawl-file facts derived from `dist/` metadata |
| `/api/portal/content` | GET | ″ | `{ configured: false, … }` (unchanged honesty) |
| `/api/portal/support` | GET | ″ | `{ configured: false, … }` |
| anything else under `/api/` | any | — | `404 { error: { code: "not_found" } }`, `no-store` |

`POST/DELETE /api/portal/session` disappear (no password, no cookie). Every response
keeps `Cache-Control: no-store`, `X-Content-Type-Options: nosniff`,
`X-Robots-Tag: noindex`, and `Vary: Cf-Access-Jwt-Assertion`.

---

## 8. CPU budget (the reason the design looks like this)

| Work per request | Estimate | Free limit |
| --- | --- | --- |
| Routing + JSON serialisation | < 1 ms | 10 ms |
| RS256 verify (WebCrypto) | < 1 ms | ″ |
| One indexed D1 query | 1–3 ms CPU (network wait is not CPU) | ″ |
| **Any password KDF (scrypt / PBKDF2 ≥ 100k)** | **10–100+ ms** | **exceeded** |

Hard rule for the implementation: **no key-derivation function may run in a request
path.** A code comment and a test will enforce it (a grep-based test asserting no
`scrypt`/`pbkdf2` import in `edge/`).

---

## 9. Rate limiting and abuse

With Access in front of `/admin`, the internet-facing sign-in surface is
Cloudflare's, and its brute-force protection applies. Our own limiter is not ported
(it would need D1 writes on every attempt, and a D1-backed limiter is a weaker
control than Access's). The public `/api/portal/status` endpoint is a constant
response with no database access, so it needs no limiter.

Documented consequence: `DEVELOPER_PORTAL.md` §5 must drop the per-account/per-IP
limiter claims and point at Access instead — no stale security claims.

---

## 10. Testing plan (and what is lost)

- `edge/tests/*.test.mjs` run with **`node --test`, zero dependencies**: the Worker
  exports `fetch(request, env)`, so tests build real `Request` objects, pass a fake
  `env` with an in-memory D1 shim, and assert on real `Response` objects.
- JWT tests use a locally generated RSA keypair and a test `kid`; the verification
  code is exercised for real (no bypass flag, no `if (test) return true`).
- Contract tests: the same fixtures used by `server/tests/portal-api.test.mjs`
  assert the JSON shapes in §7, so dashboard regressions surface as test failures.
- Coverage mapping: of the current 73 tests, ~30 (crawl files, static serving, SPA
  fallback, path traversal) move to the platform and are covered by Static Assets
  configuration plus a small set of asset-routing tests; ~25 (API contract) port
  directly; ~18 (password hashing, cookie flags, session expiry, rotation, sign-in
  rate limiting) **cease to exist** because the mechanism does — that deletion must
  be recorded in `DEVELOPER_PORTAL.md`, not silently dropped.
- Optional: `wrangler dev`/`vitest-pool-workers` for a workerd-accurate run. That is
  a **devDependency addition** and needs your approval; the zero-dependency path
  above is enough for CI-equivalent coverage.

Not verifiable locally: real Access policy behaviour, real JWKS rotation, real 429
behaviour at the free-tier ceiling. Those are staging checks after approval.

---

## 11. Deployment and operations (after approval)

```bash
cd website
pnpm build                                   # dist/ (unchanged)
npx wrangler d1 create nexora-portal         # binding id → wrangler.jsonc
npx wrangler d1 execute nexora-portal --remote --file=edge/schema/0001_init.sql
npx wrangler secret put PORTAL_CSRF_SECRET   # CSRF HMAC key (never printed)
npx wrangler deploy                          # static assets + Worker + cron
```

- Free hostname: `https://<name>.workers.dev` (no card, valid TLS). A real domain can
  be attached later; nothing here requires buying one.
- Access application created in the dashboard (Zero Trust → Access → Applications),
  path `admin*` and `api/portal/*`, policy = allow only your email.
- Indexing stays **off**: `VITE_SITE_URL`/`VITE_SITE_INDEXABLE` remain unset, so
  `robots.txt` is `Disallow: /` and `/sitemap.xml` returns `404 text/plain`, exactly
  as the current server does (§2 explains why that one path is routed to the
  Worker instead of the SPA fallback).
- Rollback: `wrangler rollback` to the previous version; D1 migrations are additive
  and idempotent.
- Backups at ₹0: `wrangler d1 export` on a schedule (a text dump you keep), plus the
  SQL seed file in the repo. No `portal.json` to lose.

---

## 12. Cost

| Item | Verified limit | This app |
| --- | --- | --- |
| Workers requests | 100,000/day free | Only `/api/*` + `/sitemap.xml`; a one-owner portal uses hundreds |
| Worker CPU | 10 ms/request free | < 3 ms per API request by design (§8) |
| Static asset requests | free and unlimited | The whole public site |
| D1 | 5 GB, 5M rows read/day, 100k rows written/day | < 1 MB, a few hundred rows/day |
| Cron | 5 per account free | 1 |
| Access (Zero Trust) | free ≤ 50 users | 1 user |
| **Total** | — | **₹0, no payment method** |

Ceilings that would break it: > 100k API requests/day (implausible for one owner, and
it returns 429 rather than a bill), or > 100k D1 rows written/day.

---

## 13. Effort (honest ranges)

| Work package | Estimate |
| --- | --- |
| `edge/http.mjs`, `access.mjs`, `jwks.mjs`, `worker.mjs` routing | 6–8 h |
| D1 layer + schema + audit bookkeeping | 3–4 h |
| Six route handlers at parity with today's JSON | 4–6 h |
| Frontend: remove sign-in, rewire session, settings/overview copy | 3–5 h |
| Tests (handler tests, JWT tests, contract fixtures) + doc updates | 6–10 h |
| Access setup runbook, `_headers`, wrangler config, deploy script | 2–3 h |
| **Total** | **~24–36 h of focused work**, plus a staging pass |

---

## 14. Risks

| Risk | Mitigation |
| --- | --- |
| Delegating auth means Cloudflare controls who can reach `/admin` | Access policy is the only allow-list; the Worker additionally checks the D1 owner row, so a valid token for another identity is still 403 |
| Access misconfiguration could expose the shell | The API rejects any request without a verified owner JWT; the shell contains no data (it fetches everything) |
| Two API implementations (Node + edge) drifting | Shared contract fixtures + a parity test list; document that `server/` is the self-hosted path |
| 10 ms CPU ceiling | No KDF in the request path (§8), enforced by a test |
| JWKS fetch failure | Fail closed with `503`; cached keys keep serving through transient outages |
| Free-tier behaviour change | All limits are published and enforced as errors/429, never as charges — that is the ₹0 guarantee |
| Access email change | One SQL `UPDATE`; documented as an operational step |
| Sign-in events no longer visible in our audit | Stated plainly in the dashboard and in `DEVELOPER_PORTAL.md` |

---

## 15. Decisions I need before writing any code

1. **Access as the identity provider** — approve retiring the password login. (This
   is the only card-free path that keeps authentication strong.)
2. **Which Access identity method**: email one-time PIN (simplest, no third-party
   account) or Google/GitHub login.
3. **Owner email for the allow-list** — a non-secret address; I will not ask for a
   password of any kind.
4. **Zero-dependency RS256 verification** (hand-written WebCrypto, ~40 lines) or the
   `jose` package. I recommend the former; the latter is a new dependency.
5. **`wrangler` as a devDependency** (dev-only tooling, not shipped) — required to
   deploy at all.
6. **Keep `server/` as the self-hosted path** (dual maintenance, documented) or
   retire it later in a separate, approved task.
7. **Where the portal lives in the UI** — unchanged: `/admin`, unlinked from public
   navigation, `noindex`, never in the sitemap.

Nothing above is implemented. Approving this spec authorises writing the code in
`website/edge/` plus the listed frontend edits and tests — **not** deploying. A
separate go-ahead would be needed for `wrangler deploy`, creating the Access
application, and pointing a hostname at it.
