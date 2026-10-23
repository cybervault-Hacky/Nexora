# Nexora Backend & Cross-Layer Architecture Decision Record (ADR)

**Status:** Approved Phase 02 Architecture Baseline (Implementation changes deferred to Phases 03–20)
**Date:** `2026-10-22`
**Branch:** `arena/c01ae5dd-nexora` (`HEAD`: `631a59e379281ae581fde401aace5ae913460e84`)
**Scope:** Canonical architecture specification across `apps/desktop/ui`, `apps/desktop/src-tauri`, local `llama-server`, `services/nexora-auth`, and PostgreSQL 16+.
**Relationship to Existing Docs:** Supersedes the early pre-implementation sketch in `docs/phase-1/03-application-architecture.md` (which proposed hypothetical `crates/nexora-*` and SQLite layers never adopted by the codebase) and governs updates to `docs/PRODUCTION_DEPLOYMENT_AND_SECURITY.md` in Phase 17 / Phase 20 without deleting historical records.

---

## 1. System Architecture Overview

Nexora enforces a strict **two-plane architecture** that separates **on-device AI inference and workspace code operations** from **online account, authentication, billing, and daily quota accounting**:

```
┌──────────────────────────────────────────────────────────────────────────────────────────┐
│ USER WINDOWS 10/11 MACHINE (Local Trust Plane — Prompts, Code & Models Never Leave PC)   │
│                                                                                          │
│  ┌────────────────────────────────────────┐    Typed Tauri 2 IPC (`invoke` + `Channel`)  │
│  │ WebView2 Renderer (`apps/desktop/ui`)  │◄──────────────────────────────────────────┐  │
│  │ React 19.3 + TypeScript 5.9 + Vite 7   │  CSP: `connect-src ipc: http://ipc...`    │  │
│  │ Zero browser `fetch`, zero raw tokens  │                                           │  │
│  └────────────────────────────────────────┘                                           │  │
│                                                                                       ▼  │
│  ┌─────────────────────────────────────────────────────────────────────────────────────┐ │
│  │ Native Desktop Core (`apps/desktop/src-tauri`, crate `nexora` / `nexora_lib`)       │ │
│  │  • `inference/*`: registry, SHA-256 verify, hardware check, SSE stream, budget      │ │
│  │  • `history/*`, `workspace/*`, `export/*`: atomic local JSON/files & path guards    │ │
│  │  • `usage/*`: tamper-evident UTC offline daily allowance (`daily-usage.txt`)        │ │
│  │  • `auth/*`: OS `keyring` vault, RFC 8252 PKCE listener, `AuthApiClient` (`reqwest`)│ │
│  │  • `licensing/*`: offline Community/key inspection (non-authoritative for quotas)   │ │
│  └──────────────┬───────────────────────────────────────────────────┬──────────────────┘ │
│                 │ Loopback HTTP SSE (`127.0.0.1:<ephemeral>`)       │ HTTPS (`reqwest`   │
│                 │ Bearer 256-bit per-launch `--api-key-file`        │ `rustls-tls`)      │
│                 ▼                                                   │                    │
│  ┌────────────────────────────────────────┐                         │                    │
│  │ Local Sidecar (`llama-server`)         │                         │                    │
│  │ Loads SHA-256-verified `.gguf` weights │                         │                    │
│  │ Mini 7B `Q4_K_M` / Sonnet 14B `Q4_K_M` │                         │                    │
│  └────────────────────────────────────────┘                         │                    │
└─────────────────────────────────────────────────────────────────────┼────────────────────┘
                                                                      │
                                      TLS 1.3 Reverse Proxy (Prod)    ▼
┌──────────────────────────────────────────────────────────────────────────────────────────┐
│ ONLINE ACCOUNT, BILLING & QUOTA PLANE (`services/nexora-auth` + PostgreSQL 16+)          │
│                                                                                          │
│  ┌─────────────────────────────────────────────────────────────────────────────────────┐ │
│  │ `services/nexora-auth` (Standalone Rust Service — `axum 0.8`, `tokio`, `sqlx 0.8`)  │ │
│  │  • `registration`, `login`, `session`, `recovery`, `rate_limit`: Argon2id + SHA-256 │ │
│  │  • `oauth`: server-side Google OAuth code exchange + RS256 JWKS ID-token validation │ │
│  │  • `email`: `EmailProvider` trait (currently `UnconfiguredEmailProvider` only)      │ │
│  │  • `account`: profile, safe Google linking, GDPR JSON export, cascade deletion      │ │
│  │  • `billing`: server plan sync, `PaymentProvider` (Stripe), HMAC-SHA256 webhooks    │ │
│  │  • `quota` & `audit`: UTC daily `FOR UPDATE` reservations + scrubbed audit events   │ │
│  └──────────────┬──────────────────────────────────┬────────────────┬──────────────────┘ │
│                 │ `sqlx` (`sslmode=require` prod)  │ HTTPS          │ HTTPS              │
│                 ▼                                  ▼                ▼                    │
│  ┌──────────────────────────────┐   ┌───────────────────┐  ┌───────────────────────────┐ │
│  │ PostgreSQL 16+ (16 tables,   │   │ Google OAuth/OIDC │  │ Stripe Checkout, Billing  │ │
│  │ 7 forward-only migrations)   │   │ + SMTP/Mail API   │  │ Portal & Signed Webhooks  │ │
│  └──────────────────────────────┘   └───────────────────┘  └───────────────────────────┘ │
└──────────────────────────────────────────────────────────────────────────────────────────┘
```

### Why No Duplicate Service or Technology Migration Is Needed
- **Verified Codebase Decision:** Keep the existing 3-artifact topology intact: `apps/desktop/ui` (React 19 + TS), `apps/desktop/src-tauri` (Tauri 2 Rust host), and standalone `services/nexora-auth` (Axum 0.8 + SQLx 0.8 + PostgreSQL 16).
- **Rationale:** `services/nexora-auth` is intentionally outside the Cargo and `pnpm` workspaces (`services/nexora-auth/Cargo.toml:9-13`) so server dependencies (`axum`, `sqlx`, `argon2`, `jsonwebtoken`) never bloat the Windows desktop binary or introduce server crates into client builds.

---

## 2. Component Responsibility Matrix & Inviolable Boundaries

| Responsibility | A. Desktop App (`ui` + `src-tauri`) | B. Local Runtime (`llama-server`) | C. Online Backend (`nexora-auth`) | D. PostgreSQL 16+ Database | Forbidden Boundary Crossings |
| --- | --- | --- | --- | --- | --- |
| UI rendering, theme, draft & route state | **Owns (`ui/`)** | None | None | None | UI must **never** call `fetch`/`XMLHttpRequest`, spawn processes, or compute authoritative quota math. |
| Typed IPC validation, filesystem containment, atomic patches & ZIP export | **Owns (`src-tauri/src/{workspace,export,history}`)** | None | None | None | Symlinks, `..` traversal, secrets (`.env`, `.pem`, `.key`), and stale file hashes must **never** bypass `src-tauri` guards. |
| OS credential storage (`keyring` DPAPI on Windows; memory fallback elsewhere) | **Owns (`src-tauri/src/auth/storage.rs`)** | None | Issues opaque raw token once on login/exchange/refresh | Stores `sha256(token)` only (`sessions.token_hash`) | Raw session tokens must **never** enter `WebView2` (`localStorage`, React state, IPC return types) or SQL plaintext columns. |
| Approved model metadata, `.part` downloads, GGUF SHA-256 verification & RAM/disk checks | **Owns (`src-tauri/src/inference/{registry,acquisition,models,readiness}.rs`)** | Loads verified `.gguf` file path passed on CLI | Maps profile IDs to quota buckets (`quota::canonical_model_id`) | Stores per-plan daily limits (`plan_model_quotas`) | Unverified `.gguf` files must **never** be loaded under an approved profile; prompts/files must **never** be sent to `nexora-auth`. |
| Token generation, SSE streaming & mid-stream stop | Orchestrates (`src-tauri/src/inference/{process,client,sse,commands}.rs`) | **Owns (`127.0.0.1:<port>`, `--api-key-file`)** | Reserves/settles 1 message count only (zero prompt payload) | Records count only (`daily_usage_counters`) | `llama-server` must **never** bind `0.0.0.0`, enable web UI, or outlive desktop shutdown (`lib.rs:42-49`). |
| Password hashing (Argon2id), email verification, password reset & sliding-window rate limits | Collects form inputs via `AuthModal` / `AccountPage` | None | **Owns (`registration`, `login`, `recovery`, `rate_limit`)** | **Persists** PHC hashes & `sha256` token digests | Plaintext passwords and raw verification/reset tokens must **never** be stored in PostgreSQL or written to logs. |
| Google OAuth 2.0 + OIDC PKCE sign-in & linking | Generates S256 PKCE + `state` + `nonce`, binds `127.0.0.1:0` (`auth/pkce.rs`) | None | **Owns (`oauth.rs`, `account.rs`)**: consumes single-use `state_hash`, exchanges code with secret, validates RS256 JWKS + claims | **Persists** `oauth_accounts` (`provider`, `provider_subject`) & `google_oauth_states` | `NEXORA_AUTH_GOOGLE_CLIENT_SECRET` must **never** exist in `VITE_*`, `src-tauri`, or installer artifacts; unverified emails are never auto-linked. |
| Plan catalogue, Stripe Checkout/Portal & signed webhooks | Renders server DTOs (`PlansPage.tsx`), opens validated `https://` URLs | None | **Owns (`billing.rs`)**: syncs env prices/quotas, signs/verifies Stripe calls & `HMAC-SHA256` webhooks | **Persists** `plans`, `plan_versions`, `subscriptions`, `payment_orders`, `webhook_events` | Server must **never** trust client-submitted prices, intervals, order statuses, or entitlement tiers. |
| Daily message quota accounting (`00:00 UTC` boundary) | Preflights via `AuthApiClient`; enforces conservative local `UsageState` **only** on `network_error` | None | **Owns (`quota.rs`)**: authoritative plan lookup + `FOR UPDATE` reservation/commit/release | **Persists** `daily_usage_counters` & `quota_reservations` | Offline local `UsageState` must **never** overwrite server counters or grant paid `Pro`/`Team` limits without server verification. |

> **Note on Legacy Local `licensing/*` Module (`apps/desktop/src-tauri/src/licensing/`):** Preserved for backward UI/test compatibility (`#/licensing`), `licensing/*` validates local offline key shapes only and **does not grant server subscription entitlements or raise `start_generation` daily quotas**. Online plan entitlements flow exclusively through `services/nexora-auth` (`billing.rs` + `quota.rs`).

---

## 3. Traced Request & Data Flows

### 3.1 Local Chat Generation + Quota Reservation Lifecycle
1. **UI (`apps/desktop/ui/src/pages/ChatPage.tsx` -> `usePersistentConversation.ts`)**: Invokes `start_generation` (`GenerateRequest { requestId, messages, maxTokens, temperature, profileId }`) with a Tauri `Channel<GenerationEvent>`. If unauthenticated, UI opens floating `AuthModal` without clearing the draft prompt.
2. **Rust Gate (`apps/desktop/src-tauri/src/inference/commands.rs:716-820`)**:
   - Reads bearer token from OS `storage::get_session_token()` (returns `unauthenticated` before touching `llama-server` if missing).
   - Validates prompt roles/lengths, verifies `request.profile_id == selected.id` and `selected.id == state.loaded_profile_id()`, checks token budget (`budget::overflow`), and probes `process::health_ok(port)`.
   - Calls `POST /v1/usage/reserve` with `{ modelId: selected.id, requestId }`:
     - **`200 OK`**: Holds server `reservationId`.
     - **`401`**: Clears local session token and returns `unauthenticated`.
     - **`429 quota_exceeded`**: Returns server quota error immediately; `llama-server` never runs.
     - **Transport `network_error`**: Falls back to local `UsageState::reserve(SystemTime::now())` (`50`/day UTC provisional local cap).
3. **Streaming & Settlement (`commands.rs:822-960`)**:
   - Streams tokens over loopback `POST http://127.0.0.1:<port>/v1/chat/completions` (`Authorization: Bearer <ephemeral_key>`).
   - If `llama-server` rejects or fails **before** streaming starts: releases server reservation (`POST /v1/usage/release`) or drops uncommitted local `UsageReservation`.
   - Once token streaming starts: commits server reservation (`POST /v1/usage/commit`) or commits local `UsageReservation`, and streams `Token` / `Done` / `Cancelled` / `Error` events over the Tauri `Channel`.

### 3.2 Desktop Google OAuth 2.0 + OIDC PKCE Sign-In & Account Linking
1. **Desktop PKCE Setup (`auth/pkce.rs`, `auth/commands.rs:255-315`)**: Generates CSPRNG 256-bit `code_verifier` (`S256` challenge), 128-bit `state`, and 128-bit `nonce` (`getrandom`); binds `127.0.0.1:0` -> `http://127.0.0.1:<port>/callback`; validates `https://` URL and opens the system browser via `rundll32.exe url.dll,FileProtocolHandler` on Windows (preventing `cmd.exe` `&` injection).
2. **Loopback Callback (`pkce.rs:119-207`)**: Accepts redirect within `120s` (cancellable via `auth_google_cancel` `AtomicBool`), checks constant equality `state == expected_state`, returns a self-closing HTML page, and passes `{ code, codeVerifier, state, redirectUri, nonce }` to `nexora-auth`. _(See Gap `G-02` below: `pkce.rs:148-156` currently lacks `%XX` URL decoding on `code`.)_
3. **Server Verification (`services/nexora-auth/src/oauth.rs:480-755`, `account.rs:196-282`)**:
   - Validates shapes + RFC 8252 §7.3 loopback redirect URI match (`matches_expected_redirect_uri`) **before** touching DB or Google.
   - Atomically inserts `sha256(state)` into `google_oauth_states` (`UNIQUE` violation -> `401 invalid_or_expired_token` replay rejection).
   - Exchanges `code` + `code_verifier` at `https://oauth2.googleapis.com/token` using server-held `NEXORA_AUTH_GOOGLE_CLIENT_SECRET`, fetches/caches Google JWKS (`1h` TTL + unknown-`kid` refresh), and verifies RS256 signature + `iss` + `aud` + `exp` + `nonce` + `email_verified == true`.
   - Keys identity strictly by immutable `(provider, provider_subject)`; auto-links only when Google-verified email matches an already-verified active account (`unverified` local account -> `409 email_conflict`).

### 3.3 Checkout, Order Reconciliation & Signed Payment Webhooks
> **Owner-Approved Phase 15 Decision**: **Razorpay** replaces **Stripe** as Nexora's selected payment provider in **Phase 15** (`PaymentProvider` trait implementation, signed Razorpay webhooks, and subscription lifecycle management). Existing Stripe scaffolding in `services/nexora-auth/src/billing.rs` and `migrations/` remains preserved untouched until **Phase 15**.

1. **Current Baseline Checkout (`PlansPage.tsx` -> `billing_create_checkout` -> `POST /v1/billing/checkout`)**: Server looks up active `plan_versions` row (`currency`, `amount_minor`, `provider_price_id`) strictly by `(plan_id, interval)`, inserts pending `payment_orders` row keyed by `UNIQUE (user_id, idempotency_key)`, creates provider checkout session carrying `order_id`, and returns an `https://` `checkoutUrl` (re-validated in `src-tauri` before launching browser).
2. **Dual Activation Path (`billing.rs:1335-1522, 1675-1865`, transitioning to Razorpay in Phase 15)**:
   - **Asynchronous Signed Webhook (`POST /v1/billing/webhooks/...`)**: Verifies provider `HMAC-SHA256` signature over raw bytes, claims `(provider, provider_event_id)` in `webhook_events`, and calls `activate_order_in_tx` inside the same SQL transaction.
   - **Synchronous Return Check (`POST /v1/billing/verify`)**: Queries `provider.verify_checkout(...)`; **only if `verified.paid == true`** runs `activate_order_in_tx` (`payment_orders.status = 'completed'`, supersedes prior subscriptions, upserts active `subscriptions` row). Unpaid sessions leave user on `free`.

---

## 4. Finalized Desktop IPC & Backend HTTP API Conventions

### 4.1 Desktop Tauri IPC Contract (`apps/desktop/src-tauri/src/lib.rs`)
- **Command Naming**: Snake-case domain-prefixed verbs registered explicitly in `tauri::generate_handler!`:
  - Runtime & models (`17`): `get_app_status`, `get_system_info`, `get_disk_space`, `get_settings`, `get_model_profile`, `select_model`, `get_model_catalog`, `check_model_readiness`, `save_settings`, `validate_model`, `probe_runtime`, `load_model`, `unload_model`, `start_generation`, `cancel_generation`, `start_model_download`, `cancel_model_download`, `verify_model` _(18 total)_.
  - Local usage, history, workspace & export (`25`): `get_usage_status`, `list_conversations`, `load_conversation`, `create_conversation`, `save_conversation`, `rename_conversation`, `delete_conversation`, `set_workspace`, `scan_workspace_command`, `scan_current_workspace`, `read_workspace_file_command`, `get_workspace_context`, `close_workspace`, `get_workspace`, `pick_workspace_dialog`, `get_workspace_settings`, `save_workspace_settings`, `create_patch_proposal`, `validate_patch_proposal`, `apply_patch_proposal`, `get_patch_diff`, `create_patch_batch`, `validate_patch_batch`, `apply_patch_batch`, `validate_patch_detailed`, `get_symbol_index`, `refresh_symbol_index`, `search_symbols`, `clear_symbol_index`, `get_dependency_graph`, `refresh_dependency_graph`, `get_file_relationships`, `get_change_impact`, `clear_analysis_graph`, `plan_workspace_export`, `suggest_export_path`, `export_single_file`, `export_zip`, `cancel_export`.
  - Auth, account, billing, server usage & licensing (`25`): `auth_get_session`, `auth_refresh_session`, `auth_login`, `auth_register`, `auth_verify_email`, `auth_resend_verification`, `auth_recover_password`, `auth_reset_password`, `auth_logout`, `auth_change_password`, `get_developer_diagnostics`, `auth_google_begin`, `auth_google_cancel`, `account_get_profile`, `account_update_profile`, `account_link_google`, `account_export_data`, `account_delete`, `billing_list_plans`, `billing_get_status`, `billing_create_checkout`, `billing_verify_order`, `billing_cancel_subscription`, `billing_change_plan`, `billing_open_portal`, `usage_get_server_summary`, `licence_get_status`, `licence_activate`, `licence_deactivate`.
- **Wire Serialization**: `#[serde(rename_all = "camelCase")]` across all IPC structs and structured error objects (`{ code: string, message: string, retryAfterSeconds?: number }`). High-frequency progress/tokens use `tauri::ipc::Channel<T>` (`GenerationEvent`, `DownloadEvent`).

### 4.2 Online HTTP API Contract (`services/nexora-auth/src/app.rs:356-403`)
- **Versioning & Content Limits**:
  - Operational unversioned liveness probe: `GET /health` -> `200 {"service":"nexora-auth","status":"ok"}`.
  - Versioned JSON API under `/v1/*` with strict `DefaultBodyLimit::max(4096)` (`4 KiB`) and `#[serde(rename_all = "camelCase", deny_unknown_fields)]` on request DTOs.
  - Isolated signed webhook router: `POST /v1/billing/webhooks/stripe` with `DefaultBodyLimit::max(65_536)` (`64 KiB`) over raw `Bytes`.
  - **CORS Posture (`app.rs`)**: Intentionally emits **no `Access-Control-Allow-Origin` header**. Native Rust (`reqwest`) is unaffected by browser CORS, while omitting CORS prevents cross-origin browser scripts from probing local/remote `nexora-auth` endpoints.
- **Authentication Requirements**:
  - Public endpoints: `GET /health`, `GET /v1/plans`, `POST /v1/auth/{register,verify-email,resend-verification,login,recovery-request,reset-password,google/exchange}`, and HMAC-authenticated `POST /v1/billing/webhooks/stripe`.
  - Protected endpoints: Require `Authorization: Bearer <64-hex-token>` validated by `session::authenticate` (verifies SHA-256 hash lookup, `revoked_at IS NULL`, `expires_at > now()`, `last_used_at + idle_ttl > now()`, and active `users.status = 'active'`).
- **Common Error Envelope & Stable Error Codes**:
  - Envelope: `{"error": {"code": "<machine_code>", "message": "<generic_safe_message>"}}` (plus `"quota": {...}` on `429 quota_exceeded`, and `Retry-After: <seconds>` header on `429 rate_limited`).
  - Canonical codes: `validation_failed` (`400`), `unknown_model` (`400`), `invalid_webhook_signature` (`400`), `invalid_credentials` (`401`), `invalid_or_expired_token` (`401`), `email_unverified` (`403`), `not_found` (`404`), `method_not_allowed` (`405`), `email_conflict` / `identity_already_linked` / `billing_conflict` (`409`), `rate_limited` (`429`), `quota_exceeded` (`429`), `internal_error` (`500`), `payment_provider_error` (`502`), `dependency_unavailable` / `payment_provider_unconfigured` (`503`).
- **Timeouts, Idempotency & Account Privacy**:
  - Timeouts: `AuthApiClient` `10s` per request; `StripeHttpProvider` `15s`; Google loopback listener `120s`; `llama-server` health/load `180s`.
  - Idempotency: `POST /v1/billing/checkout` keyed by `(user_id, idempotency_key)`; `POST /v1/usage/reserve` keyed by `(user_id, request_id)` returning `replayed: true`; webhooks keyed by `(provider, provider_event_id)` returning `deduplicated: true`.
  - Account Privacy: `GET /v1/account/export` returns portable JSON archive of profile, OAuth links, subscriptions, orders, and daily usage; `POST /v1/account/delete` requires normalized `confirmEmail` (+ `currentPassword` when `has_password == true`), cascades deletion across all child user tables (`ON DELETE CASCADE`), and records a non-PII `audit_logs` entry (`actor_user_id = NULL`).

---

## 5. Explicit Resolution of the 10 Architecture Questions (`ADR-01` – `ADR-10`)

1. **`ADR-01` — Secure Desktop-to-Backend Connectivity**:
   - Only `apps/desktop/src-tauri` (`AuthApiClient`) connects to `services/nexora-auth` via `reqwest` (`rustls-tls`, `redirect::Policy::none()`, `5s` connect / `10s` total timeout). WebView2 CSP blocks network egress (`connect-src ipc: http://ipc.localhost`).
   - **Implemented in Phase 03 (`apps/desktop/src-tauri/{build.rs,src/auth/client.rs}`)**: Resolves `option_env!("NEXORA_AUTH_API_URL")` for release builds with safe dev/test overrides, **rejects `http://` for all non-loopback hosts in every mode**, allows loopback `http://` only in debug builds or with `NEXORA_ALLOW_LOOPBACK_HTTP=true`, and enters safe non-panicking `EndpointState::Unconfigured` (`backend_unconfigured`) when unset in release builds without inventing a domain.
2. **`ADR-02` — Desktop Sessions, Refresh & Revocation Without Embedded Secrets**:
   - The desktop binary embeds **zero server secrets**. Opaque 256-bit random session tokens are generated server-side, stored hashed (`SHA-256`) in `sessions`, and held client-side in Windows Credential Manager (`keyring` DPAPI).
   - **Desktop half implemented in Phase 03 (`apps/desktop/src-tauri/src/auth/{mod,client,commands}.rs`)**: `AuthState::refresh_session` (`auth_refresh_session`) coalesces concurrent rotations behind an async `tokio::sync::Mutex` so parallel IPC callers issue a single `POST /v1/auth/refresh` without triggering reuse revocation, redacts bearer tokens in `Debug`/errors, and purges stale credentials centrally on `401`.
3. **`ADR-03` — Google OAuth Callback Validation & CSRF Protection**:
   - Desktop generates 256-bit `code_verifier` (`S256`), 128-bit `state`, and 128-bit `nonce` via OS CSPRNG, binds `127.0.0.1:0`, checks `state` on loopback callback, percent-decodes query params _(Phase 11 fix `G-02`)_, and sends payload to `nexora-auth`. Server re-checks loopback URI shape, burns single-use `sha256(state)` in `google_oauth_states`, exchanges code with server-only `NEXORA_AUTH_GOOGLE_CLIENT_SECRET`, and verifies RS256 Google JWKS signature + `nonce` + `email_verified`.
4. **`ADR-04` — Transactional Email Provider Abstraction (`EmailProvider`)**:
   - **Honest Baseline Fact**: `services/nexora-auth/src/email.rs` currently implements **only** `UnconfiguredEmailProvider`; no real mail transport exists yet.
   - **Locked Architecture Contract (for Phase 10 implementation)**: Retain `pub trait EmailProvider: Send + Sync` (`send(&self, EmailMessage) -> Future<Result<(), EmailError>>`) with redacted `Debug` on `EmailMessage` (raw single-use verification/reset tokens appear only inside outbound message body, never logged or returned by HTTP). In Phase 10, add a real transport selected strictly by `NEXORA_AUTH_EMAIL_PROVIDER`:
     - `unconfigured`: dev/local mode; returns `EmailDelivery::Unconfigured` (and Desktop IPC must pass the server's honest message through rather than claiming an email was sent — see `G-05`).
     - `smtp` (STARTTLS/SMTPS via `NEXORA_AUTH_SMTP_HOST`, `PORT`, `USERNAME`, `PASSWORD`, `NEXORA_AUTH_EMAIL_FROM`) **or** `resend`/`postmark` HTTPS API via `reqwest` (reusing existing `rustls-tls` dependency without heavy native deps), failing fast at startup in `NEXORA_ENV=production` if unconfigured.
5. **`ADR-05` — Local AI Inference Independence from Account/Payment Connectivity**:
   - Model files (`.gguf`), `llama-server`, prompt assembly, workspace scanning, diff patching, and conversation history live 100% locally. Zero prompt or file content ever traverses `AuthApiClient`. Browsing conversations, editing code, managing models, and switching themes require zero network connectivity.
6. **`ADR-06` — Online Quotas vs Temporary Offline Fallback & Reconnection**:
   - When online, PostgreSQL (`quota.rs`) is authoritative for the user's active tier (`Free` / `Pro` / `Team`) and per-model daily allowance (`00:00:00 UTC`).
   - When an already-signed-in user has no route to `nexora-auth` (`err.code == "network_error"`), `src-tauri` falls back to tamper-evident local `UsageState` (`daily-usage.txt`, UTC day index, clock-rollback guard) with `offlineFallback: true`, and automatically resumes authoritative server accounting as soon as connectivity returns.
7. **`ADR-07` — Transactional Payment Webhooks & Subscription Lifecycle (`Razorpay` Approved for Phase 15)**:
   - **Approved Provider Decision**: **Razorpay** replaces **Stripe** as the payment and subscription provider in **Phase 15** (no Razorpay code, SDKs, or schema changes are introduced prior to Phase 15).
   - Webhooks verify provider `HMAC-SHA256` signatures over raw bytes first, then execute `INSERT INTO webhook_events ... ON CONFLICT DO NOTHING` + order/subscription updates inside a **single PostgreSQL transaction**.
   - **Critical Architecture Rule Locked for Phase 15 (`G-01`)**: `PaymentProvider` MUST own provider-side `cancel_subscription` and `change_subscription_plan` — `billing.rs` must **never** flip `subscriptions.plan_id` to a higher paid tier in PostgreSQL alone without verified Razorpay payment confirmation!
8. **`ADR-08` — Environment Configuration Across `development`, `test`, and `production`**:
   - `development`: Loopback bind (`127.0.0.1:8787`), optional DB/providers (`503` honest responses when unconfigured), local `.env`.
   - `test`: Isolated `NEXORA_AUTH_TEST_DATABASE_URL` + deterministic trait doubles (`StubGoogleProvider`, `StubPaymentProvider`, `CaptureEmail`).
   - `production` (`NEXORA_ENV=production`): Enforces non-placeholder `NEXORA_AUTH_DATABASE_URL`, rejects `sk_test_*`, `REPLACE_WITH*`, `CHANGE_ME*`, `nexora.local`, requires paired secrets, and withholds secret values from all startup/runtime logs (`exit(2)`).
9. **`ADR-09` — Database Migrations & Startup Coordination**:
   - Forward-only SQLx migrations in `services/nexora-auth/migrations/` run automatically in `main.rs` (`SELECT 1` -> `run_migrations(&pool)` -> `billing::sync_plan_catalog(&pool, &catalog)`) **before** `TcpListener::bind`, guaranteeing `/health` never serves traffic against an unmigrated schema. Multi-replica deployments remain safe because SQLx uses PostgreSQL advisory locks during `migrate!`.
10. **`ADR-10` — Cross-Layer DTO Consistency Strategy**:
    - Maintain `camelCase` JSON across TS <-> Tauri IPC <-> `nexora-auth` HTTP, `deny_unknown_fields` on HTTP inputs, canonical model ID mapping (`quota::canonical_model_id`), and extend `apps/desktop/ui/src/lib/rustContract.test.ts` + live TCP wire test (`live_desktop_client_roundtrip_over_tcp`) to cover all `auth`, `account`, `billing`, and `quota` DTOs.

---

## 6. Discovered Architecture Inconsistencies, Gaps & Roadmap Phase Assignments

Every item below was verified directly from source code during Phase 01–02 inspection (**no application files were modified in Phase 02**):

| ID | Severity | Verified Evidence (Exact File & Lines) | Architectural Defect / Gap | Assigned Roadmap Phase |
| --- | --- | --- | --- | --- |
| **`G-01`** | **CRITICAL** | `services/nexora-auth/src/billing.rs:1525-1635` | `cancel_subscription` and `change_subscription_plan` mutate PostgreSQL `subscriptions` rows directly without calling `PaymentProvider` (`Pro -> Team` `$0` upgrade bug + uncancelled provider recurring billing). Will be resolved cleanly as part of the **approved Razorpay replacement** in Phase 15. | **Phase 15** (Razorpay billing backend) |
| **`G-02`** | **HIGH** | `apps/desktop/src-tauri/src/auth/pkce.rs:148-156` | Loopback query parser `param.split_once('=')` does not URL-percent-decode `code` (`4%2F0A...` -> `4/0A...`). Forwarding raw `4%2F...` causes `reqwest` form encoding to double-encode `%` as `%25`, breaking live Google OAuth token exchange with `invalid_grant`. | **Phase 11** (Google OAuth) |
| **`G-03`** | **HIGH** | `apps/desktop/src-tauri/{build.rs,src/auth/client.rs}` & `commands.rs` | **Auth API URL resolved in Phase 03**: `option_env!("NEXORA_AUTH_API_URL")` build-time config + strict non-loopback `https://` enforcement + safe `backend_unconfigured` release state landed. Remaining `GOOGLE_OAUTH_CLIENT_ID` compile-time wiring assigned to **Phase 11 / Phase 17**. | **Phase 03 (Done)** & **Phase 11/17** |
| **`G-04`** | **HIGH** | `services/nexora-auth/src/app.rs:176-186,475-492` & `docs/PRODUCTION_DEPLOYMENT_AND_SECURITY.md:51` | `nexora-auth` keys IP rate limits strictly on peer TCP `ConnectInfo`. Behind a loopback reverse proxy (`127.0.0.1:8787`), **all global users share key `127.0.0.1`**, throttling global sign-ups after `5/1h`. Must add explicit opt-in `NEXORA_AUTH_TRUSTED_PROXY_IPS` (default empty). | **Phase 10** & **Phase 17** |
| **`G-05`** | **HIGH** | `services/nexora-auth/src/email.rs:1-106` & `apps/desktop/src-tauri/src/auth/{client,commands}.rs` | **Desktop response pass-through resolved in Phase 03**: `AuthApiClient` and IPC commands now forward honest server `message` + `emailDelivery`. Server SMTP/HTTPS `EmailProvider` implementation remains assigned to **Phase 10**. | **Phase 03 (Desktop Done)** & **Phase 10** |
| **`G-06`** | **MEDIUM** | `services/nexora-auth/src/oauth.rs:733` & `apps/desktop/src-tauri/src/auth/{client,commands}.rs` | **Desktop half resolved in Phase 03**: Single-flight `auth_refresh_session` (`POST /v1/auth/refresh`), unified `401 -> session_expired` credential purge, and `429 Retry-After` parsing landed. Backend `oauth::google_exchange` TTL & `/ready` assigned to **Phase 10**. | **Phase 03 (Desktop Done)** & **Phase 10** |
| **`G-07`** | **MEDIUM** | `services/nexora-auth/src/quota.rs:21,425-470` | `RESERVATION_TTL_SECS = 900` (`15m`): if a slow CPU `Sonnet 14B` generation streams past 15 minutes, sweeping marks the reservation `'expired'`, and `settle_reservation(..., true)` ignores `'expired'` rows rather than transitioning `'expired' -> 'committed'`. | **Phase 16** (Daily limits & entitlements) |
| **`G-08`** | **MEDIUM** | `apps/desktop/ui/src/lib/rustContract.test.ts` | **Auth/session/diagnostics DTO parity added in Phase 03** (`15` Rust <-> TS DTOs + whole-surface IPC handler check); remaining `billing`, server `usage`, `workspace`, and `history` struct shapes scheduled for **Phase 18**. | **Phase 03 (Partial)** & **Phase 18** |
| **`G-09`** | **HIGH** | `scripts/windows/smoke-verify.ps1:31,67` | Checks Electron-style `%LOCALAPPDATA%\Programs\Nexora\nexora.exe` instead of Tauri `2.12.5` `currentUser` NSIS path `%LOCALAPPDATA%\Nexora\nexora.exe`, and omits `_?=$InstallRoot` on silent `uninstall.exe /S`. | **Phase 19** (Windows packaging) |
| **`G-10`** | **LOW** | `docs/PRODUCTION_DEPLOYMENT_AND_SECURITY.md:54,67,80` | References stale `plan_prices`, `Rust 1.92.0`, and `NEXORA_PLAN_*`/`NEXORA_QUOTA_*` names instead of `plan_versions` / `plan_model_quotas`, `rust-toolchain.toml` (`1.93.0`), and `NEXORA_{FREE,PRO,TEAM}_*`. | **Phase 17** & **Phase 20** |
| **`B-01..04`** | **EXTERNAL** | Sandbox OS/network (`3.8 GiB` RAM, `HTTP 000` egress, no external keys) | Real 7B/14B GGUF execution (`Phase 04/05/18`), live Google OAuth (`Phase 11/18`), live Razorpay sandbox (`Phase 15/18`), native Windows + signing (`Phase 19/20/21`). | **Phases 04, 05, 11, 15, 18–21** |

---

## 7. Approved & Open Product Decisions

### Approved Owner Decisions
- **Payment Provider Selection (Approved for Phase 15)**: **Razorpay** replaces **Stripe** for paid plans, checkout orders, subscriptions, and signed webhooks in **Phase 15** (`G-01` will be eliminated inside the Razorpay `PaymentProvider` implementation so paid tier changes require verified Razorpay payment confirmation).

### Remaining Open Product & Operational Decisions
1. **Transactional Email Transport Choice (for Phase 10)**: Standard **HTTPS transactional email API** (e.g., Resend / Postmark / SendGrid over existing `reqwest` + `rustls-tls`) vs **SMTP STARTTLS**, behind `NEXORA_AUTH_EMAIL_PROVIDER`?
2. **Razorpay Plan Change & Upgrade Flow (for Phase 15)**: Razorpay Subscriptions plan update with immediate charge confirmation vs new Razorpay Order/Subscription checkout for tier upgrades?
3. **Final Commercial Plan Pricing & Daily Per-Model Quotas (for Phase 14–16 / 20)**: Confirm final currency (`INR` / `USD` / multi-currency), amounts, and daily per-model allowances (`Free`, `Pro`, `Team`, and offline fallback cap).
4. **Production Auth Origin & Trusted Proxy Topology (for Phase 17 / 20)**: Production HTTPS domain (`https://auth.<your-domain>`) for `NEXORA_AUTH_API_URL` build injection and reverse-proxy IP CIDR (`NEXORA_AUTH_TRUSTED_PROXY_IPS`).
