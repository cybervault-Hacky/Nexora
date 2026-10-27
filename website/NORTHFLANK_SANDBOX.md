# Temporary ₹0 hosting — Northflank Sandbox + Netlify

**Status: configuration only; nothing is deployed.** No Northflank account,
payment method, hostname, Owner credential, or production change has been
created. This is a proposed **temporary, best-effort** setup, not a claim of
production-grade reliability. Northflank describes its free tier as unsuitable
for production applications and provides no SLA on Sandbox.

The existing Node server, authentication, authorization, CSRF/origin checks,
secure cookies, rate limits, and fail-closed configuration are unchanged.

## 1. Repository-provided configuration

| File | Purpose |
| --- | --- |
| `Dockerfile` | Builds the isolated `website/` pnpm workspace on Node 22, then runs only `server/`, `dist/`, and `package.json` as the unprivileged `node` user. |
| `.dockerignore` | Keeps dependencies, previous builds, tests, `.env` files, local overrides, and `.portal-data` out of the Docker build context. |
| `scripts/prepare-netlify-redirects.mjs` | Generates an API proxy only when a real backend origin is explicitly configured. |
| `server/tests/container.test.mjs` | Checks the container contract and runs the final runtime layout without `node_modules`. |
| `server/tests/netlify-proxy.test.mjs` | Checks safe opt-in, fail-closed validation, and stale-proxy removal. |

The final image needs no runtime npm packages: `website/server/*.mjs` imports
only Node built-ins and local modules. `pnpm build` still runs the existing
TypeScript check and Vite build. No `ARG`, Owner variable, password, token, or
secret file is accepted by the image.

## 2. Free-tier decision and limits

Provider terms were checked on 2026-10-27. Re-check them before creating the
service:

- Northflank advertises Sandbox with up to **$20/month of resources**, two
  services, persistent volumes, and **always on, no sleeping**.
- The proposed `nf-compute-20` service (0.2 vCPU, 512 MiB) plus a 1 GiB volume
  is approximately **$5.52/month of that allowance** at published rates, before
  builds, backup storage, and egress. This is an estimate, not a spending cap or
  invoice guarantee.
- Northflank's pricing page says **“No card needed”**, while its documentation
  says all users must add a payment method for identity verification. Treat a
  card as possibly required. Do not add one unless the Owner accepts the current
  signup and billing terms.
- Sandbox has best-effort support, no SLA, and no high availability. A
  single-read/write volume means one instance and downtime during replacement.
- Netlify Free has its own monthly hard quota. Its proxy timeout is 26 seconds.

Sources:
<https://northflank.com/pricing>,
<https://northflank.com/docs/v1/application/billing/pricing-on-northflank>,
<https://northflank.com/docs/v1/application/billing/resource-plans-and-quotas>,
<https://northflank.com/docs/v1/application/databases-and-persistence/add-a-volume>,
<https://docs.netlify.com/manage/routing/redirects/rewrites-proxies/>.

## 3. Northflank service settings (manual, after deployment approval)

Use one combined build/deployment service connected to the approved production
branch of `cybervault-Hacky/Nexora`:

| Setting | Required value |
| --- | --- |
| Build type | Dockerfile |
| Build context | `website` |
| Dockerfile path | `Dockerfile` (relative to that context) |
| Build plan | Smallest build-eligible option shown for Sandbox. The current public list marks `nf-compute-400-16` as build-capable; confirm the account UI rather than selecting a larger/continuous workload. |
| Deployment plan | `nf-compute-20` — 0.2 vCPU / 512 MiB |
| Instances | **1**; no horizontal autoscaling |
| Start command | Image default: `node server/index.mjs` |
| Internal port | `4174` |
| Port protocol/access | `HTTP`, public; use the generated `code.run` HTTPS endpoint |
| Readiness check | HTTP `/api/portal/status` on port `4174` |
| Persistent volume | **Single Read/Write**, initially 1 GiB |
| Container mount path | **`/data`** |
| Region | Closest available Sandbox region that supports both the service and its persistent volume; confirm in the UI rather than assuming a region |

Do not use a local/ephemeral disk for the store. Do not add a second replica,
and do not attach the volume to another live writer. The JSON store keeps an
in-memory copy and supports only one process. The readiness URL checks the HTTP
process; after every start, also verify that its JSON has `available: true` and
that the log says `store=ready`. Do not treat an HTTP 200 alone as proof that
the volume opened correctly.

The container runs as `node`, not root. `/data` must be writable by that user.
If the mounted volume fails the store permission check, stop and correct the
Northflank volume ownership/mount configuration; do not work around it by
making the web root writable, using ephemeral storage, or weakening file modes.

## 4. Runtime environment

Set these as **Northflank runtime variables**, not Docker build arguments:

```dotenv
NEXORA_PORTAL_ENV=production
NEXORA_PORTAL_ORIGIN=https://get-nexora-desktop.netlify.app
NEXORA_PORTAL_SECURE_COOKIES=true
NEXORA_PORTAL_HOST=0.0.0.0
NEXORA_PORTAL_PORT=4174
NEXORA_PORTAL_DATA_DIR=/data
NEXORA_PORTAL_STATIC_DIR=/app/dist
NEXORA_PORTAL_TRUST_PROXY=false
```

`NEXORA_PORTAL_ORIGIN` is the browser-facing Netlify origin, **not** the
Northflank hostname. Production startup refuses a missing/non-HTTPS origin,
insecure cookies, invalid session bounds, or invalid proxy configuration.

Keep `NEXORA_PORTAL_TRUST_PROXY=false` initially. Northflank documents that its
load balancer attaches `X-Forwarded-For`, but Nexora enables forwarded-address
trust only after verifying that the proxy overwrites untrusted client input.
Otherwise the limiter uses the socket address, which can mean shared proxy-IP
rate limiting rather than per-client limiting.

Do not set `NEXORA_PORTAL_BACKEND_ORIGIN` on Northflank. That variable belongs
only to the Netlify build described below. No portal secret uses a `VITE_*`
name or belongs in the frontend.

## 5. First Owner provisioning

Provision through the existing first-start flow so only the live server writes
to its store:

1. Confirm the Single Read/Write volume is attached at `/data` and the service
   is still configured for exactly one instance.
2. The Owner directly enters the following as Northflank **runtime** variables.
   Never send the password through chat, Git, Docker build arguments, Netlify,
   or frontend configuration:

   ```dotenv
   NEXORA_PORTAL_OWNER_EMAIL=<owner-address>
   NEXORA_PORTAL_OWNER_NAME=<display-name>
   NEXORA_PORTAL_OWNER_PASSWORD=<fresh-12+-character-password>
   ```

3. Start/redeploy the one service. The server creates an Owner only when none
   exists and stores only a scrypt hash.
4. Confirm the log reports `store=ready` and `owner=provisioned`, without
   expecting or requesting any credential in logs.
5. Delete `NEXORA_PORTAL_OWNER_PASSWORD` immediately. The email/name bootstrap
   variables can also be removed. Restart/redeploy and confirm the Owner remains
   provisioned from the persistent volume.

`ensureOwnerFromEnvironment()` never overwrites an existing Owner, even if a
stale password or different email remains configured. Startup provisioning is
not password rotation. Do not run `server/provision-owner.mjs` in a shell while
the live server has the same store loaded; that would create a second writer.
Rotation requires a separate controlled-downtime maintenance procedure.

## 6. Connect Netlify without hardcoding an unknown hostname

The tracked `netlify.toml` retains these static protections:

```text
/api   -> /404.txt  404
/api/* -> /404.txt  404
```

The Netlify build command is `pnpm --dir website build:netlify`. It runs the
normal build and then `scripts/prepare-netlify-redirects.mjs`.

After Northflank supplies its **actual** public HTTPS hostname, set this Netlify
environment variable in the **Production deploy context, Builds scope**:

```dotenv
NEXORA_PORTAL_BACKEND_ORIGIN=https://<actual-northflank-hostname>
```

Use the bare origin only. Do not include `/api`, a path, query, fragment,
credentials, or a made-up hostname. On the next approved Netlify build, the
helper generates `website/dist/_redirects` containing:

```text
/api https://<actual-northflank-hostname>/api 200!
/api/* https://<actual-northflank-hostname>/api/:splat 200!
```

Netlify processes the publish-directory `_redirects` before `netlify.toml`, so
these forced rules proxy only `/api` while the SPA fallback remains last.
Deploy previews and branch deploys should not receive this variable; they
retain the static API 404 instead of calling the production Owner API.

Safety behavior:

- **Unset/blank variable:** no `_redirects` is emitted (and a stale generated
  file is removed), so the tracked API 404s remain active.
- **Invalid/non-HTTPS value:** the build exits non-zero; it never falls back to
  an insecure or invented proxy.
- **Rollback:** remove the variable and perform an approved production rebuild.
  The next build has no generated proxy and restores the static API 404s.

The backend hostname is public routing configuration, not a secret. Owner
credentials and session material must never be placed in this variable or any
other Netlify setting.

## 7. Backup and restore

A persistent volume is not a backup. Northflank volume snapshots are
provider-managed recovery points and can miss in-flight writes; this setup does
not provide independent off-provider disaster recovery.

### Before provisioning and before changes

1. Pause/scale the service to zero for a write-quiet maintenance window.
2. In the service's **Volumes** area, create a manual snapshot of the portal
   volume. For a brand-new empty volume, a snapshot is optional; after Owner
   provisioning it is mandatory before further maintenance.
3. Start exactly one instance and verify `/api/portal/status`.
4. Monitor snapshot/backup storage against the Sandbox allowance.

Keep at least one known-good snapshot before upgrades. Test restoration rather
than assuming a snapshot is usable. Project deletion, volume deletion, account
problems, and provider failure remain outside the application's control.

### Restore to a new volume

1. Pause/scale the service to zero so no writer is active.
2. From the selected volume backup, create/restore a **new** Single Read/Write
   volume of at least the original size.
3. With the service still stopped, detach the old volume and attach the restored
   volume at **`/data`**. Do not attach old and restored copies to two live
   services.
4. Start one instance. Confirm `store=ready`, `owner=provisioned`, and have the
   Owner verify access and recent audit state.
5. Keep the old volume until restoration is verified. Deleting it is a separate,
   explicit destructive decision.

## 8. Required live verification (cannot be completed locally)

After an approved deployment, and without simulated sign-in or shared
credentials:

- `GET /api/portal/status` through Netlify returns `200` JSON.
- Signed-out `GET /api/portal/session` returns `401` JSON, never the SPA shell.
- The Owner performs the real sign-in. The Netlify-origin cookie is
  `HttpOnly; Secure; SameSite=Strict`.
- Methods, bodies, `Origin`, `Sec-Fetch-Site`, cookies, and `Set-Cookie` survive
  Netlify → Northflank. CSRF/origin rejection and Owner authorization still work.
- Sign-out revokes the session. A backend restart preserves the Owner and valid
  persisted sessions.
- API/security headers remain `no-store`, `noindex`, HSTS, frame denial, and
  `nosniff` as implemented by the server.
- Volume permissions, usage/cost, backup creation, and backup restoration are
  verified on the actual service.

If this chain cannot preserve the security contract, remove the Netlify backend
variable and restore the static API 404s. Do not disable security checks to make
the proxy work.
