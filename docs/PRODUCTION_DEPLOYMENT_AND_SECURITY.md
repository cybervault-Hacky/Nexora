# Nexora Production Deployment, Security & Operations Guide

> **Pre-release reference only (2026-10-25): NO-GO.** This guide describes source/configuration and prospective controls; it is not evidence of a deployed service, production validation, legal approval, or release readiness. Step 1 remains **IMPLEMENTED — VALIDATION BLOCKED**; Step 2 remains **PARTIAL — IMPLEMENTATION COMPLETE, VERIFICATION BLOCKED**; Step 3 remains **PARTIAL — IMPLEMENTATION COMPLETE, WINDOWS VERIFICATION BLOCKED**; Step 4 remains **PARTIAL — runtime, legal, and operational release gates blocked**. Step 5 final gate is **BLOCKED — NOT VERIFIED**. Do not provision production systems or present account, billing, model, or Windows behavior as verified from this document. See the [current Public Beta preparation report](PUBLIC_BETA_PREPARATION_WINDOWS.md), [security reporting placeholders](../SECURITY.md), [support/diagnostics prerequisites](SUPPORT_AND_DIAGNOSTICS.md), and [incomplete third-party notices draft](THIRD_PARTY_NOTICES_DRAFT.md).

## 1. System Architecture Overview

Nexora separates **local AI inference** completely from **account, subscription, and daily quota management**:

1. **Windows Desktop Application (`apps/desktop`)**:
   - **Frontend (`apps/desktop/ui`)**: React 19 + TypeScript 5.9 + Vite 7 single-window desktop interface with strict Content Security Policy (`connect-src ipc: http://ipc.localhost`). Zero browser-level HTTP access.
   - **Desktop Rust Core (`apps/desktop/src-tauri`)**: Manages `llama-server` (`llama.cpp` commit `c0fd786`) bound strictly to `127.0.0.1:<ephemeral_port>` with a per-launch API key in a create-new file (`0600` on Unix; protected by the per-user app-data directory ACL on Windows), verifies Qwen2.5-Coder 7B (`nexora-mini`) and 14B (`nexora-sonnet`) GGUF SHA-256 digests before activation, stores Windows session tokens in Credential Manager and uses process-only memory on non-Windows, and communicates with `services/nexora-auth` over validated HTTPS from Rust (`reqwest`). Local chat does not require sign-in or account-service availability.
2. **Account, Billing & Daily-Usage Backend (`services/nexora-auth`)**:
   - Rust (`axum 0.8`, `sqlx 0.8`, `argon2 0.5`, `hmac`/`sha2`) + PostgreSQL 16+.
   - Contains account/auth flows and Stripe-oriented billing source code; no live payment behavior is asserted here. The production `QuotaPolicy` defaults to unconfigured, so daily quota/entitlement enforcement is not active. Server usage summaries do not assert a plan, limit, or reset without explicit policy configuration.

---

## 2. Environment Configuration & Secret Hygiene

1. Copy `.env.example` to `.env` on the backend host only:
   ```bash
   cp .env.example .env
   chmod 600 .env
   ```
2. **Never commit `.env`** and **never prefix any secret with `VITE_`**.
3. Startup validation and activation rules:
   - Release `nexora-auth` requires explicit `NEXORA_ENV=development|test|production`; production requires `NEXORA_AUTH_DATABASE_URL` and rejects placeholder values. Remote production PostgreSQL URLs must specify `sslmode=require`, `verify-ca`, or `verify-full`; loopback DBs are exempt.
   - Desktop environment mode is pinned at build time (`production` by default for release builds); a runtime mode mismatch is rejected so it cannot downgrade a production package. Production account endpoints are compiled into the desktop binary and runtime endpoint overrides are rejected. Set the build-time mode and URL deliberately for release QA.
   - Optional Google OAuth must be configured as a complete client-ID/loopback-redirect pair; placeholder client IDs/secrets and non-loopback redirects are rejected. The confidential OAuth client secret stays backend-only. Email, SMTP, and remote auth services remain explicitly unconfigured when absent.
   - Stripe settings remain audit-only: production composition always uses `UnconfiguredPaymentProvider` and passes no webhook secret, even if a complete Stripe credential pair is present. Credential presence never enables checkout, portal, webhooks, a price, quota, or entitlement.
   - Startup diagnostics and request/debug output redact secret-bearing values; the static frontend build check rejects backend-only variable names and recognizable credential patterns in the built UI.
   - Provider credentials do not approve pricing, quotas, plan features, or entitlement rules. Step 1 removes the client-selected active/trialing plan-change path from current source and makes change/cancel requests fail closed, but Rust/PostgreSQL validation is blocked; do not treat unverified rows or payment/backend failures as Premium/quota evidence. See [Phase 17 configuration/security reference](PHASE_17_CONFIGURATION_SECURITY.md) for source, sensitivity, defaults, and validation of settings.

---

## 3. Deploying `services/nexora-auth` + PostgreSQL 16

1. **Provision PostgreSQL 16+** with daily encrypted backups and remote TLS (`sslmode=require`, `verify-ca`, or `verify-full`); the startup validator exempts loopback DB hosts.
2. **Build release binary**:
   ```bash
   cargo build --release --locked --manifest-path services/nexora-auth/Cargo.toml
   ```
3. **Run behind TLS reverse proxy** (Caddy / Nginx / Cloudflare Tunnel / AWS ALB terminating TLS 1.3 and forwarding to `127.0.0.1:8787`):
   - All 9 SQL migrations (`migrations/20261018000001_*.sql` through `20261024000009_subscription_provider_event_ordering.sql`) are embedded in the release binary and execute automatically at startup before serving `/health`.
   - Startup syncs the repository's draft plan catalogue and stored model-limit rows to `plans`, `plan_versions`, and `plan_model_quotas`. Persisted rows are not proof of commercial approval or paid entitlement. `AppState` starts with `QuotaPolicy::unconfigured`; `/v1/plans` returns no catalogue, `/v1/usage` omits plan/limit/reset fields, and reservation requests return typed `quota_policy_unconfigured` (`503`). Payment-provider composition is forcibly unconfigured in this build regardless of Stripe credentials. Client-selected plan changes and cancellation also fail closed in current source; the focused Rust/PostgreSQL tests remain unexecuted. This is static evidence, not a verified billing deployment. Keep paid access disabled.
4. **Existing Stripe-Oriented Billing Source (disabled in service composition)**:
   - The source retains checkout, portal, and webhook handlers, but production composition always supplies an unconfigured provider and no webhook secret; routes requiring a provider fail as unavailable. Desktop browser launch also rejects every non-Google destination. Do not infer verified live payment behavior from route/source presence.
   - G-01 Step 1 source correction: checkout activation now requires the stored Stripe order, owner, checkout session, provider, paid/completed state, matching server-stored amount/currency, active subscription, provider-reported period end, and refund-trackable payment intent to match; subscription IDs cannot be upserted across accounts. Webhook transitions are event-time ordered and serialized by provider identity; unpaid, paused, unknown, or terminal provider states cannot leave stale paid status active, prior refunds/terminal states cannot be bypassed by delayed activation, terminal states cannot reactivate, and plan changes/cancellation remain fail closed. A new additive migration (`20261024000009_subscription_provider_event_ordering.sql`) adds the event watermark and refund/lifecycle evidence columns without rewriting billing records. Rust/PostgreSQL regression tests and this migration could not run in the current environment, so mark the correction as implemented but validation blocked—not release-verified. Do not rely on unverified subscription rows as paid/Premium/quota evidence.
5. **Google OAuth 2.0 Configuration**:
   - Create a **Desktop app** OAuth 2.0 Client ID in Google Cloud Console (per RFC 8252 §7.3, Google Desktop OAuth clients automatically permit dynamic loopback ports on `http://127.0.0.1:<port>/callback`).
   - Set `NEXORA_AUTH_GOOGLE_CLIENT_ID`, `NEXORA_AUTH_GOOGLE_CLIENT_SECRET`, and `NEXORA_AUTH_GOOGLE_REDIRECT_URI=http://127.0.0.1/callback` on the server, and `GOOGLE_OAUTH_CLIENT_ID` when building the desktop app.

---

## 4. Daily Usage Accounting, Entitlements & Local Chat

- **Current production quota-policy state**: `AppState` uses `QuotaPolicy::unconfigured`; no Free/Premium quota, model-specific allowance, token cap, trial, paid entitlement, feature-access rule, or quota reset/billing period is approved by this work. Stripe checkout/subscription handlers and draft plan defaults remain in source; production provider composition is unconfigured and unsupported plan/cancel handlers fail closed. Rust/PostgreSQL verification remains blocked, so keep provider/payment configuration disabled. A missing quota policy is neither unlimited paid access nor zero Free access. The server returns an unconfigured summary without plan/limit/reset values; reserve operations return typed `quota_policy_unconfigured` (`503`).
- **Local usage record**: The desktop stores a UTC-day count of accepted local generations for informational presentation. There is no hardcoded local/offline allowance, no `offlineFallback` entitlement, and no claim that the device has Free/Premium access. Corrupt/unreadable usage is left unknown rather than reset to a fresh count.
- **Generation independence**: Local chat and conversation history remain available signed out and when the session vault, auth/payment backend, or account quota policy is unavailable. A token is sent only when a readable session exists; server identity is derived from the verified session. The desktop never invents an account, token, plan, or successful account-usage result.
- **Best-effort account accounting**: With a valid session, generation attempts an idempotent server reservation before local streaming. Reservation, quota, and session errors do not gate local inference. If the local engine accepts the request, the desktop attempts idempotent settlement; transient settlement failures are retried once. Remaining account reservation/settlement and local-recording errors are surfaced as usage warnings while the local reply and conversation remain available. No prompt or file content is sent to the auth service.
- **UTC and reservation mechanics**: When an explicitly reviewed account quota policy is configured in a future authorized deployment, the backend uses PostgreSQL UTC-day boundaries and transactional reservations. Stale reservations are swept at 15 minutes; expired commits are rejected after expiry bookkeeping. This is accounting mechanics only, not approval of a daily allowance or entitlement. Rust/PostgreSQL runtime validation remains pending.

---

## 5. Building, Signing & Verifying the Windows NSIS Installer

On a Windows 10/11 x64 build machine with Node.js `22.22.3`, `pnpm@9.15.4`, the pinned Rust/Cargo toolchain `1.93.0` from `rust-toolchain.toml`, and Visual Studio 2022 C++ Build Tools plus the Windows SDK (run from an x64 developer shell):

```powershell
# 1. Build the per-user NSIS installer + SHA256SUMS.txt. Optional signing requires an approved certificate in CurrentUser\My and SIGNTOOL_CERT_THUMBPRINT; PFX path/password inputs are rejected.
./scripts/windows/build-installer.ps1

# 2. Run install, shortcut, same-build reinstall, uninstall, data-preservation, and reinstall-after-uninstall checks.
#    GUI launch is tested only in an interactive local session; headless CI reports that it skipped GUI verification.
./scripts/windows/smoke-verify.ps1
```

The guarded build installs the frozen pnpm lockfile, runs TypeScript/Vitest and the
installer-contract tests, explicitly runs `pnpm build` before native Cargo checks
(because `generate_context!` embeds `ui/dist`), then checks Rust formatting,
Clippy, and tests before invoking the Tauri production build. Tauri's configured
build hook runs the frontend/security/contract scans again. The source
configuration is NSIS/current-user only, with a silent WebView2 Evergreen
bootstrapper; no model weights or runtime are configured as bundle resources.
Tauri 2.12.5 pins NSIS 3.13 and validates its upstream toolchain/package hashes
using SHA-1 (not an installer signature). The build refuses existing `.exe` or
`SHA256SUMS.txt` files rather than reusing or overwriting stale output, requires
one fresh setup executable, and creates the SHA-256 manifest with create-new
semantics after optional Authenticode signing. Signing requires a current-user
certificate with a private key, Code Signing EKU, and current validity; the
result must verify as `Valid` and match the configured thumbprint. The pinned
NSIS template offers an explicit interactive app-data deletion checkbox, skips
data deletion during an upgrade, and does not remove the shared WebView2 runtime.
The smoke script refuses test install paths overlapping the default install or
data roots, checks data sentinels and WebView2 across its silent
uninstall/reinstall cycle, and leaves changed sentinels/unexpected files for
manual review instead of recursively deleting them. These procedures have not
yet been parsed/executed on Windows; authored smoke checks and pinned
upstream-template inspection are not Windows acceptance evidence.

**Step 3 evidence status (2026-10-24):** On Linux x86_64, the available
`PATH="/tmp/nexora-pnpm-bin:$PATH" corepack pnpm check` completed successfully:
TypeScript, 88 Vitest files / 873 tests, installer-contract 17/17, production
frontend build, config-security scan, and installer-contract scan. The production
contract scan reported 4 frontend assets / 720,456 bytes and no model/runtime/
credential payload. Vite emitted non-fatal warnings for `theme-init.js`, mixed
static/dynamic imports, and a 590.21 kB minified JS chunk. The local host has no
PowerShell, Rust/Cargo, MSVC/Windows SDK, NSIS, or SignTool, so the scripts remain
unparsed/unexecuted and no Windows installer/hash/signature/install/lifecycle
result exists. The existing `windows-2022` workflow was configured to initialize
x64 MSVC, but no run was triggered and no run exists for the current branch.

---

## 6. Release Readiness & Rollback Checklist

- [ ] Obtain explicit product/legal approval for prices, plans, per-model quotas, token allowances, trials, features, and policy-dependent periods before configuring any commercial policy; current quotas remain unconfigured.
- [ ] Provision production PostgreSQL 16 database and verify encrypted backup restoration test.
- [ ] Keep paid access disabled. The client plan-change/cancellation paths fail closed in source, but no provider-side lifecycle operation has been implemented or verified; the isolated Rust/PostgreSQL regression tests remain blocked. Stripe stays disabled here; no live payment/webhook test was run.
- [ ] Keep Stripe/provider credentials unconfigured. Production composition remains unavailable even if credentials exist; the fail-closed source correction is not runtime/database verification, and the unconfigured quota policy is not itself entitlement approval.
- [ ] Razorpay remains blocked and unimplemented; do not assume an order, subscription, refund, migration, or sandbox capability exists.
- [ ] Configure Google Cloud Console OAuth consent screen + Desktop Client ID and test live Google PKCE sign-in & account linking.
- [ ] Obtain approval for the publisher/signing identity and signing procedure; sign the final installer only with the approved certificate-store identity, then verify its Authenticode signature on Windows. No signing identity or certificate is configured or verified by this repository audit.
- [ ] Test both real approved GGUF weights (`qwen2.5-coder-7b-instruct-q4_k_m.gguf` and `qwen2.5-coder-14b-instruct-q4_k_m.gguf`) on target Windows 10 and Windows 11 hardware profiles (`8 GB`, `16 GB`, `32 GB` RAM).
- [ ] **Rollback procedure**: Keep previous `nexora-auth` binary + container image tagged; all DB migrations in `services/nexora-auth/migrations` are strictly additive. Re-deploy prior tag and restore PostgreSQL PITR snapshot if schema rollback is ever required.
