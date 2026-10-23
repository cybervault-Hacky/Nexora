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
//! Rules: never point this at a production database; never use real user data
//! or credentials (fixtures are synthetic `example.test` addresses); never
//! print the connection string. Without PostgreSQL these tests are reported
//! as "not run" — see docs/phase-23b/README.md.

use std::sync::atomic::{AtomicUsize, Ordering};

use sqlx::postgres::PgPoolOptions;
use sqlx::Row;

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
    nexora_auth::run_migrations(&pool)
        .await
        .expect("migrations must apply cleanly");
    pool
}

#[tokio::test]
#[ignore = "requires an isolated PostgreSQL test database (NEXORA_AUTH_TEST_DATABASE_URL)"]
async fn migrations_apply_in_order_and_are_recorded() {
    let pool = test_pool().await;

    let rows = sqlx::query("SELECT version FROM _sqlx_migrations ORDER BY version")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert!(
        rows.len() >= 3,
        "all migrations through 23C must be recorded"
    );

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
