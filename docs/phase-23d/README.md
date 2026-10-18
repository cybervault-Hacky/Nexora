# Phase 23D — Login & Secure Sessions

**Status:** Implemented in the local working tree; uncommitted. **No Rust code has been compiled or executed** (environment unchanged: `cargo`/`rustc`/`rustfmt` exit 127, crates.io TLS-blocked exit 35, PostgreSQL absent) — artifacts are reviewable and consistent with Phases 23A–23C, but nothing is compiler-verified. Phase 23E+ was not started.

**Specification followed:** `docs/phase-23/README.md` §6 (API contracts), §4 (threat-model controls), §5 (entities), §7 (env inventory) — the plan's sequence **step 3** ("Login + sessions + logout + refresh (+ keyring-backed Rust `auth_*` commands; wire `#/signin`)"), executed as the roadmap's Phase 23D on top of 23A (scaffold), 23B (Argon2id + users schema), and 23C (registration/verification). **Not included** (later steps): password recovery/reset (step 4), Google OAuth (step 5), desktop IPC/keyring wiring (step 3's client half — see §5), full abuse hardening + secret-leakage suite (step 6's remainder).

## 1. Login — `POST /v1/auth/login {email,password}` → `200 {user, session:{token, expiresAt}}`

- Email normalized exactly like registration (trim + lowercase, `local@domain.tld`, ≤254); parameterized lookup only.
- Passwords verified exclusively through the Phase 23B Argon2id module — **never stored or logged in plaintext**; hashing parameters untouched.
- **Generic `invalid_credentials` (401) for unknown accounts AND wrong passwords**, with a dummy Argon2id verification for unknown accounts so timing does not separate the two (plan §4 enumeration defence).
- Account-status model (23B `active|locked|deleted`): locked/deleted → the same generic `invalid_credentials`. Correct password on an **unverified** account → `email_unverified` (403, plan §6 stable code) — verification-first policy (plan §9 decision 4). The caller proved the credential, so this is not an enumeration channel.
- Request bodies are strict (`deny_unknown_fields`), 4 KiB limit preserved; responses never contain password hashes, raw credentials, DB errors, or secret config values. Login validates shape but does **not** enforce the 8–128 policy against the candidate (that would leak policy state per account).
- **Token placement (plan §6 clarification):** the raw session token appears in the login/refresh response body **to the trusted desktop Rust client over the approved transport**, which stores it in the OS credential store; it is never delivered to the WebView (CSP-blocked from this API), never logged, never placed in URLs. The plan's table abbreviates `session:{expiresAt}` for the WebView-facing shape.

## 2. Sessions — migration `20261018000004_create_sessions_and_login_attempts.sql`

`sessions` per plan §5: `token_hash bytea UNIQUE` = **sha256(raw 32-byte token)** (raw never stored), `family_id` (rotation/reuse grouping), `created_at`, `expires_at` (absolute, default 30 d), `last_used_at` (idle anchor, default idle 7 d), `revoked_at`, `app_version`/`platform`; CHECK `expires_at > created_at`; indexes on `user_id`/`family_id`. Forward-only; previous migrations untouched; documented forward-recovery reversal.

**Lifecycle (enforced on every authenticated request):**
- Bearer `Authorization: <opaque token>`; missing/malformed → generic `invalid_or_expired_token` (401).
- Absolute + idle expiry both enforced; `revoked_at` checked every time (plan §4 "revocation gap"); `last_used_at` refreshed throttled (≤1 write/min) with `WHERE revoked_at IS NULL` — concurrent requests can never revive a revoked session.
- **`POST /v1/auth/logout`** → 204: revokes the whole family (no surviving rotated siblings). Monotonic `revoked_at = COALESCE(revoked_at, now())` — safe under concurrency.
- **`POST /v1/auth/refresh`** → 200 rotated: new token **in the same family**, old one revoked inside a transaction (a concurrent double-rotate loses cleanly). Presenting an already-revoked token = **family reuse detected** → the entire family is revoked and the caller gets the generic token error (plan §4 rotation + family reuse detection). Clients must single-flight refresh — a concurrent double-refresh kills the family by design.
- **`GET /v1/auth/me`** → `200 {user}`: the authenticated-request enforcement example (id, name, email only).
- Session tokens are 32 `OsRng` bytes (64-hex), high-entropy, single-value credentials; sha256-at-rest is correct for high-entropy tokens (indexed lookup) — passwords remain Argon2id.

## 3. Rate limiting (DB-backed v1 — honest scope note)

Implemented as the plan's §5 `login_attempts` store: **shared PostgreSQL, so counts hold across multiple backend instances** (no in-memory counters). Sliding windows over hashed keys only (`e:`+sha256(email), `i:`+sha256(ip) via `ConnectInfo`): login counts **failures only** (a legitimate user signing in repeatedly is never limited); register/resend count **all attempts** (volume abuse). Exceeded windows return **429 `rate_limited` + `Retry-After`** — **window-based only, no permanent account lockout** (which would let attackers deny service). Defaults: login 10/15m, register 5/60m, resend 5/15m (`NEXORA_AUTH_RATE_LIMIT_LOGIN|REGISTER|RESEND`, plan §7 `5/15m` syntax).

**Honest limitations:** this is the plan's v1 table design ("can move to Redis without contract changes"); the step-6 track still owns proxy/X-Forwarded-For trust, tuning, and the secret-leakage suite. The service is **not production-grade** while compiler-unverified — nothing here is claimed as production-ready.

## 4. Configuration

New env vars (validated at startup; values never printed; redacting `Debug`): `NEXORA_AUTH_SESSION_TTL_SECONDS` (300–31536000, default 2592000), `NEXORA_AUTH_SESSION_IDLE_TTL_SECONDS` (default 604800), `NEXORA_AUTH_RATE_LIMIT_LOGIN/_REGISTER/_RESEND` (`5/15m` syntax; malformed values rejected with "(Value withheld.)"). All prior Phase 23B/23C config behaviour unchanged. No `.env` files, no real credentials.

## 5. Tauri/frontend integration — BLOCKED (documented, not faked)

Per brief §5: secure desktop transport is unavailable, so **only the safe backend portion is implemented**. The desktop wiring (plan step 3's client half: `auth_*` IPC commands, keyring/DPAPI storage of the raw token, `#/signin` integration) requires the Tauri crate + a verified toolchain and is **blocked**. Frontend is **byte-unchanged**: the sign-in page keeps its honest disconnected state (no fake login, no mock success); local AI, local projects, and local settings remain fully usable without login or internet (nothing in `apps/` references this service).

## 6. Tests — written vs actually run

| Suite | Written | Ran? | Result |
|---|---|---|---|
| Frontend `pnpm typecheck` / `pnpm test` | (existing) | **YES** | **PASS — 50 files / 569 tests** (app untouched) |
| `git diff --check` | — | **YES** | **PASS** |
| `session.rs` units (5) | YES | **NO** | cargo absent (127) + crates.io blocked (35) |
| `login.rs` units (6) | YES | **NO** | same |
| `rate_limit.rs` units (4) | YES | **NO** | same |
| `config.rs` new units (3; 19 total) | YES | **NO** | same |
| `tests/login_http.rs` (7, no DB) | YES | **NO** | same — intended to run first in CI |
| `tests/login_db.rs` (10, **real PostgreSQL**, `#[ignore]`) | YES | **NO** | PostgreSQL absent; `NEXORA_AUTH_TEST_DATABASE_URL=… cargo test --test login_db -- --ignored` |
| 23C suites (unchanged; regression covered in `login_db.rs`) | (33) | **NO** | same blocker |
| `cargo fmt` / `cargo check` | — | **NO** | toolchain absent |

Coverage intent: correct/incorrect passwords; unknown-account generic equality (incl. timing note); unverified/locked refusal; token entropy + sha256-at-rest; absolute + idle expiry; logout family revocation; rotation single-use + family-reuse revocation; concurrent revoke-vs-use; rate-limit windows without lockout; honest 503s and JSON envelopes without credential echoes; Phase 23C regressions (register→verify→login, concurrent register).

## 7. Known limitations / follow-ups (existing plan)

- **Compiler-unverified blocker (same as 23B/23C):** `cargo fmt --check`, `cargo check --all-targets`, `cargo test`, then the `--ignored` DB suites on a disposable PostgreSQL must pass before reliance.
- Desktop IPC + keyring storage + `#/signin` wiring (plan step 3 client half) — blocked (§5).
- Recovery/reset (step 4), Google OAuth (step 5), rate-limit hardening + secret-leakage suite (step 6 remainder) — later plan steps, not started.
- Session tokens in the API response body are visible to the desktop Rust client only; a deployment must terminate TLS (plan §4 MITM control) — loopback/dev posture here.

## 8. Git

Local and uncommitted on `arena/1008499f-nexora`, layered on the preserved Phase 19–23C + multi-file-patch tree. No commit, push, merge, release, visibility change, reset, clean, or history rewrite occurred. Phase 23E was **not** started.
