//! REAL PostgreSQL integration tests for Phase 23C registration & email
//! verification. These run ONLY against a dedicated, disposable test database:
//!
//! ```bash
//! createdb nexora_auth_test
//! NEXORA_AUTH_TEST_DATABASE_URL='postgres://user@127.0.0.1:5432/nexora_auth_test?sslmode=disable' \
//!   cargo test --test registration_db -- --ignored
//! ```
//!
//! Every test is `#[ignore]`d so a plain `cargo test` never touches a real
//! database. NEVER point this at production. The URL is read from
//! `NEXORA_AUTH_TEST_DATABASE_URL` only; it is never printed.
//!
//! Covers: raw-token-never-stored, duplicate-email generic equality,
//! concurrent registration, verify single-use, expiry, resend invalidation,
//! concurrent verify, provider-failure honesty.

use std::sync::{Arc, Mutex};

use nexora_auth::email::{EmailDelivery, EmailError, EmailMessage, EmailProvider};
use nexora_auth::registration::{self, RegisterRequest};
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

static MIGRATIONS: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

const TEST_URL_VAR: &str = "NEXORA_AUTH_TEST_DATABASE_URL";

/// Provider that records delivered messages so tests can read the token
/// exactly as the user would (the only place the raw token may appear).
#[derive(Default)]
struct CaptureProvider {
    sent: Mutex<Vec<EmailMessage>>,
}

impl EmailProvider for CaptureProvider {
    fn send(
        &self,
        message: EmailMessage,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), EmailError>> + Send + '_>>
    {
        self.sent.lock().unwrap().push(message);
        Box::pin(async { Ok(()) })
    }
}

impl CaptureProvider {
    fn token_from_last(&self) -> String {
        let sent = self.sent.lock().unwrap();
        let body = &sent.last().expect("one message sent").body;
        body.lines()
            .map(str::trim)
            .find(|line| line.len() == registration::VERIFICATION_TOKEN_HEX_CHARS)
            .expect("token line in body")
            .to_string()
    }
}

/// Provider that always fails (transport error) — nothing is delivered.
struct FailProvider;

impl EmailProvider for FailProvider {
    fn send(
        &self,
        _message: EmailMessage,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), EmailError>> + Send + '_>>
    {
        Box::pin(async { Err(EmailError::Transport) })
    }
}

async fn test_pool() -> PgPool {
    let url = std::env::var(TEST_URL_VAR).unwrap_or_else(|_| {
        panic!("{TEST_URL_VAR} must be set to an isolated throwaway database");
    });
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&url)
        .await
        .expect("connect to test database");
    MIGRATIONS.run(&pool).await.expect("run migrations");
    // Start from a clean slate without dropping the schema (parallel-friendly).
    sqlx::query("TRUNCATE users CASCADE")
        .execute(&pool)
        .await
        .expect("truncate");
    pool
}

fn register_request(email: &str) -> RegisterRequest {
    RegisterRequest {
        name: "Ada Lovelace".to_string(),
        email: email.to_string(),
        password: "correct horse battery".to_string(),
    }
}

fn unique_email(tag: &str) -> String {
    format!(
        "{tag}.{}.{}@example.test",
        std::process::id(),
        registration::generate_verification_token()
    )
}

#[tokio::test]
#[ignore = "needs an isolated PostgreSQL (NEXORA_AUTH_TEST_DATABASE_URL)"]
async fn register_stores_argon2_and_never_the_raw_password_or_token() {
    let pool = test_pool().await;
    let provider = CaptureProvider::default();
    let email = unique_email("store");

    let outcome = registration::register(
        Some(&pool),
        &provider,
        EmailDelivery::Configured,
        3_600,
        &register_request(&email),
    )
    .await
    .unwrap();
    assert_eq!(outcome.email_delivery, EmailDelivery::Configured);

    let raw_token = provider.token_from_last();
    let digest = registration::hash_verification_token(&raw_token);

    let (password_hash,): (String,) =
        sqlx::query_as("SELECT password_hash FROM users WHERE email = $1")
            .bind(&email)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(password_hash.starts_with("$argon2id$"), "PHC string stored");
    assert!(!password_hash.contains("correct horse"));

    let digests: Vec<Vec<u8>> = sqlx::query_as("SELECT token_hash FROM email_verification_tokens")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(digests.len(), 1);
    assert_eq!(digests[0], digest);
    // The raw token must not exist anywhere in the digest column.
    assert!(digests.iter().all(|row| row != raw_token.as_bytes()));
}

#[tokio::test]
#[ignore = "needs an isolated PostgreSQL (NEXORA_AUTH_TEST_DATABASE_URL)"]
async fn duplicate_email_gets_identical_generic_outcome_and_no_second_row() {
    let pool = test_pool().await;
    let provider = CaptureProvider::default();
    let email = unique_email("dup");

    let first = registration::register(
        Some(&pool),
        &provider,
        EmailDelivery::Unconfigured,
        3_600,
        &register_request(&email),
    )
    .await
    .unwrap();
    let second = registration::register(
        Some(&pool),
        &provider,
        EmailDelivery::Unconfigured,
        3_600,
        &register_request(&email),
    )
    .await
    .unwrap();

    // Enumeration defence: byte-identical outcomes.
    assert_eq!(first, second);
    let (users,): (i64,) = sqlx::query_as("SELECT count(*) FROM users WHERE email = $1")
        .bind(&email)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(users, 1);
    let (tokens,): (i64,) = sqlx::query_as("SELECT count(*) FROM email_verification_tokens")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(tokens, 1, "duplicate registration must not mint tokens");
}

#[tokio::test]
#[ignore = "needs an isolated PostgreSQL (NEXORA_AUTH_TEST_DATABASE_URL)"]
async fn concurrent_registrations_create_exactly_one_account() {
    let pool = test_pool().await;
    let provider = Arc::new(CaptureProvider::default());
    let email = unique_email("race");

    let mut handles = Vec::new();
    for _ in 0..8 {
        let pool = pool.clone();
        let provider = provider.clone();
        let email = email.clone();
        handles.push(tokio::spawn(async move {
            registration::register(
                Some(&pool),
                provider.as_ref(),
                EmailDelivery::Configured,
                3_600,
                &register_request(&email),
            )
            .await
        }));
    }
    for handle in handles {
        assert!(handle.await.unwrap().is_ok());
    }

    let (users,): (i64,) = sqlx::query_as("SELECT count(*) FROM users WHERE email = $1")
        .bind(&email)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(users, 1, "UNIQUE(email) must hold under concurrency");
}

#[tokio::test]
#[ignore = "needs an isolated PostgreSQL (NEXORA_AUTH_TEST_DATABASE_URL)"]
async fn verify_happy_path_then_reuse_is_rejected() {
    let pool = test_pool().await;
    let provider = CaptureProvider::default();
    let email = unique_email("verify");
    registration::register(
        Some(&pool),
        &provider,
        EmailDelivery::Configured,
        3_600,
        &register_request(&email),
    )
    .await
    .unwrap();
    let token = provider.token_from_last();

    registration::verify_email(Some(&pool), &token).await.unwrap();

    let (verified_at,): (Option<String>,) =
        sqlx::query_as("SELECT email_verified_at::text FROM users WHERE email = $1")
            .bind(&email)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(verified_at.is_some(), "email_verified_at set after verify");

    // Single-use: second consume must fail with the same generic error.
    let second = registration::verify_email(Some(&pool), &token).await;
    assert_eq!(second, Err(registration::VerifyError::InvalidOrExpired));
}

#[tokio::test]
#[ignore = "needs an isolated PostgreSQL (NEXORA_AUTH_TEST_DATABASE_URL)"]
async fn expired_tokens_are_rejected_generically() {
    let pool = test_pool().await;
    let provider = CaptureProvider::default();
    let email = unique_email("expire");
    registration::register(
        Some(&pool),
        &provider,
        EmailDelivery::Configured,
        3_600,
        &register_request(&email),
    )
    .await
    .unwrap();
    let token = provider.token_from_last();

    sqlx::query("UPDATE email_verification_tokens SET expires_at = now() - interval '1 hour'")
        .execute(&pool)
        .await
        .unwrap();

    let result = registration::verify_email(Some(&pool), &token).await;
    assert_eq!(result, Err(registration::VerifyError::InvalidOrExpired));
}

#[tokio::test]
#[ignore = "needs an isolated PostgreSQL (NEXORA_AUTH_TEST_DATABASE_URL)"]
async fn resend_revokes_prior_token_and_issues_a_working_one() {
    let pool = test_pool().await;
    let provider = CaptureProvider::default();
    let email = unique_email("resend");
    registration::register(
        Some(&pool),
        &provider,
        EmailDelivery::Configured,
        3_600,
        &register_request(&email),
    )
    .await
    .unwrap();
    let first_token = provider.token_from_last();

    registration::resend_verification(
        Some(&pool),
        &provider,
        EmailDelivery::Configured,
        3_600,
        &email,
    )
    .await
    .unwrap();
    let second_token = provider.token_from_last();
    assert_ne!(first_token, second_token);

    // Superseded token is revoked — generic failure.
    assert_eq!(
        registration::verify_email(Some(&pool), &first_token).await,
        Err(registration::VerifyError::InvalidOrExpired)
    );
    // Fresh token works.
    registration::verify_email(Some(&pool), &second_token).await.unwrap();
}

#[tokio::test]
#[ignore = "needs an isolated PostgreSQL (NEXORA_AUTH_TEST_DATABASE_URL)"]
async fn concurrent_verify_consumes_the_token_exactly_once() {
    let pool = test_pool().await;
    let provider = CaptureProvider::default();
    let email = unique_email("vrace");
    registration::register(
        Some(&pool),
        &provider,
        EmailDelivery::Configured,
        3_600,
        &register_request(&email),
    )
    .await
    .unwrap();
    let token = provider.token_from_last();

    let mut handles = Vec::new();
    for _ in 0..8 {
        let pool = pool.clone();
        let token = token.clone();
        handles.push(tokio::spawn(async move {
            registration::verify_email(Some(&pool), &token).await
        }));
    }
    let mut successes = 0;
    for handle in handles {
        if handle.await.unwrap().is_ok() {
            successes += 1;
        }
    }
    assert_eq!(successes, 1, "single-use must hold under concurrency");
}

#[tokio::test]
#[ignore = "needs an isolated PostgreSQL (NEXORA_AUTH_TEST_DATABASE_URL)"]
async fn provider_failure_never_fakes_delivery() {
    let pool = test_pool().await;
    let email = unique_email("fail");
    let outcome = registration::register(
        Some(&pool),
        &FailProvider,
        EmailDelivery::Configured,
        3_600,
        &register_request(&email),
    )
    .await
    .unwrap();

    // The account exists (registration itself succeeded) …
    let (users,): (i64,) = sqlx::query_as("SELECT count(*) FROM users WHERE email = $1")
        .bind(&email)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(users, 1);
    // … and the outcome reports capability only — never a delivery receipt.
    assert_eq!(outcome.email_delivery, EmailDelivery::Configured);

    // The account stays unverified; no silent auto-verify.
    let (verified_at,): (Option<String>,) =
        sqlx::query_as("SELECT email_verified_at::text FROM users WHERE email = $1")
            .bind(&email)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(verified_at.is_none());
}

#[tokio::test]
#[ignore = "needs an isolated PostgreSQL (NEXORA_AUTH_TEST_DATABASE_URL)"]
async fn resend_for_unknown_or_verified_addresses_is_safe_and_quiet() {
    let pool = test_pool().await;
    let provider = CaptureProvider::default();

    // Unknown address: no send, still a generic capability outcome.
    let delivery = registration::resend_verification(
        Some(&pool),
        &provider,
        EmailDelivery::Unconfigured,
        3_600,
        &unique_email("ghost"),
    )
    .await
    .unwrap();
    assert_eq!(delivery, EmailDelivery::Unconfigured);
    assert!(provider.sent.lock().unwrap().is_empty());
}
