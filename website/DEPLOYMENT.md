# Deployment checklist — Nexora website + developer portal

**Status: nothing in this repository has been deployed, published, or indexed.**
This document is preparation. It describes what a deployment *would* require and
what the owner must decide before one happens. It contains no credentials, no
hostnames, and no provider choices — none were supplied, so none are assumed.

The site and the portal are **one Node process**: `node server/index.mjs` serves
the built static site from `dist/` and the private portal API from `/api/portal/*`
on the same origin. A static-only host cannot run this (sign-in is server-side).

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
corepack pnpm@9.15.4 test:portal                 # 73 tests, Node's runner, no deps
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
curl -sI https://<domain>/admin | grep -i x-robots-tag    # expect noindex
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
- [ ] `X-Robots-Tag: noindex` on `/admin`; absent on public pages.
- [ ] `robots.txt` and `sitemap.xml` reflect the intended indexing state; `/sitemap.xml` has exactly nine URLs when indexable.
- [ ] Restart the service: the owner can still sign in (persistent storage works) and no account is recreated.
- [ ] Back up the store, restore it into a scratch instance, sign in there.
- [ ] `/../`, `/.env`, `/.portal-data/portal.json` on the real host: no file contents.
- [ ] Response headers show HSTS, `X-Frame-Options`, `frame-ancestors 'none'` — or a documented reason the proxy replaces them.
- [ ] Screen-reader, real-device, and Lighthouse passes, if required for launch. **None has been performed for this project**, and none is claimed.

---

## 13. What this repository deliberately does not contain

- No deploy scripts, CI/CD pipelines, Dockerfiles, or host configuration.
- No domain, DNS, TLS certificate, or hosting-account artefacts.
- No production credentials, tokens, or keys — not in docs, not in code, not in
  the build output.
- No analytics, payment, CMS, or deployment integration.
- No consent banner, because nothing sets a non-essential cookie or loads an
  external resource.

Everything above is a decision for the owner (§0), not an omission to be filled
in silently.
