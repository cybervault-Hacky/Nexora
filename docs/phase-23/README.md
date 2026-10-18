# Phase 23 — Authentication Backend Audit & Security Architecture Plan

**Status:** Planning document only. **No application code, tests, configuration, dependencies, lockfiles, environment files, schemas, migrations, or build scripts were changed in this phase.** Nothing below is implemented; all endpoints, tables, and flows are proposals.

**Checkout audited:** `arena/1008499f-nexora`, HEAD `128781618917fd15d2923aeb8eec04e9a5e90e98` (base, unchanged), working tree carrying the uncommitted Phase 19–22 implementations. Audit date: 2026-10-18.

---

## 1. Repository audit findings

### 1.1 Branch, HEAD, working tree (verified live)

| Item | Observed |
|---|---|
| Branch | `arena/1008499f-nexora` |
| HEAD | `128781618917fd15d2923aeb8eec04e9a5e90e98` (merge of PR #3; no new commits) |
| Modified tracked files | 8 (`README.md`, `App.interaction.test.tsx`, `App.test.tsx`, `App.tsx`, `Icon.tsx`, `Sidebar.tsx`, `TitleBar.tsx`, `styles/app.css`) — cumulative Phases 19–20 work |
| Untracked entries | 16: `App.navigation.test.tsx`, `App.recovery.test.tsx`, `App.signin.test.tsx`, `App.signup.test.tsx`, `App.welcome.test.tsx`, `GoogleMark.tsx`, `RouterLink.tsx`, `WelcomePreview.tsx`, `formValidation.ts`, `router.ts`, `router.test.ts`, `pages/`, `docs/phase-19/`, `docs/phase-20/`, `docs/phase-21/`, `docs/phase-22/` — Phases 19–22 work |
| `docs/phase-23/` | Did not exist before this document (no conflict; nothing overwritten) |
| `.env*` files | None anywhere outside `node_modules` |
| Capability files | `src-tauri/capabilities/main.json` only |

All Phases 19–22 work is intact exactly as found. Every inspection command was read-only (`git status`, `git log`, `git diff --stat`, `git diff --check`, `cat`, `grep`, `ls`, `find`, `sed`). No `git add/clean/stash/reset`, no dependency installation, no build.

### 1.2 Checks run and exact outcomes

| Check | Outcome | Notes |
|---|---|---|
| `git rev-parse HEAD` | `128781618917fd15d2923aeb8eec04e9a5e90e98` | Unchanged from Phases 20–22. |
| `pnpm typecheck` | **PASS** | `tsc --noEmit`, writes nothing. |
| `pnpm test` | **PASS — 50 files / 558 tests** | Confirms the Phase 22 report is still accurate for this tree. |
| `git diff --check` | **PASS** | Read-only. |
| `pnpm build` | **Not run** | It rewrites `dist/` build artifacts, which the no-modification rule prohibits. The last observed result (Phase 22, identical tree): PASS, 110 modules, two pre-existing Vite chunk warnings. |
| `cargo test` / `cargo check` | **Not run** | No `cargo`/`rustc` toolchain in this Linux sandbox (documented since Phase 2). Would be safe/read-only but is unavailable; `Cargo.toml`/`Cargo.lock` were read instead. **[Correction (Phase 23B): no `Cargo.lock` exists anywhere in this repository — no Rust build has ever run here; only `apps/desktop/src-tauri/Cargo.toml` was read.]** |
| Tauri/Windows runtime tests | **Not run** | Linux sandbox. |

### 1.3 Existing backend / authentication capabilities discovered

**There is no authentication system, API server, database, session management, OAuth integration, or secret-management system in the repository.** Specifically:

- **Rust (`src-tauri/src/`)** registers **38 `#[tauri::command]` handlers** (via `lib.rs` `invoke_handler`): `inference::*` (10), `history::*` (6), `workspace::*` (22). All are inference-engine control, local conversation history, workspace scanning/patching, and code analysis. None touch accounts, passwords, email, or sessions.
- **Local persistence is JSON files** in `app_config_dir`: `conversations.json` (history/storage.rs, atomic tempfile writes) and `workspace-settings.json` (workspace/settings.rs, same atomic-write pattern). **No SQL/SQLite/Postgres**; no migration tooling.
- **The only "session key"** in the codebase is `inference/process.rs::SESSION_KEY_FILE` — a per-launch random key (`getrandom` + `sha2`) that authenticates the Tauri shell to its own child llama.cpp HTTP server on `127.0.0.1`. It is unrelated to user authentication and never leaves the machine.
- **The only outbound HTTP client** is `reqwest` in `inference/client.rs`, pointed at the local llama.cpp server (`127.0.0.1:<port>`). No remote endpoints, no API base URL, no telemetry.
- **Frontend network/IPC surface:** every `invoke` call lives in `lib/inference.ts`, `lib/history.ts`, `lib/analysis.ts`, and `lib/workspace.ts` (via `@tauri-apps/api/core`). **The five auth pages contain zero `invoke`/`fetch`/network calls** — grep across `pages/SignInPage.tsx`, `CreateAccountPage.tsx`, `VerifyEmailPage.tsx`, `RecoverPasswordPage.tsx`, `ResetPasswordPage.tsx` finds none. The honest status strings ("…not connected yet…") are purely local UI state. **A frontend status message therefore correctly indicates the absence of authentication, not its presence** — and tests (`App.signin/signup/recovery.test.tsx`) assert no storage writes, no `fetch`, and no console logging around submissions.
- **Dependencies:** frontend `@tauri-apps/api`, `diff`, `react`, `react-dom`; Rust `serde`, `serde_json`, `tauri`, `reqwest`, `futures-util`, `tokio`, `getrandom`, `sha2`, `similar`, `regex`, `tempfile`, `windows-sys` (RAM probe only). No auth/OAuth/JWT/password-hashing/database crates, no auth npm packages. Lockfiles (`pnpm-lock.yaml`, `Cargo.lock`) unchanged this phase. **[Correction (Phase 23B): `pnpm-lock.yaml` was unchanged; no `Cargo.lock` exists in the repository at all.]**
- **Tauri security posture (existing strengths to preserve):** `capabilities/main.json` grants only `core:default` + four window controls (`minimize`, `toggle-maximize`, `close`, `start-dragging`). Production CSP: `default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: asset: http://asset.localhost; font-src 'self'; connect-src ipc: http://ipc.localhost` — **the WebView cannot make arbitrary HTTP requests at all**; all I/O must pass through Rust commands. This is an unusually strong base for credential handling.

### 1.4 Auth-frontend inventory (what exists today)

| Route | Component | Real behaviour (local only) |
|---|---|---|
| `#/signin` | `SignInPage.tsx` | Email/password validation, visibility toggle, honest "not connected" status, informational Google button |
| `#/signup` | `CreateAccountPage.tsx` | Name/email/password/confirm validation (`lib/formValidation.ts`), honest "details have not been submitted" |
| `#/verify-email` | `VerifyEmailPage.tsx` | Static honest verification-pending page (no code/token/resend controls) |
| `#/recover-password` | `RecoverPasswordPage.tsx` | Email validation, honest "no reset email has been sent" |
| `#/reset-password` | `ResetPasswordPage.tsx` | New/confirm password validation, honest "no password has been changed" |

Shared: `lib/formValidation.ts` (`PASSWORD_MIN_LENGTH = 8`, syntactic email check), `role="status"`/`role="alert"` patterns, `aria-invalid`/`aria-describedby`, repeat-submission guards. **Gap: everything behind the UI** — registration, verification, login, sessions, recovery, reset, OAuth, rate limiting, email delivery, and storage of any kind.

---

## 2. Recommended architecture (and alternatives considered)

### 2.1 Headline recommendation

**A small standalone remote auth API — Rust `axum` + PostgreSQL (`sqlx`), `argon2id` password hashing, transactional email provider — with the Tauri shell as the sole credential holder** (session tokens stored in Windows Credential Manager via the `keyring` crate; the React WebView never sees raw tokens; all API traffic originates from Rust `reqwest`, matching the existing CSP). Google sign-in uses OAuth 2.0 Authorization Code + PKCE with the system browser and a loopback redirect, with the client secret kept server-side.

Why this shape:

1. **Matches the repository.** The team already maintains a Rust backend surface (`src-tauri`), `reqwest`, `serde`, `tokio`, and `sha2`/`getrandom`. An `axum` service is the smallest step into server-side Rust; `sqlx` adds compile-checked SQL and migrations without an ORM.
2. **Keeps secrets off the frontend by construction.** The existing CSP (`connect-src ipc: http://ipc.localhost`) already forbids WebView HTTP. Designing auth to flow through Rust commands preserves that control instead of fighting it.
3. **Local-first compatible.** A remote auth API is only consulted for account features; inference, models, projects, and settings remain local JSON/llama.cpp exactly as audited in §1.3 (see §2.5).
4. **Windows distribution friendly.** No bundled database, no local auth server in the installer; the NSIS build stays a client.

### 2.2 Backend options compared

| Option | Security & maintenance | Deployment & cost | Local dev & testing | Windows distribution fit | Local-first fit | Verdict |
|---|---|---|---|---|---|---|
| **A. Rust `axum` + PostgreSQL (`sqlx`)** | One language across shell+server; `argon2`/`sqlx` actively maintained; small dep tree | Single binary + managed Postgres (~$5–25/mo at low tiers: Fly.io/Railway/AWS RDS); trivial container | `cargo test` + testcontainers/`sqlx::test`; migrations in-repo | Neutral (client-only) | Excellent | **Recommended** |
| B. Node/Fastify (TS) + Postgres | Two runtimes; huge ecosystem (more supply-chain surface); TS types can be shared with the UI | Similar cost; very common hosting | Jest/Vitest parity with frontend | Neutral | Excellent | Viable fallback if the team prefers TS server-side |
| C. Managed BaaS (Supabase/GoTrue, Clerk, Auth0) | Vendor maintenance; fast start; weaker control over error semantics/enumeration | $0–25+ tiers; per-MAU pricing risk; external dependency & uptime coupling | Emulators exist; vendor test kits | Neutral | Weakest (accounts, email, and token logic live off-repo; data residency/privacy story depends on vendor) | Rejected for v1; revisit if ops capacity never materializes |
| D. Local accounts (SQLite in the desktop app) | Email verification cannot be genuine; any "secret" ships inside a distributed binary and is extractable; no cross-device identity | No server cost | Easy | Ships in installer | Strong but **cannot honestly deliver "email verification", "recovery email", or Google client-secret hygiene** | Rejected (would recreate Phase 21–22 honesty problems in a new form) |

### 2.3 Session architecture for a Tauri 2 desktop app

Browser cookie guidance does **not** map onto this runtime: the UI is a WebView2 shell at `tauri://`-style origins, requests already flow through Rust, and the WebView cookie store (on-disk under the app's WebView2 data folder) is the wrong place for a long-lived credential. Compared explicitly:

| Approach | Mechanics | Threat model | Revocation / expiry / refresh / logout | CSRF | Verdict |
|---|---|---|---|---|---|
| **1. Server-managed opaque sessions, credentials held in Rust** (recommended) | Login happens via a Rust command; API returns an opaque random session token (+ rotating refresh token); Rust stores it in **Windows Credential Manager (`keyring` crate)** and a memory cache; React receives only `user` profile data via IPC | Token at rest is OS-protected (DPAPI-backed Credential Manager); memory-only in the WebView; server stores only `sha256(token)` | Instant server-side revocation (row `revoked_at`); absolute + idle TTLs; rotation on refresh with reuse-detection (revoke family on replay); logout = revoke + delete keyring entry | Largely N/A — no ambient credential is attached automatically by the WebView; each call is an explicit Rust `Authorization: Bearer` header | **Recommended** |
| 2. JWT access tokens + refresh rotation | Same storage, but self-contained access tokens (short TTL ~15 min) | Steal-window ≤ access TTL; signing-key management; no free revocation of a valid JWT without a denylist | Logout/revocation needs a server-side denylist/session table anyway (diminishing the "stateless" benefit); refresh rotation required | Same as (1) | Viable at large scale; adds complexity with no current need |
| 3. WebView2 cookie sessions (`HttpOnly`/`SameSite`) | Backend `Set-Cookie` consumed by the WebView jar | Cookie jar on disk, ambient credential attached to every WebView request; app origin is not a normal site; poorly testable; behavior differs across WebView2 versions | Cookie expiry only; server logout endpoint; no clean per-device management | **Required** (SameSite + CSRF tokens + origin checks) — exactly the browser guidance the brief warns not to transplant | Rejected as primary; may be revisited only for a future web-app client |
| 4. Per-provider OAuth tokens only (no app session) | Google access/refresh token in keyring | Doesn't cover email/password at all; Google token scope creep; refresh tokens can be revoked by Google unilaterally | Provider-controlled | N/A | Rejected as the only mechanism; part of its shape (keyring storage) is kept |

**Recommendation:** Approach 1. Single opaque session token per device/installation: 32 random bytes (`getrandom`), sent once over TLS, stored server-side as `sha256`, TTL 30 days absolute with 7-day idle expiry, explicit refresh/rotation command, family-based reuse detection. (Approach 2 remains the documented alternative if horizontal scale ever requires stateless verification.)

**Desktop OAuth flow (Google):** Rust generates `state` (CSRF) + PKCE `code_verifier` (both kept in process memory, never in the WebView), opens the **system default browser** (`shell:open`-equivalent via a scoped custom command or `tauri-plugin-shell` allowlist limited to the Google authorization host) at Google's authorization endpoint with `redirect_uri=http://127.0.0.1:<ephemeral-port>/callback`; a loopback listener in the Tauri process receives the code; Rust forwards `code + code_verifier + state` to the auth API, which validates state one-time-ness and performs the token exchange. **Client secret, if the Google client type requires one, exists only in the server's secret store.** No OAuth credentials, endpoints, or success responses are invented here; actual values are Phase-24+ configuration.

**Capabilities guidance (minimum necessary):** keep `core:default` + window controls as-is; do **not** add `http`/`fetch` capabilities to the WebView; if the system-browser or loopback plugins are added later, scope them (`shell:allow-open` restricted to `https://accounts.google.com/...`, or a custom `open_external` command with an HTTPS-host allowlist). Privileged material (session tokens, OAuth client secret, SMTP credentials) must never cross into the React frontend; the IPC `auth_*` commands return user profile DTOs only.

---

## 2.4 Email & password authentication (planned behaviour)

- **Registration:** validated name (trimmed, ≤ 128 chars) + email (syntactic + lowercased/canonical, ≤ 254) + password (**≥ 8 characters, ≤ 128** — the upper bound prevents argon2 CPU DoS; no composition rules, consistent with `lib/formValidation.ts`'s documented NIST stance). Hash with **argon2id** (`argon2` crate; `m=19456 KiB, t=2, p=1` — OWASP recommended baseline; random 16-byte salt per user). Store only `hash` + algorithm params. **Never** plaintext, anywhere.
- **Duplicate handling / enumeration:** registration and recovery endpoints return the same generic success response whether or not the email exists ("If this address can be registered, a verification email is on its way."). Verification emails are only actually sent to new/unverified addresses; the response never distinguishes.
- **Email verification:** 32 random bytes, sent as URL-safe base64 in a `#/verify-email?token=…` link; server stores **`sha256(token)` only**, single-use (`used_at`), TTL 24 h; requesting a resend invalidates outstanding tokens. Constant-time comparison on use.
- **Sign-in / sign-out:** generic `401 Invalid email or password` for unknown email and wrong password alike (constant-time-ish behaviour with a dummy argon2 verify for unknown users to blunt timing probes). *Documented trade-off:* a distinct `403 email_unverified` is returned **only after a correct password**, because at that point the caller has already proven account knowledge — this keeps the app able to say "verify your email" without enabling enumeration. Logout revokes the session server-side and clears the keyring entry.
- **Password change:** requires current password; revokes all other sessions; audit-logged (metadata only).
- **Password recovery:** request endpoint always `202`; token 32 random bytes, `sha256` at rest, TTL 60 min, single-use; completing a reset revokes **all** sessions and invalidates all outstanding reset/verification tokens for that user. Never store password-reset tokens plaintext; never log them.
- **Rate limiting / abuse:** per-IP and per-account sliding windows on `login`, `register`, `recovery-request`, `resend-verification`, `google/exchange` (e.g. 5/15 min per key, exponential lockout after repeated failures, `429` + `Retry-After`); backed by a `login_attempts` table in v1 (Redis only if load demands); generic error bodies.
- **Input validation & errors:** server re-validates everything (never trust the UI); error responses are `{ error: { code, message } }` with stable machine codes; messages never include secrets, hashes, tokens, or stack traces; 5xx responses are opaque to clients.
- **Account state:** `status` (`active | locked | deleted`) + `email_verified_at`; locked accounts get generic errors from login (no "locked" disclosure beyond what correct-password callers may see for support UX — to be finalized in implementation).

**Never store plaintext passwords or password-reset tokens. Never log passwords, session credentials, OAuth secrets, or verification/reset tokens** (log scrubbing middleware; token values only ever appear in outbound email links and then die server-side as hashes).

## 2.5 Google OAuth (planned behaviour)

- Flow: **OAuth 2.0 Authorization Code + PKCE** (S256) with OpenID Connect identity claims, executed in the system browser with loopback redirect (desktop pattern above).
- **State validation:** random 128-bit `state` generated alongside the PKCE verifier, held in the Tauri process and mirrored (hashed) server-side; single-use, 10-min TTL; mismatch ⇒ hard failure. **Nonce** included and validated in the ID token (OIDC) where the response type yields one.
- **Redirect URI validation:** exact match against registered URIs (`http://127.0.0.1:<port>/callback` pattern registered with Google as a loopback redirect for a *Desktop app* client); the exchange refuses any other `redirect_uri`.
- **Code exchange:** performed by the **auth server** (or, if Google's desktop client type is used without a secret, still routed through the server for uniform auditing and account linking). Client secret lives only in the server secret store; no secret in the binary, repo, or WebView.
- **Account-linking / takeover rules:** a Google identity may only auto-link to an existing account when the Google-verified email **exactly matches an already-verified** account email. Unverified matches require proof of control of the existing account (re-authentication) before linking. Never auto-link on unverified provider emails. A `oauth_accounts` row is keyed by immutable `provider_subject` (Google `sub`), not by email.
- **Dev vs prod configuration:** separate Google Cloud OAuth clients (`Nexora Dev`, `Nexora Prod`) with distinct redirect URIs and consent screens; env-driven (`GOOGLE_OAUTH_*` placeholders in §8); dev may use loopback `127.0.0.1`, production registers the same loopback pattern per desktop-app rules. No credentials are invented in this document.

## 2.6 Offline-first behaviour (non-negotiable constraints)

1. **Local models remain usable offline** — inference (`inference/`, llama.cpp child process) never calls the auth API. No code path may gate `start_generation` on session validity.
2. **No mandatory external AI API** — preserved by construction (the only `reqwest` target is `127.0.0.1`); auth is a separate, account-only dependency.
3. **Network outage must not corrupt projects or erase settings** — all persistence (conversations.json, workspace-settings.json, future patches) stays local and atomic (existing tempfile+sync pattern); auth state changes are write-through to the server only for account entities.
4. **Account features require connectivity; offline AI does not** — account-dependent features (future sync/license) must degrade to an explicit "Offline — account features unavailable" state.
5. **Auth must not silently block local projects** — no startup auth gate; the workspace opens regardless of session state.
6. **Offline honesty rule:** the UI may show a cached profile *label* (e.g. "Signed in as Ada (offline)") but must never present cached state as verified authentication; actions that need proof (e.g. sync) revalidate or fail closed with an explanation. A failed network check is **not** a logout; only an explicit `401/revoked` from the server ends a session.

---

## 3. Route-by-route frontend integration plan (design contracts only)

Common conventions: requests are made by **Rust commands** (`auth_login`, `auth_register`, …) wrapping `reqwest` against `AUTH_API_BASE_URL`; the WebView never receives tokens. All forms keep current client-side validation (UX) while the backend re-validates (authority). Loading = existing `aria-busy`/disabled pattern; errors map to existing `role="alert"` field errors or `role="status"` blocks; network errors show "Can't reach the Nexora account service. Check your connection and try again." with retry preserved and **no credential clearing beyond current Phase 22 behaviour** (passwords still clear only after completed submissions).

### `#/signin`
- **Request:** `POST /v1/auth/login` `{ email, password }` (from `auth_login` IPC).
- **Success (200):** `{ user: { id, name, email, emailVerified }, session: { expiresAt } }` — token goes to keyring inside Rust; UI transitions to a signed-in state (back to `#/home` or the originating flow step).
- **Errors:** `401 invalid_credentials` → generic field-agnostic message "Invalid email or password."; `403 email_unverified` (correct password only) → status block with a *Resend verification* action; `429 rate_limited` → "Too many attempts. Try again in N minutes."; `5xx/network` → connectivity message.
- **Validation:** frontend non-empty/format; backend full validation + argon2.
- **Session transition:** none → active session (keyring + server row).
- **Rate limit/network behaviour:** button re-enables on failure; attempts counted server-side; UI shows `Retry-After` when present.
- **A11y/nav:** errors tied to fields via `aria-describedby`; status block `role="status"`; Google button remains a distinct entry (see below).

### `#/signup`
- **Request:** `POST /v1/auth/register` `{ name, email, password }`.
- **Success (202):** `{ status: "verification_pending" }` — identical response whether or not the email already existed (enumeration defence). UI: existing honest status area is replaced by a real "Check your email to continue" state that links to `#/verify-email`.
- **Errors:** `400 validation` → field errors; `429` → rate-limit message.
- **Session transition:** none (registration does not sign in until verification, per chosen policy — *unresolved decision #4*).
- **Token handling:** none client-side (verification token travels only in the email link).
- **A11y:** keep `aria-invalid`/`aria-describedby`, focus-first-invalid.

### `#/verify-email`
- **Deep link:** `#/verify-email?token=…` (router gains query-string parsing; hash router unchanged). Page states: *pending* (no token — matches today's honest page), *verifying*, *verified*, *invalid/expired*.
- **Requests:** `POST /v1/auth/verify-email` `{ token }`; `POST /v1/auth/resend-verification` `{ email }` (new, rate-limited).
- **Success (200):** `{ status: "verified" }` → "Email verified — you can sign in." + link to `#/signin`.
- **Errors:** `400 invalid_or_expired_token` (single generic message) with resend action; `429` for resend abuse. **No token is ever displayed, logged, or re-sent by the UI.**
- **Session transition:** none.

### `#/recover-password`
- **Request:** `POST /v1/auth/recovery-request` `{ email }`.
- **Success (202):** always `{ status: "accepted" }` — "If that address has an account, a reset link is on its way." (never confirms existence).
- **Errors:** `400 validation`, `429 rate_limited`; network as above. No "email sent" success theatre — the message is conditional and honest.
- **Rate limit:** strict (e.g. 3/hour per email + IP windows).

### `#/reset-password`
- **Deep link:** `#/reset-password?token=…` from the recovery email (today's page documents that tokens are not yet accepted).
- **Request:** `POST /v1/auth/reset-password` `{ token, password }`.
- **Success (204):** "Password updated — sign in with your new password."; **all sessions revoked**; token burned.
- **Errors:** `400 invalid_or_expired_token` (generic, offers `#/recover-password` restart), `400 validation` (min/max length), `429`.
- **Session transition:** all → revoked.

### Google button (on `#/signin`)
- **Flow:** `auth_google_begin` IPC → Rust builds state+PKCE → system browser → loopback → `POST /v1/auth/google/exchange` `{ code, codeVerifier, state, redirectUri }` → `200 { user, session }` (or `409 email_conflict` → guided linking flow).
- **Errors:** state mismatch/expired → generic failure + retry; user-closed browser → cancellable idle state; `409` → "An account with this email already exists. Sign in with your password to link Google."

---

## 4. Threat model and security controls

| Threat | Control |
|---|---|
| Password theft (server breach) | argon2id (OWASP params), unique salts; no plaintext anywhere; DB backups encrypted |
| Password theft (client) | passwords only in React state during typing; cleared after completed submissions (existing P22 behaviour); never logged/stored/transmitted pre-backend; CSP blocks WebView exfiltration channels |
| Session token theft (disk) | Windows Credential Manager (`keyring`/DPAPI); server-side `sha256` at rest; rotation + family reuse detection; remote revocation |
| Session token theft (memory/process) | tokens live in the Rust process only; WebView excluded by design |
| Account enumeration | generic registration/recovery responses; dummy argon2 verify for unknown logins; uniform 202s |
| Credential stuffing / brute force | per-IP+per-account rate limits, lockout backoff, `Retry-After`, generic errors; optional breached-password check later (NIST practice) |
| Reset/verification token abuse | 32-byte random, hashed at rest, single-use, short TTL, invalidation on use/password-change, never logged |
| OAuth CSRF | single-use `state`, 10-min TTL, server-mirrored |
| OAuth code interception | PKCE S256; loopback `127.0.0.1` redirect with ephemeral port; exact redirect-URI match |
| OAuth account takeover via linking | link only on verified-email match or prior re-auth; identity keyed by `sub` |
| XSS in WebView → credential theft | existing strict CSP (`default-src 'self'`, no `unsafe-inline` scripts in prod); no `dangerouslySetInnerHTML` in the UI tree; tokens never in JS |
| MITM | TLS 1.2+ only (`reqwest` default roots), HSTS on the API; certificate pinning listed as a later hardening option, not a v1 requirement |
| Secret extraction from installer | no client secrets in the binary; Google exchange routed through the server; SMTP/DB/OAuth secrets server-side only |
| Log leakage | structured logging with a scrub list (password, token, authorization, cookie, code_verifier); token values appear only in email links |
| Supply chain | keep dependency additions minimal (`axum`, `sqlx`, `argon2`, `keyring`, `lettre`/provider SDK); `pnpm-lock.yaml`/`Cargo.lock` committed; CI `cargo audit`/`pnpm audit` |
| Revocation gap | logout/reset/change all revoke server-side; short idle TTL; `revoked_at` checked per request |

---

## 5. Database entity outline (PostgreSQL)

```
users
  id                  uuid pk
  email               citext unique not null          -- canonical lowercase
  name                text not null
  password_hash       text null                       -- argon2id PHC string; null = OAuth-only
  email_verified_at   timestamptz null
  status              text not null default 'active'  -- active | locked | deleted
  created_at / updated_at timestamptz

sessions
  id                  uuid pk
  user_id             uuid fk -> users
  token_hash          bytea unique not null           -- sha256(opaque token)
  family_id           uuid not null                   -- rotation/reuse grouping
  created_at          timestamptz
  expires_at          timestamptz                     -- absolute
  last_used_at        timestamptz                     -- idle expiry
  revoked_at          timestamptz null
  app_version / platform text

email_verification_tokens
  id uuid pk, user_id uuid fk
  token_hash bytea unique not null
  expires_at timestamptz, used_at timestamptz null

password_reset_tokens
  id uuid pk, user_id uuid fk
  token_hash bytea unique not null
  expires_at timestamptz, used_at timestamptz null

oauth_accounts
  id uuid pk, user_id uuid fk
  provider text not null                    -- 'google'
  provider_subject text not null            -- Google 'sub' (immutable)
  provider_email text
  created_at timestamptz
  unique (provider, provider_subject)

login_attempts                              -- rate limiting / abuse (v1)
  id bigserial pk
  scope text not null                       -- 'login' | 'register' | 'recovery' | 'resend' | 'google'
  key text not null                         -- ip | email-hash | account-id
  outcome text not null
  created_at timestamptz
```

Notes: tokens exist **only** as `sha256` digests; no password-reset or verification token is ever stored in plaintext. Sessions are rows (revocable). `login_attempts` can move to Redis without contract changes. Schema changes ship as versioned `sqlx` migrations; no schema or migration is created in this phase.

---

## 6. Proposed API contracts (summary)

Base: `https://<AUTH_API_BASE_URL>/v1/auth/*`, JSON in/out, TLS only. Errors: `{ error: { code, message } }` with stable codes (`validation_failed`, `invalid_credentials`, `email_unverified`, `invalid_or_expired_token`, `rate_limited`, `email_conflict`, `internal`). Authenticated calls send `Authorization: Bearer <opaque session token>` (attached by Rust).

| Endpoint | Req → Res | Notes |
|---|---|---|
| `POST /register` | `{name,email,password}` → `202 {status:"verification_pending"}` | Generic; sends verification email to new users |
| `POST /verify-email` | `{token}` → `200 {status:"verified"}` | Single-use; hashed at rest |
| `POST /resend-verification` | `{email}` → `202 {}` | Always 202; rate-limited |
| `POST /login` | `{email,password}` → `200 {user, session:{expiresAt}}` | Token never in response body to the WebView (goes to keyring via Rust) |
| `POST /logout` | – → `204` | Revokes session + clears keyring |
| `POST /refresh` | `{refreshToken}` handled in Rust → `200` rotated | Family reuse detection |
| `GET /me` | – → `200 {user}` | Session validity checks |
| `POST /recovery-request` | `{email}` → `202 {}` | Always 202 |
| `POST /reset-password` | `{token,password}` → `204` | Revokes all sessions |
| `POST /password-change` | `{currentPassword,newPassword}` → `204` | Revokes other sessions |
| `POST /google/exchange` | `{code, codeVerifier, state, redirectUri}` → `200 {user, session}` \| `409 email_conflict` | Server holds client secret |

---

## 7. Environment-variable inventory (placeholders only — never real values)

| Variable | Purpose | Example shape |
|---|---|---|
| `AUTH_ENVIRONMENT` | development / staging / production semantics | `<development\|staging\|production>` |
| `DATABASE_URL` | Postgres connection string | `postgres://<user>:<placeholder>@<host>:5432/<db>` |
| `SESSION_TTL_SECONDS` / `SESSION_IDLE_TTL_SECONDS` | absolute / idle session lifetimes | `2592000` / `604800` |
| `VERIFICATION_TTL_SECONDS` / `RESET_TTL_SECONDS` | email token lifetimes | `86400` / `3600` |
| `GOOGLE_OAUTH_CLIENT_ID` | Google OAuth client id | `<placeholder>` |
| `GOOGLE_OAUTH_CLIENT_SECRET` | **server-side secret store only** | `<placeholder — never in repo/binary/chat>` |
| `GOOGLE_OAUTH_REDIRECT_URI` | registered loopback redirect | `http://127.0.0.1:<port>/callback` |
| `EMAIL_PROVIDER` / `SMTP_HOST` / `SMTP_PORT` / `SMTP_USERNAME` / `SMTP_PASSWORD` / `EMAIL_FROM` | transactional email (or provider API key as alternative) | `<placeholder>` |
| `AUTH_RATE_LIMIT_LOGIN` / `_REGISTER` / `_RECOVERY` / `_RESEND` | window sizes | e.g. `5/15m`, `3/60m` |
| `AUTH_API_BASE_URL` | desktop-side API origin (public config, not secret) | `https://<host>` |
| `RUST_LOG` | structured log level | `<info\|debug>` |

**Secret storage:** dev `.env` files gitignored and never committed (none exist today); staging/production secrets in the host's secret manager (Fly/Railway/AWS SM), injected at start; **rotation:** OAuth client secret and SMTP credentials rotate via provider consoles + redeploy; session secret material needs no rotation (per-token random, hashed). **Backups:** encrypted DB snapshots with restore drills; token tables included (hashes only). **Migrations:** `sqlx migrate` run as a gated deploy step with prior backup. **Deployment prerequisites:** TLS termination + HSTS, private network between API and Postgres, least-privilege DB role, log scrubbing enabled, `cargo audit`/`pnpm audit` in CI, and a smoke checklist (register→verify→login→logout→reset) before release.

---

## 8. Testing & deployment strategy (future — not run in this phase)

**Backend:** password hashing/verification (param sanity, wrong-password, timing-safe compares); registration validation + duplicate/generic-response equality; verification happy-path, expiry, reuse prevention, resend invalidation; login success/failure/unverified/locked; session create/expire/idle/revocation/logout/refresh-rotation + reuse detection; Google state/nonce/PKCE validation, redirect-URI mismatch, code-replay, linking rules (verified match, unverified reject, conflict 409); recovery request always-202, reset single-use, reset-reuse prevention, all-sessions-revoked; rate-limit windows and `Retry-After`; secret-leakage tests (log capture asserts no password/token/verifier/authorization material in any log line or error body); fuzz/property tests on validators.

**Frontend:** keep the 558-test suite green; add contract tests for each route's loading/success/error mapping (mocked IPC), verification/reset deep-link state machine, network-error retry UX, keyboard/a11y coverage for new states, and the existing "no storage/logging of credentials" spies extended to the IPC boundary.

**Offline/local-AI regression:** automated suite proving generation, models, projects, and settings work with the auth API unreachable (link-down simulation) and that no auth gate blocks the workspace.

**Tauri/Windows:** capability review (no WebView HTTP), keyring round-trip on Windows, WebView2 autofill behaviour, installer smoke (NSIS), runtime behaviour under standard-user (non-admin) installs.

**Environments:** `development` (local docker compose Postgres + mailpit/MailHog catcher + Google dev client), `staging` (staging Google client, real email to allowlisted addresses), `production`. CI gates: typecheck + 558+ tests + `cargo test` + `cargo audit` + migration dry-run; release checklist documents backup→migrate→smoke→tag.

---

## 9. Implementation sequence (for a future, separately approved phase)

1. API skeleton: `axum` service, config loader (§7), `sqlx` migrations for §5 tables, health endpoint, error envelope, log scrubbing. *(No UI changes.)*
2. Register + verify-email + resend (email delivery behind a provider trait; dev mailpit).
3. Login + sessions + logout + refresh (+ keyring-backed Rust `auth_*` commands; wire `#/signin`).
4. Recovery + reset + password-change (wire `#/recover-password`, `#/reset-password` deep links).
5. Google OAuth (state/PKCE/loopback + linking rules; wire the Google button).
6. Rate limiting + abuse hardening + secret-leakage test suite.
7. Offline-AI regression + Windows/manual verification pass + docs update.

**Unresolved decisions (need owner input before implementation):**
1. Hosting target (Fly/Railway/AWS/self-host) and budget.
2. Email provider (SMTP vs Resend/Postmark) and domain/DNS (SPF/DKIM/DMARC).
3. Google client type (Desktop app vs Web client + server exchange) — affects where the secret lives.
4. Registration policy: sign-in allowed before verification or not (plan assumes verification-first).
5. Session model: opaque+refresh (recommended) vs JWT access tokens.
6. Whether any paid/licence features ride on accounts (out of scope regardless — see §10).
7. Account deletion/self-service and data-export requirements (GDPR posture).

---

## 10. Explicit scope boundaries

- **Phase 23 (this document):** audit + architecture plan only. **No implementation, no dependencies, no migrations, no OAuth configuration, no behaviour changes** were permitted or made.
- **Phase 24+ implementation (future, separately approved):** the sequence in §9 — backend, then desktop wiring — each step gated on explicit approval.
- **Out of scope entirely (later phases or never):** Developer Account / Founder Control Centre; payments, subscriptions, licensing, monetization; glassmorphism (29); animation/aurora system (30); workspace redesigns; any change to local inference, models, projects, or settings behaviour; mandatory online mode.
- **Standing constraints carried forward:** never fabricate success; never log or store plaintext credentials/tokens; preserve the local-first guarantees in §2.6; keep the WebView free of secrets and raw HTTP; preserve all Phases 19–22 work and the 558-test baseline.

## 11. Git

This document is the only file created in Phase 23 (`docs/phase-23/README.md`; the directory did not previously exist — nothing was overwritten). All Phases 19–22 work remains uncommitted and intact on `arena/1008499f-nexora` at `1287816`. No commit, push, PR, release, visibility change, branch switch, reset, clean, or history rewrite occurred. Phase 24 was **not** started.
