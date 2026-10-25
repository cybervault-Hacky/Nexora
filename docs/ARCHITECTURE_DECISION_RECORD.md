# Nexora Backend & Cross-Layer Architecture Decision Record (ADR)

**Status:** Historical Phase 02 architecture baseline; current corrections and Phase 16 status are recorded below.
**Baseline date:** `2026-10-22`
**Baseline branch/HEAD:** `arena/c01ae5dd-nexora` (`631a59e379281ae581fde401aace5ae913460e84`)
**Phase 16 scope:** Usage-accounting and entitlement-boundary corrections only; no approved commercial policy or Razorpay implementation is introduced.
**Scope:** Canonical architecture specification across `apps/desktop/ui`, `apps/desktop/src-tauri`, local `llama-server`, `services/nexora-auth`, and PostgreSQL 16+.
**Relationship to Existing Docs:** Supersedes the early pre-implementation sketch in `docs/phase-1/03-application-architecture.md` (which proposed hypothetical `crates/nexora-*` and SQLite layers never adopted by the codebase) and governs updates to `docs/PRODUCTION_DEPLOYMENT_AND_SECURITY.md` without deleting historical records.

> **Phase 16 correction (2026-10-24):** Local chat does not require a signed-in session. Session-vault, auth-backend, or unconfigured account-policy failures do not block local inference; valid sessions are used only for best-effort server accounting, with failures surfaced as warnings. The production quota policy remains unconfigured. The Phase 15 Razorpay work is blocked and not implemented; Stripe-oriented billing remains under audit, including an unverified client-selected subscription-plan change path. The historical statements below are updated where they conflict with these facts; no payment, price, quota, or entitlement policy is approved here.

> **Subsequent G-01 status (2026-10-24):** The Windows Public Beta preparation review changed client-selected plan changes and cancellation to fail closed and tightened checkout activation to require a completed, paid subscription checkout plus a pending order. Focused Rust/PostgreSQL regression tests were added but could not be executed in the available environment. The Phase 16 G-01 rows below are historical; this source-only mitigation is not a release approval. See [the current Public Beta preparation report](PUBLIC_BETA_PREPARATION_WINDOWS.md).

---

## 1. System Architecture Overview

Nexora separates **on-device AI inference and workspace code operations** from **online account, authentication, billing, and optional daily-usage accounting**. Account services are not a prerequisite for local chat; no production quota entitlement is enabled by default:

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
│  │  • `usage/*`: device-local UTC generation record; no approved local cap             │ │
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
│  │  • `quota` & `audit`: UTC reservation accounting; quota policy defaults unconfigured│ │
│  └──────────────┬──────────────────────────────────┬────────────────┬──────────────────┘ │
│                 │ `sqlx` (`sslmode=require` prod)  │ HTTPS          │ HTTPS              │
│                 ▼                                  ▼                ▼                    │
│  ┌──────────────────────────────┐   ┌───────────────────┐  ┌───────────────────────────┐ │
│  │ PostgreSQL 16+ (16 tables,   │   │ Google OAuth/OIDC │  │ Existing Stripe-oriented   │ │
│  │ 8 forward-only migrations)   │   │ + SMTP/Mail API   │  │ billing code (not verified)│ │
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
| Approved model metadata, `.part` downloads, GGUF SHA-256 verification & RAM/disk checks | **Owns (`src-tauri/src/inference/{registry,acquisition,models,readiness}.rs`)** | Loads verified `.gguf` file path passed on CLI | Canonicalizes profile IDs for server accounting (`quota::canonical_model_id`) | Stores plan/model quota rows, which are not active entitlements while policy is unconfigured | Unverified `.gguf` files must **never** be loaded under an approved profile; prompts/files must **never** be sent to `nexora-auth`. |
| Token generation, SSE streaming & mid-stream stop | Orchestrates (`src-tauri/src/inference/{process,client,sse,commands}.rs`); local inference is not auth-gated | **Owns (`127.0.0.1:<port>`, `--api-key-file`)** | Best-effort reservation/settlement only with a verified session and configured policy (zero prompt payload) | Records counts only when account policy is configured | `llama-server` must **never** bind `0.0.0.0`, enable web UI, or outlive desktop shutdown (`lib.rs:42-49`). |
| Password hashing (Argon2id), email verification, password reset & sliding-window rate limits | Collects form inputs via `AuthModal` / `AccountPage` | None | **Owns (`registration`, `login`, `recovery`, `rate_limit`)** | **Persists** PHC hashes & `sha256` token digests | Plaintext passwords and raw verification/reset tokens must **never** be stored in PostgreSQL or written to logs. |
| Google OAuth 2.0 + OIDC PKCE sign-in & linking | Generates S256 PKCE + `state` + `nonce`, binds `127.0.0.1:0` (`auth/pkce.rs`) | None | **Owns (`oauth.rs`, `account.rs`)**: consumes single-use `state_hash`, exchanges code with secret, validates RS256 JWKS + claims | **Persists** `oauth_accounts` (`provider`, `provider_subject`) & `google_oauth_states` | `NEXORA_AUTH_GOOGLE_CLIENT_SECRET` must **never** exist in `VITE_*`, `src-tauri`, or installer artifacts; unverified emails are never auto-linked. |
| Plan catalogue, Stripe-oriented billing & signed webhooks | Shows informational account status; no checkout or plan-change action is active in `PlansPage.tsx` | None | Existing Stripe-oriented `billing.rs` code remains under audit; client-selected plan changes are not provider-verified | **Persists** plan, subscription, order, and webhook rows | Server must **never** trust client-submitted prices, intervals, order statuses, or plan/account/quota/Premium claims as verified entitlements. |
| Daily usage accounting (`00:00 UTC` boundary when configured) | Device-local `UsageState` records accepted generations without an approved local cap; it never gates chat on account availability | None | **Owns (`quota.rs`)**: `FOR UPDATE` reservation/commit/release only when explicit quota policy is configured; default is unconfigured | **Persists** counters/reservations, not proof of paid access | Local usage must **never** overwrite server counters or grant Free/Premium/paid entitlements; missing policy is neither unlimited nor a zero-valued Free quota. |

> **Note on Legacy Local `licensing/*` Module (`apps/desktop/src-tauri/src/licensing/`):** Preserved for backward UI/test compatibility (`#/licensing`), `licensing/*` validates local offline key shapes only and **does not grant server subscription entitlements or raise `start_generation` daily quotas**. The backend policy currently defaults to unconfigured, and the Stripe-oriented client-selected plan-change path is not provider-verified; neither local licence state nor subscription rows are presently sufficient evidence of paid/Premium access.

---

## 3. Traced Request & Data Flows

### 3.1 Local Chat Generation + Optional Usage Accounting
1. **UI (`apps/desktop/ui/src/pages/ChatPage.tsx` -> `usePersistentConversation.ts`)**: Invokes `start_generation` (`GenerateRequest { requestId, messages, maxTokens, temperature, profileId }`) with a Tauri `Channel<GenerationEvent>`. Sign-in is optional; local chat remains available when signed out.
2. **Local inference path (`apps/desktop/src-tauri/src/inference/commands.rs`)**:
   - Validates prompt roles/lengths, preserves the selected Mini/Sonnet profile, verifies the running profile, checks token budget, and probes the local engine. These local checks are independent of account/payment connectivity.
   - Attempts to read a session token only for optional server accounting. A missing session, keychain read error, unconfigured backend/policy, reservation failure, quota response, or invalid session does not gate local inference; invalid sessions are purged and accounting problems are surfaced as typed usage warnings. No token or account is fabricated.
   - If a valid token is available, calls `POST /v1/usage/reserve` with `{ modelId, requestId }`. The verified server session determines identity. The production quota policy defaults to unconfigured; its summary omits plan/limit/reset claims and reserve returns a typed `quota_policy_unconfigured` response.
   - Device-local `UsageState` records accepted generations but has no approved local limit. It does not claim a Free/Premium allowance or act as an offline account entitlement.
3. **Streaming & Settlement**:
   - Streams tokens over loopback `POST http://127.0.0.1:<port>/v1/chat/completions` (`Authorization: Bearer <ephemeral_key>`); prompts and files are not sent to `nexora-auth`.
   - If the local engine fails before streaming starts, any server reservation is released; an uncommitted local accounting reservation is dropped.
   - When the engine accepts the request, the server reservation is committed (or released if no stream began) and local usage is recorded. Settlement is idempotent; transient server settlement errors are retried once. Remaining reservation, settlement, or local-recording failures are surfaced as warnings without turning a successful local reply into a failure. Conversation history and Mini/Sonnet routing are preserved.

### 3.2 Desktop Google OAuth 2.0 + OIDC PKCE Sign-In & Account Linking
1. **Desktop PKCE Setup (`auth/pkce.rs`, `auth/commands.rs:255-315`)**: Generates CSPRNG 256-bit `code_verifier` (`S256` challenge), 128-bit `state`, and 128-bit `nonce` (`getrandom`); binds `127.0.0.1:0` -> `http://127.0.0.1:<port>/callback`; validates `https://` URL and opens the system browser via `rundll32.exe url.dll,FileProtocolHandler` on Windows (preventing `cmd.exe` `&` injection).
2. **Loopback Callback (`pkce.rs:119-207`)**: Accepts redirect within `120s` (cancellable via `auth_google_cancel` `AtomicBool`), checks constant equality `state == expected_state`, returns a self-closing HTML page, and passes `{ code, codeVerifier, state, redirectUri, nonce }` to `nexora-auth`. _(See Gap `G-02` below: `pkce.rs:148-156` currently lacks `%XX` URL decoding on `code`.)_
3. **Server Verification (`services/nexora-auth/src/oauth.rs:480-755`, `account.rs:196-282`)**:
   - Validates shapes + RFC 8252 §7.3 loopback redirect URI match (`matches_expected_redirect_uri`) **before** touching DB or Google.
   - Atomically inserts `sha256(state)` into `google_oauth_states` (`UNIQUE` violation -> `401 invalid_or_expired_token` replay rejection).
   - Exchanges `code` + `code_verifier` at `https://oauth2.googleapis.com/token` using server-held `NEXORA_AUTH_GOOGLE_CLIENT_SECRET`, fetches/caches Google JWKS (`1h` TTL + unknown-`kid` refresh), and verifies RS256 signature + `iss` + `aud` + `exp` + `nonce` + `email_verified == true`.
   - Keys identity strictly by immutable `(provider, provider_subject)`; auto-links only when Google-verified email matches an already-verified active account (`unverified` local account -> `409 email_conflict`).

### 3.3 Existing Stripe-Oriented Billing (Audit Only)
> **Phase 15 status:** Razorpay work is blocked and was not implemented. The existing Stripe-oriented `billing.rs`, migrations, and records were not changed by Phase 16. No live Stripe or Razorpay behavior was validated. Client-selected changes to active/trialing subscription plans without provider verification remain a blocker; those subscription rows must not be treated as verified Premium or quota evidence.

The repository contains Stripe-oriented catalogue, checkout, order-verification, portal, webhook, and subscription code. This is a source-level description, not a claim that payment is enabled or the commercial terms are approved. `PlansPage.tsx` presents no active checkout or plan-change action. Order verification/webhook handling does not close the separate client-selected subscription-plan change gap. Do not infer paid entitlements from a client request, a local profile/licence, a subscription row, or a backend/payment failure. No Razorpay API, table, migration, refund flow, or checkout behavior is present.

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
  - Public endpoints: `GET /health`, `GET /v1/plans` (returns no plan catalogue while quota policy is unconfigured), `POST /v1/auth/{register,verify-email,resend-verification,login,recovery-request,reset-password,google/exchange}`, and HMAC-authenticated `POST /v1/billing/webhooks/stripe`.
  - Protected endpoints: Require `Authorization: Bearer <64-hex-token>` validated by `session::authenticate` (verifies SHA-256 hash lookup, `revoked_at IS NULL`, `expires_at > now()`, `last_used_at + idle_ttl > now()`, and active `users.status = 'active'`).
- **Common Error Envelope & Stable Error Codes**:
  - Envelope: `{"error": {"code": "<machine_code>", "message": "<generic_safe_message>"}}` (plus `"quota": {...}` on `429 quota_exceeded`, and `Retry-After: <seconds>` header on `429 rate_limited`).
  - Canonical codes: `validation_failed` (`400`), `unknown_model` (`400`), `invalid_webhook_signature` (`400`), `invalid_credentials` (`401`), `invalid_or_expired_token` (`401`), `email_unverified` (`403`), `not_found` (`404`), `method_not_allowed` (`405`), `email_conflict` / `identity_already_linked` / `billing_conflict` (`409`), `rate_limited` (`429`), `quota_exceeded` (`429`), `internal_error` (`500`), `payment_provider_error` (`502`), `dependency_unavailable` / `payment_provider_unconfigured` / `quota_policy_unconfigured` (`503`).
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
6. **`ADR-06` — Local Inference Independence & Optional Account Accounting**:
   - Local generation is available while signed out and when the account backend, session vault, or quota policy is unavailable. Local device usage is informational and has no approved hard limit.
   - A valid server session may be used for optional account reservation/settlement. The production quota policy defaults unconfigured; accounting errors are surfaced as warnings and never grant a Free/Premium entitlement or deny local inference. Policy-dependent UTC reset periods are not activated until approved configuration exists.
7. **`ADR-07` — Payment Provider & Subscription Lifecycle (Phase 15 Blocked)**:
   - The Phase 15 Razorpay work is blocked and was not implemented. Existing Stripe-oriented code remains; this ADR does not claim Razorpay support or make a provider switch.
   - Existing webhook/order operations do not verify client-selected active/trialing plan changes. `G-01` remains open; subscription records are not trusted paid/quota evidence until provider verification is addressed. No billing records or Stripe code were modified in Phase 16.
8. **`ADR-08` — Environment Configuration Across `development`, `test`, and `production`**:
   - `development`: Loopback bind (`127.0.0.1:8787`), optional DB/providers (`503` honest responses when unconfigured), local `.env`.
   - `test`: Isolated `NEXORA_AUTH_TEST_DATABASE_URL` + deterministic trait doubles (`StubGoogleProvider`, `StubPaymentProvider`, `CaptureEmail`).
   - `production` (`NEXORA_ENV=production`): Enforces non-placeholder `NEXORA_AUTH_DATABASE_URL`, rejects `sk_test_*`, `REPLACE_WITH*`, `CHANGE_ME*`, `nexora.local`, requires paired secrets, and withholds secret values from all startup/runtime logs (`exit(2)`).
9. **`ADR-09` — Database Migrations & Startup Coordination**:
   - Forward-only SQLx migrations in `services/nexora-auth/migrations/` are embedded in the service binary and run automatically in `main.rs` (`SELECT 1` -> `run_migrations(&pool)` -> `billing::sync_plan_catalog(&pool, &catalog)`) **before** `TcpListener::bind`, so traffic is not served against an unmigrated schema. SQLx tracks versions/checksums in `_sqlx_migrations` and coordinates concurrent migrators with PostgreSQL advisory locks.
10. **`ADR-10` — Cross-Layer DTO Consistency Strategy**:
    - Maintain `camelCase` JSON across TS <-> Tauri IPC <-> `nexora-auth` HTTP, `deny_unknown_fields` on HTTP inputs, canonical model ID mapping (`quota::canonical_model_id`), and extend `apps/desktop/ui/src/lib/rustContract.test.ts` + live TCP wire test (`live_desktop_client_roundtrip_over_tcp`) to cover all `auth`, `account`, `billing`, and `quota` DTOs.

---

## 6. Discovered Architecture Inconsistencies, Gaps & Roadmap Phase Assignments

Every item below was verified directly from source code during Phase 01–02 inspection (**no application files were modified in Phase 02**):

| ID | Severity | Verified Evidence (Exact File & Lines) | Architectural Defect / Gap | Assigned Roadmap Phase |
| --- | --- | --- | --- | --- |
| **`G-01`** | **CRITICAL / OPEN** | `services/nexora-auth/src/billing.rs` subscription plan-change path | Client-selected plan changes can mutate active/trialing subscription records without provider verification. Phase 15 Razorpay was blocked and not implemented; Stripe-oriented billing was audited only in Phase 16. These records are not trusted Premium/quota evidence. | **Unresolved blocker; no Phase16 billing change** |
| **`G-02`** | **HIGH** | `apps/desktop/src-tauri/src/auth/pkce.rs:148-156` | Loopback query parser `param.split_once('=')` does not URL-percent-decode `code` (`4%2F0A...` -> `4/0A...`). Forwarding raw `4%2F...` causes `reqwest` form encoding to double-encode `%` as `%25`, breaking live Google OAuth token exchange with `invalid_grant`. | **Phase 11** (Google OAuth) |
| **`G-03`** | **HIGH** | `apps/desktop/src-tauri/{build.rs,src/auth/client.rs}` & `commands.rs` | **Auth API URL resolved in Phase 03**: `option_env!("NEXORA_AUTH_API_URL")` build-time config + strict non-loopback `https://` enforcement + safe `backend_unconfigured` release state landed. Remaining `GOOGLE_OAUTH_CLIENT_ID` compile-time wiring assigned to **Phase 11 / Phase 17**. | **Phase 03 (Done)** & **Phase 11/17** |
| **`G-04`** | **HIGH** | `services/nexora-auth/src/app.rs:176-186,475-492` & `docs/PRODUCTION_DEPLOYMENT_AND_SECURITY.md:51` | `nexora-auth` keys IP rate limits strictly on peer TCP `ConnectInfo`. Behind a loopback reverse proxy (`127.0.0.1:8787`), **all global users share key `127.0.0.1`**, throttling global sign-ups after `5/1h`. Must add explicit opt-in `NEXORA_AUTH_TRUSTED_PROXY_IPS` (default empty). | **Phase 10** & **Phase 17** |
| **`G-05`** | **HIGH** | `services/nexora-auth/src/email.rs:1-106` & `apps/desktop/src-tauri/src/auth/{client,commands}.rs` | **Desktop response pass-through resolved in Phase 03**: `AuthApiClient` and IPC commands now forward honest server `message` + `emailDelivery`. Server SMTP/HTTPS `EmailProvider` implementation remains assigned to **Phase 10**. | **Phase 03 (Desktop Done)** & **Phase 10** |
| **`G-06`** | **MEDIUM** | `services/nexora-auth/src/oauth.rs:733` & `apps/desktop/src-tauri/src/auth/{client,commands}.rs` | **Desktop half resolved in Phase 03**: Single-flight `auth_refresh_session` (`POST /v1/auth/refresh`), unified `401 -> session_expired` credential purge, and `429 Retry-After` parsing landed. Backend `oauth::google_exchange` TTL & `/ready` assigned to **Phase 10**. | **Phase 03 (Desktop Done)** & **Phase 10** |
| **`G-07`** | **MEDIUM / MITIGATED IN CODE, UNVALIDATED** | `services/nexora-auth/src/quota.rs` reservation sweep/settlement | Phase 16 code now sweeps stale reservations before settlement, persists expiry bookkeeping before rejecting an expired commit, makes same-outcome settlement idempotent, rejects opposite/replayed outcomes, and checks counter updates. Rust/DB validation is blocked, so this is not runtime-verified. | **Phase 16; validation pending** |
| **`G-08`** | **MEDIUM** | `apps/desktop/ui/src/lib/rustContract.test.ts` | **Auth/session/diagnostics DTO parity added in Phase 03** (`15` Rust <-> TS DTOs + whole-surface IPC handler check); remaining `billing`, server `usage`, `workspace`, and `history` struct shapes scheduled for **Phase 18**. | **Phase 03 (Partial)** & **Phase 18** |
| **`G-09`** | **HIGH / CODE CORRECTED, WINDOWS UNVERIFIED** | `scripts/windows/smoke-verify.ps1` | Smoke script now targets Tauri `2.12.5` `currentUser` NSIS path `%LOCALAPPDATA%\Nexora\nexora.exe` and passes the quoted `_?=$InstallRoot` argument to silent uninstallation; it has not been executed on Windows. | **Phase 19** (code correction; native validation pending) |
| **`G-10`** | **LOW / DOCUMENTATION CORRECTED (Phase 20)** | `docs/PRODUCTION_DEPLOYMENT_AND_SECURITY.md`; `README.md` | The current deployment guide no longer contains the previously reported `plan_prices` or `NEXORA_PLAN_*`/`NEXORA_QUOTA_*` names. Phase 20 corrected its stale Rust pin (`1.92.0` → pinned `1.93.0`) and obsolete PFX-signing instructions; the root README now labels commercial defaults as unapproved and removes active-offer wording. Historical audit documents are explicitly marked superseded. | **Phase 17** & **Phase 20** |
| **`B-01..04`** | **EXTERNAL** | Sandbox OS/network, unavailable toolchains, no external provider credentials | Real 7B/14B GGUF execution, live Google OAuth, isolated PostgreSQL verification, and native Windows/signing remain unverified. Razorpay sandbox work was blocked and not implemented. | **Deferred to the relevant authorized phase** |

---

## 7. Approved & Open Product Decisions

### Recorded Provider Decision and Implementation Status
- The Phase 15 plan recorded Razorpay as the intended replacement provider. That work was blocked and no Razorpay API, schema, checkout, or subscription implementation was delivered. Existing Stripe-oriented code remains and is under audit; no provider switch is claimed here.

### Remaining Open Product & Operational Decisions
1. **Transactional Email Transport Choice (for Phase 10)**: Standard **HTTPS transactional email API** (e.g., Resend / Postmark / SendGrid over existing `reqwest` + `rustls-tls`) vs **SMTP STARTTLS**, behind `NEXORA_AUTH_EMAIL_PROVIDER`?
2. **Razorpay Implementation Scope (Phase 15 blocker)**: Provider-specific order/subscription lifecycle, event verification, refunds, and lifecycle mapping remain unimplemented; no Razorpay behavior is assumed.
3. **Commercial terms and quota policy**: Currency, prices, Free/Premium plan structure, per-model limits, token allowances, trials, feature access, and reset/billing periods remain unapproved. No local offline cap exists, and an absent quota is not unlimited paid access or zero Free access.
4. **Production Auth Origin & Trusted Proxy Topology (for Phase 17 / 20)**: Production HTTPS domain (`https://auth.<your-domain>`) for `NEXORA_AUTH_API_URL` build injection and reverse-proxy IP CIDR (`NEXORA_AUTH_TRUSTED_PROXY_IPS`).

---

## 8. Phase 16 Implementation & Validation Status

- Phase 16 changes cover quota-policy defaults, UTC counter/reservation expiry and settlement checks, local usage accounting without a configured allowance, best-effort server accounting, and warning propagation. Sign-in/session-vault/account-policy failures do not gate local chat; Mini/Sonnet profile IDs and conversation behavior remain intact.
- No commercial terms are approved by this work; production `QuotaPolicy` defaults to unconfigured, the browser fallback is empty, and the public plan-list endpoint hides stored catalogue rows while policy is unconfigured. Draft plan defaults and Stripe checkout/subscription handlers remain in `billing.rs` under audit; keep provider/payment configuration disabled and do not treat those records as entitlements. Stripe code was not modified; G-01 remains open. Razorpay remains blocked and unimplemented.
- **UI validation passed:** `corepack pnpm --filter @nexora/desktop typecheck`; `corepack pnpm --filter @nexora/desktop test` (88 files, 870 tests); and `corepack pnpm --filter @nexora/desktop build` (successful, with non-fatal Vite warnings).
- **Rust/PostgreSQL validation is blocked:** Native Rust formatter/compiler/Clippy and PostgreSQL/container tools are unavailable. An auxiliary `@scalar/rust-fmt` WASM pass checked 34 modified/untracked Rust files and reported formatting differences in 17; broad formatting was not applied to preserved Phases 09–15 work. Rust unit/integration tests, migration checks, and isolated-PostgreSQL tests have not run; no database was contacted. Phase 10–13 backend validation remains open, and the Phase 16 server changes are not runtime-validated.
- Work stops at Phase 16. No Phase 17+ changes, commits, pushes, merges, releases, or publication actions are included.
