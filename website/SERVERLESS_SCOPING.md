# Serverless ₹0 route — scoping report (investigation only)

**Status: nothing built, nothing deployed, no dependency added.** This document
scopes what it would take to run the existing website **and** the working
`/admin` portal on a free serverless platform, because that is the direction you
asked me to check first. It ends with the decisions only you can make.

Companion documents: `LAUNCH_PLAN_ZERO_COST.md` (the VM plan and the full provider
comparison), `DEPLOYMENT.md` (the general launch checklist),
`DEVELOPER_PORTAL.md` (portal architecture, storage, migration path).

---

## 1. What "serverless ₹0" actually means for this app

The only credible zero-cost serverless pair for a durable Node-ish app today:

| Layer | Product | Why this one |
| --- | --- | --- |
| Static site (`dist/`) | **Cloudflare Workers Static Assets** | Requests to static assets are **free and unlimited**; no egress charge |
| API (`/api/portal/*`) | **Cloudflare Workers** (free plan) | 100,000 requests/day, no payment method needed for the free plan |
| Store (accounts, sessions, audit) | **Cloudflare D1** (SQLite) | 5 GB storage, 5M rows read/day, 100k rows written/day, no egress charge |

**Deno Deploy is out.** On 2026-10-09 Deno announced it is joining Cloudflare:
"Deno Deploy will continue operating for six months before shutting down"
(`deno.com/blog/cloudflare`). Launching a portal on a host with an announced
shutdown is not a plan.

Google Cloud Run is the other technically viable serverless host with enough CPU
budget, but it needs a billing account with a card, a cold-start-prone container,
and an external database (Firestore) — so it is *more* migration than Cloudflare,
not less. It is noted in §5 for completeness.

---

## 2. Verified free-tier numbers (read 2026-10-27)

| Limit | Workers Free | Consequence for this app |
| --- | --- | --- |
| Requests | 100,000/day | ~3M/month; a one-owner portal uses a rounding error of this |
| **CPU time** | **10 ms per request** | **Decisive — see §3, blocker B1** |
| Memory | 128 MB per isolate | The server needs ~57 MB; the *converted* API would need far less |
| Worker size / startup | 64 MiB / 1 s | Fine |
| Static asset files | 20,000 per version, 25 MiB each | `dist/` is 4 files |
| Static asset requests | **free and unlimited** | The public site costs nothing and never hits the 100k ceiling |
| D1 storage / reads / writes | 5 GB / 5M rows read/day / 100k rows written/day | Ample: a sign-in is a handful of rows |
| Cron Triggers | 5 per account | Enough for session pruning |
| Workers per account | 100 | One is needed |

Sources: `developers.cloudflare.com/workers/platform/limits/`,
`.../workers/static-assets/billing-and-limitations/`,
`.../d1/platform/pricing/`. Free-plan limits reset daily; exceeding them returns
errors rather than a bill, which is the behaviour we want at ₹0.

---

## 3. Compatibility of the existing server with Workers

The server is ~2,000 lines of zero-dependency Node ESM. On Workers, the parts that
touch the host disappear:

| Module | Today | On Workers | Verdict |
| --- | --- | --- | --- |
| `index.mjs` | `node:http` server, static files from `dist/`, SPA fallback, graceful shutdown | Worker `fetch` handler + Static Assets routing | **Rewrite** |
| `http.mjs` | `node:fs` reads, MIME table, `fs.Stat` cache headers | `env.ASSETS.fetch()` | Rewrite (smaller) |
| `store.mjs` | JSON document, atomic rename, `0700`/`0600`, single-process write queue | **No filesystem.** Becomes D1 tables + SQL, or KV | **Rewrite** |
| `auth.mjs` | `node:crypto` **scrypt** (N=2^15, r=8, p=1) + `timingSafeEqual` | scrypt exists in `node:crypto`, but see **B1** | Blocked |
| `rate-limit.mjs` | in-memory sliding window per process | Isolates are ephemeral and replicated → D1/DO counters | Rewrite |
| `config.mjs` / `env-file.mjs` | env + `.env` loader | `wrangler` secrets/vars; no `.env` file | Rewrite (small) |
| `owner-config.mjs` / `provision-owner.mjs` | CLI that writes the JSON store | No CLI, no disk → one-time bootstrap endpoint or `wrangler d1 execute` | **Rewrite** |
| `api.mjs` | routing, cookies, CSRF, sessions | Same logic, different primitives | Rewrite (largest piece) |
| `server/tests/*` (73 tests) | `node --test` against a real `node:http` server | Must run against `workerd` (wrangler dev) or be re-targeted | **Rework** |
| `src/portal/*` (React) | `fetch("/api/portal/…")`, cookie session | Unchanged — same-origin API | **Survives** |

### The blockers, ranked

**B1 — 10 ms CPU vs password hashing (decisive).**
The Free plan allows 10 ms of CPU per request. Password verification is
deliberately expensive: our scrypt at N=2^15 costs ~50–100 ms. Even the cheapest
"acceptable" alternative is over budget — `crypto.subtle` PBKDF2 in `workerd` is
**hard-capped at 100,000 iterations**, and community measurements put 100k
iterations at **~10–30 ms of CPU**, i.e. at or above the entire free allowance;
OWASP's recommended 600,000 is impossible on the platform. Workers does document
"built-in flexibility" for infrequent overruns, but relying on an unguaranteed
exception for the authentication path is exactly the kind of assumption I will not
put in a launch plan. **Consequence: on Workers Free, a portal password cannot be
verified at recommended strength.** The only ways out are
(a) weaken the KDF — a security downgrade, rejected;
(b) Workers **Paid** ($5/month) for the 30-second CPU limit — not ₹0;
(c) remove the portal's own password login and put `/admin` behind
**Cloudflare Access** (Zero Trust free up to 50 users) — that replaces the
authentication architecture this project just built, which you told me not to do
to make hosting easier;
(d) don't use Workers.

**B2 — there is no filesystem, so the store must be replaced.**
Accounts, sessions and the audit log live in `portal.json`. D1 can hold them
cleanly (three tables and a migration), but this is a data-layer rewrite plus a new
provisioning path — the CLI cannot exist on a platform with no disk.

**B3 — rate limiting loses its home.** The in-memory limiter is per-instance; on
Workers it would reset constantly. It must move to D1 counters (writes count
against the 100k/day budget) or a Durable Object (a new, stateful component).
Doable, but it is new logic, not a port.

**B4 — operational semantics change.** No `PORT`, no `SIGTERM` handler, no
`Host`-based origin derivation; the origin check must read the request URL. Secrets
move from a `600` env file to `wrangler secret put`. All small, all real.

**B5 — tests.** 73 tests drive a real HTTP server on an ephemeral port. They would
need to run against `workerd` or be rewritten around a binding mock. That is the
largest single piece of work and it is the part that protects you from shipping a
broken login.

---

## 4. What the ₹0 serverless version would look like if we did it

```
Cloudflare edge
├── /, /features, /assets/* …   → Static Assets (free, unlimited)
├── /api/portal/session         → Worker: D1 lookup, PBKDF2 verify (B1 risk), cookie
├── /api/portal/overview        → Worker: D1 query, owner-only
└── /robots.txt, /sitemap.xml   → generated by the build, served as assets
D1: users, sessions, audit, rate_limit
Cron: prune expired sessions daily
Secrets: wrangler secret put NEXORA_PORTAL_* (owner email/password only for the
         one-time bootstrap; no origin path trickery)
```

**Effort:** roughly a week of focused work — data layer + API rewrite (~1,200 lines
touched), provisioning bootstrap, rate limiting, config, then reworking 73 tests and
the deployment docs. Calling it "a config change" would be dishonest.

**What it buys:** zero cost forever, no server to patch, no idle-reclamation risk,
global CDN for static assets.
**What it costs:** the authentication path is either weakened, gated by a different
identity provider, or paid for; and the portal stops being the thing this project
designed and tested.

---

## 5. Honest comparison of the three ₹0-shaped paths

| Path | ₹0? | Durable? | Auth stays ours? | Work | Verdict |
| --- | --- | --- | --- | --- | --- |
| Oracle Always Free VM (plan A) | Yes | Yes | Yes, unchanged | Copy files + nginx + certbot, ~1 h | Recommended |
| Cloudflare Workers + D1 + Access gate | Yes | Yes | **No** — Access replaces the login | ~1 week + auth redesign | Only if you decide to delegate admin auth |
| Cloudflare Workers + D1, our own login, weaker KDF | Yes | Yes | Yes, but degraded | ~1 week | **Not recommended** — weakens authentication to fit a free tier |
| Cloud Run + Firestore | Yes within free tier | Yes | Yes | ~1–2 weeks + billing account | More work than Cloudflare for the same result |
| Deno Deploy | Yes | Yes | Yes | ~1 week | **Exclude** — shutting down (announced 2026-10-09) |

---

## 6. My recommendation

Do **not** port the portal to Workers for this launch. The portal exists to prove a
server-authenticated admin surface; moving it to a runtime whose free tier cannot
afford a password hash, and whose auth would then have to be delegated or weakened,
trades the actual requirement (a working, properly authenticated portal) for a
hosting-footprint preference.

If the hard constraint is "₹0 **and** no card anywhere", the only clean option is
Cloudflare Workers **with `/admin` behind Cloudflare Access** — a real
authentication change that I would need you to approve explicitly, and which I would
scope separately (it also removes the `/admin` sign-in page you already have).

If the hard constraint is "₹0 and keep the portal as built", the VM plan stands and
its only real cost is a card on file for account verification, with a temporary hold
and no charge inside Always Free limits.

---

## 7. Decisions needed from you

1. **Is a card on file acceptable** for identity verification only (Oracle, no
   charge), or is "no payment method at all" a hard rule?
2. If no card: **may I replace the portal's password login with Cloudflare Access**
   (Zero Trust, free ≤ 50 users) and keep the dashboard behind it? That is the only
   ₹0, card-free, properly authenticated path — and it deletes the sign-in flow this
   project built.
3. **Is a data-layer rewrite acceptable** (JSON file → D1) if we go serverless,
   knowing the Owner store has no production data yet, so the migration itself is
   trivial but the code and 73 tests must be reworked?
4. **May I weaken the password KDF** to fit 10 ms of CPU? My recommendation is no.
   I will not do it without your explicit, written approval, and I would document it
   as a security downgrade in `DEVELOPER_PORTAL.md`.

Nothing above has been implemented. No dependency, file, account, or deployment
was created by this investigation.
