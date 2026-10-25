# Phase 13 — Database and Migrations Audit

**Phase 13 status as of 2026-10-23: implementation changes were in the worktree; acceptance remained BLOCKED pending Rust and disposable-PostgreSQL validation.** No database was contacted or migrated, and no production data state was inferred. Current Phase 16 follow-up is recorded at the end; it remains unvalidated.

## Schema and contracts reviewed

The repository has eight ordered SQLx migrations (`20261018000001`–`20261018000006`, `20261022000007`, and the new forward-only `20261023000008`). Together they define 16 tables: `users`, `oauth_accounts`, `email_verification_tokens`, `sessions`, `login_attempts`, `password_reset_tokens`, `google_oauth_states`, `plans`, `plan_versions`, `plan_model_quotas`, `subscriptions`, `payment_orders`, `webhook_events`, `daily_usage_counters`, `quota_reservations`, and `audit_logs`.

Key contracts confirmed in the SQL and runtime queries:

- Normalized unique user email; provider identity uniqueness on `(provider, provider_subject)`; Google sign-in/linking is keyed by immutable subject, not email.
- Session and verification/reset token writers store SHA-256 digests only. The new migration adds validated 32-byte `bytea` checks for session, verification, reset, and Google OAuth state hashes. It does not rewrite or delete rows. SQLx runs this migration transactionally; a pre-existing invalid digest makes it fail and roll back rather than silently changing data. The read-only preflight below and any table-lock/duration assessment remain unrun.
- Verification/reset consumption, session rotation, Google identity/session creation, signed Stripe webhook processing, and quota reservation/settlement use transactions for their related state changes. User/token ownership and session creation share account-row locking in the relevant paths.
- Existing foreign keys and indexes support identity/hash lookups, per-user token/session work, login-window counts, Google-state pruning, plan/subscription reads, recent orders, quota idempotency/sweeps, and audit chronology. The existing Stripe schema and payment behavior are unchanged.

Read-only preflight for the new digest constraints (not run here):

```sql
SELECT 'email_verification_tokens' AS table_name, count(*) AS invalid_rows
FROM email_verification_tokens WHERE octet_length(token_hash) <> 32
UNION ALL
SELECT 'password_reset_tokens', count(*)
FROM password_reset_tokens WHERE octet_length(token_hash) <> 32
UNION ALL
SELECT 'sessions', count(*)
FROM sessions WHERE octet_length(token_hash) <> 32
UNION ALL
SELECT 'google_oauth_states', count(*)
FROM google_oauth_states WHERE octet_length(state_hash) <> 32;
```

## Migration and SQLx findings

- SQLx is locked at 0.8.6. Runtime SQL remains dynamically checked; the `postgres`, `runtime-tokio`, `migrate`, and `macros` features support the embedded `sqlx::migrate!` runner. `tls-rustls-ring-webpki` enables the production PostgreSQL TLS mode documented by the deployment guide; no TLS handshake was tested here. A small `build.rs` watches the migration directory so additions/removals trigger recompilation. Embedding removes the previous working-directory search, which could select an unrelated `migrations/` folder, and avoids requiring migration files beside a deployed binary.
- SQLx records migration versions/checksums in `_sqlx_migrations`; startup checks connectivity, runs migrations, syncs the configured plan catalog, then binds the listener. CI provisions PostgreSQL 16 and uses a dedicated `nexora_test` database with `sslmode=disable` only for that local ephemeral service; production TLS configuration is separate.
- The DB integration-test helpers now verify `current_database()` ends in `_test` before applying migrations or truncating fixtures. The migration test asserts the exact eight-version sequence and reruns the migrator to exercise checksum/idempotence behavior. New DB coverage rejects short digests and accepts 32-byte values.
- Existing migrations 5–7 use `IF NOT EXISTS` for some objects. That avoids duplicate-object errors but does not reconcile a manually pre-existing table with a different shape. Migration checksums and actual applied versions/checksums in any external database cannot be verified from this checkout; available Git history is shallow/grafted, and no database was queried.

## Findings intentionally left unchanged

- `payment_orders.provider_payment_intent_id` is filtered by refund-webhook queries but has no dedicated index in the current schema. No `EXPLAIN` plan could be collected without PostgreSQL, and no Stripe schema/index change was made under the instruction to preserve Stripe files, schema, records, and behavior.
- Google OAuth states are pruned inline and stale quota reservations are marked expired during quota operations. No general retention job was found for expired sessions, verification/reset tokens, login-attempt rows, daily usage counters, settled quota reservations, or billing/webhook history. No data purge or expiry policy was introduced; account deletion's billing cascades are noted below.
- Deleting a user cascades to auth records, subscriptions, payment orders, daily usage counters, and quota reservations; `audit_logs.actor_user_id` becomes `NULL`. The deletion event also stores the user UUID in the non-FK `audit_logs.subject_id` text column, so that identifier remains after deletion. The route writes its best-effort audit event before the separate user `DELETE`; the preceding `SELECT ... FOR UPDATE` runs through the pool rather than a transaction, so its row lock does not span both writes, and an audit event may remain if deletion later fails. Stripe payment/order history is among the cascaded rows. No behavior or retention policy was changed; review account-deletion atomicity, identifier retention, and billing-retention requirements before any operation that deletes real accounts.
- At the time of this Phase 13 audit, no Razorpay work or Phase 14+ implementation had started. Subsequent Phase 14/15 work exists in the worktree; Phase 15 Razorpay remains blocked and unimplemented. Phase 16 does not modify billing records or migration history.

## Validation status

| Check | Result | Notes |
|---|---|---|
| Worktree preservation and `git diff --check` | **PASS** | Branch and pre-existing changes are preserved; no migration file from the original seven was edited. |
| Rust formatting, compile, Clippy, unit/HTTP tests | **BLOCKED** | Rust 1.93.0 toolchain is pinned but `cargo`, `rustc`, and `rustfmt` are unavailable in this environment. |
| PostgreSQL migration, upgrade, and auth/billing integration tests | **NOT RUN / BLOCKED** | `NEXORA_AUTH_TEST_DATABASE_URL` is unset; PostgreSQL, Docker, and Podman tools are unavailable. No test or production database was contacted. |
| External `_sqlx_migrations` contents / production data preflight | **NOT RUN** | Applied migration state and row compatibility remain unknown. |

Commands for a later authorized disposable-DB run (use a database whose name ends in `_test`; do not use production data):

```bash
cargo fmt --manifest-path services/nexora-auth/Cargo.toml --all -- --check
cargo clippy --manifest-path services/nexora-auth/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path services/nexora-auth/Cargo.toml --locked
NEXORA_AUTH_TEST_DATABASE_URL="$DISPOSABLE_TEST_DATABASE_URL" cargo test --manifest-path services/nexora-auth/Cargo.toml --test db_migrations -- --ignored --test-threads=1
NEXORA_AUTH_TEST_DATABASE_URL="$DISPOSABLE_TEST_DATABASE_URL" cargo test --manifest-path services/nexora-auth/Cargo.toml -- --include-ignored --test-threads=1
```

Phase 10 authentication Rust/backend checks and Phase 11 Google OAuth Rust/backend/live-sign-in remain unverified. Phase 12 desktop UI checks previously passed (88 files, 865 tests, typecheck, and build), but its PostgreSQL profile test did not run; these do not verify backend-dependent phases.

> The existing `docs/phase-13/README.md` describes a different patch-generation roadmap item and was left untouched. This audit is in a separate file to avoid overwriting that existing documentation.

## Phase 16 Follow-up (2026-10-24)

- Phase 16 added no migration and did not edit migrations 1–8, the Stripe tables, or billing records. The quota service defaults to `QuotaPolicy::unconfigured`; absent policy returns no plan, quota, or reset values, and reservation attempts return typed `quota_policy_unconfigured` (`503`). The plans endpoint suppresses catalogue rows while policy is unconfigured so stored/default plan terms are not presented as approved. `main.rs` still syncs the repository's draft catalogue defaults into the DB, and Stripe checkout/subscription handlers remain in source; an unconfigured quota policy is not a billing kill switch, so provider/payment configuration must remain disabled until billing verification and entitlement policy are approved.
- Quota reservation/settlement code now sweeps expired reservations before settlement, persists expiry counter adjustments, rejects expired commits, enforces same-outcome idempotency and opposite-outcome conflicts, rejects settled-request replay, and checks counter update counts. `tests/plans_billing_quota_db.rs` contains new unconfigured-policy, UTC summary, expiry, replay, and idempotency assertions. These are code/test changes only; they are not database-validated.
- Desktop validation passed: `corepack pnpm --filter @nexora/desktop typecheck`; `corepack pnpm --filter @nexora/desktop test` (88 files / 870 tests); `corepack pnpm --filter @nexora/desktop build` (success, with non-fatal Vite warnings). The initial `corepack pnpm check` wrapper attempt failed because the script invokes nested `pnpm`, which was not on `PATH`; after a frozen-lockfile install, the constituent UI checks were run directly and passed.
- An auxiliary `@scalar/rust-fmt` WASM check (edition/style 2021) examined 34 modified/untracked Rust files and reported formatting differences in 17. Broad reformatting was not applied because many files include preserved Phases 09–15 work and the pinned native Rust formatter is unavailable; this is not a clean Rust formatting pass.
- No PostgreSQL connection was attempted. `cargo`, `rustc`, `rustfmt`, `clippy-driver`, `psql`, `postgres`, `initdb`, Docker, and Podman are unavailable; `NEXORA_AUTH_TEST_DATABASE_URL` is unset. Rust formatting, compile, Clippy, Rust tests, migration tests, and isolated-PostgreSQL integration tests remain blocked. The exact command list above remains the deferred disposable-database procedure.
- The Phase 10–13 backend/auth/OAuth validation backlog remains open. Phase 15 Razorpay remains blocked and unimplemented; existing Stripe-oriented billing remains audit-only, with the unverified client-selected plan-change path unresolved. No plan, price, quota, trial, entitlement, or feature policy is approved by this follow-up.

## Step 1 G-01 migration follow-up (2026-10-24)

Step 1 adds migration `20261024000009_subscription_provider_event_ordering.sql`, an additive nullable provider-event timestamp on `subscriptions` plus signed webhook event-time, provider payment/subscription identity, subscription status, and full-refund evidence columns/indexes. Existing rows and migration files were not rewritten, deleted, or migrated during this task. The focused migration-sequence test now expects nine versions. Rust and PostgreSQL validation remain blocked; the new migration has not been applied to any database. The Phase 13 and Phase 16 observations above remain historical snapshots, not evidence for this follow-up.

## Step 4 Account-Deletion Atomicity Follow-up (2026-10-25)

Source review confirmed the earlier account-deletion concern: the `SELECT ... FOR UPDATE` was run directly through `PgPool`, so its statement-scoped transaction did not retain the row lock through credential verification and deletion; the best-effort success audit was also written before the `DELETE` outcome was known. The code now holds the user row lock in one SQLx transaction, validates confirmation/password while that lock is held, inserts the sanitized success audit record in that transaction, deletes the user and commits both together, and emits the success log only after commit. Failed validation or a failed delete therefore cannot produce a committed `account_deleted` success record. The existing deletion cascades and retained audit `subject_id` behavior are unchanged; no migration, retention rule, or existing record was rewritten.

A focused assertion was added to the ignored `account_profile_db` PostgreSQL test for anonymous rejection, mismatched-email no-op, successful deletion, the committed audit row, and session invalidation. The test helper first verifies the connected database name ends in `_test` before applying migrations or writing fixtures. **The test is authored but not executed**: Cargo/Rust/PostgreSQL tooling and `NEXORA_AUTH_TEST_DATABASE_URL` are unavailable. No database was contacted. This is a source correction with blocked runtime validation, not verified deletion/retention behavior; run it only against a reviewed disposable `_test` database and obtain legal review of deletion/retention semantics before public release.

## Step 5 Billing Webhook Input/Replay Follow-up (2026-10-25)

A final source review found two fail-closed hardening opportunities. First, Stripe signature tolerance used signed subtraction plus `.abs()` on the timestamp parsed from an unauthenticated header. Extreme `i64` timestamps could overflow before the HMAC comparison. The verifier now uses unsigned `i64::abs_diff`, rejects negative tolerance, and has a unit regression for `i64::MIN`, `i64::MAX`, and negative tolerance. Second, webhook rows already stored a SHA-256 of the signed raw payload, but an existing `(provider, provider_event_id)` always returned `deduplicated: true` without comparing the digest. Duplicate IDs now receive the normal acknowledgement only when the payload digest matches; a different signed body for the same event ID returns a conflict without applying state changes. A database regression was added alongside the exact-duplicate coverage.

These are source changes and authored tests only. The Rust code was accepted by an auxiliary Rust 2021 WASM parser, but Cargo, Clippy, native Rust tests, PostgreSQL, and the disposable `NEXORA_AUTH_TEST_DATABASE_URL` are unavailable; neither new regression ran, and no database was contacted. No migration changed. Production composition continues to use `UnconfiguredPaymentProvider` and unconfigured quota policy; these changes do not enable checkout, subscriptions, webhook secrets, or paid entitlements. Validate the focused unit and `plans_billing_quota_db` tests in the authorized CI/disposable `_test` environment before treating the fix as runtime-verified.
