# Deployment checklist — Nexora website + developer portal

**Current status:** the static website is deployed on Netlify at
`https://get-nexora-desktop.netlify.app/`, but no Node portal backend is connected
to it. The routing correction in this repository is **not live until Netlify
rebuilds and deploys it**. No backend, Owner account, credential, or indexing
change is created by that static deployment.

The full-stack design is **one Node process**: `node server/index.mjs` can serve
the built static site from `dist/` and the private portal API from `/api/portal/*`
on the same origin. The current Netlify deployment instead serves `dist/` as
static files, so a working portal needs the same Node API on a separate host with
Netlify proxying `/api/*` to it. Netlify static hosting never runs
`website/server/index.mjs`; serving the `/admin` shell is not the same as making
sign-in available.

| Document | Covers |
| --- | --- |
| this file | launch sequence, hosting, domain, backups, rollback |
| `README.md` | the site itself — routes, design system, build |
| `DEVELOPER_PORTAL.md` | the portal — auth model, storage, API, limitations |
| `LAUNCH_PLAN_ZERO_COST.md` | the ₹0 hosting investigation and recommended architecture (card-verification VM) |
| `SERVERLESS_SCOPING.md` | the serverless ₹0 alternative (Cloudflare Workers + D1) and its blocker |

---

## 0. Decisions that need the owner before launch

Nothing below can be executed until these are answered. They are recorded here
rather than guessed at in code.

| # | Decision | Why it blocks launch |
| --- | --- | --- |
| 1 | The **production domain** (e.g. `https://<domain>`) | `NEXORA_PORTAL_ORIGIN`, the session cookie, `VITE_SITE_URL`, `robots.txt` and `sitemap.xml` are all derived from it. Indexing stays *disabled* until it is confirmed. |
| 2 | Whether the portal is **public at all** | `/admin` can be public-with-login, or restricted at the edge (IP allow-list, SSO proxy, VPN). The app supports either; the choice is the owner's. |
| 3 | The **Node host** and how TLS terminates | Determines `NEXORA_PORTAL_HOST`, `NEXORA_PORTAL_TRUST_PROXY`, and where the security headers are set (app or proxy). |
| 4 | The **persistent volume** for `NEXORA_PORTAL_DATA_DIR` | Without it, every restart loses the owner account and all sessions. |
| 5 | Whether to keep the **JSON store** | It is single-instance by design. Multi-instance or multi-operator use needs the migration in `DEVELOPER_PORTAL.md` §11 — a separate, approved task. |
| 6 | Whether to **enable public indexing** | Requires decision 1 plus a review of the pages; see §7. |

The repository deliberately contains no DNS records, host config, TLS
certificates, or provider files. Adding them is a deployment action, not a code
change.

---

## 1. Pre-flight (run in `website/`)

```bash
cd website

corepack pnpm@9.15.4 install --frozen-lockfile   # clean, reproducible install
corepack pnpm@9.15.4 test:portal                 # 74 tests, Node's runner, no deps
corepack pnpm@9.15.4 typecheck                   # tsc --noEmit
corepack pnpm@9.15.4 build                       # emits dist/
```

All four must pass. `pnpm build` runs the type check first and fails the build
if it fails.

Then confirm the build is clean of secrets and of private routes:

```bash
ls dist                     # index.html, assets/, robots.txt (sitemap.xml only when configured)
grep -rIl "NEXORA_PORTAL_" dist || echo "no server variable names in dist"
grep -rIl "/admin" dist/assets || echo "no portal route in the bundle"
find . -name ".env" -not -path "./node_modules/*" -exec ls -l {} \;
```

`.env` must exist **only** on the server, never inside `dist/`, and must be mode
`600`. Note that `dist/index.html` legitimately contains no secret — everything
in `src/` is public by definition.

---

## 2. Environment variables

Server-only; **none is a `VITE_*` variable**, so none can reach the browser
bundle. Full reference: `DEVELOPER_PORTAL.md` §8.

### Required in production

| Variable | Value | Notes |
| --- | --- | --- |
| `NEXORA_PORTAL_ENV` | `production` | Anything else is treated as development and skips the production checks. |
| `NEXORA_PORTAL_ORIGIN` | `https://<confirmed-domain>` | Bare origin — no path, query, fragment, credentials, or wildcard. Used for the CSRF origin check. `http://` is refused in production. |
| `NEXORA_PORTAL_SECURE_COOKIES` | `true` | Production defaults to `true`; setting `false` is a fatal configuration error. |
| `NEXORA_PORTAL_HOST` | `0.0.0.0` | Required behind a proxy or container; `127.0.0.1` is the local default. |
| `NEXORA_PORTAL_PORT` | e.g. `4174` | Whatever the proxy forwards to. |
| `NEXORA_PORTAL_DATA_DIR` | `/var/lib/nexora-portal` (example) | **Absolute**, persistent, outside the web root, writable only by the service account. |

### Optional

| Variable | Default | Notes |
| --- | --- | --- |
| `NEXORA_PORTAL_STATIC_DIR` | `dist` | Absolute path to the built site. |
| `NEXORA_PORTAL_SESSION_TTL_SECONDS` | `28800` | Absolute session lifetime, 300 … 604800. |
| `NEXORA_PORTAL_SESSION_IDLE_SECONDS` | `1800` | Idle timeout, ≥ 60 and ≤ the absolute lifetime. |
| `NEXORA_PORTAL_TRUST_PROXY` | `false` | Set `true` **only** when a proxy you control overwrites `X-Forwarded-For`; it is what the sign-in rate limiter keys on. |
| `NEXORA_PORTAL_ENV_FILE` | `<cwd>/.env` | Point it at a file outside the deploy directory (e.g. `/etc/nexora-portal.env`). |
| `NEXORA_PORTAL_OWNER_EMAIL` / `_NAME` / `_PASSWORD` | unset | Owner provisioning — see §3. |

### Production startup refuses to run when

- `NEXORA_PORTAL_ORIGIN` is missing, malformed, or not `https://`;
- `NEXORA_PORTAL_SECURE_COOKIES` is `false` or unparseable;
- a numeric variable is out of range (`PORT`, session TTL, idle TTL > TTL);
- `NEXORA_PORTAL_TRUST_PROXY` is not a boolean.

It exits non-zero and names the offending **variable** — never a value. This is
covered by `server/tests/deployment.test.mjs` and `auth.test.mjs`.

### Build-time variables (public, and only for indexing)

| Variable | Effect |
| --- | --- |
| `VITE_SITE_URL` | The confirmed origin used for canonical URLs, `og:url`, `sitemap.xml` and the `Sitemap:` line in `robots.txt`. |
| `VITE_SITE_INDEXABLE` | `true` emits `Allow: /` and `index, follow`. |

These two must be set **together** at build time. Unset = a private build:
`Disallow: /`, `noindex, follow`, no sitemap. Marking the site indexable for an
unconfirmed domain is not a launch step this repository performs.

---

## 3. Owner account: provisioning and rotation

There is exactly **one** account. There is no registration endpoint, no setup
URL, and no password reset over HTTP — by design.

**First run (creates the account once):**

```bash
# A .env on the server (mode 600), or a secret-manager-injected environment:
NEXORA_PORTAL_ENV=production
NEXORA_PORTAL_ORIGIN=https://<confirmed-domain>
NEXORA_PORTAL_SECURE_COOKIES=true
NEXORA_PORTAL_HOST=0.0.0.0
NEXORA_PORTAL_PORT=4174
NEXORA_PORTAL_DATA_DIR=/var/lib/nexora-portal
NEXORA_PORTAL_OWNER_EMAIL=<owner-address>
NEXORA_PORTAL_OWNER_NAME=<display name>
NEXORA_PORTAL_OWNER_PASSWORD=<12+ characters, not used anywhere else>

node server/index.mjs
```

The password is stored only as a scrypt hash. A restart **never** overwrites an
existing account, so leaving the password in `.env` is a standing secret.

**Then remove `NEXORA_PORTAL_OWNER_PASSWORD` from `.env`.** It is not needed
after the first start, and the account keeps working without it.

**Rotation (explicit, never implicit):**

```bash
node server/provision-owner.mjs --rotate-password        # hidden prompt
node server/provision-owner.mjs --from-env --rotate-password   # from the environment
```

Changing the password in `.env` and restarting does **nothing** — that is
deliberate. Rotation also revokes every existing session. A changed email is
refused rather than silently renaming the owner.

Never paste a real password into a shell history, a chat, a document, or a
tracked file. Use the hidden prompt, a pipe from a secret manager, or a
mode-`600` file.

---

## 4. Hosting the Node process

Any host that can run Node `>=22.12.0` (the `engines` field) and keep a process alive works. Two rules:

1. `NEXORA_PORTAL_HOST=0.0.0.0` (or the proxy's address) so the proxy can reach it.
2. It must be a **long-running process**, not a static upload. Sessions and the
   store live in memory and in `NEXORA_PORTAL_DATA_DIR`.

Supervisor example (systemd; adjust paths, user, and hardening to the host):

```ini
[Unit]
Description=Nexora website and developer portal
After=network.target

[Service]
User=nexora
WorkingDirectory=/srv/nexora/website
EnvironmentFile=/etc/nexora-portal.env
ExecStart=/usr/bin/node server/index.mjs
Restart=on-failure
RestartSec=3
# The service account needs write access to NEXORA_PORTAL_DATA_DIR and nothing else.
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ReadWritePaths=/var/lib/nexora-portal

[Install]
WantedBy=multi-user.target
```

The server logs one line at startup — environment, whether an origin is
configured, cookie mode, store readiness, and owner state — plus the config
variables it loaded **by name only**. It handles `SIGTERM`/`SIGINT` by closing
the listener and exiting 0, so `systemctl restart` and container stops are clean
(asserted in `deployment.test.mjs`).

### Start / stop

```bash
pnpm build                 # or: node node_modules/vite/bin/vite.js build
node server/index.mjs      # serves dist/ + /api/portal/* on NEXORA_PORTAL_PORT
```

Use `pnpm portal` in development only; it is the same command with the local
`NODE_ENV`.

### Current Netlify deployment: static frontend + separate backend

The root `netlify.toml` publishes only `website/dist`. Its routing rules are
ordered deliberately:

1. `/api` and `/api/*` return the static `404.txt` failure body while no backend
   is configured. They must never fall through to a `200` HTML response that
   looks like a successful API request.
2. An absent `/sitemap.xml` and missing `/assets/*` file likewise remain `404`.
   These rules are not forced, so an emitted sitemap and real hashed assets
   shadow them and are served normally.
3. The final `/* → /index.html` rewrite serves the React shell for `/admin`,
   `/admin/*`, and the public History API routes. The existing client-side
   `/admin` metadata remains `noindex, nofollow`; no new edge-authentication or
   indexing mechanism is introduced.

After this configuration is deployed, the admin **frontend** can load directly
and on refresh. Until a backend is connected, it must show its unavailable
state; `/api/portal/*` intentionally remains `404`. Setting `NEXORA_PORTAL_*`
variables in Netlify's build environment does not start the server and must not
be used as a substitute. Never put Owner credentials in `netlify.toml`, the
static build, or Netlify build variables.

#### Connecting the real API (required for sign-in)

First deploy `server/index.mjs` to a host that provides all of §4: Node
`>=22.12.0`, one long-running instance, HTTPS, and a persistent
`NEXORA_PORTAL_DATA_DIR` outside the web root. Build `dist/` there as well; the
server and its status panels expect that directory. Configure the backend with
the **browser-facing** origin, not its internal hostname:

```dotenv
NEXORA_PORTAL_ENV=production
NEXORA_PORTAL_ORIGIN=https://get-nexora-desktop.netlify.app
NEXORA_PORTAL_SECURE_COOKIES=true
NEXORA_PORTAL_PORT=4174
NEXORA_PORTAL_DATA_DIR=/var/lib/nexora-portal
NEXORA_PORTAL_STATIC_DIR=/srv/nexora/website/dist
```

Set `NEXORA_PORTAL_HOST` to the interface used by that host's reverse proxy
(`127.0.0.1` for same-VM nginx, or `0.0.0.0` where the container/platform
requires it). Set `NEXORA_PORTAL_TRUST_PROXY=true` only when a proxy you control
overwrites `X-Forwarded-For`; do not enable it merely because Netlify is in the
request path. All other production validation in §2 remains mandatory.

Once the backend has a real HTTPS origin, **replace both API 404 rules** in
`netlify.toml` with the following (substituting only the actual origin), and
keep them before the SPA fallback:

```toml
[[redirects]]
  from = "/api"
  to = "https://<backend-origin>/api"
  status = 200
  force = true

[[redirects]]
  from = "/api/*"
  to = "https://<backend-origin>/api/:splat"
  status = 200
  force = true
```

Do not leave the old 404 rules in front: Netlify uses the first matching rule.
Do not point the browser at a second origin or weaken the server's origin,
CSRF, cookie, or CORS rules. The Netlify rewrite is what preserves the existing
same-origin API contract.

Provision the Owner **on the backend host**, after its persistent volume is
ready, using the existing hidden-prompt CLI in §3 and a fresh password chosen by
the Owner. Static deployment must not provision an account. Remove any
bootstrap password from the environment after first-run provisioning, as §3
requires.

A real proxied deployment must then be verified end to end: JSON content types,
`GET /api/portal/status` → `200`, signed-out `GET /api/portal/session` → `401`,
sign-in setting `HttpOnly; Secure; SameSite=Strict` on the Netlify origin,
Owner-only API authorization, CSRF/origin rejection, sign-out revocation, and
persistence across a backend restart. Also confirm that Netlify and the backend
preserve request methods, bodies, `Origin`/`Sec-Fetch-Site`, cookies, and
`Set-Cookie`. If the provider chain cannot do so, serve the entire site from
the Node process instead; do not disable security checks to accommodate it.
Netlify documents a 26-second proxy rewrite timeout, so an always-on backend is
preferred over a sleeping service.

**Free-hosting limitation:** the current Netlify static deployment supplies no
Node runtime and no persistent portal volume. A separate backend is unavoidable
for the existing authentication design. The ₹0 VM option and its card/capacity
requirements are recorded in `LAUNCH_PLAN_ZERO_COST.md`; free PaaS instances
with ephemeral filesystems are not durable Owner stores. A purely static,
config-only change cannot provide a working authenticated portal.

---

## 5. Domain, HTTPS, and the reverse proxy

The Node process speaks plain HTTP; terminate TLS in front of it. Requirements:

- **HTTPS only.** A `Secure` session cookie is not sent over `http://`, so an
  HTTP origin produces a portal nobody can sign into — which is why the config
  check refuses `http://` in production.
- **Forward the original host** unchanged (`proxy_set_header Host $host;`), or
  the CSRF origin comparison fails. Do not rewrite the host to an internal name.
- **Set `X-Forwarded-For` yourself** if you want proxy-aware rate limiting, and
  only then set `NEXORA_PORTAL_TRUST_PROXY=true`.
- The app sets `Strict-Transport-Security`, `X-Frame-Options: DENY`,
  `Content-Security-Policy: frame-ancestors 'none'` and `X-Content-Type-Options`
  in production, so the proxy does not have to add them — but a proxy that
  strips or replaces headers is a risk worth checking once after go-live.

Nginx example (the routing rules are the important part, §6):

```nginx
server {
    listen 443 ssl;
    server_name <confirmed-domain>;
    # ssl_certificate / ssl_certificate_key ...

    # Portal API: never cached, never rewritten to index.html.
    location /api/ {
        proxy_pass http://127.0.0.1:4174;
        proxy_set_header Host $host;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_http_version 1.1;
    }

    location / {
        proxy_pass http://127.0.0.1:4174;
        proxy_set_header Host $host;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
}
```

Note that the app already performs the SPA fallback itself (unknown
**extensionless** paths return `index.html`; unknown paths with a file extension
return 404), so the proxy should pass requests through rather than apply its own
`try_files` rewrite.

---

## 6. Routing the host must preserve

| Path | Behaviour the server implements | What breaks if the host rewrites it |
| --- | --- | --- |
| `/api/portal/*` | JSON, `Cache-Control: no-store`, cookie session, no CORS | A host that maps everything to `index.html` answers `200 text/html` where the API means `401`/`404`, and sign-in dies. |
| `/robots.txt` | The built file (`Disallow: /` or `Allow: /`), `text/plain` | A rewritten path serves the app shell; crawlers read HTML as a robots file. |
| `/sitemap.xml` | The built file, `application/xml`; an honest **404** with a one-line explanation when the build had no confirmed domain | A rewritten path serves the shell with `200`, which is a broken sitemap rather than a missing one. |
| `/assets/*` | `Cache-Control: public, max-age=31536000, immutable` for hashed filenames | A host that adds its own cache policy or rewrites the path defeats CDN caching. |
| `/`, `/features`, `/download`, `/privacy`, … | `200` app shell, `Cache-Control: no-cache` | — |
| `/admin`, `/admin/*` | `200` app shell + `X-Robots-Tag: noindex, nofollow` | — |
| any other extensionless path | `200` app shell rendering the not-found page | Known limitation: the status is `200`, not `404`. Fixing it needs host-side cooperation and is out of scope. |
| any other path with an extension | `404 text/plain` | — |

Path traversal (`/../`, `/%2e%2e/`) is rejected, and `.env`,
`.portal-data/portal.json` and `server/*` are not reachable — asserted in
`deployment.test.mjs`.

---

## 7. Public indexing (only after decision 6)

Indexing stays **off** until the domain is confirmed. To launch indexable:

```bash
VITE_SITE_URL=https://<confirmed-domain> \
VITE_SITE_INDEXABLE=true \
corepack pnpm@9.15.4 build
```

That single configuration produces, consistently:

- `dist/robots.txt` → `Allow: /` plus `Sitemap: https://<domain>/sitemap.xml`;
- `dist/sitemap.xml` → exactly the nine public routes, no `/admin`, no
  `lastmod`/`changefreq`/`priority` invented;
- `<meta name="robots" content="index, follow">` in `index.html`;
- canonical URLs and `og:url` on the same origin.

Verify after deploying:

```bash
curl -s https://<domain>/robots.txt
curl -s https://<domain>/sitemap.xml | grep -c "<loc>"    # expect 9
curl -sI https://<domain>/admin | grep -i x-robots-tag    # Node-hosted full stack: expect noindex
```

`robots.txt` is crawler guidance, not access control: if `/admin` must be
private from *people*, restrict it at the edge — it is password-protected, but
its shell is publicly fetchable by design.

---

## 8. Health checks

```bash
curl -sf -o /dev/null -w "%{http_code}\n" https://<domain>/api/portal/status   # 200
curl -sfI https://<domain>/api/portal/status                                   # HEAD must also be 200
curl -sf -o /dev/null -w "%{http_code}\n" https://<domain>/                     # 200
curl -s  -o /dev/null -w "%{http_code}\n" https://<domain>/sitemap.xml         # 404 (unconfigured) or 200 (configured)
```

`GET` and `HEAD` on `/api/portal/status` both answer `200` and return no
account details; a `503` means the store is unavailable. Use `HEAD` for uptime
probes so the check does not fetch a body. `GET /api/portal/status` is safe to
expose: it reports only configuration booleans, never paths, values, or secrets.

---

## 9. Persistent storage, backup, and recovery

`NEXORA_PORTAL_DATA_DIR` holds `portal.json`: scrypt password hashes, sha256
session digests, and the last 200 audit events. The directory is created `0700`
and the file `0600`; both must stay readable only by the service account.

**Back up:** copy the directory (or the single file) while the service is
stopped, or accept the millisecond-scale risk of copying a file mid-write. Keep
the copy off the web root, encrypted at rest, and treat it as a credential store
— the hashes are crackable offline if weak.

**Restore:** stop the service, restore the file and its permissions, start the
service. Sessions that were live before the backup are still valid until they
expire or are revoked.

**Recovery when the file is lost:** the owner account is gone. Restore from
backup, or provision a fresh owner — `node server/provision-owner.mjs
--from-env` creates an account when none exists. Deleting the directory removes
every account and session.

**Not in the store:** no plaintext password, no session token (only its sha256),
no personal data beyond the owner's email and display name.

---

## 10. Limitations and their mitigations

These are properties of the current design, documented rather than silently
worked around.

| Limitation | Mitigation |
| --- | --- |
| Single process, single instance; sessions and rate limits are in memory + one file | Run exactly one instance. Multi-instance needs the migration in `DEVELOPER_PORTAL.md` §11. |
| No second factor, no recovery email, no email delivery at all | Keep the password in a password manager; recovery is the CLI, which needs server access. Put `/admin` behind an edge allow-list if that is acceptable. |
| Rate limiting is per-instance and resets on restart | Add edge rate limiting if the portal is internet-facing. |
| Unknown paths return `200` with the not-found page | Known since Phase 14; the page is `noindex`. A true `404` needs host-side routing and is out of scope. |
| The `/admin` shell is publicly fetchable (login required inside) | Edge restriction, or accept that the shell is public and the data is not. |
| `.env` is a file on the server | Use the hidden-prompt provisioning path or `NEXORA_PORTAL_ENV_FILE` pointing at a secret-manager-rendered file; the loader warns if the file is group/world-readable. |
| No audit export, no analytics, no CMS, no billing | Out of scope by instruction; the dashboard shows these as unconfigured rather than inventing figures. |
| No real-device, screen-reader, Lighthouse, or hosting-provider test has been run | Those remain manual pre-launch checks (§12). |

---

## 11. Rollback

The deployment is one process + one directory + one build:

1. **Stop** the service (`systemctl stop` / container stop). Do not delete
   `NEXORA_PORTAL_DATA_DIR`.
2. **Redeploy the previous build** (`dist/`) and the previous server code, or
   the previous image tag.
3. **Restore the store** only if the release changed its schema — this one does
   not; the format is unchanged since it was introduced.
4. **Start** and re-run the health checks in §8.
5. If the rollback is because of a suspected compromise, **rotate the owner
   password** (`--rotate-password`, which revokes all sessions) before reopening
   the portal.

A rollback must never re-enable indexation or repoint the origin at a different
host: the origin is part of the CSRF check and the cookie.

---

## 12. Manual pre-launch checks (not automatable here)

Run on the real host, after TLS, with the real domain:

- [ ] `https://<domain>/` and all nine routes render over HTTPS, direct visit and refresh.
- [ ] `https://<domain>/assets/...js` returns `cache-control: immutable`; the shell returns `no-cache`.
- [ ] `/api/portal/status` answers `200` to `GET` and `HEAD`; a wrong path under `/api/` answers JSON `404`, not HTML.
- [ ] `/admin` loads, sign-in works with the real owner, the wrong password is a generic error, and repeated failures are rate limited.
- [ ] The session cookie is `HttpOnly; Secure; SameSite=Strict` in the browser's storage inspector; it is absent after sign-out.
- [ ] Sign-out, then refresh: no dashboard content is served from cache.
- [ ] On the Node-hosted full stack, `X-Robots-Tag: noindex` is present on
  `/admin` and absent on public pages. On the Netlify static shell, verify the
  rendered `/admin` meta robots directive instead; do not assume the Node
  server's headers apply to Netlify's static responses.
- [ ] `robots.txt` and `sitemap.xml` reflect the intended indexing state; `/sitemap.xml` has exactly nine URLs when indexable.
- [ ] Restart the service: the owner can still sign in (persistent storage works) and no account is recreated.
- [ ] Back up the store, restore it into a scratch instance, sign in there.
- [ ] `/../`, `/.env`, `/.portal-data/portal.json` on the real host: no file contents.
- [ ] Response headers show HSTS, `X-Frame-Options`, `frame-ancestors 'none'` — or a documented reason the proxy replaces them.
- [ ] Screen-reader, real-device, and Lighthouse passes, if required for launch. **None has been performed for this project**, and none is claimed.

---

## 13. What this repository deliberately does not contain

- No backend deploy scripts, CI/CD pipelines, Dockerfiles, or backend host
  configuration. The root Netlify file configures only the static build,
  frontend routing, and the documented future API proxy location.
- No domain, DNS, TLS certificate, or hosting-account artefacts.
- No production credentials, tokens, or keys — not in docs, not in code, not in
  the build output.
- No analytics, payment, CMS, or deployment integration.
- No consent banner, because nothing sets a non-essential cookie or loads an
  external resource.

Everything above is a decision for the owner (§0), not an omission to be filled
in silently.
