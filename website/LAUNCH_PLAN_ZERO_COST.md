# ₹0 public launch plan — website + working developer portal

**Status: proposal, nothing deployed.** This document is the plan Sarthak reviews
before any account, repository connection, DNS record, or deployment is created.
It records what the code actually does (audited, not assumed), what free hosting
currently offers (verified against provider documentation on 2026-10-27), and the
one architecture that keeps the real portal working at ₹0.

Short version: **the only ₹0 option that keeps a server-authenticated `/admin`
portal with durable Owner storage and needs no code changes is a free virtual
machine — Oracle Cloud Infrastructure "Always Free" — running the existing Node
process behind nginx + Let's Encrypt.** Every free PaaS examined either discards
its filesystem on restart (portal account lost) or no longer offers a free tier to
new accounts.

> **Follow-up:** you asked to investigate the serverless ₹0 route first. That
> scoping is in [`SERVERLESS_SCOPING.md`](./SERVERLESS_SCOPING.md) — it covers the
> Cloudflare Workers + D1 option, its verified limits, and the blocker that decides
> the question (the free plan's 10 ms CPU budget versus password hashing). Nothing
> in either document has been implemented.

---

## 1. What the implementation actually is (audited)

| Item | Fact | Source |
| --- | --- | --- |
| Runtime | Node.js `>=22.12.0`, ESM (`.mjs`) | `website/package.json` → `engines` |
| Server dependencies | **Zero** third-party runtime packages — `node:http`, `node:fs`, `node:crypto`, `node:path` and local modules only. No native modules, no server build step | `website/server/*.mjs` import audit |
| Build | `pnpm build` → `tsc --noEmit && vite build` → `dist/` (~444 KB: one JS, one CSS, `index.html`, `robots.txt`) | `package.json`, `dist/` |
| Start | `node server/index.mjs` (alias `pnpm portal`) | `server/index.mjs` |
| Package manager | pnpm 9.15.4 via corepack; `website/pnpm-lock.yaml` is standalone (website is **not** in the root pnpm workspace) | `pnpm-workspace.yaml` (only `apps/*`) |
| Process model | One process serves the static site **and** `/api/portal/*` on the same origin | `server/index.mjs` |
| Memory footprint | measured 57 MB RSS at idle | local measurement |
| Bind | `NEXORA_PORTAL_HOST` (default `127.0.0.1`), `NEXORA_PORTAL_PORT` (default `4174`) | `server/config.mjs` |
| Env loading | reads `website/.env` by name-wise merge (process env wins), or `NEXORA_PORTAL_ENV_FILE`; logs key names only | `server/env-file.mjs` |

### HTTP routing the host must not break

Handled **inside the app**, so a plain reverse proxy is enough:

| Path | Behaviour |
| --- | --- |
| `/api/portal/*` | JSON, `Cache-Control: no-store`, cookie session, no CORS (preflight → 405) |
| `/api/portal/status` | Public. Configuration booleans only: `available`, `ownerProvisioned`, `environment` |
| `/api/portal/session` | `POST` sign-in, `GET` current session, `DELETE` sign-out (CSRF token required) |
| `/api/portal/{overview,website,content,support}` | Owner-only `GET` |
| `/robots.txt` | The built file (`Disallow: /` while indexing is off) |
| `/sitemap.xml` | Built file when a domain is configured; otherwise an honest `404 text/plain` — never the SPA shell |
| `/assets/*` | Hashed files, `Cache-Control: public, max-age=31536000, immutable` |
| `/admin`, `/admin/*` | App shell, `X-Robots-Tag: noindex, nofollow`, `Cache-Control: no-cache` |
| other extensionless path | App shell, HTTP `200` (documented limitation: not a real 404) |
| other path with an extension | `404 text/plain` |

`GET` and `HEAD` both work on every route (deployment tests assert it).

### Security requirements the host must satisfy

- **HTTPS**, because `NEXORA_PORTAL_SECURE_COOKIES=true` means the session cookie is
  `Secure`; over plain HTTP nobody can sign in (and the config refuses `http://`
  origins in production).
- **Original `Host` forwarded** — the CSRF origin check compares against
  `NEXORA_PORTAL_ORIGIN`.
- **A single instance** (see §3).
- Production-only headers are set by the app itself: HSTS, `X-Frame-Options: DENY`,
  `Content-Security-Policy: frame-ancestors 'none'`, `X-Content-Type-Options`.
- Sign-in protections already in place: CSRF token + strict origin check,
  `HttpOnly; Secure; SameSite=Strict` cookie, session rotation on sign-in,
  absolute (8 h) + idle (30 min) expiry, per-account (5/15 min), per-IP
  (10/15 min) and per-IP hourly (30/h) rate limits, generic errors,
  constant-time unknown-account path.

### Environment variables (names only — no values here or anywhere)

Required in production: `NEXORA_PORTAL_ENV=production`,
`NEXORA_PORTAL_ORIGIN=https://<confirmed-host>`, `NEXORA_PORTAL_SECURE_COOKIES=true`,
`NEXORA_PORTAL_HOST`, `NEXORA_PORTAL_PORT`, `NEXORA_PORTAL_DATA_DIR`.
Recommended behind a proxy: `NEXORA_PORTAL_TRUST_PROXY=true`.
Optional: `NEXORA_PORTAL_STATIC_DIR`, `NEXORA_PORTAL_SESSION_TTL_SECONDS`,
`NEXORA_PORTAL_SESSION_IDLE_SECONDS`, `NEXORA_PORTAL_ENV_FILE`.
Owner provisioning only: `NEXORA_PORTAL_OWNER_EMAIL`, `_NAME`, `_PASSWORD`.
Build-time, public: `VITE_SITE_URL`, `VITE_SITE_INDEXABLE` (both stay **unset** until
a production origin is confirmed).

---

## 2. Developer account storage — durability answer

**Where:** one JSON document, `portal.json`, in `NEXORA_PORTAL_DATA_DIR`
(default `.portal-data/`, relative to the working directory). Directory `0700`,
file `0600`, atomic write (temp file + rename), corrupt file = hard `503`, never
silently reset.

**What it contains:** Owner record (`id`, normalised email, display name, role,
status, **scrypt** password hash, timestamps), sessions (sha256 token digests +
CSRF tokens + expiry), and the last 200 audit events. No plaintext password, no raw
session token.

**Behaviour, verified locally today** (not on any host):

| Event | Result |
| --- | --- |
| Process restart, same `DATA_DIR` | Owner survives; password unchanged; sessions still valid (tested: sign-in `200` after restart) |
| `NEXORA_PORTAL_OWNER_PASSWORD` removed from env after first start | Sign-in still works — the hash lives in the file, not the environment (tested) |
| Redeploy = new code in the same directory | Survives, because the data dir is outside the deploy tree (by configuration) |
| Crash mid-write | Atomic rename means no truncated file; a genuinely corrupt file is a hard `503`, never a fresh empty store |
| `DATA_DIR` empty (new disk, restored image, wiped container) | `ownerProvisioned:false`, sign-in `401`, UI shows an unconfigured state (tested). If owner env vars are present, a **new** account is created — a new hash, all old sessions gone |
| Two processes on the same file | **Unsupported.** Each keeps an in-memory copy and writes the whole document; last writer wins, so sessions/audit can be lost. Single instance only |

**Therefore:** durability is a property of the *host's disk*, not of the app.
An ephemeral filesystem is not persistence, and no amount of "the app writes to
disk" changes that.

---

## 3. Hosting options checked against current documentation (2026-10-27)

| Provider | Free compute for a new account today | Durable disk at ₹0 | Verdict |
| --- | --- | --- | --- |
| **Oracle Cloud — Always Free** | Yes: 1,500 OCPU-hours + 9,000 GB-hours/month of Ampere A1 (= **2 OCPU / 12 GB** continuously), or 2 × AMD micro (1/8 OCPU, 1 GB) | **Yes**: 200 GB block/boot storage included, home region, for the life of the account; 10 TB/month egress | **Recommended.** Boot volume is real persistence; no code change |
| **Render** | Yes: one free web service (512 MB, 0.1 CPU, 750 instance-hours/month) | **No.** Free instances have an ephemeral filesystem, cannot attach a persistent disk, and lose local files on every redeploy, restart or spin-down. Free Postgres **expires 30 days after creation** | ✗ unsupported for a durable portal |
| **Fly.io** | No. New organisations get a trial only (2 machine-hours or 7 days); no free allowance since Oct 2024; volumes $0.15/GB-month | Only via a paid volume | ✗ not ₹0 |
| **Railway** | Free plan $0 with **$1 of usage credit/month**; RAM billed at $10/GB-month, so a 0.5 GB always-on service (~$5/month) exceeds the credit and stops | Volume storage is $0.15/GB-month and draws on the same credit | ✗ not sustainable at ₹0 |
| **Koyeb** | One free web service is documented, but Koyeb's pricing FAQ states that on entering a card at sign-up (a $29 pre-authorisation hold) they **also charge the pro-rated amount for the plan you select — currently Pro at $29/month**; free instances **cannot attach volumes** | No | ✗ costs money at sign-up |
| **Google Cloud Always Free** | Yes: 1 e2-micro + 30 GB standard PD, but only 1 GB egress/month; Google's VPC pricing bills an **in-use external IPv4 at $0.005/hour (≈$3.65/month)**, and the free tier covers one hour a month | Durable, but not reliably ₹0 for a *public* site | ✗ charge risk |
| **Cloudflare Workers / Deno Deploy** | Yes, with durable KV/D1 free tiers | Durable, **but** requires rewriting the server (no `node:http`/`node:fs`) and adding dependencies | ✗ needs a migration you have not approved |
| **Static hosts** (Pages, Netlify, Vercel, S3) | Yes | Not applicable | ✗ cannot run the portal; would mean removing authentication |

Sources, all provider documentation read on 2026-10-27:
`render.com/docs/free`, `render.com/docs/disks`, `fly.io/docs/about/pricing`,
`docs.railway.com/pricing/plans`, `koyeb.com/docs/faqs/pricing` +
`koyeb.com/docs/reference/instances`, `docs.oracle.com/en-us/iaas/Content/FreeTier/`
`freetier_topic-Always_Free_Resources.htm`, `cloud.google.com/free`,
`cloud.google.com/vpc/network-pricing`. Third-party write-ups were used only to
locate the relevant documentation, never as evidence of a limit.

### One extra compatibility fact that rules out the PaaS options here

`git ls-files website` returns **0 files** — the whole `website/` directory is
untracked on this branch (150 untracked paths, nothing staged). Git-integration
hosts (Render, Railway, Fly, Vercel…) build *from the repository*, so they would
deploy an empty site unless `website/` were committed — a git action that is
explicitly forbidden without approval. A VM is provisioned by copying files over
SSH, so it needs no commit today.

---

## 4. Recommended architecture (₹0)

```
Internet ──HTTPS(443)──▶ nginx (Ubuntu package)
                          ├── /            → 127.0.0.1:4174 (Node, systemd)
                          ├── /api/portal/*→ 127.0.0.1:4174
                          └── TLS: Let's Encrypt via certbot (auto-renew)
Oracle Cloud Always Free VM (Ubuntu 24.04, A1 1 OCPU / 6 GB, 47–50 GB boot volume)
   /srv/nexora/website      ← site + server (rsync from this machine)
   /var/lib/nexora-portal   ← NEXORA_PORTAL_DATA_DIR (durable, 0700, portal.json)
   systemd unit             ← EnvironmentFile=/etc/nexora-portal.env (mode 600)
```

- **Why a VM:** the app is a long-running Node process with a file-based store;
  a VM gives a real disk, a stable hostname, and no sleep/cold-start behaviour.
- **Why nginx + certbot:** both come from the Ubuntu archive (no third-party repo),
  and the app already performs its own SPA fallback and crawl-file handling, so
  nginx only has to proxy and terminate TLS.
- **Sizing:** measured 57 MB RSS for the server; `dist/` is 444 KB. A 1 GB micro is
  enough to *run* it; 6 GB (A1) is chosen so the TypeScript/Vite **build** can also
  run on the box. If A1 capacity is unavailable in the home region, deploy a prebuilt
  `dist/` onto an AMD micro instead — the runtime fits comfortably in 1 GB.
- **No changes to the repository are required.** No new dependency, no store
  migration, no auth change.

### If you reject Oracle (no card on file with a cloud provider)

Honest alternatives, in order of how little they change:

1. **A paid VM or PaaS, cheapest verified option.** Fly.io has no free tier, but its
   published prices make a small always-on deployment cheap: a `shared-cpu-1x`
   Machine with 256 MB RAM is **$2.19/month** and a 1 GB volume is **$0.15/month**
   (`fly.io/docs/about/pricing`) — about **$2.34/month**, billed in USD plus any
   taxes. That is not ₹0, so it needs your explicit approval, but it is the smallest
   safe alternative and needs **no code change either**.
2. **A free serverless platform with a durable store** (Cloudflare Workers + D1/KV,
   Deno Deploy + Deno KV). ₹0 and durable, but the current `server/index.mjs` would
   have to be rewritten: there is no `node:http` or `node:fs` there, so the JSON
   store and the cookie/CSRF layer must be ported. That is a substantial
   architecture migration and needs separate approval before any code is written.
3. **Render free web service, accepting resets.** ₹0, but its filesystem is
   ephemeral: every spin-down, restart or redeploy would lose the Owner account and
   all sessions, and either re-provision from env on each cold start (a new password
   hash, no continuity) or leave the portal unprovisioned. **Not recommended** — a
   portal that silently resets its account is worse than no portal.

---

## 5. Verified cost

| Item | Cost | Notes |
| --- | --- | --- |
| Oracle Always Free compute (A1 or AMD micro) | **₹0** | Free "for the life of the account" within Always Free limits |
| 200 GB block/boot storage | **₹0** | Boot volume counts toward the 200 GB |
| Outbound transfer (10 TB/month) | **₹0** | Far above this site's needs |
| nginx, certbot, Let's Encrypt certificate | **₹0** | Ubuntu packages; LE certs are free |
| Hostname for HTTPS | **₹0 option:** a free `*.duckdns.org` subdomain from DuckDNS (community service, no card; confirm its current terms when you sign up) | A real domain is **not** free and is not included in this plan |
| Object Storage backup (20 GB Always Free) | **₹0** | Optional off-instance copy |
| **Card requirement** | Card (not prepaid/virtual) is required at signup **for identity verification**; Oracle places a temporary authorisation hold and charges nothing for Always Free usage | Not a charge, but it is a card on file. Do not upgrade the account to Pay As You Go |
| Currency/tax | Prices above are ₹0 by definition; any paid resource would be billed in USD plus applicable taxes/GST — outside this plan | No paid resource is part of this plan |

Free-tier terms change: Oracle already halved the A1 allowance in June 2026
(4 OCPU/24 GB → 2 OCPU/12 GB), so the plan is sized to the **current** documented
allowance, not the older headline.

---

## 6. Risks, and what happens on sleep/restart/eviction

| Risk | Effect | Mitigation |
| --- | --- | --- |
| Oracle **reclaims idle** Always Free instances (documented: CPU 95th pct < 20 %, network < 20 %, memory < 20 % over 7 days) | VM deleted → portal account, sessions and audit history gone | Off-instance backup (§8) + documented re-provisioning (~30 min). Traffic alone may not prevent it; if it becomes a real problem, the only safe fix is a paid tier — your decision, not automatic |
| **"Out of host capacity"** in the home region (common for A1) | Cannot create the A1 VM | Fall back to the 2 × AMD micro allowance, or prebuilt `dist/` on a micro; retry later |
| Sign-up rejected (identity/card checks are strict) | No account | Nothing is created until you sign up yourself; alternative providers are listed in §3 as honest fallbacks |
| Single VM, no redundancy; reboots/security patches are ours | Short downtime on restart | systemd `Restart=on-failure`; the store is crash-safe; sessions survive a restart |
| No sleep/cold-start behaviour | — | Unlike Render's free tier (15-min spin-down, ~1 min wake), a VM stays up |
| Home region is permanent | Latency/region cannot be changed later | Pick the India region (Mumbai/Hyderabad) at signup |
| Free hostname is a `duckdns.org` subdomain | Not a brand domain; poor long-term SEO | Use it as a **preview**; indexing stays off; a real domain is a separate, non-₹0 decision |
| The deploy directory is untracked in git | Files copied by hand, not reproducible from the repository | Documented in §10; resolving it needs a commit, which is your call |
| Session/rate-limit state is in one file and one process | A second instance would conflict | Exactly one systemd service, never scale out |

---

## 7. Secrets and Owner provisioning (no secrets in this document)

1. Prod env lives in `/etc/nexora-portal.env` — `chmod 600`, `chown root:nexora`
   (or the service user), loaded by systemd `EnvironmentFile`. **Never** copy the
   local `website/.env`, and never commit any `.env`.
2. A **fresh** Owner password is generated at provisioning time (12+ characters,
   not reused from local development) and typed at a hidden prompt or piped — not
   into shell history, chat, or a file that outlives the step.
3. First start creates the account once (`owner=provisioned`); a restart never
   overwrites it. After that, remove `NEXORA_PORTAL_OWNER_PASSWORD` from the env
   file — the hash is in the data dir, and sign-in keeps working (verified locally).
4. Rotation is explicit: `node server/provision-owner.mjs --from-env --rotate-password`
   (revokes all sessions). Email changes are refused.
5. Local development uses its own `website/.env` and `.portal-data/`; **neither is
   copied to the server**.

---

## 8. Backup and restore at ₹0

- **Primary copy:** `/var/lib/nexora-portal/portal.json` on the boot volume.
- **Off-instance copy, ₹0:** either `scp`/`rsync` the file to your own machine on a
  schedule, or push it to Oracle Object Storage's Always Free 20 GB with `rclone`
  (available in the Ubuntu archive). A copy on the *same* disk is not a backup.
- **Restore:** stop the service → copy the file back into `NEXORA_PORTAL_DATA_DIR`
  → `chown`/`chmod 600` → start → sign in. Sessions live in the backup too, so
  revoke by rotating the password if the copy was exposed.
- **Recovery without a backup:** provision a fresh Owner with the CLI. That is a
  new account and all sessions are invalidated — acceptable, and documented.

---

## 9. Indexing and public-launch policy

Unchanged from Phase 17 until you confirm the final origin:

- `VITE_SITE_URL` / `VITE_SITE_INDEXABLE` stay **unset** → `robots.txt` =
  `Disallow: /`, `sitemap.xml` = honest `404`, page metadata `noindex, follow`.
- `/admin` is `noindex` by header and is never in `SITEMAP_PATHS`.
- A `duckdns.org` preview host must **not** be indexed. When you confirm the final
  domain, one rebuild with both variables set produces `Allow: /`, a 9-URL sitemap
  (no `/admin`), canonical URLs and `og:url` on that origin.
- No download claim changes: `/download` still states that no Windows installer
  exists. Nothing in this plan touches that.

---

## 10. Exact steps for your approval (nothing below has been executed)

**Phase A — you create the account (only you can; card verification is yours)**
1. Sign up for Oracle Cloud Free Tier, choose the **India (Mumbai/Hyderabad) home
   region**, complete identity verification. Do **not** upgrade to Pay As You Go.
2. Create one Ubuntu 24.04 VM: A1.Flex 1 OCPU / 6 GB if capacity allows, else
   `VM.Standard.E2.1.Micro`; boot volume ~50 GB (within the 200 GB allowance).
3. In the VCN security list, allow ingress TCP 80 and 443. On the instance, open
   80/443 in `iptables` (OCI Ubuntu images ship with them closed) and persist it.
4. Create a free `*.duckdns.org` subdomain pointing at the VM's public IP (preview
   only), or supply a real domain you already own.

**Phase B — deploy (I can do this over SSH once you approve and provide access)**
5. Install Node 22 (official tarball or NodeSource; Ubuntu's `nodejs` is 18),
   then `nginx` and `certbot` from the Ubuntu archive.
6. Create the service user, `/srv/nexora/website`, `/var/lib/nexora-portal` (0700).
7. Copy the working tree — **excluding** `.env`, `.portal-data/`, `node_modules/`,
   `dist/` — then `corepack pnpm install --frozen-lockfile` and `pnpm build` on the VM.
8. Write `/etc/nexora-portal.env` (mode 600) with the variable names in §1 and the
   confirmed origin; `NEXORA_PORTAL_HOST=127.0.0.1`, `NEXORA_PORTAL_PORT=4174`,
   `NEXORA_PORTAL_TRUST_PROXY=true`.
9. Install the systemd unit (see `website/DEPLOYMENT.md` §4), `enable --now`.
10. nginx server block proxying `/` and `/api/` to `127.0.0.1:4174` with
    `Host` + `X-Forwarded-For` + `X-Forwarded-Proto`, then `certbot --nginx -d <host>`
    (LE certificate, automatic renewal).
11. **Provision the production Owner** with a fresh password over a hidden prompt;
    confirm sign-in works, then remove the owner password from the env file and
    restart once to prove persistence.
12. Health checks: `GET` **and** `HEAD` on `/api/portal/status` → 200; sign-in →
    `Secure; HttpOnly; SameSite=Strict` cookie; wrong password → generic 401;
    sign-out → session gone; `/admin` → `X-Robots-Tag: noindex`;
    `/sitemap.xml` → 404 (indexing off); restart → still signed in.
13. Schedule the backup (§8) and set a reminder for certificate renewal (automatic)
    and for watching Oracle's idle-reclamation notices.

**Phase C — later, only with separate approval**
14. Point your real domain at the VM, rebuild with `VITE_SITE_URL` +
   `VITE_SITE_INDEXABLE=true`, and only then consider indexing.

---

## 11. What was verified, and what still needs a live test

**Verified here today:** runtime/build/start commands, zero third-party server
dependencies, the exact env-var surface, the routing table and crawl-file behaviour,
cookie flags and CSRF/origin enforcement, production-only headers, restart
persistence on a real filesystem, the empty-store behaviour (`401` /
`ownerProvisioned:false`), measured RSS (57 MB), and that `pnpm install
--frozen-lockfile` works standalone inside `website/`. Provider limits above are
from each provider's current documentation, not from a deployment.

**Not verified (needs the real host, after approval):** that Oracle's Always Free
capacity is available in the chosen region for this account; that signup passes
their verification; TLS issuance for the chosen hostname; the nginx ↔ app proxy in
front of a real TLS terminator; that the VM is not reclaimed as idle; real-device
and screen-reader behaviour; and end-to-end latency from India.

---

## 12. Nothing was changed, and nothing was deployed

No account, project, domain, DNS record, certificate, or credential was created. No
file in this repository was modified, staged, committed or pushed by this task.
The only artifact is this document. Indexing remains disabled.

**Approval requested:** confirm (a) Oracle Cloud Always Free VM as the host,
(b) nginx + Let's Encrypt, (c) the free `duckdns.org` preview hostname for now with
indexing off, and (d) that you will create the Oracle account and pass me SSH access
for Phase B. If you would rather not put a card on file with Oracle, say so and I
will price the smallest paid option instead — I will not present an ephemeral free
tier as a working portal.
