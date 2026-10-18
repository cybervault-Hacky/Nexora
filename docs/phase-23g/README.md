# Phase 23G — Authentication Security Hardening (plan §9 step 6)

**Status:** Implemented in the local working tree; uncommitted. **No Rust code has been compiled or executed** (environment re-verified unchanged this phase: `cargo`/`rustc`/`rustfmt` absent (127), crates.io TLS-blocked (exit 35, http 000), PostgreSQL absent). Rust code is reviewable but **compiler-unverified**. Step 7 (offline-AI regression + Windows/manual verification) was **not** started.

**Exact roadmap item implemented:** `docs/phase-23/README.md` §9 step 6 — **"Rate limiting + abuse hardening + secret-leakage test suite."** Supporting requirements: §4 (rate-limit rows: per-IP/per-account windows on `login`, `register`, `recovery-request`, `resend-verification`, `google/exchange`, **"exponential lockout after repeated failures"**, `429` + `Retry-After`, `login_attempts`-backed, generic bodies; "Log leakage" row: **"structured logging with a scrub list (password, token, authorization, cookie, code_verifier)"**; password change "audit-logged (metadata only)"; "messages never include secrets, hashes, tokens, or stack traces"), §7 (**"secret-leakage tests (log capture asserts no password/token/verifier/authorization material in any log line or error body); fuzz/property tests on validators"**). The working label 23G continues the repository's 23A–23F naming of the §9 sequence.

## 1. Security findings (audit) and fixes

| Finding | Fix |
|---|---|
| **Derived `Debug` on 8 secret-bearing types** rendered passwords, raw session tokens, verification/reset tokens, OAuth codes/PKCE verifiers/state/nonce, and **email bodies containing raw reset links** in any `{:?}`/panic/trace output | Redacting `Debug` impls (via `scrub::REDACTED`) for `LoginRequest`, `RegisterRequest`, `VerifyEmailRequest`, `ResetPasswordRequest`, `ChangePasswordRequest`, `GoogleExchangeRequest`, `CreatedSession`, `EmailMessage`, `IdTokenClaims` |
| **OAuth ID tokens were payload-decoded without signature verification** (23F's documented OIDC direct-receipt posture) — not production-safe | **Proper OIDC verification** (`src/oauth.rs`): RS256 signature against Google's JWKS (`https://www.googleapis.com/oauth2/v3/certs`, cached 1 h, refetched on unknown `kid` = key rotation) + registered-claim checks (`iss` ∈ Google issuers, `aud` == client id, `exp`); `alg:"none"`/symmetric algs refused; **unverified payloads are never trusted**. Dependency `base64` (23F payload-only decode) replaced by `jsonwebtoken = "9"` |
| **No log scrubbing layer** (plan §4 scrub list) | New `src/scrub.rs`: `scrub()` redacts `key=value`/`key: value` pairs on the plan's list (password, token, authorization, cookie, code_verifier + equivalent secret names) and any `Bearer <…>`; the startup line passes through it |
| Password change not audit-logged (plan §4) | `scrub::audit(action, subject_id, outcome)` metadata-only lines (`password_change ok/denied`) — no emails, no credentials, scrubbed before printing |
| **No exponential lockout backoff** (plan §4: "exponential lockout after repeated failures"; threat row "lockout backoff") | `rate_limit::escalated_retry_after`: Retry-After doubles per additional exhausted window (sliding-horizon failure count), **capped at 24 h — no indefinite lockout** (keeps 23D's anti-DoS stance); wired into `check()` |
| Proxy-header trust (brief §2) | The plan defines **no trusted-proxy configuration**, so `X-Forwarded-For`/`Forwarded` are **never trusted** by design — keys come from the socket peer (`ConnectInfo`) only; covered by a real-DB test with spoofed headers |

Concurrency-sensitive boundaries (exactly-once token consume, account row locks vs password changes, session-family revocation) were audited against the plan and remain as implemented in 23C–23F (tests kept; no weakening changes). Input limits (4 KiB bodies, `deny_unknown_fields`, length caps on every validator) verified by the property suite.

## 2. Secret-leakage test suite (plan §7)

`tests/secret_leakage.rs` plants sentinel passwords/tokens/codes/verifiers/states/DB-URLs/OAuth secrets/ bearer tokens and asserts they never appear in: every redacting `Debug` rendering; every stable `message()` surface (`LoginError`/`RecoveryError`/`OAuthError`/`ConfigError`); scrubbed log lines (all plan scrub-list forms); and **real HTTP error envelopes from every auth route** (login/register/verify/reset/change/google/recovery + bearer rejections on logout/refresh/me) — request secrets are never echoed. Deliberate response fields (the session token returned to the trusted client by design, 23D placement rules) are out of scope and documented as such.

`tests/validator_props.rs` (plan §7 "fuzz/property tests on validators"): deterministic pseudo-random corpus (LCG — no new dependencies) over email/password/token-shape/PKCE/state/redirect/rate-parse/Google-settings validators — **totality (never panic)** + acceptances imply documented bounds + boundary cases. Full `cargo-fuzz` targets remain toolchain-gated (step 6's documented remainder in this environment).

## 3. Rate-limit/abuse hardening state (plan §4)

Shared-database `login_attempts` windows (multi-instance-safe) cover every plan scope (`login`, `register`, `recovery-request`, `resend-verification`, `google/exchange` — plus `recovery` reset/change defense-in-depth from 23E). Per-IP **and** per-account keys (hashed `e:`/`i:`/`t:`); failures-only for login, all-attempts for volume scopes; `429` + `Retry-After` with generic bodies; exponential backoff for persistent offenders (unit + real-DB tests); no permanent account lockout. Tuning stays an ops concern (plan: Redis only if load demands).

## 4. Tests — written vs actually run

| Suite | Written | Ran? | Result |
|---|---|---|---|
| Frontend `pnpm typecheck` / `pnpm test` | (existing) | **YES** | **PASS — 50 files / 569 tests** (apps/ byte-unchanged this phase) |
| `git diff --check` | — | **YES** | **PASS** |
| `scrub.rs` units (5) | YES | **NO** | cargo absent (127) + crates.io blocked (35) |
| `rate_limit.rs` new backoff unit (1; 5 total) | YES | **NO** | same |
| `oauth.rs` units (9; incl. **2 real RS256 signature tests** with a generated test-only RSA fixture — valid, tampered, wrong aud/iss/expired, unknown/missing kid, alg-none, junk) | YES | **NO** | same |
| `tests/secret_leakage.rs` (6) | YES | **NO** | same |
| `tests/validator_props.rs` (3) | YES | **NO** | same |
| `tests/hardening_db.rs` (5, **real PostgreSQL**, `#[ignore]`) | YES | **NO** | PostgreSQL absent; `NEXORA_AUTH_TEST_DATABASE_URL=… cargo test --test hardening_db -- --ignored` |
| 23C–23F suites (unchanged) | (81) | **NO** | same blocker |
| `cargo fmt` / `cargo check` | — | **NO** | toolchain absent |

Crate total: **178 nexora-auth Rust tests written, 0 executed** (desktop 101 unchanged). Static cross-checks (symbols/redaction/dep hygiene) pass — consistency evidence only.

## 5. OAuth verification status & remaining security limitations

**After this phase the ID-token path performs real OIDC verification** (RS256 signature vs Google JWKS + iss/aud/exp). Remaining honest limitations: **nothing is compiler-verified in this environment**; live-Google end-to-end is untested here (no network/credentials); `jsonwebtoken`/`reqwest` dependency resolution is unverified (no crates.io, no `Cargo.lock`); JWKS caching is per-process (no shared cache); TLS/HSTS, private DB networking, least-privilege DB roles, `cargo audit`/`pnpm audit`, and backup/migration drills are deployment prerequisites (plan §8), not code; full `cargo-fuzz` corpora await a toolchain; desktop IPC/keyring wiring (step 5 client half) remains blocked. OAuth is **not claimed production-safe** beyond the code-level contracts above.

## 6. Files created / modified

**Created:** `src/scrub.rs`, `tests/secret_leakage.rs`, `tests/validator_props.rs`, `tests/hardening_db.rs`, `docs/phase-23g/README.md`. **Modified:** `src/oauth.rs` (JWKS verification + 2 redacting Debugs + tests), `src/rate_limit.rs` (backoff + test), `src/login.rs` / `src/registration.rs` / `src/recovery.rs` / `src/email.rs` / `src/session.rs` (redacting Debugs), `src/app.rs` (audit lines), `src/main.rs` (scrubbed startup log), `src/lib.rs` (module), `Cargo.toml` (`jsonwebtoken` replaces `base64` — justification inline), `README.md`. **Apps/desktop byte-unchanged.** No new migrations (schema from 23B–23F suffices; nothing prior touched).

## 7. Git

Local and uncommitted on `arena/1008499f-nexora`, layered on the preserved Phase 19–23F + multi-file-patch tree. No commit, push, merge, release, visibility change, reset, clean, or history rewrite. Step 7 was **not** started and requires explicit approval.
