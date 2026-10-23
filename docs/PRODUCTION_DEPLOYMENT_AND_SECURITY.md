# Nexora Production Deployment, Security & Operations Guide

## 1. System Architecture Overview

Nexora separates **local AI inference** completely from **account, subscription, and daily quota management**:

1. **Windows Desktop Application (`apps/desktop`)**:
   - **Frontend (`apps/desktop/ui`)**: React 19 + TypeScript 5.9 + Vite 7 single-window desktop interface with strict Content Security Policy (`connect-src ipc: http://ipc.localhost`). Zero browser-level HTTP access.
   - **Desktop Rust Core (`apps/desktop/src-tauri`)**: Manages `llama-server` (`llama.cpp` commit `c0fd786`) bound strictly to `127.0.0.1:<ephemeral_port>` with a 256-bit per-launch API key file (`0600` permissions), verifies Qwen2.5-Coder 7B (`nexora-mini`) and 14B (`nexora-sonnet`) GGUF SHA-256 digests before activation, stores session tokens exclusively in the OS Credential Vault (`Windows Credential Manager` on Windows), and communicates with `services/nexora-auth` over HTTPS from Rust (`reqwest`).
2. **Account, Billing & Daily Quota Backend (`services/nexora-auth`)**:
   - Rust (`axum 0.8`, `sqlx 0.8`, `argon2 0.5`, `hmac`/`sha2`) + PostgreSQL 16+.
   - Handles Argon2id password registration/login, email verification, password reset, Google OAuth 2.0 + OIDC PKCE loopback code exchange & account linking, Stripe Checkout + Billing Portal + HMAC-SHA256 verified webhooks, and transactional per-plan/per-model daily quota reservations (`00:00:00 UTC` daily boundary).

---

## 2. Environment Configuration & Secret Hygiene

1. Copy `.env.example` to `.env` on the backend host only:
   ```bash
   cp .env.example .env
   chmod 600 .env
   ```
2. **Never commit `.env`** and **never prefix any secret with `VITE_`**.
3. Startup validation rules enforced automatically when `NEXORA_ENV=production`:
   - `NEXORA_AUTH_DATABASE_URL` must be present and free of placeholder markers (`CHANGE_ME`, `REPLACE_WITH`, `example`).
   - If `STRIPE_SECRET_KEY` is configured, it must not use `sk_test_*` in `production`, and `STRIPE_WEBHOOK_SECRET` (`whsec_*`) is mandatory.
   - Startup diagnostics never print secret values to stdout/stderr or logs (`scrub::scrub_secrets` + custom redacted `Debug` implementations on all config/request structs).

---

## 3. Deploying `services/nexora-auth` + PostgreSQL 16

1. **Provision PostgreSQL 16+** with daily encrypted backups and `sslmode=require`.
2. **Build release binary**:
   ```bash
   cargo build --release --locked --manifest-path services/nexora-auth/Cargo.toml
   ```
3. **Run behind TLS reverse proxy** (Caddy / Nginx / Cloudflare Tunnel / AWS ALB terminating TLS 1.3 and forwarding to `127.0.0.1:8787`):
   - All 7 SQL migrations (`migrations/20261018000001_*.sql` through `20261022000007_plans_billing_quotas_audit.sql`) execute automatically at startup before serving `/health`.
   - Server-configured plan catalogue and daily quotas sync automatically to `plans` / `plan_prices` at startup.
4. **Stripe Webhook Endpoint**:
   - Configure Stripe to deliver `checkout.session.completed`, `customer.subscription.created`, `customer.subscription.updated`, `customer.subscription.deleted`, `invoice.paid`, `invoice.payment_failed`, and `charge.refunded` to `POST https://<auth-host>/v1/billing/webhooks/stripe`.
   - Payloads are size-capped (`64 KiB`), verified with constant-time `HMAC-SHA256` against `STRIPE_WEBHOOK_SECRET` with a `±300s` replay window, and deduplicated via `webhook_events.provider_event_id`.
5. **Google OAuth 2.0 Configuration**:
   - Create a **Desktop app** OAuth 2.0 Client ID in Google Cloud Console (per RFC 8252 §7.3, Google Desktop OAuth clients automatically permit dynamic loopback ports on `http://127.0.0.1:<port>/callback`).
   - Set `NEXORA_AUTH_GOOGLE_CLIENT_ID`, `NEXORA_AUTH_GOOGLE_CLIENT_SECRET`, and `NEXORA_AUTH_GOOGLE_REDIRECT_URI=http://127.0.0.1/callback` on the server, and `GOOGLE_OAUTH_CLIENT_ID` when building the desktop app.

---

## 4. Daily Quota Policy & Explicit Offline Behaviour

- **Authoritative Online Mode**: Every chat generation reserves 1 unit via `POST /v1/usage/reserve` (keyed idempotently by `requestId`) **before** starting `llama-server` streaming, commits (`POST /v1/usage/commit`) once tokens begin streaming, or releases (`POST /v1/usage/release`) immediately if local model execution fails before streaming. Unsettled reservations expire automatically after 15 minutes (`expire_stale_reservations_sql`).
- **Reset Boundary**: `00:00:00 UTC` daily; exact next reset timestamp (`resetsAtUtc`) is returned by `/v1/usage` and shown directly in the Chat composer bar and Plans page.
- **Documented Offline Policy**: If an already-authenticated user temporarily has no internet connection (`network_error` reaching `nexora-auth`), Nexora does not falsely claim unlimited Pro quota; it enforces the local tamper-evident conservative daily allowance (`UsageState`) and marks `offlineFallback: true` in the UI until connectivity returns. Unauthenticated users cannot start chat generations until signing in once.

---

## 5. Building, Signing & Verifying the Windows NSIS Installer

On a Windows 10/11 x64 build machine with Node.js 22, `pnpm@9.15.4`, Rust `1.92.0` (`rustup` will read `rust-toolchain.toml`), and Visual Studio C++ Build Tools:

```powershell
# 1. Build per-user NSIS installer + SHA256SUMS.txt (signs with signtool if SIGNTOOL_PFX_PATH is set)
./scripts/windows/build-installer.ps1

# 2. Execute native silent install, process launch sanity check, and clean uninstall check
./scripts/windows/smoke-verify.ps1
```

---

## 6. Release Readiness & Rollback Checklist

- [ ] Review and finalize production pricing & daily per-model quota numbers in `.env` (`NEXORA_PLAN_*`, `NEXORA_QUOTA_*`).
- [ ] Provision production PostgreSQL 16 database and verify encrypted backup restoration test.
- [ ] Configure live Stripe products/keys (`STRIPE_SECRET_KEY=sk_live_...`, `STRIPE_WEBHOOK_SECRET=whsec_...`) and verify live webhook delivery (`200 OK`).
- [ ] Configure Google Cloud Console OAuth consent screen + Desktop Client ID and test live Google PKCE sign-in & account linking.
- [ ] Sign Windows NSIS installer with EV / Azure Trusted Signing certificate and verify Windows SmartScreen / `Get-AuthenticodeSignature` status is `Valid`.
- [ ] Test both real approved GGUF weights (`qwen2.5-coder-7b-instruct-q4_k_m.gguf` and `qwen2.5-coder-14b-instruct-q4_k_m.gguf`) on target Windows 10 and Windows 11 hardware profiles (`8 GB`, `16 GB`, `32 GB` RAM).
- [ ] **Rollback procedure**: Keep previous `nexora-auth` binary + container image tagged; all DB migrations in `services/nexora-auth/migrations` are strictly additive. Re-deploy prior tag and restore PostgreSQL PITR snapshot if schema rollback is ever required.
