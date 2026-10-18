# Phase 23B — Database Foundation & Password Security

**Status:** Implemented in the local working tree; uncommitted. **Every Rust artifact in this phase is UNVERIFIED by any compiler** (see §6) — nothing here has been compiled or executed, and no test is claimed as passing. Schema proposals only: no registration, login, sessions, email, recovery, OAuth, or frontend integration exists.

**Scope:** database foundations (schema + migrations), account data modelling, and Argon2id password hashing/verification, per `docs/phase-23/README.md` (plan) and `docs/phase-23a/README.md` (service scaffold). Out of scope, by instruction: account API endpoints, sessions/JWT/credential manager, email/verification/reset tokens, Google OAuth or account linking logic, frontend calls, production deployment, Phase 23C.

**Checkout:** `arena/1008499f-nexora`, base HEAD `128781618917fd15d2923aeb8eec04e9a5e90e98`. Phases 19–23A work preserved (verified before and after; tree matched the previous report exactly).

## 0. Mandatory audit outcome (Phase 23A verification gate)

| Gate check | Exact result |
|---|---|
| `cargo test` in `services/nexora-auth` | **exit 127 — `cargo: command not found`** |
| Toolchain anywhere on disk (`which`/`find`) | **None** (`rustc`/`rustup` also absent) |
| crates.io / index.crates.io reachability | **Blocked** — `curl` exit 35 (TLS connect failure), `http_code=000` |
| PostgreSQL (`psql`/`pg_ctl`/port 5432) | **Not installed / not running** |
| Pinned toolchain | `rust-toolchain.toml` → `1.93.0` (rustup-managed; cannot be provisioned here) |
| `Cargo.lock` files | **None exist in the repository** (no Rust build has ever run here) |

**Gate verdict:** Phase 23A's source has still **never been compiled or tested**. The gate's missing-toolchain branch applies ("document the exact limitation; do not install toolchains") — no toolchain was installed and no pinned version changed. The stop-condition ("compilation failures that cannot be safely resolved") is not determinable without a compiler; consistent with this brief's own degraded-environment instructions ("Without PostgreSQL, report database integration tests as not run", "Run Rust formatting, compilation, and tests **when the toolchain is available**", "write migrations and tests that can be reviewed without pretending"), the phase deliverables below were written as reviewable artifacts. **Hard blocker carried forward: Phase 23C must not start until a Rust-capable environment (toolchain 1.93.0 + crates.io access) actually compiles and tests Phases 23A+23B.**

## 1. New Rust modules and dependencies

| Artifact | Purpose |
|---|---|
| `services/nexora-auth/src/password.rs` | Argon2id hashing/verification, policy constants, 10 unit tests (written, not run) |
| `services/nexora-auth/src/config.rs` (extended) | `NEXORA_AUTH_DATABASE_URL` validation (values never echoed), redacting `Debug` for `AuthConfig`, 3 new unit tests; Phase 23A API/tests untouched |
| `services/nexora-auth/migrations/20261018000001_create_users.sql` | `users` table |
| `services/nexora-auth/migrations/20261018000002_create_oauth_accounts.sql` | `oauth_accounts` linking table (schema only) |
| `services/nexora-auth/tests/db_migrations.rs` | 3 REAL PostgreSQL integration tests (`#[ignore]`d) |

New dependencies (justified, minimal):

| Crate | Where | Why |
|---|---|---|
| `argon2 = "0.5"` | dependencies | The maintained Argon2id implementation (RustCrypto `password-hashes`); PHC encoded format; no custom crypto. Used only by `password.rs`. |
| `sqlx = { version = "0.8", default-features = false, features = ["postgres", "runtime-tokio", "migrate"] }` | **dev-dependencies** | Driver for the real-database integration tests and the chosen migration convention (`sqlx::migrate!`). Runtime queries only — no compile-time DB access. TLS features deliberately omitted (loopback test DB, `sslmode=disable`). Nothing in the service binary uses it yet. |

No other dependency changed; `apps/desktop/src-tauri/Cargo.toml` and all frontend manifests/lockfiles are untouched. Since no Rust build has ever run, there is still no `Cargo.lock`; the first successful `cargo build` in a toolchain environment will generate `services/nexora-auth/Cargo.lock`, which should then be committed (plan §4 supply-chain control).

## 2. Database schema and migration sequence

SQLx convention: `migrations/<14-digit-timestamp>_<name>.sql`, applied in lexicographic order, recorded in `_sqlx_migrations`. **Order:** `20261018000001_create_users` → `20261018000002_create_oauth_accounts` (FK). **Reversal strategy:** forward-recovery — SQLx has no down step; each file documents its reversal statement (`DROP TABLE …`) to be shipped as a NEW migration if ever needed.

**`users`** — `id uuid PK DEFAULT gen_random_uuid()` (PostgreSQL 13+), `display_name text` (non-blank, ≤128), `email text` (**normalized** trim+lowercase enforced by `users_email_normalized_ck`, uniqueness by `users_email_key UNIQUE`), `password_hash text NULL` (Argon2id **PHC string only — never plaintext**; NULL = future OAuth-only account), `email_verified_at timestamptz NULL`, `status text` (`active|locked|deleted` via `users_status_ck`), `created_at`/`updated_at timestamptz`. **Indexes:** only the UNIQUE(email) implicit index — every planned lookup is by PK or email; future tables bring their own justified indexes.

**`oauth_accounts`** — `id uuid PK`, `user_id uuid FK → users ON DELETE CASCADE`, `provider text`, `provider_subject text` (immutable provider `sub`), `provider_email text NULL`, `created_at`; `UNIQUE (provider, provider_subject)`. OAuth identities are kept **separate from password credentials**; provider access tokens are **not stored**. (Linking logic remains out of scope; this is modelling only.)

**How applied/tested:** `sqlx migrate run --source services/nexora-auth/migrations` against a dedicated dev database, or `sqlx::migrate!("./migrations").run(&pool)` in service/tests later; integration tests run the migrator themselves. **Never against production from a dev machine.** No database was created, contacted, or migrated in this phase.

## 3. Password-hashing algorithm and parameters

`password.rs` is the single policy point, independent of HTTP/frontend:

- **Argon2id**, version 0x13, via `argon2` 0.5 (`Algorithm::Argon2id`, `Version::V0x13`).
- **Parameters (explicit, OWASP baseline):** `m = 19,456 KiB`, `t = 2`, `p = 1`, output `32 bytes` (`Params::new(19_456, 2, 1, Some(32))`).
- **Salt:** fresh 16-byte `SaltString::generate(&mut OsRng)` per hash — identical passwords always yield distinct encoded hashes.
- **Encoding:** the library's PHC string (`$argon2id$v=19$m=19456,t=2,p=1$…`) carrying algorithm, parameters, and salt.
- **Policy:** new passwords 8–128 characters (matches `apps/desktop/ui/src/lib/formValidation.ts`); the upper bound caps hashing cost (DoS).
- **Verification:** `PasswordHash::new` + `verify_password`; **one generic error** (`Password verification failed.`) for wrong password, malformed/foreign encoded hashes, and oversized candidates alike. Errors and `Debug` output never contain input material. No custom crypto, no reversible encryption, no MD5/SHA-1/unsalted/fast hashes, no hardcoded credentials.

## 4. Configuration requirements

New: `NEXORA_AUTH_DATABASE_URL` — optional in this phase (nothing connects). If set: must start with `postgres://` or `postgresql://` and include a host; errors name the variable and expected shape but **never echo the value** (connection strings embed credentials); `AuthConfig`'s `Debug` impl renders `<redacted>`. Existing `NEXORA_AUTH_BIND` / `NEXORA_AUTH_ALLOW_NON_LOOPBACK` behaviour is unchanged (loopback default preserved). No `.env` files; no hardcoded URLs/credentials; docs use placeholders (`postgres://user@127.0.0.1:5432/nexora_auth_test`). Tauri CSP/capabilities/frontend untouched; no new listeners or auto-start.

**App config vs secret storage:** these environment variables are *application configuration* (shape/limits). Actual credential values belong to the deployment environment's secret store (Phase 23 plan §7) — never to the repository, logs, or chat.

## 5. Commands (unit + integration tests)

```bash
cd services/nexora-auth
cargo fmt --check                    # formatting
cargo check --all-targets            # compilation
cargo test                           # unit + router tests (db tests are skipped: #[ignore])
# Real database tests — ONLY against an isolated throwaway DB:
createdb nexora_auth_test
NEXORA_AUTH_TEST_DATABASE_URL='postgres://user@127.0.0.1:5432/nexora_auth_test?sslmode=disable' \
  cargo test -- --ignored
```

Frontend regression: `pnpm typecheck`, `pnpm test`, `git diff --check` at the repo root.

## 6. Which tests genuinely ran, and which could not

| Suite | Written | Ran? | Result |
|---|---|---|---|
| Frontend (`pnpm typecheck`, `pnpm test`) | (existing 50 files / 558 tests) | **YES** | **PASS — 50 files / 558 tests** (app untouched) |
| `git diff --check` | — | **YES** | **PASS** |
| `password.rs` unit tests (10) | YES | **NO** | `cargo` absent (exit 127) + crates.io TLS-blocked (exit 35) — **not run, not passing** |
| config unit tests (existing 9 + new 3) | YES | **NO** | same |
| HTTP-surface/bind tests (Phase 23A, 9) | YES | **NO** | same — still never compiled |
| `tests/db_migrations.rs` (3, real PostgreSQL) | YES | **NO** | PostgreSQL absent; `#[ignore]`d by design — **no mock substitute is offered or labelled as database verification** |
| `cargo fmt` / `cargo check` | — | **NO** | toolchain absent |

Nothing above is a pass except the two YES rows.

## 7. Known limitations and security review items

- **Compiler-unverified (blocker).** Phases 23A+23B must be compiled (`cargo check --all-targets`), linted, and tested (toolchain 1.93.0 + crates.io) before any further Rust work. Minor API-drift fixes (e.g. `argon2` constructor signatures) may surface; they must preserve the security behaviour documented in §3.
- No live database was used; migration SQL is review-only until the `--ignored` suite runs against a throwaway DB.
- `updated_at` has no trigger yet (writer-side maintenance comes with account flows); rate limiting/token tables are later migrations; argon2 parameters should be re-reviewed against current OWASP guidance at implementation time.
- Do not log passwords/PHC strings: future request logging must scrub `password`, `password_hash`, `token`, `authorization` (plan §4).
- Reviewers should confirm: no plaintext anywhere in schema; `password_hash` nullable-by-design; error paths never echo secrets.

## 8. Why no authentication endpoints

Same honesty rule as Phases 20–23A: hashing a password or creating a table is not authentication. Account endpoints (register/login/reset) arrive only with the flows that make them true — Phase 23C+ per the plan's sequence. Adding endpoints now would create surface that pretends to authenticate.

## 9. Later account flows without blocking offline AI

Future account APIs live entirely in `services/nexora-auth`; the desktop app will reach them through Rust-side IPC commands in a later approved phase, never via the WebView (CSP already forbids WebView HTTP) and never by auto-starting this service. Local AI (llama.cpp, `inference/`, local JSON stores) has zero dependencies on this crate; plan §2.6's offline guarantees remain: no auth gate on models, projects, or settings.

## 10. Corrections to prior documents

Per instruction ("update … only if a necessary correction is found"), two prior claims were corrected **in place with `[Correction (Phase 23B)]` markers** (text otherwise preserved): `docs/phase-23/README.md` (×2) and `docs/phase-23a/README.md` (×1) referred to an existing `Cargo.lock`; none exists in the repository (no Rust build has run here). No other prior content changed.

## 11. Git

All work is local and uncommitted on `arena/1008499f-nexora`, layered on the preserved Phase 19–23A tree. No commit, push, PR, release, visibility change, branch switch, reset, clean, or history rewrite occurred. Phase 23C was **not** started.
