# Nexora Developer Account Portal — architecture and operations

**Scope of this document:** the private developer portal at `/admin` and the
server that backs it.

Status: **implemented; static frontend and backend are separate deployment
concerns.** The current Netlify site serves only the static frontend and has no
Node portal backend connected. The SPA routing correction is not live until
Netlify redeploys it, and even then authentication remains unavailable until the
real backend is deployed and proxied as described in `DEPLOYMENT.md` §4.

---

## 1. Why this exists

The website is a client-rendered static site with no authenticated surface.
This work adds one:

- `/admin` — a sign-in page for authorised Nexora developers;
- a private dashboard for the Owner/Super Admin account, covering overview,
  website status, content, support, and developer settings.

There is **no public registration**. Exactly one account can exist until a
future, separately approved phase adds more: the Owner/Super Admin.

---

## 2. Audit of what already existed (before this work)

| Area | Finding |
| --- | --- |
| Website | React 19 + Vite 7 + TypeScript SPA, 9 public routes + fallback, client-rendered, no server runtime of its own |
| Vite config | `vite.config.ts` + `vite/seo-plugin.ts` (robots/sitemap, fail-closed indexing) |
| Existing backend in repo | **`services/nexora-auth`** — a Rust/Axum service: Argon2id password hashing, PostgreSQL via `sqlx`, opaque session tokens (sha256 at rest), rotation with family reuse detection, rate limiting, registration, recovery, Google OAuth, billing/quota seams |
| Auth provider configured for the website | **none** |
| Database available to the website | **none** |
| Cookie/session model in `nexora-auth` | **none** — tokens are returned in JSON for the desktop Rust client's OS credential store; there are no cookies, no CORS surface, and no browser transport |
| Admin/role concept in `nexora-auth` | **none** — `users` has `status` (`active`/`locked`/`deleted`) but no role column, and no admin routes |
| Toolchain in this environment | Node 22.22.3, pnpm 9.15.4. **No Rust toolchain**, no crates registry access, no PostgreSQL. `services/nexora-auth` cannot be built or run here |

### Decision: do not reuse `services/nexora-auth` for the browser portal

Reusing it would be the wrong call on three independent grounds, and it is worth
stating plainly rather than silently duplicating work:

1. **It is a different trust model.** It is designed for a first-party desktop
   client that stores its bearer token in the OS credential store. A browser
   portal needs an `HttpOnly` cookie session (no tokens readable by JavaScript),
   a CSRF story, and an origin allowlist — none of which that service has. Making
   it browser-facing is a security-relevant redesign of a service outside
   `website/`, which this task explicitly requires approval for.
2. **It has no authorisation model.** Nothing there distinguishes an Owner from
   any other account, so "Owner/Super Admin only" would require schema
   migrations plus new middleware in `services/`.
3. **None of it can be executed or verified here.** No Rust toolchain, no crates
   access, no PostgreSQL. Shipping an unverified auth path would violate the
   repository's own rule that unverified behaviour is not claimed as working.

The portal therefore implements a **small, zero-dependency Node server inside
`website/`**, using only Node built-ins, and documents the migration path to
`nexora-auth` later (§11). No file outside `website/` is modified.

---

## 3. Architecture

```
browser ──TLS──▶ host (Node process, one origin)
                 │
                 ├── /api/portal/*      JSON API, cookie sessions, no CORS
                 ├── /admin, /admin/*   the SPA shell (client-rendered)
                 └── /*                 the public static site (dist/)
```

One process, one origin. There is no CORS surface at all: the server never emits
`Access-Control-Allow-*`, so a cross-origin page cannot read an API response even
if it sends a request.

| Concern | Implementation | File |
| --- | --- | --- |
| HTTP server, routing, static files | `node:http`, manual routing | `server/index.mjs`, `server/http.mjs` |
| Configuration + fail-closed validation | env parsing, no `VITE_*` | `server/config.mjs` |
| Password hashing | `node:crypto` `scrypt` (OpenSSL's scrypt, N=32768, r=8, p=1, 32-byte key, 16-byte random salt, encoded `scrypt$N$r$p$salt$key`) | `server/auth.mjs` |
| Session tokens | 32 random bytes from `crypto.randomBytes`, delivered in an `HttpOnly` cookie, stored only as `sha256(token)` | `server/auth.mjs` |
| Persistence | single JSON file, atomic replace, mode `0600` | `server/store.mjs` |
| Abuse protection | in-memory sliding window per IP and per account | `server/rate-limit.mjs` |
| Owner provisioning | offline CLI, server-side only | `server/provision-owner.mjs` |

### Why not a hosted identity provider

A hosted provider (Auth0, Clerk, Supabase, WorkOS…) is a reasonable choice for a
public product with many users. Here it would mean: a new third-party account
(this environment cannot create one), a production domain, a client secret, and
a vendor dependency for a portal with one user — while the repository already
contains a first-party auth service as the long-term home for identity. The
portal is built so that swapping in an OIDC provider later changes
`server/auth.mjs` and one API handler, not the dashboard.

### Why not "hide it in the frontend"

The React side is presentation only. Every dashboard fact comes from an
authenticated API request; there is no privileged data in the bundle, and the
route guard exists purely so an unauthenticated visitor sees the sign-in page
instead of a flash of empty panels. Deleting the guard from the bundle grants
nothing.

---

## 4. Account model and authorisation

- Roles: `owner` and (defined, unused) `developer`. Only `owner` can be
  provisioned by the CLI today; the enum exists so a future approved phase can
  add developers without a schema change.
- Every privileged request resolves the session → user server-side and checks
  `role === "owner"` at the handler. A role in the request body, cookie, or
  header is never read.
- There is no registration route, no invitation flow, and no way to create a
  second account through HTTP.

### Owner provisioning — two server-side routes, no HTTP route

**A. From `website/.env` (the normal route).** Create the git-ignored file and
set three variables:

```dotenv
NEXORA_PORTAL_OWNER_EMAIL=you@your-domain.example
NEXORA_PORTAL_OWNER_NAME=Nexora Owner
NEXORA_PORTAL_OWNER_PASSWORD=<at least 12 characters, generated>
```

Then start the server (`pnpm portal`). On startup:

| Situation | What the server does |
| --- | --- |
| no owner variables | nothing; logs that the portal cannot provision an owner |
| variables invalid (missing/weak password, bad email) | **nothing is provisioned**; every problem is logged by variable name, never value; the server keeps serving the public site and `/admin` honestly reports that no owner exists |
| no owner in the store | hashes the password with scrypt and creates the owner |
| owner exists with the same email | **leaves the password untouched** and says so |
| owner exists with a different email | changes nothing; warns that the configured email does not match |

Startup **never** rotates a password, so a stale `.env` cannot silently reset a
live credential on a restart.

**B. With a prompt (no password on disk).** If you would rather the credential
never touches a file:

```bash
cd website
NEXORA_PORTAL_DATA_DIR=.portal-data \
  node server/provision-owner.mjs --email owner@example.com
# password: hidden prompt, or piped on stdin
```

### Rotation (explicit, never implicit)

```bash
# 1. change NEXORA_PORTAL_OWNER_PASSWORD (and the email, if it must change —
#    a changed email is refused, because renaming the owner is not a rotation)
# 2. then:
cd website && node server/provision-owner.mjs --from-env --rotate-password
```

Rotation re-hashes the password **and revokes every live session** for the
account: a credential change that leaves a stolen cookie working would not be a
credential change. Without `--rotate-password`, the command refuses.

- The password is never taken from `argv` (visible in `ps`), never echoed, never
  logged, and written nowhere except as a scrypt hash. `--from-env` reads it from
  the file or the process environment instead of a prompt.
- There is deliberately **no** "first visitor claims the account" endpoint. The
  absence of that endpoint is the security control; a setup URL guarded by a
  secret would be an internet-reachable account-claim surface, which is exactly
  what this task forbids.

---

## 5. Session and cookie behaviour

| Property | Value |
| --- | --- |
| Cookie name | `nexora_portal_session` |
| Flags | `HttpOnly; Path=/; SameSite=Strict` + `Secure` when `NEXORA_PORTAL_SECURE_COOKIES=true` |
| Rotation | a new token is issued on every successful sign-in; the previous token is revoked |
| Absolute lifetime | `NEXORA_PORTAL_SESSION_TTL_SECONDS` (default 8 h) |
| Idle timeout | `NEXORA_PORTAL_SESSION_IDLE_SECONDS` (default 30 min) |
| At rest | `sha256(token)` only; the raw token exists in the cookie and nowhere else |
| Sign-out | `DELETE /api/portal/session` revokes server-side and clears the cookie |
| Expiry | any request on an expired/revoked session is `401` and clears the cookie |

Session fixation is not reachable: no pre-authentication session exists, so
there is nothing to fixate, and the post-login token is freshly random.

### CSRF

Three independent layers, because a cookie session without them is a real hole:

1. `SameSite=Strict` — the cookie is not sent on cross-site requests.
2. **Origin check** — every state-changing request must carry an `Origin` (or
   `Sec-Fetch-Site: same-origin`) that matches the configured portal origin;
   otherwise `403`.
3. **Session-bound token** — `GET /api/portal/session` returns a CSRF token; any
   mutating call must echo it in `X-Portal-CSRF`. It is compared against the
   session's stored `sha256` with a timing-safe compare.

Note on the sign-in request itself: it is protected by layers 1 and 2 (login CSRF
is a real attack — it forces a victim into the attacker's account). Because the
session cookie is only issued on success and is `SameSite=Strict`, an attacker's
cross-site POST is both origin-rejected and unable to receive the cookie.

---

## 6. API surface

All responses are `Cache-Control: no-store`, `X-Content-Type-Options: nosniff`.
Authenticated responses carry `Vary: Cookie`. No CORS headers, ever.

| Method | Path | Auth | Purpose |
| --- | --- | --- | --- |
| `GET` | `/api/portal/status` | public | `{ available, ownerProvisioned }` — lets the sign-in page say "not configured yet" instead of pretending to work. Reveals no secret and no account identity |
| `POST` | `/api/portal/session` | public + origin + rate limit | sign in; `200` sets cookie + returns identity and CSRF token |
| `GET` | `/api/portal/session` | cookie | current identity + session expiry + CSRF token, or `401` |
| `DELETE` | `/api/portal/session` | cookie + CSRF | sign out, revoke |
| `GET` | `/api/portal/overview` | owner | portal/environment facts |
| `GET` | `/api/portal/website` | owner | build presence, configured origin, sitemap/robots state, installer state |
| `GET` | `/api/portal/content` | owner | `{ configured: false }` — no CMS exists |
| `GET` | `/api/portal/support` | owner | `{ configured: false }` — no ticket store exists |

Error envelope: `{ "error": { "code": "...", "message": "..." } }`. Validation
failures name the field; authentication failures are one generic
`invalid_credentials` for unknown account, wrong password, and ineligible
account alike.

Fail-closed behaviour: if the data directory is missing or unreadable, or the
JSON store is corrupt, the API answers `503 portal_unavailable` and the UI shows
an unavailable state. It never falls back to "authenticated".

---

## 7. Frontend

- Route: `/admin` (plus `/admin/overview`, `/admin/website`, `/admin/content`,
  `/admin/support`, `/admin/settings`).
- `/admin` renders the sign-in page when unauthenticated and the overview when
  authenticated; a protected sub-route redirects to the literal `/admin` when
  unauthenticated. There is no `?next=` parameter, so there is no open redirect
  to validate.
- Metadata: every `/admin*` path is `noindex, nofollow`, is absent from
  `PAGE_METADATA`'s indexable set, and therefore cannot enter `SITEMAP_PATHS`.
- No navigation link to `/admin` is added to the public header or footer.
- Session state lives in memory only (a React provider). Nothing is written to
  `localStorage` or `sessionStorage`; a refresh re-validates against the server.

## 8. Configuration

Server-only environment variables (see `website/.env.example`); **none** of them
is a `VITE_*` variable, so none can leak into the browser bundle.

| Variable | Default | Required in production |
| --- | --- | --- |
| `NEXORA_PORTAL_ENV` | `development` | yes (`production`) |
| `NEXORA_PORTAL_ORIGIN` | derived from the request host in development | **yes** — the exact public origin, used for the CSRF origin check and the cookie |
| `NEXORA_PORTAL_HOST` | `127.0.0.1` | yes (`0.0.0.0` behind a proxy) |
| `NEXORA_PORTAL_PORT` | `4174` | yes |
| `NEXORA_PORTAL_DATA_DIR` | `.portal-data` | yes (persistent volume, outside the web root) |
| `NEXORA_PORTAL_STATIC_DIR` | `dist` | yes |
| `NEXORA_PORTAL_SECURE_COOKIES` | `false` | **yes** (`true`, HTTPS only) |
| `NEXORA_PORTAL_SESSION_TTL_SECONDS` | `28800` | optional |
| `NEXORA_PORTAL_SESSION_IDLE_SECONDS` | `1800` | optional |
| `NEXORA_PORTAL_TRUST_PROXY` | `false` | only behind a trusted proxy (`X-Forwarded-For`) |
| `NEXORA_PORTAL_ENV_FILE` | `<cwd>/.env` | optional — point the loader at a file outside the deploy directory |
| `NEXORA_PORTAL_OWNER_EMAIL` | unset | to provision the owner from configuration |
| `NEXORA_PORTAL_OWNER_NAME` | local part of the email | optional display name |
| `NEXORA_PORTAL_OWNER_PASSWORD` | unset | to provision/rotate the owner (≥ 12 characters) |

In production the server refuses to start unless `NEXORA_PORTAL_ORIGIN` is an
`https://` origin and secure cookies are enabled. It logs configuration
*presence*, never values.

---

## 9. Local development

Three processes, or one if you only need the API:

```bash
cd website

# 1. Create the local, git-ignored environment file.
cp .env.example .env
chmod 600 .env
#    …then fill in your own values:
#      NEXORA_PORTAL_DATA_DIR=.portal-data
#      NEXORA_PORTAL_OWNER_EMAIL=you@example.invalid
#      NEXORA_PORTAL_OWNER_NAME=Local Owner
#      NEXORA_PORTAL_OWNER_PASSWORD=<12+ characters, not reused anywhere>

# 2. Build the site the portal serves (and that the SEO tests expect).
pnpm build

# 3. Start the portal (serves dist/ + /api/portal/* on :4174). The first start
#    creates the owner from .env; later starts leave it alone.
pnpm portal
```

The server loads `website/.env` itself (`server/env-file.mjs`), applying only
variables the process environment does not already set, and logs **key names
only** — never a value. `NEXORA_PORTAL_ENV_FILE` points the loader at a file
elsewhere, which is what the tests use to stay hermetic.

A `.env` that is readable by other users (mode above `600`) produces a startup
warning; a missing one is not an error.

Then either open `http://localhost:4174/admin`, or run `pnpm dev` and use
`http://localhost:5173/admin` — Vite proxies `/api` to `127.0.0.1:4174`
(`NEXORA_PORTAL_PROXY_TARGET` overrides the target). The proxy keeps the
browser's `Host` header, so the portal's origin check sees the same host the
browser used.

`.portal-data/` is git-ignored and holds `portal.json` (scrypt hashes and sha256
session digests, mode `0600`). Deleting it removes the account and every
session; the next `provision-owner` run recreates a fresh account.

**Reset a forgotten password:**

```bash
NEXORA_PORTAL_DATA_DIR=.portal-data \
  node server/provision-owner.mjs --email <owner> --rotate-password
```

Rotation also revokes every existing session.

## 10. Production requirements (backend not connected to Netlify)

> The step-by-step launch sequence — pre-flight, environment variables, owner
> provisioning and rotation, supervisor example, proxy example, health checks,
> backups, rollback, and the manual pre-launch checks — is
> [`DEPLOYMENT.md`](./DEPLOYMENT.md). Its §4 also covers the current Netlify
> static deployment and the exact same-origin API proxy required later. This
> section is the portal-side summary.

Netlify has static routing configuration, but **no production portal backend is
connected**. A working authenticated deployment must provide:

1. **A Node process** (`node server/index.mjs`, or the same command under a
   supervisor). A static host alone cannot run this: authentication is
   server-side by design.
2. **TLS**, with `NEXORA_PORTAL_ENV=production`,
   `NEXORA_PORTAL_ORIGIN=https://<confirmed-domain>` and secure cookies. The
   server refuses to start otherwise.
3. **A persistent volume** for `NEXORA_PORTAL_DATA_DIR`, outside the web root,
   readable only by the service account. Single-instance only — see §13.
4. **The site's SPA rewrite must exclude `/api` and `/api/*`**, and should
   preserve real `/robots.txt`, `/sitemap.xml` and `/assets/*` responses. The
   Netlify configuration therefore puts API and missing-file `404` rules before
   its final SPA fallback; real files shadow those rules. Once a backend exists,
   the API rules must be replaced by the same-origin proxy in `DEPLOYMENT.md` §4
   — not supplemented with CORS or a browser-visible second origin.
5. **Security headers.** The server sets `X-Content-Type-Options` and
   `Referrer-Policy` always, `X-Robots-Tag: noindex, nofollow` on `/admin` and
   on every API response, and — only when `NEXORA_PORTAL_ENV=production` with
   secure cookies — `Strict-Transport-Security`,
   `X-Frame-Options: DENY` and `Content-Security-Policy: frame-ancestors
   'none'`. They are configuration-driven rather than constant so that the
   development server and the sandbox preview (which embeds the site in a
   frame, over plain http) keep working while production is hardened. A
   reverse proxy may add a request-size limit; it does not need to duplicate
   these.
6. **Rate limiting at the edge** if the portal is exposed to the internet. The
   in-process limiter (§3) is per-instance and resets on restart; it slows online
   guessing, it does not stop a distributed attack.

## 11. Migration path to `services/nexora-auth`

The portal's authorisation surface is intentionally small:
`resolveSession()` in `server/api.mjs` is the only place that turns a request
into an account. Replacing it with a call to a hardened identity service changes
one function plus the cookie handling:

1. Add cookie sessions (or an OIDC code flow with PKCE) to `nexora-auth`, plus a
   `role` column and an owner-only middleware.
2. Point `NEXORA_PORTAL_ORIGIN` at the same origin, keep the cookie name and
   attributes, and map the service's bearer-token identity onto the session.
3. Delete `server/store.mjs` and the local password code once sessions are
   issued by the service. The dashboard components do not change: they only know
   the `/api/portal/*` contract.

That work is **outside this task's scope** and requires changes to files outside
`website/`, which the task explicitly gates behind approval.

## 12. Testing

```bash
cd website
pnpm test:portal          # full portal/security suite, Node's built-in runner
```

| File | Covers |
| --- | --- |
| `server/tests/auth.test.mjs` | scrypt encoding/verification (including tampered and absurdly-costed hashes), token and CSRF rules, password policy, email normalisation, rate-limit windows, configuration fail-closed behaviour and origin parsing, and that the config description leaks neither paths nor secrets |
| `server/tests/owner-env.test.mjs` | the `.env` parser and loader (quoting, escapes, no-override, permissions), Owner configuration validation, first-run provisioning, "a restart never overwrites the password", rotation with session revocation, safe invalid-configuration startup, log/API/build-output leakage checks, and the `--from-env` CLI |
| `server/tests/deployment.test.mjs` | the deployment contract: Netlify's ordered static routing rules and non-forced file shadowing, SPA deep links, assets served separately, missing files as `404` (not the shell), path-traversal refusal, `/api/*` never falling through to the shell, `HEAD` answered like `GET`, `robots.txt` and `/sitemap.xml` in both the configured and the unconfigured build, public pages indexable while `/admin` and the API stay `noindex`, immutable caching for hashed assets and `no-cache` for the shell, production-only security headers (absent in development), a production start that refuses incomplete configuration, and a clean `SIGTERM` shutdown |
| `server/tests/portal-api.test.mjs` | public routes and crawl files; the admin shell; status without leakage; successful, failed, and rate-limited sign-in; cookie attributes; CSRF on reads, sign-out, and cross-origin requests; role/identity forgery; rotation; idle and absolute expiry; sign-out revocation; configuration and store fail-closed paths; no CORS; body/method limits; secret-free storage; error hygiene; no redirects or destination parameters; the provisioning CLI (create, refuse a second account, weak password, rotation) |

Each integration test runs its own server on an ephemeral port with a temporary
data directory and a **test-only** password, so nothing touches the development
account and no assertion weakens the production path.

The browser pass (sign-in, dashboard, keyboard, reduced motion, nine widths,
axe-core, rendered contrast) was run against the dev server with headless
Chromium; its results are reported in the portal report, not in this repository's
test suite.

## 13. Known limitations

- **Single process, single instance.** Sessions and accounts live in one JSON
  file guarded by an in-process write queue. Two instances behind a load
  balancer would not share sessions or rate-limit counters.
- **No second factor.** There is one password-protected account and no TOTP,
  WebAuthn, or recovery codes.
- **No account recovery flow.** A lost password is fixed by the CLI (§9), which
  requires server access. That is deliberate for a one-account portal.
- **Environment provisioning is first-run only.** The `.env` values are read at
  startup to *create* an account; changing the password in `.env` and restarting
  does nothing until `--from-env --rotate-password` is run. That is the point,
  but it is also the one thing an operator is likely to be surprised by.
- **The password is in a file.** `.env` is git-ignored and warned about when
  world-readable, but it is still a file on the server. The prompt/pipe route
  (`provision-owner` without `--from-env`) exists for deployments that would
  rather keep the secret in the environment or a secret manager only.
- **No email delivery**, so no verification, reset links, or sign-in alerts.
- **The in-process rate limiter is memory only**: it resets on restart and is
  per-instance.
- **No audit export.** The last 200 events are kept in the same file and shown
  read-only on the Overview panel.
- **No analytics, content management, support inbox, or deployment
  integration.** The dashboard reports these as unconfigured rather than
  inventing figures; that is the honest state, not an unfinished screen.
- **No production portal backend is connected.** The Netlify site is static only;
  it has no Node process, persistent Owner store, or provisioned API. A deployed
  `/admin` shell alone must never be reported as a working authenticated portal.

## 14. What the portal does *not* expose

- No public registration, invitation, or account-claim endpoint — the only way
  to create an account is the offline CLI.
- No password change over HTTP.
- No access to the repository, the filesystem beyond the build directory, the
  desktop application, or any model/inference surface.
- No privileged data in the JavaScript bundle: the dashboard fetches everything
  from authenticated endpoints, and the route guard is cosmetic.
