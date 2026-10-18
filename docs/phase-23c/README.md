# Phase 23C — Registration & Email Verification

**Status:** Implemented in the local working tree; uncommitted. **No Rust code in this phase has been compiled or executed** (see §5) — the environment has no Rust toolchain, no crates.io access, and no PostgreSQL. Artifacts are written to be reviewable and are consistent with the Phase 23A/23B conventions; they are **not** verified by any compiler or test run. Phase 23D+ was not started.

**Scope (plan sequence step 2, `docs/phase-23/README.md` §9):** `POST /v1/auth/register`, `POST /v1/auth/verify-email`, `POST /v1/auth/resend-verification` with real account storage and a genuine (honest) email-delivery abstraction. **Explicitly not included** (later steps per the plan): login/sessions/logout/refresh, password recovery/reset, Google OAuth, rate limiting (sequence step 6), desktop IPC wiring (step 3, separately gated). Frontend stays in its existing honest disconnected state (§4).

## 1. Endpoints (plan §6 contracts)

| Endpoint | Request | Success | Notes |
|---|---|---|---|
| `POST /v1/auth/register` | `{name, email, password}` (strict, `deny_unknown_fields`) | **202** `{status:"verification_pending", emailDelivery:"configured"\|"unconfigured", message}` | Enumeration-safe: **byte-identical** response for new and duplicate emails (plan §6). Name 1–128 trimmed; email trimmed+lowercased, ≤254, `local@domain.tld` shape (matches `formValidation.ts` and the DB CHECK); password policy 8–128 via Argon2id module. |
| `POST /v1/auth/verify-email` | `{token}` | **200** `{status:"verified"}` | Token = 64 hex chars (32 bytes). Missing/empty → 400 `validation_failed`; malformed/expired/used/revoked → **one generic** 400 `invalid_or_expired_token` (no oracle). |
| `POST /v1/auth/resend-verification` | `{email}` | **202** `{emailDelivery, message}` | Safe for unknown/verified addresses (nothing happens, identical response). Revokes the account's earlier live tokens, issues a fresh one. |

Errors: `{error:{code,message}}` with stable codes `validation_failed` (400), `invalid_or_expired_token` (400), `dependency_unavailable` (503, honest "not configured" state), `internal` (500), plus the 23A `not_found`/`method_not_allowed` envelopes. Bodies never echo inputs, passwords, tokens, hashes, connection strings, or provider credentials. The 4 KiB body limit and loopback-default binding (23A) are preserved.

**Registration semantics:** validate → Argon2id hash (never logged/stored in plaintext) → `INSERT INTO users` (parameterized) → on UNIQUE violation (`23505`) return the identical generic outcome with **no** token/email side effects (concurrency-safe: 8 parallel registrations of one address create exactly 1 row — tested). New accounts get a verification token; delivery is attempted via the provider trait.

## 2. Verification tokens (plan §4/§5)

- **32 cryptographically secure random bytes** (`OsRng`), hex-encoded — the raw token exists only inside the email body.
- Database stores **only `sha256(raw)`** (`bytea`, UNIQUE) — never the raw token.
- Row: `email_verification_tokens` (migration `20261018000003`): `user_id` FK (cascade), `token_hash bytea UNIQUE`, `expires_at` (default TTL 86400 s, plan §7), `used_at` (consumed), `revoked_at` (invalidated), `created_at`; CHECKs: consumed XOR revoked, `expires_at > created_at`. Index on `user_id` (resend/cascade).
- **Single-use + expiry + revocation enforced in SQL** (`UPDATE … WHERE used_at IS NULL AND revoked_at IS NULL AND expires_at > now() RETURNING user_id::text`) inside a transaction that then sets `users.email_verified_at` — concurrent verifications race on one atomic UPDATE (tested). Resend revokes prior live tokens; a superseded token fails with the same generic error.

## 3. Email delivery abstraction

`src/email.rs`: `EmailProvider` trait (object-safe, boxed futures — no extra deps) + `EmailMessage`. The **only shipped implementation is `UnconfiguredEmailProvider`**, which returns `NotConfigured` — the API reports `emailDelivery: "unconfigured"` with the explicit message *"Email delivery is not configured on this server; no verification email was sent."* **No email is ever claimed to be delivered when it is not**; accounts are never auto-verified; no fake verification links exist. The configured-capability response text uses conditional phrasing (*"If this address can be registered, a verification email is on its way."*) so duplicates stay non-disclosing. A future SMTP/provider-SDK transport (plan §7 `EMAIL_PROVIDER`/`SMTP_*`/`EMAIL_FROM`) plugs into the same trait; setting any `NEXORA_AUTH_SMTP_*` variable today is a **startup error** (silently ignoring credentials would fake delivery). Transport failures never leak into responses; accounts remain unverified and recoverable via resend.

**Config (all validated at startup, values never printed; redacting `Debug`):** `NEXORA_AUTH_VERIFICATION_TTL_SECONDS` (300–604800, default 86400), `NEXORA_AUTH_EMAIL_PROVIDER` (only `unconfigured`), `NEXORA_AUTH_EMAIL_FROM` (shape-checked, redacted), `NEXORA_AUTH_SMTP_*` (reserved → rejected), plus the existing `NEXORA_AUTH_DATABASE_URL` (now actually used: parameterized `sqlx` queries only). App config vs secrets: values belong to the deployment environment/secret store — no `.env` files, no example credentials (synthetic `example.test` fixtures only).

## 4. Frontend integration decision (§6 of the brief)

**No frontend code changed.** The brief allows integration "only where a real, secure transport can be supported by the existing architecture"; none exists: the WebView CSP (`connect-src ipc: http://ipc.localhost`) forbids calling the service, the service never auto-starts with Nexora, and desktop IPC wiring is the plan's step 3 (separately approved). The existing pages already tell the truth (`"Account creation is not connected yet. Your details have not been submitted."`, `"No email has been sent."`) and must stay that way until a confirmed backend response can back any success state. Local AI, local projects, and local settings remain fully usable without login or internet.

## 5. Tests — written vs actually run

| Suite | Written | Ran? | Result |
|---|---|---|---|
| Frontend `pnpm typecheck` / `pnpm test` | (existing) | **YES** | **PASS — 50 files / 569 tests** (app untouched) |
| `git diff --check` | — | **YES** | **PASS** |
| `registration.rs` unit tests (10) | YES | **NO** | `cargo` absent (exit 127) + crates.io TLS-blocked (exit 35) |
| `email.rs` unit tests (2) | YES | **NO** | same |
| `config.rs` new tests (4; 15 prior untouched) | YES | **NO** | same |
| `tests/registration_http.rs` (8, no DB needed) | YES | **NO** | same — intended to run first in CI |
| `tests/registration_db.rs` (9, **real PostgreSQL**, `#[ignore]`) | YES | **NO** | PostgreSQL absent; run only against a throwaway DB via `NEXORA_AUTH_TEST_DATABASE_URL=… cargo test --test registration_db -- --ignored` |
| `tests/db_migrations.rs` (3 + table checks) | YES | **NO** | same; **fixed** the `to_regclass(…)::text` decode defect found in the verification gate |
| `cargo fmt` / `cargo check` | — | **NO** | toolchain absent |

Coverage intent: registration validation + hashing, duplicate/concurrent registration (exactly one row, identical outcomes), raw password/token never in the DB (capture-provider asserts PHC + sha256 digests only), verification happy path, single-use under concurrency (8 racers → 1 win), expiry, resend invalidation, provider failure (no fake delivery, account unverified), unconfigured-provider honesty, JSON error envelopes, no input echoing, health preserved.

## 6. Known limitations / follow-ups (existing plan, not new scope)

- **Compiler-unverified blocker (same as 23B):** a toolchain run (`cargo fmt --check`, `cargo check --all-targets`, `cargo test` in `services/nexora-auth`, then the `--ignored` DB suites on a disposable PostgreSQL) must pass before this is relied upon. Minor API-drift fixes may surface; they must preserve the security behaviour documented here.
- Rate limiting (`AUTH_RATE_LIMIT_*`) is plan sequence step 6 — not implemented (documented, not silently skipped).
- SMTP/real-provider transport is not implemented; the trait + config guard make the gap explicit and honest.
- The verification email delivers the code as text (no app deep link) until a link origin is configured for a deployment.
- Duplicate registrations don't send "account exists" mail (deliberate: no enumeration side channels).
- Desktop wiring (plan step 3) remains a separately approved phase.

## 7. Git

Everything is local and uncommitted on `arena/1008499f-nexora`, layered on the preserved Phase 19–23B + multi-file-patch tree. No commit, push, merge, release, visibility change, reset, clean, or history rewrite occurred. Phase 23D was **not** started.
