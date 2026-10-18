# Phase 23E — Password Recovery & Reset

**Status:** Implemented in the local working tree; uncommitted. **No Rust code has been compiled or executed** (environment re-verified unchanged this phase: `cargo`/`rustc`/`rustfmt` exit 127, crates.io TLS-blocked exit 35, PostgreSQL absent) — artifacts are reviewable and consistent with Phases 23A–23D, but nothing is compiler-verified. Phase 23F+ was not started.

**Specification followed:** `docs/phase-23/README.md` §6 (API contracts), §4 (threat model: "Password recovery: request endpoint always `202`; token 32 random bytes, `sha256` at rest, TTL 60 min, single-use; completing a reset revokes **all** sessions and invalidates all outstanding reset/verification tokens"), §5 (`password_reset_tokens` entity; `login_attempts` scopes), §7 (`RESET_TTL_SECONDS` default `3600`, `AUTH_RATE_LIMIT_RECOVERY` e.g. `5/15m`), and the plan's sequence step 4 ("Recovery + reset + password-change") — which explicitly carries the `POST /password-change {currentPassword,newPassword} → 204` ("Revokes other sessions") contract, so all three endpoints are in this phase's scope. **Not included:** Google OAuth (step 5), desktop IPC/keyring wiring (client half — see §6), remaining step-6 hardening.

## 1. Recovery request — `POST /v1/auth/recovery-request {email}` → `202 {status:"accepted", emailDelivery, message}`

- Email validated/normalized exactly like registration (parameterized queries only).
- **Enumeration-safe:** one constant 202 body for every input — existing/unknown, verified/unverified, locked/deleted — never confirms existence, verification, or eligibility. `emailDelivery`/`message` are config-level (the Phase 23C clarification), byte-identical for every account.
- **Token only for eligible accounts** (exists + `active`; locked/deleted get none). 32 `OsRng` bytes → 64 hex, stored **only** as sha256 (never plaintext; never logged; never returned in any API response). Default TTL 60 min (`NEXORA_AUTH_RESET_TTL_SECONDS`, validated), single-use, revocable.
- **Outstanding reset tokens are superseded:** a new request revokes all previously live reset tokens for the account (at most one live reset token).
- **No fabricated delivery:** with no email provider configured the body honestly says no email was sent ("outgoing email is not configured in this build"). The capture-provider tests assert the raw token appears ONLY in the outbound email. The email body carries the plan's deep-link fragment `#/reset-password?token=…`; composing an absolute origin is a future provider/deployment step — **no URL origin is invented** (see §5 for why the desktop is not wired).
- Rate limiting: DB-backed `login_attempts` windows (scope `recovery`, hashed `e:`/`i:` keys, all-attempts mode, 429 + `Retry-After`, no permanent lockout) — the same multi-instance-safe architecture as 23D.

## 2. Reset — `POST /v1/auth/reset-password {token,password}` → `204`

- **Validation before consumption:** token shape (64-hex) and the **existing 8–128 password policy** (now exported from `password.rs` as the single policy source) run first — invalid input never consumes a token.
- **Atomic single-use consume + update in ONE transaction:** consume (`used_at` IS NULL AND `revoked_at` IS NULL AND unexpired — exactly one concurrent winner) → **account row lock (`FOR UPDATE`)** → re-check eligibility under the lock → Argon2id hash via the existing 23B implementation → **revoke EVERY session of the account (all families)** → invalidate all outstanding reset AND verification tokens (plan §4). Unknown/expired/revoked/used tokens all return one generic `invalid_or_expired_token` (400) — no state distinction, no account or database detail.
- **No stale sessions after a password change (concurrency):** `login` now verifies the password and inserts the session **while holding the same account row lock** (23E hardening of 23D `login`/`create_session`, behaviour otherwise unchanged); `reset_password`/`change_password` hold that lock from before the password update until every session is revoked. Any interleaving is safe: a racing login either commits first (reset revokes its session) or verifies against the new password (old credential fails). Refresh/rotation is already safe via its exactly-once revoke check (rotation of a revoked session rolls back its insert).

## 3. Password change — `POST /v1/auth/password-change {currentPassword,newPassword}` → `204` (plan step-4 contract)

Bearer-authenticated (same `invalid_or_expired_token` enforcement as 23D). Existing policy on the new password; wrong current password → generic `invalid_credentials` (401). Keeps the caller's session **family** (plan: "Revokes other sessions" — the caller's device keeps working, refresh included) and revokes every other family; invalidates outstanding reset/verification tokens (plan §4 "invalidation on use/password-change"). Same account-lock discipline as reset.

## 4. Database & configuration

- **Migration `20261018000005_create_password_reset_tokens.sql`** (forward-only; nothing prior touched; documented forward-recovery): plan §5 shape — `id`, `user_id` FK, `token_hash bytea UNIQUE NOT NULL`, `expires_at` (CHECK `> created_at`), `used_at`, `revoked_at` (CHECK used/revoked mutually exclusive), user index — mirroring `email_verification_tokens` (23C) and matching the concurrency model (exactly-once consume via `UPDATE … WHERE used_at IS NULL`).
- New validated envs (values never printed; redacted `Debug`; "(Value withheld.)" errors): `NEXORA_AUTH_RESET_TTL_SECONDS` (60–604800, default 3600 per plan), `NEXORA_AUTH_RATE_LIMIT_RECOVERY` (`5/15m` default per plan). All 23A–23D config behaviour preserved. No new crates, frameworks, or databases — everything reuses sqlx/argon2/sha2/axum already in `Cargo.toml`.
- `login_attempts.scope` already permits `recovery` (23D migration CHECK included it in advance).

## 5. Email & frontend status

Email reuses the existing `EmailProvider` trait (23C): only `UnconfiguredEmailProvider` exists in this build; SMTP vars still fail startup loudly; `emailDelivery:"unconfigured"` means **no email is sent and none is claimed**. The frontend recovery/reset pages (`RecoverPasswordPage.tsx`, `ResetPasswordPage.tsx`, Phase 22) are **byte-unchanged**: integration is **BLOCKED** — there is still no approved secure desktop transport (CSP `connect-src` excludes this service; the service is not auto-started; keyring/IPC is desktop-crate work), and the brief permits integration only where those boundaries support it. The pages keep their honest states ("No reset email has been sent." / "No password has been changed."); nothing claims success that did not happen; local AI/projects/settings remain fully usable without login or internet. (Plan note: the `#/reset-password?token=…` deep-link state machine is part of that blocked client integration.)

## 6. Tests — written vs actually run

| Suite | Written | Ran? | Result |
|---|---|---|---|
| Frontend `pnpm typecheck` / `pnpm test` | (existing) | **YES** | **PASS — 50 files / 569 tests** (app byte-unchanged) |
| `git diff --check` | — | **YES** | **PASS** |
| `recovery.rs` units (6) | YES | **NO** | cargo absent (127) + crates.io blocked (35) |
| `config.rs` new units (2; 21 total) | YES | **NO** | same |
| `tests/recovery_http.rs` (8, no DB) | YES | **NO** | same — intended to run first in CI |
| `tests/recovery_db.rs` (10, **real PostgreSQL**, `#[ignore]`) | YES | **NO** | PostgreSQL absent; `NEXORA_AUTH_TEST_DATABASE_URL=… cargo test --test recovery_db -- --ignored` |
| 23C/23D suites (unchanged; regressions covered in `recovery_db.rs`) | (68) | **NO** | same blocker |
| `cargo fmt` / `cargo check` | — | **NO** | toolchain absent |

Coverage intent: enumeration-safe responses; unknown/verified/ineligible accounts; token entropy + sha256-at-rest; expiry/revocation/single-use; malformed/unknown tokens sharing one generic error; policy enforcement without consuming tokens; atomic consume under concurrency; session revocation after reset; concurrent login-vs-reset staleness; password-change semantics (wrong current, other-sessions revocation, token invalidation); recovery rate windows; registration/login/session regressions.

## 7. Known limitations / follow-ups (existing plan)

- **Compiler-unverified blocker (same as 23B–23D):** `cargo fmt --check`, `cargo check --all-targets`, `cargo test`, then the `--ignored` DB suites on a disposable PostgreSQL must pass before reliance.
- Desktop IPC + keyring + `#/recover-password`/`#/reset-password` deep-link wiring — blocked (§5). Absolute reset-link origin composition belongs to the future provider/deployment step.
- Google OAuth (step 5), log-scrubbing middleware + secret-leakage/fuzz suites and proxy trust (step 6 remainder) — later plan steps, not started.

## 8. Git

Local and uncommitted on `arena/1008499f-nexora`, layered on the preserved Phase 19–23D + multi-file-patch tree. No commit, push, merge, release, visibility change, reset, clean, or history rewrite occurred. The next phase was **not** started.
