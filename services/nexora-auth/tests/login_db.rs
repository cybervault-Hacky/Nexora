//! REAL PostgreSQL integration tests for Phase 23D login & sessions. Run ONLY
//! against a dedicated, disposable test database:
//!
//! ```bash
//! createdb nexora_auth_test
//! NEXORA_AUTH_TEST_DATABASE_URL='postgres://user@127.0.0.1:5432/nexora_auth_test?sslmode=disable' \
//!   cargo test --test login_db -- --ignored
//! ```
//!
//! Every test is `#[ignore]`d. NEVER point this at production. The URL is read
//! from `NEXORA_AUTH_TEST_DATABASE_URL` only and is never printed.
//!
//! Covers: generic credential errors (unknown vs wrong password), account
//! status model (unverified/locked), session hash-at-rest, absolute/idle
//! expiry, logout family revocation, rotation + family reuse detection,
//! concurrent revoke/use, DB-backed rate limiting, and Phase 23C regressions.

use std::sync::{Arc, Mutex};

use nexora_auth::email::{EmailDelivery, EmailError, EmailMessage, EmailProvider};
use nexora_auth::login::{self, LoginRequest};
use nexora_auth::registration::{self, RegisterRequest};
use nexora_auth::session;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

const TEST_URL_VAR: &str = "NEXORA_AUTH_TEST_DATABASE_URL";

const GOOD_PASSWORD: &str = "correct horse battery";

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

async fn test_pool() -> PgPool {
    let url = std::env::var(TEST_URL_VAR).unwrap_or_else(|_| {
        panic!("{TEST_URL_VAR} must be set to an isolated throwaway database");
    });
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&url)
        .await
        .expect("connect to test database");
    nexora_auth::run_migrations(&pool)
        .await
        .expect("run migrations");
    sqlx::query("TRUNCATE users, login_attempts, google_oauth_states CASCADE")
        .execute(&pool)
        .await
        .expect("truncate");
    pool
}

fn unique_email(tag: &str) -> String {
    format!(
        "{tag}.{}.{}@example.test",
        std::process::id(),
        registration::generate_verification_token()
    )
}

/// Register + verify through the real Phase 23C path (also its regression).
async fn create_verified_user(pool: &PgPool, tag: &str) -> String {
    let provider = CaptureProvider::default();
    let email = unique_email(tag);
    registration::register(
        Some(pool),
        &provider,
        EmailDelivery::Configured,
        3_600,
        &RegisterRequest {
            name: "Ada Lovelace".to_string(),
            email: email.clone(),
            password: GOOD_PASSWORD.to_string(),
        },
    )
    .await
    .expect("register");
    let token = provider.token_from_last();
    registration::verify_email(Some(pool), &token)
        .await
        .expect("verify");
    email
}

fn login_request(email: &str, password: &str) -> LoginRequest {
    LoginRequest {
        email: email.to_string(),
        password: password.to_string(),
    }
}

#[tokio::test]
#[ignore = "needs an isolated PostgreSQL (NEXORA_AUTH_TEST_DATABASE_URL)"]
async fn login_succeeds_and_stores_only_the_token_hash() {
    let pool = test_pool().await;
    let email = create_verified_user(&pool, "login").await;

    let outcome = login::login(
        Some(&pool),
        session::DEFAULT_SESSION_TTL_SECS,
        &login_request(&email, GOOD_PASSWORD),
        None,
        None,
    )
    .await
    .unwrap();
    assert_eq!(outcome.email, email);
    assert!(outcome.session.expires_at_rfc3339.contains('T'));

    // Hash at rest: the raw token must not appear in the sessions table.
    let digests: Vec<Vec<u8>> = sqlx::query_scalar("SELECT token_hash FROM sessions")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(digests.len(), 1);
    assert_eq!(
        digests[0],
        session::hash_session_token(&outcome.session.raw_token)
    );
    assert!(digests
        .iter()
        .all(|row| row != outcome.session.raw_token.as_bytes()));

    // The issued token authenticates.
    let context = session::authenticate(&pool, &outcome.session.raw_token, 604_800)
        .await
        .unwrap();
    assert_eq!(context.user.email, email);
}

#[tokio::test]
#[ignore = "needs an isolated PostgreSQL (NEXORA_AUTH_TEST_DATABASE_URL)"]
async fn unknown_account_and_wrong_password_are_indistinguishable() {
    let pool = test_pool().await;
    let email = create_verified_user(&pool, "generic").await;

    let wrong = login::login(
        Some(&pool),
        session::DEFAULT_SESSION_TTL_SECS,
        &login_request(&email, "not the password"),
        None,
        None,
    )
    .await
    .unwrap_err();
    let unknown = login::login(
        Some(&pool),
        session::DEFAULT_SESSION_TTL_SECS,
        &login_request(&unique_email("ghost"), GOOD_PASSWORD),
        None,
        None,
    )
    .await
    .unwrap_err();

    // Same code, same message — no enumeration.
    assert_eq!(wrong.code(), unknown.code());
    assert_eq!(wrong.message(), unknown.message());
    assert_eq!(wrong.code(), "invalid_credentials");
}

#[tokio::test]
#[ignore = "needs an isolated PostgreSQL (NEXORA_AUTH_TEST_DATABASE_URL)"]
async fn unverified_and_locked_accounts_are_refused() {
    let pool = test_pool().await;
    let provider = CaptureProvider::default();

    // Unverified: correct password → email_unverified (plan §6 code).
    let unverified_email = unique_email("unverified");
    registration::register(
        Some(&pool),
        &provider,
        EmailDelivery::Configured,
        3_600,
        &RegisterRequest {
            name: "Unverified User".to_string(),
            email: unverified_email.clone(),
            password: GOOD_PASSWORD.to_string(),
        },
    )
    .await
    .unwrap();
    let error = login::login(
        Some(&pool),
        session::DEFAULT_SESSION_TTL_SECS,
        &login_request(&unverified_email, GOOD_PASSWORD),
        None,
        None,
    )
    .await
    .unwrap_err();
    assert_eq!(error.code(), "email_unverified");

    // Locked: correct password → generic invalid_credentials.
    let locked_email = create_verified_user(&pool, "locked").await;
    sqlx::query("UPDATE users SET status = 'locked' WHERE email = $1")
        .bind(&locked_email)
        .execute(&pool)
        .await
        .unwrap();
    let error = login::login(
        Some(&pool),
        session::DEFAULT_SESSION_TTL_SECS,
        &login_request(&locked_email, GOOD_PASSWORD),
        None,
        None,
    )
    .await
    .unwrap_err();
    assert_eq!(error.code(), "invalid_credentials");
}

#[tokio::test]
#[ignore = "needs an isolated PostgreSQL (NEXORA_AUTH_TEST_DATABASE_URL)"]
async fn absolute_and_idle_expiry_both_reject_sessions() {
    let pool = test_pool().await;
    let email = create_verified_user(&pool, "expiry").await;

    // Absolute expiry.
    let outcome = login::login(
        Some(&pool),
        session::DEFAULT_SESSION_TTL_SECS,
        &login_request(&email, GOOD_PASSWORD),
        None,
        None,
    )
    .await
    .unwrap();
    sqlx::query(
        "UPDATE sessions SET created_at = now() - interval '2 hours', last_used_at = now() - interval '2 hours', expires_at = now() - interval '1 second'",
    )
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        session::authenticate(&pool, &outcome.session.raw_token, 604_800).await,
        Err(session::SessionError::Expired)
    );

    // Idle expiry (last_used_at older than the idle TTL).
    let outcome = login::login(
        Some(&pool),
        session::DEFAULT_SESSION_TTL_SECS,
        &login_request(&email, GOOD_PASSWORD),
        None,
        None,
    )
    .await
    .unwrap();
    sqlx::query("UPDATE sessions SET last_used_at = now() - interval '30 days'")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        session::authenticate(&pool, &outcome.session.raw_token, 604_800).await,
        Err(session::SessionError::Expired)
    );
}

#[tokio::test]
#[ignore = "needs an isolated PostgreSQL (NEXORA_AUTH_TEST_DATABASE_URL)"]
async fn logout_revokes_the_whole_family_including_rotated_siblings() {
    let pool = test_pool().await;
    let email = create_verified_user(&pool, "logout").await;
    let outcome = login::login(
        Some(&pool),
        session::DEFAULT_SESSION_TTL_SECS,
        &login_request(&email, GOOD_PASSWORD),
        None,
        None,
    )
    .await
    .unwrap();

    let (rotated, _) = session::rotate_session(
        &pool,
        &outcome.session.raw_token,
        session::DEFAULT_SESSION_TTL_SECS,
        604_800,
        None,
        None,
    )
    .await
    .unwrap();

    // Logout with the rotated (live) token kills the family.
    session::revoke_family(&pool, &rotated.raw_token, 604_800)
        .await
        .unwrap();

    // Both the rotated token and the original are dead — and reuse detection
    // reports the same generic error.
    for token in [&rotated.raw_token, &outcome.session.raw_token] {
        let error = session::authenticate(&pool, token, 604_800)
            .await
            .unwrap_err();
        assert_eq!(error.code(), "invalid_or_expired_token");
    }
    let (live,): (i64,) = sqlx::query_as("SELECT count(*) FROM sessions WHERE revoked_at IS NULL")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(live, 0);
}

#[tokio::test]
#[ignore = "needs an isolated PostgreSQL (NEXORA_AUTH_TEST_DATABASE_URL)"]
async fn rotation_is_single_use_and_family_reuse_revokes_everything() {
    let pool = test_pool().await;
    let email = create_verified_user(&pool, "rotate").await;
    let outcome = login::login(
        Some(&pool),
        session::DEFAULT_SESSION_TTL_SECS,
        &login_request(&email, GOOD_PASSWORD),
        None,
        None,
    )
    .await
    .unwrap();

    let (rotated, _) = session::rotate_session(
        &pool,
        &outcome.session.raw_token,
        session::DEFAULT_SESSION_TTL_SECS,
        604_800,
        None,
        None,
    )
    .await
    .unwrap();
    assert_ne!(rotated.raw_token, outcome.session.raw_token);

    // The fresh token works…
    assert!(session::authenticate(&pool, &rotated.raw_token, 604_800)
        .await
        .is_ok());
    // …until the OLD token is presented again (reuse) → family revoked.
    let _ = session::authenticate(&pool, &outcome.session.raw_token, 604_800)
        .await
        .unwrap_err();
    let error = session::authenticate(&pool, &rotated.raw_token, 604_800)
        .await
        .unwrap_err();
    assert_eq!(error.code(), "invalid_or_expired_token");
}

#[tokio::test]
#[ignore = "needs an isolated PostgreSQL (NEXORA_AUTH_TEST_DATABASE_URL)"]
async fn concurrent_revocation_cannot_be_revived() {
    let pool = test_pool().await;
    let email = create_verified_user(&pool, "concurrent").await;
    let outcome = login::login(
        Some(&pool),
        session::DEFAULT_SESSION_TTL_SECS,
        &login_request(&email, GOOD_PASSWORD),
        None,
        None,
    )
    .await
    .unwrap();

    let token = outcome.session.raw_token.clone();
    let revoke_pool = pool.clone();
    let revoke =
        tokio::spawn(async move { session::revoke_family(&revoke_pool, &token, 604_800).await });
    let use_pool = pool.clone();
    let use_token = outcome.session.raw_token.clone();
    let used =
        tokio::spawn(async move { session::authenticate(&use_pool, &use_token, 604_800).await });

    let _ = revoke.await.unwrap();
    let first = used.await.unwrap();
    // Any success must have happened strictly before revocation; afterwards
    // every attempt fails.
    if first.is_ok() {
        // (race window) — but the post-revocation attempt must fail.
    }
    let after = session::authenticate(&pool, &outcome.session.raw_token, 604_800).await;
    assert!(after.is_err(), "revoked sessions must not revive");
}

#[tokio::test]
#[ignore = "needs an isolated PostgreSQL (NEXORA_AUTH_TEST_DATABASE_URL)"]
async fn rate_limiting_windows_block_excess_failures_without_lockout() {
    use nexora_auth::rate_limit::{self, CountMode, RateLimitRule};

    let pool = test_pool().await;
    let rule = RateLimitRule {
        max_failures: 2,
        window_secs: 900,
    };
    let key = rate_limit::email_key("brute@example.test");

    for _ in 0..2 {
        rate_limit::check(&pool, "login", &key, rule, CountMode::FailuresOnly)
            .await
            .unwrap();
        rate_limit::record(&pool, "login", &key, "failure").await;
    }
    let retry_after = rate_limit::check(&pool, "login", &key, rule, CountMode::FailuresOnly)
        .await
        .unwrap_err();
    assert_eq!(retry_after, 900, "Retry-After reports the window");

    // Successes do not consume the failure window (no lockout for real users
    // with a correct password).
    rate_limit::record(&pool, "login", &key, "success").await;
    let still_blocked =
        rate_limit::check(&pool, "login", &key, rule, CountMode::FailuresOnly).await;
    assert!(still_blocked.is_err());
}

#[tokio::test]
#[ignore = "needs an isolated PostgreSQL (NEXORA_AUTH_TEST_DATABASE_URL)"]
async fn phase23c_registration_and_verification_still_work() {
    // End-to-end regression through the Phase 23C path.
    let pool = test_pool().await;
    let email = create_verified_user(&pool, "regression").await;
    let (verified_at,): (Option<String>,) =
        sqlx::query_as("SELECT email_verified_at::text FROM users WHERE email = $1")
            .bind(&email)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(verified_at.is_some());
    // Login now succeeds with the same credential.
    assert!(login::login(
        Some(&pool),
        session::DEFAULT_SESSION_TTL_SECS,
        &login_request(&email, GOOD_PASSWORD),
        None,
        None,
    )
    .await
    .is_ok());
}

#[tokio::test]
#[ignore = "needs an isolated PostgreSQL (NEXORA_AUTH_TEST_DATABASE_URL)"]
async fn concurrent_registrations_still_create_exactly_one_account() {
    let pool = test_pool().await;
    let provider = Arc::new(CaptureProvider::default());
    let email = unique_email("race23d");

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
                &RegisterRequest {
                    name: "Ada".to_string(),
                    email,
                    password: GOOD_PASSWORD.to_string(),
                },
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
    assert_eq!(users, 1, "UNIQUE(email) must hold under concurrency (23C)");
}
