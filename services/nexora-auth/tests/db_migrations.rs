//! PostgreSQL integration tests for the Phase 23B schema (REAL database —
//! no mocks, no substitutes).
//!
//! These tests are `#[ignore]`d by default so `cargo test` never pretends a
//! database was exercised. Run them only against an explicitly isolated,
//! throwaway development database:
//!
//! ```bash
//! createdb nexora_auth_test            # local throwaway DB, trust auth
//! NEXORA_AUTH_TEST_DATABASE_URL='postgres://user@127.0.0.1:5432/nexora_auth_test?sslmode=disable' \
//!     cargo test -- --ignored
//! ```
//!
//! Rules: use only a disposable database whose name ends in `_test` (checked
//! before migrations), never use production data or credentials (fixtures are
//! synthetic `example.test` addresses), and never print the connection string.
//! Without PostgreSQL these tests are reported as "not run" — see
//! `docs/database-migrations-audit.md`.

use std::sync::atomic::{AtomicUsize, Ordering};

use sqlx::postgres::PgPoolOptions;
use sqlx::Row;

mod support;

static UNIQUE: AtomicUsize = AtomicUsize::new(0);

/// Synthetic, collision-resistant fixture email — never a real address.
fn unique_email(tag: &str) -> String {
    let n = UNIQUE.fetch_add(1, Ordering::SeqCst);
    let pid = std::process::id();
    format!("{tag}.{pid}.{n}@example.test")
}

async fn test_pool() -> sqlx::PgPool {
    let url = std::env::var("NEXORA_AUTH_TEST_DATABASE_URL").expect(
        "NEXORA_AUTH_TEST_DATABASE_URL must name an isolated throwaway PostgreSQL database",
    );
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&url)
        .await
        .expect("could not connect to the test database (is PostgreSQL running?)");
    support::assert_test_database(&pool).await;
    nexora_auth::run_migrations(&pool)
        .await
        .expect("migrations must apply cleanly");
    pool
}

#[tokio::test]
#[ignore = "requires an isolated PostgreSQL test database (NEXORA_AUTH_TEST_DATABASE_URL)"]
async fn migrations_apply_in_order_and_are_recorded() {
    let pool = test_pool().await;

    let versions: Vec<i64> = sqlx::query_scalar(
        "SELECT version FROM _sqlx_migrations WHERE success ORDER BY version",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        versions,
        vec![
            20261018000001,
            20261018000002,
            20261018000003,
            20261018000004,
            20261018000005,
            20261018000006,
            20261022000007,
            20261023000008,
            20261024000009,
        ],
        "the complete migration sequence must be recorded exactly once and in order"
    );

    // A second run validates stored checksums and must not add duplicate
    // history rows or rerun already-applied migrations.
    nexora_auth::run_migrations(&pool)
        .await
        .expect("re-running the migrator must be idempotent");
    let versions_after_rerun: Vec<i64> = sqlx::query_scalar(
        "SELECT version FROM _sqlx_migrations WHERE success ORDER BY version",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(versions_after_rerun, versions);

    let users = sqlx::query("SELECT to_regclass('users')::text AS name")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(users.get::<Option<String>, _>("name").is_some());

    let oauth = sqlx::query("SELECT to_regclass('oauth_accounts')::text AS name")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(oauth.get::<Option<String>, _>("name").is_some());

    let tokens = sqlx::query("SELECT to_regclass('email_verification_tokens')::text AS name")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(tokens.get::<Option<String>, _>("name").is_some());

    let subscription_watermark: bool = sqlx::query_scalar(
        "SELECT EXISTS (
             SELECT 1 FROM information_schema.columns
             WHERE table_schema = current_schema()
               AND table_name = 'subscriptions'
               AND column_name = 'last_provider_event_created_at'
         )",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(subscription_watermark);

    let webhook_ordering_columns: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM information_schema.columns
         WHERE table_schema = current_schema()
           AND table_name = 'webhook_events'
           AND column_name IN (
               'provider_event_created_at',
               'provider_payment_intent_id',
               'provider_subscription_id',
               'provider_subscription_status',
               'is_full_refund'
           )",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(webhook_ordering_columns, 5);
}

#[tokio::test]
#[ignore = "requires an isolated PostgreSQL test database (NEXORA_AUTH_TEST_DATABASE_URL)"]
async fn users_constraints_hold() {
    let pool = test_pool().await;

    // Valid row with a synthetic (clearly fake) PHC-shaped hash string.
    let email = unique_email("with-hash");
    sqlx::query("INSERT INTO users (display_name, email, password_hash) VALUES ($1, $2, $3)")
        .bind("Test Person")
        .bind(&email)
        .bind("$argon2id$v=19$m=19456,t=2,p=1$c2FsdHNhbHQ$kY7sRplaceholderfixtureonly")
        .execute(&pool)
        .await
        .expect("valid insert must succeed");

    // OAuth-only shape: password_hash NULL is allowed.
    sqlx::query("INSERT INTO users (display_name, email) VALUES ($1, $2)")
        .bind("OAuth Only")
        .bind(unique_email("oauth-only"))
        .execute(&pool)
        .await
        .expect("passwordless (future OAuth-only) insert must succeed");

    // Normalized-email uniqueness is enforced by the database.
    let duplicate = sqlx::query("INSERT INTO users (display_name, email) VALUES ($1, $2)")
        .bind("Duplicate")
        .bind(&email)
        .execute(&pool)
        .await;
    assert!(duplicate.is_err(), "duplicate email must be rejected");

    // Normalization invariant: mixed-case email violates users_email_normalized_ck.
    let unnormalized = sqlx::query("INSERT INTO users (display_name, email) VALUES ($1, $2)")
        .bind("Case Bad")
        .bind("Ada@Example.Test")
        .execute(&pool)
        .await;
    assert!(
        unnormalized.is_err(),
        "non-normalized email must be rejected"
    );

    // Empty display name rejected.
    let empty_name = sqlx::query("INSERT INTO users (display_name, email) VALUES ($1, $2)")
        .bind("   ")
        .bind(unique_email("empty-name"))
        .execute(&pool)
        .await;
    assert!(empty_name.is_err(), "blank display name must be rejected");

    // Unknown status rejected.
    let bad_status =
        sqlx::query("INSERT INTO users (display_name, email, status) VALUES ($1, $2, $3)")
            .bind("Status Bad")
            .bind(unique_email("bad-status"))
            .bind("admin")
            .execute(&pool)
            .await;
    assert!(bad_status.is_err(), "unknown status must be rejected");
}

#[tokio::test]
#[ignore = "requires an isolated PostgreSQL test database (NEXORA_AUTH_TEST_DATABASE_URL)"]
async fn oauth_accounts_constraints_hold() {
    let pool = test_pool().await;

    let owner_email = unique_email("oauth-owner");
    // uuid <-> text casts keep the test free of extra crate dependencies.
    let owner = sqlx::query(
        "INSERT INTO users (display_name, email) VALUES ($1, $2) RETURNING id::text AS id",
    )
    .bind("OAuth Owner")
    .bind(&owner_email)
    .fetch_one(&pool)
    .await
    .expect("owner insert must succeed");
    let owner_id: String = owner.get("id");

    sqlx::query(
        "INSERT INTO oauth_accounts (user_id, provider, provider_subject)
         VALUES (CAST($1 AS uuid), $2, $3)",
    )
    .bind(&owner_id)
    .bind("google")
    .bind("google-subject-fixture-1")
    .execute(&pool)
    .await
    .expect("oauth link insert must succeed");

    // The same provider identity cannot link twice.
    let duplicate_subject = sqlx::query(
        "INSERT INTO oauth_accounts (user_id, provider, provider_subject)
         VALUES (CAST($1 AS uuid), $2, $3)",
    )
    .bind(&owner_id)
    .bind("google")
    .bind("google-subject-fixture-1")
    .execute(&pool)
    .await;
    assert!(
        duplicate_subject.is_err(),
        "duplicate (provider, subject) must be rejected"
    );

    // Foreign key: unknown user cannot own a link.
    let orphan = sqlx::query(
        "INSERT INTO oauth_accounts (user_id, provider, provider_subject)
         VALUES (CAST($1 AS uuid), $2, $3)",
    )
    .bind("00000000-0000-0000-0000-000000000000")
    .bind("google")
    .bind("google-subject-fixture-2")
    .execute(&pool)
    .await;
    assert!(orphan.is_err(), "orphaned oauth link must be rejected");
}

#[tokio::test]
#[ignore = "requires an isolated PostgreSQL test database (NEXORA_AUTH_TEST_DATABASE_URL)"]
async fn auth_token_hashes_require_sha256_width() {
    let pool = test_pool().await;
    let owner_id: String = sqlx::query_scalar(
        "INSERT INTO users (display_name, email) VALUES ($1, $2) RETURNING id::text",
    )
    .bind("Digest Constraint Fixture")
    .bind(unique_email("digest-constraints"))
    .fetch_one(&pool)
    .await
    .expect("synthetic fixture user insert must succeed");

    let invalid_hash = vec![0xA5_u8; 31];
    let valid_hash = nexora_auth::session::hash_session_token(&unique_email("valid-digest"));

    let verification_invalid = sqlx::query(
        "INSERT INTO email_verification_tokens (user_id, token_hash, expires_at) \
         VALUES ($1::uuid, $2, now() + interval '1 hour')",
    )
    .bind(&owner_id)
    .bind(&invalid_hash)
    .execute(&pool)
    .await;
    assert!(
        verification_invalid.is_err(),
        "verification token digests shorter than SHA-256 must be rejected"
    );

    let reset_invalid = sqlx::query(
        "INSERT INTO password_reset_tokens (user_id, token_hash, expires_at) \
         VALUES ($1::uuid, $2, now() + interval '1 hour')",
    )
    .bind(&owner_id)
    .bind(&invalid_hash)
    .execute(&pool)
    .await;
    assert!(
        reset_invalid.is_err(),
        "password reset digests shorter than SHA-256 must be rejected"
    );

    let session_invalid = sqlx::query(
        "INSERT INTO sessions (user_id, token_hash, family_id, expires_at) \
         VALUES ($1::uuid, $2, gen_random_uuid(), now() + interval '1 hour')",
    )
    .bind(&owner_id)
    .bind(&invalid_hash)
    .execute(&pool)
    .await;
    assert!(
        session_invalid.is_err(),
        "session digests shorter than SHA-256 must be rejected"
    );

    let state_invalid = sqlx::query("INSERT INTO google_oauth_states (state_hash) VALUES ($1)")
        .bind(&invalid_hash)
        .execute(&pool)
        .await;
    assert!(
        state_invalid.is_err(),
        "Google OAuth state digests shorter than SHA-256 must be rejected"
    );

    sqlx::query(
        "INSERT INTO email_verification_tokens (user_id, token_hash, expires_at) \
         VALUES ($1::uuid, $2, now() + interval '1 hour')",
    )
    .bind(&owner_id)
    .bind(&valid_hash)
    .execute(&pool)
    .await
    .expect("32-byte verification digest must be accepted");

    sqlx::query(
        "INSERT INTO password_reset_tokens (user_id, token_hash, expires_at) \
         VALUES ($1::uuid, $2, now() + interval '1 hour')",
    )
    .bind(&owner_id)
    .bind(&valid_hash)
    .execute(&pool)
    .await
    .expect("32-byte reset digest must be accepted");

    sqlx::query(
        "INSERT INTO sessions (user_id, token_hash, family_id, expires_at) \
         VALUES ($1::uuid, $2, gen_random_uuid(), now() + interval '1 hour')",
    )
    .bind(&owner_id)
    .bind(&valid_hash)
    .execute(&pool)
    .await
    .expect("32-byte session digest must be accepted");

    sqlx::query("INSERT INTO google_oauth_states (state_hash) VALUES ($1)")
        .bind(&valid_hash)
        .execute(&pool)
        .await
        .expect("32-byte OAuth state digest must be accepted");
}
