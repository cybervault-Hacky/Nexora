//! REAL PostgreSQL integration tests for Phase 23E password recovery & reset.
//! Run ONLY against a dedicated, disposable test database:
//!
//! ```bash
//! createdb nexora_auth_test
//! NEXORA_AUTH_TEST_DATABASE_URL='postgres://user@127.0.0.1:5432/nexora_auth_test?sslmode=disable' \
//!   cargo test --test recovery_db -- --ignored
//! ```
//!
//! Every test is `#[ignore]`d. NEVER point this at production. The URL is read
//! from `NEXORA_AUTH_TEST_DATABASE_URL` only and is never printed.
//!
//! Covers: enumeration-safe recovery responses, eligibility gating, token
//! hash-at-rest, expiry/supersession/single-use, invalid-input non-consumption,
//! atomic consumption under concurrency, session revocation after reset,
//! login-vs-reset staleness, password-change semantics, recovery rate
//! windows, and registration/login/session regressions (23C/23D).

use nexora_auth::email::{EmailDelivery, EmailError, EmailMessage, EmailProvider};
use nexora_auth::login::{self, LoginRequest};
use nexora_auth::recovery::{self, ChangePasswordRequest, RecoveryRequest, ResetPasswordRequest};
use nexora_auth::registration::{self, RegisterRequest};
use nexora_auth::session;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::sync::{Arc, Mutex};

const TEST_URL_VAR: &str = "NEXORA_AUTH_TEST_DATABASE_URL";

mod support;

const GOOD_PASSWORD: &str = "correct horse battery";
const NEW_PASSWORD: &str = "a brand new password";

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
    /// Extract the 64-hex reset token from `#/reset-password?token=…`.
    fn reset_token_from_last(&self) -> String {
        let sent = self.sent.lock().unwrap();
        let body = &sent.last().expect("one message sent").body;
        let (_, tail) = body.split_once("token=").expect("token link in body");
        let token: String = tail.chars().take_while(|c| c.is_ascii_hexdigit()).collect();
        assert_eq!(token.len(), recovery::RESET_TOKEN_HEX_CHARS);
        token
    }

    fn verification_token_from_last(&self) -> String {
        let sent = self.sent.lock().unwrap();
        let body = &sent.last().expect("one message sent").body;
        if let Some((_, tail)) = body.split_once("token=") {
            let token: String = tail.chars().take_while(|c| c.is_ascii_hexdigit()).collect();
            if token.len() == registration::VERIFICATION_TOKEN_HEX_CHARS {
                return token;
            }
        }
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
    support::assert_test_database(&pool).await;
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
    let token = provider.verification_token_from_last();
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

async fn request_reset(pool: &PgPool, provider: &CaptureProvider, email: &str) {
    let delivery = recovery::request_recovery(
        Some(pool),
        provider,
        EmailDelivery::Configured,
        recovery::DEFAULT_RESET_TTL_SECS,
        &RecoveryRequest {
            email: email.to_string(),
        },
    )
    .await
    .expect("recovery request");
    assert_eq!(delivery, EmailDelivery::Configured);
}

fn reset_request(token: &str, password: &str) -> ResetPasswordRequest {
    ResetPasswordRequest {
        token: token.to_string(),
        password: password.to_string(),
    }
}

async fn live_sessions(pool: &PgPool) -> i64 {
    let (count,): (i64,) = sqlx::query_as("SELECT count(*) FROM sessions WHERE revoked_at IS NULL")
        .fetch_one(pool)
        .await
        .unwrap();
    count
}

#[tokio::test]
#[ignore = "needs an isolated PostgreSQL (NEXORA_AUTH_TEST_DATABASE_URL)"]
async fn recovery_request_is_enumeration_safe_and_eligibility_gated() {
    let pool = test_pool().await;
    let provider = CaptureProvider::default();

    let verified = create_verified_user(&pool, "enum-ok").await;
    let unknown = unique_email("enum-ghost");
    let locked = create_verified_user(&pool, "enum-locked").await;
    sqlx::query("UPDATE users SET status = 'locked' WHERE email = $1")
        .bind(&locked)
        .execute(&pool)
        .await
        .unwrap();

    let mut deliveries = Vec::new();
    for email in [&verified, &unknown, &locked] {
        let delivery = recovery::request_recovery(
            Some(&pool),
            &provider,
            EmailDelivery::Configured,
            recovery::DEFAULT_RESET_TTL_SECS,
            &RecoveryRequest {
                email: (*email).to_string(),
            },
        )
        .await
        .unwrap();
        deliveries.push(delivery);
    }
    // Identical delivery capability for every account state (enumeration).
    assert_eq!(
        deliveries,
        vec![
            EmailDelivery::Configured,
            EmailDelivery::Configured,
            EmailDelivery::Configured
        ]
    );

    // Side effects exist ONLY for the eligible account.
    let (for_verified,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM password_reset_tokens t JOIN users u ON u.id = t.user_id \
         WHERE u.email = $1 AND t.revoked_at IS NULL",
    )
    .bind(&verified)
    .fetch_one(&pool)
    .await
    .unwrap();
    let (for_others,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM password_reset_tokens t JOIN users u ON u.id = t.user_id \
         WHERE u.email IN ($1, $2) AND t.revoked_at IS NULL",
    )
    .bind(&unknown)
    .bind(&locked)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(for_verified, 1);
    assert_eq!(for_others, 0, "unknown and locked accounts get no token");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "needs an isolated PostgreSQL (NEXORA_AUTH_TEST_DATABASE_URL)"]
async fn concurrent_recovery_requests_leave_only_the_newest_token_live() {
    let pool = test_pool().await;
    let email = create_verified_user(&pool, "concurrent-recovery-request").await;
    let provider_a = Arc::new(CaptureProvider::default());
    let provider_b = Arc::new(CaptureProvider::default());

    let pool_a = pool.clone();
    let email_a = email.clone();
    let provider_a_task = Arc::clone(&provider_a);
    let request_a = tokio::spawn(async move {
        recovery::request_recovery(
            Some(&pool_a),
            provider_a_task.as_ref(),
            EmailDelivery::Configured,
            recovery::DEFAULT_RESET_TTL_SECS,
            &RecoveryRequest { email: email_a },
        )
        .await
    });
    let pool_b = pool.clone();
    let email_b = email.clone();
    let provider_b_task = Arc::clone(&provider_b);
    let request_b = tokio::spawn(async move {
        recovery::request_recovery(
            Some(&pool_b),
            provider_b_task.as_ref(),
            EmailDelivery::Configured,
            recovery::DEFAULT_RESET_TTL_SECS,
            &RecoveryRequest { email: email_b },
        )
        .await
    });

    assert_eq!(request_a.await.unwrap().unwrap(), EmailDelivery::Configured);
    assert_eq!(request_b.await.unwrap().unwrap(), EmailDelivery::Configured);
    let (live_tokens,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM password_reset_tokens t \
         JOIN users u ON u.id = t.user_id \
         WHERE u.email = $1 AND t.used_at IS NULL AND t.revoked_at IS NULL \
           AND t.expires_at > now()",
    )
    .bind(&email)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        live_tokens,
        1,
        "only the most recently committed reset is live"
    );
}

#[tokio::test]
#[ignore = "needs an isolated PostgreSQL (NEXORA_AUTH_TEST_DATABASE_URL)"]
async fn reset_token_is_hashed_at_rest_and_single_use() {
    let pool = test_pool().await;
    let provider = CaptureProvider::default();
    let email = create_verified_user(&pool, "hash-at-rest").await;
    request_reset(&pool, &provider, &email).await;
    let raw = provider.reset_token_from_last();

    // Hash at rest: raw never appears; digest is sha256(raw).
    let digests: Vec<Vec<u8>> = sqlx::query_scalar("SELECT token_hash FROM password_reset_tokens")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(digests.len(), 1);
    assert_eq!(digests[0], recovery::hash_reset_token(&raw));
    assert!(digests.iter().all(|row| row != raw.as_bytes()));

    // Single use: the reset succeeds once, then the token is dead.
    recovery::reset_password(Some(&pool), &reset_request(&raw, NEW_PASSWORD))
        .await
        .unwrap();
    let error = recovery::reset_password(Some(&pool), &reset_request(&raw, NEW_PASSWORD))
        .await
        .unwrap_err();
    assert_eq!(error.code(), "invalid_or_expired_token");
}

#[tokio::test]
#[ignore = "needs an isolated PostgreSQL (NEXORA_AUTH_TEST_DATABASE_URL)"]
async fn superseded_expired_and_malformed_tokens_fail_generically() {
    let pool = test_pool().await;
    let provider = CaptureProvider::default();
    let email = create_verified_user(&pool, "token-states").await;

    // Supersession: a second request revokes the first token.
    request_reset(&pool, &provider, &email).await;
    let first = provider.reset_token_from_last();
    request_reset(&pool, &provider, &email).await;
    let second = provider.reset_token_from_last();
    assert_ne!(first, second);
    let error = recovery::reset_password(Some(&pool), &reset_request(&first, NEW_PASSWORD))
        .await
        .unwrap_err();
    assert_eq!(error.code(), "invalid_or_expired_token", "superseded token");

    // Expiry.
    sqlx::query(
        "UPDATE password_reset_tokens SET created_at = now() - interval '2 hours', expires_at = now() - interval '1 second'",
    )
        .execute(&pool)
        .await
        .unwrap();
    let error = recovery::reset_password(Some(&pool), &reset_request(&second, NEW_PASSWORD))
        .await
        .unwrap_err();
    assert_eq!(error.code(), "invalid_or_expired_token", "expired token");

    // Unknown-but-well-formed token: same generic error.
    let error =
        recovery::reset_password(Some(&pool), &reset_request(&"c".repeat(64), NEW_PASSWORD))
            .await
            .unwrap_err();
    assert_eq!(error.code(), "invalid_or_expired_token", "unknown token");
}

#[tokio::test]
#[ignore = "needs an isolated PostgreSQL (NEXORA_AUTH_TEST_DATABASE_URL)"]
async fn invalid_input_never_consumes_a_valid_token() {
    let pool = test_pool().await;
    let provider = CaptureProvider::default();
    let email = create_verified_user(&pool, "no-consume").await;
    request_reset(&pool, &provider, &email).await;
    let raw = provider.reset_token_from_last();

    let error = recovery::reset_password(Some(&pool), &reset_request(&raw, "short"))
        .await
        .unwrap_err();
    assert_eq!(error.code(), "validation_failed");

    // The token is still valid afterwards.
    recovery::reset_password(Some(&pool), &reset_request(&raw, NEW_PASSWORD))
        .await
        .unwrap();
    assert!(login::login(
        Some(&pool),
        session::DEFAULT_SESSION_TTL_SECS,
        &login_request(&email, NEW_PASSWORD),
        None,
        None,
    )
    .await
    .is_ok());
}

#[tokio::test]
#[ignore = "needs an isolated PostgreSQL (NEXORA_AUTH_TEST_DATABASE_URL)"]
async fn reset_changes_password_revokes_all_sessions_and_invalidates_tokens() {
    let pool = test_pool().await;
    let provider = CaptureProvider::default();
    let email = create_verified_user(&pool, "reset-effects").await;

    // A live session and a live verification token exist at reset time.
    let outcome = login::login(
        Some(&pool),
        session::DEFAULT_SESSION_TTL_SECS,
        &login_request(&email, GOOD_PASSWORD),
        None,
        None,
    )
    .await
    .unwrap();
    let verification_token = registration::generate_verification_token();
    let verification_hash = session::hash_session_token(&verification_token);
    sqlx::query(
        "INSERT INTO email_verification_tokens (user_id, token_hash, expires_at)
         SELECT id, $1, now() + interval '1 hour' FROM users WHERE email = $2",
    )
    .bind(&verification_hash)
    .bind(&email)
    .execute(&pool)
    .await
    .unwrap();
    request_reset(&pool, &provider, &email).await;
    let raw = provider.reset_token_from_last();
    assert!(live_sessions(&pool).await >= 1);

    recovery::reset_password(Some(&pool), &reset_request(&raw, NEW_PASSWORD))
        .await
        .unwrap();

    // Every session family is dead.
    assert_eq!(live_sessions(&pool).await, 0);
    assert!(
        session::authenticate(&pool, &outcome.session.raw_token, 604_800)
            .await
            .is_err()
    );
    // Outstanding reset + verification tokens are invalidated.
    let (dead_tokens,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM password_reset_tokens WHERE revoked_at IS NULL AND used_at IS NULL",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(dead_tokens, 0);
    assert!(registration::verify_email(Some(&pool), &verification_token)
        .await
        .is_err());
    // Old credential dead, new credential works.
    assert!(login::login(
        Some(&pool),
        session::DEFAULT_SESSION_TTL_SECS,
        &login_request(&email, GOOD_PASSWORD),
        None,
        None,
    )
    .await
    .is_err());
    assert!(login::login(
        Some(&pool),
        session::DEFAULT_SESSION_TTL_SECS,
        &login_request(&email, NEW_PASSWORD),
        None,
        None,
    )
    .await
    .is_ok());
}

#[tokio::test]
#[ignore = "needs an isolated PostgreSQL (NEXORA_AUTH_TEST_DATABASE_URL)"]
async fn concurrent_resets_redeem_a_token_exactly_once() {
    let pool = test_pool().await;
    let provider = CaptureProvider::default();
    let email = create_verified_user(&pool, "race-reset").await;
    request_reset(&pool, &provider, &email).await;
    let raw = provider.reset_token_from_last();

    let mut handles = Vec::new();
    for password in ["first new password", "second new password"] {
        let pool = pool.clone();
        let request = reset_request(&raw, password);
        handles.push(tokio::spawn(async move {
            recovery::reset_password(Some(&pool), &request).await
        }));
    }
    let mut successes = 0;
    for handle in handles {
        if handle.await.unwrap().is_ok() {
            successes += 1;
        }
    }
    assert_eq!(successes, 1, "single-use consume under concurrency");
}

#[tokio::test]
#[ignore = "needs an isolated PostgreSQL (NEXORA_AUTH_TEST_DATABASE_URL)"]
async fn concurrent_login_and_reset_leave_no_stale_session() {
    let pool = test_pool().await;
    let provider = CaptureProvider::default();
    let email = create_verified_user(&pool, "race-login").await;
    request_reset(&pool, &provider, &email).await;
    let raw = provider.reset_token_from_last();

    let login_pool = pool.clone();
    let login_email = email.clone();
    let login_task = tokio::spawn(async move {
        login::login(
            Some(&login_pool),
            session::DEFAULT_SESSION_TTL_SECS,
            &login_request(&login_email, GOOD_PASSWORD),
            None,
            None,
        )
        .await
    });
    let reset_pool = pool.clone();
    let reset_task = tokio::spawn(async move {
        recovery::reset_password(Some(&reset_pool), &reset_request(&raw, NEW_PASSWORD)).await
    });

    let login_result = login_task.await.unwrap();
    let reset_result = reset_task.await.unwrap();
    reset_result.expect("reset wins regardless");

    // Whatever the interleaving, a login that raced the reset can never keep a
    // valid session: either it failed, or its session was revoked.
    if let Ok(outcome) = login_result {
        assert!(
            session::authenticate(&pool, &outcome.session.raw_token, 604_800)
                .await
                .is_err(),
            "stale session survived the password change"
        );
    }
    // Old credential is definitively dead; the new one works.
    assert!(login::login(
        Some(&pool),
        session::DEFAULT_SESSION_TTL_SECS,
        &login_request(&email, GOOD_PASSWORD),
        None,
        None,
    )
    .await
    .is_err());
    assert!(login::login(
        Some(&pool),
        session::DEFAULT_SESSION_TTL_SECS,
        &login_request(&email, NEW_PASSWORD),
        None,
        None,
    )
    .await
    .is_ok());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "needs an isolated PostgreSQL (NEXORA_AUTH_TEST_DATABASE_URL)"]
async fn concurrent_reset_and_refresh_cannot_leave_a_live_session() {
    let pool = test_pool().await;
    let provider = CaptureProvider::default();
    let email = create_verified_user(&pool, "race-refresh-reset").await;
    let login = login::login(
        Some(&pool),
        session::DEFAULT_SESSION_TTL_SECS,
        &login_request(&email, GOOD_PASSWORD),
        None,
        None,
    )
    .await
    .unwrap();
    request_reset(&pool, &provider, &email).await;
    let reset_token = provider.reset_token_from_last();
    let reset_request = reset_request(&reset_token, NEW_PASSWORD);

    let rotate_pool = pool.clone();
    let raw_session = login.session.raw_token.clone();
    let rotate = tokio::spawn(async move {
        session::rotate_session(
            &rotate_pool,
            &raw_session,
            session::DEFAULT_SESSION_TTL_SECS,
            604_800,
            None,
            None,
        )
        .await
    });
    let reset_pool = pool.clone();
    let reset = tokio::spawn(async move {
        recovery::reset_password(Some(&reset_pool), &reset_request).await
    });
    let rotate_result = rotate.await.unwrap();
    let reset_result = reset.await.unwrap();
    reset_result.expect("password reset must complete");

    match rotate_result {
        Ok((rotated, _)) => assert!(
            session::authenticate(&pool, &rotated.raw_token, 604_800)
                .await
                .is_err(),
            "refresh that raced reset left a live session"
        ),
        Err(error) => assert_eq!(error.code(), "invalid_or_expired_token"),
    }
    assert_eq!(live_sessions(&pool).await, 0);
}

#[tokio::test]
#[ignore = "needs an isolated PostgreSQL (NEXORA_AUTH_TEST_DATABASE_URL)"]
async fn password_change_keeps_current_family_and_revokes_others() {
    let pool = test_pool().await;
    let provider = CaptureProvider::default();
    let email = create_verified_user(&pool, "change").await;

    let session_a = login::login(
        Some(&pool),
        session::DEFAULT_SESSION_TTL_SECS,
        &login_request(&email, GOOD_PASSWORD),
        None,
        None,
    )
    .await
    .unwrap();
    let session_b = login::login(
        Some(&pool),
        session::DEFAULT_SESSION_TTL_SECS,
        &login_request(&email, GOOD_PASSWORD),
        None,
        None,
    )
    .await
    .unwrap();
    let context_a = session::authenticate(&pool, &session_a.session.raw_token, 604_800)
        .await
        .unwrap();

    // Wrong current password → generic credential failure.
    let error = recovery::change_password(
        Some(&pool),
        &context_a,
        &ChangePasswordRequest {
            current_password: "not the password".to_string(),
            new_password: NEW_PASSWORD.to_string(),
        },
    )
    .await
    .unwrap_err();
    assert_eq!(error.code(), "invalid_credentials");

    // Correct change: A's family survives, B's family dies.
    recovery::change_password(
        Some(&pool),
        &context_a,
        &ChangePasswordRequest {
            current_password: GOOD_PASSWORD.to_string(),
            new_password: NEW_PASSWORD.to_string(),
        },
    )
    .await
    .unwrap();
    assert!(
        session::authenticate(&pool, &session_a.session.raw_token, 604_800)
            .await
            .is_ok()
    );
    assert!(
        session::authenticate(&pool, &session_b.session.raw_token, 604_800)
            .await
            .is_err()
    );

    // An outstanding reset token dies with the change (plan §4).
    request_reset(&pool, &provider, &email).await;
    let raw = provider.reset_token_from_last();
    recovery::change_password(
        Some(&pool),
        &context_a,
        &ChangePasswordRequest {
            current_password: NEW_PASSWORD.to_string(),
            new_password: "yet another password".to_string(),
        },
    )
    .await
    .unwrap();
    let error = recovery::reset_password(Some(&pool), &reset_request(&raw, NEW_PASSWORD))
        .await
        .unwrap_err();
    assert_eq!(error.code(), "invalid_or_expired_token");
}

#[tokio::test]
#[ignore = "needs an isolated PostgreSQL (NEXORA_AUTH_TEST_DATABASE_URL)"]
async fn concurrent_reset_and_password_change_follow_one_account_first_lock_order() {
    let pool = test_pool().await;
    let provider = CaptureProvider::default();
    let email = create_verified_user(&pool, "reset-change-lock-order").await;
    let session = login::login(
        Some(&pool),
        session::DEFAULT_SESSION_TTL_SECS,
        &login_request(&email, GOOD_PASSWORD),
        None,
        None,
    )
    .await
    .unwrap();
    let context = session::authenticate(&pool, &session.session.raw_token, 604_800)
        .await
        .unwrap();
    request_reset(&pool, &provider, &email).await;
    let raw_token = provider.reset_token_from_last();

    let reset_pool = pool.clone();
    let reset = tokio::spawn(async move {
        recovery::reset_password(
            Some(&reset_pool),
            &reset_request(&raw_token, NEW_PASSWORD),
        )
        .await
    });
    let change_pool = pool.clone();
    let change = tokio::spawn(async move {
        recovery::change_password(
            Some(&change_pool),
            &context,
            &ChangePasswordRequest {
                current_password: GOOD_PASSWORD.to_string(),
                new_password: NEW_PASSWORD.to_string(),
            },
        )
        .await
    });

    let reset_result = reset.await.unwrap();
    let change_result = change.await.unwrap();
    assert!(
        matches!(
            (&reset_result, &change_result),
            (Ok(()), Err(recovery::RecoveryError::InvalidCredentials))
                | (Err(recovery::RecoveryError::InvalidOrExpiredToken), Ok(()))
        ),
        "one account-first operation must win without a deadlock: reset={reset_result:?}, change={change_result:?}"
    );
}

#[tokio::test]
#[ignore = "needs an isolated PostgreSQL (NEXORA_AUTH_TEST_DATABASE_URL)"]
async fn recovery_rate_window_blocks_excess_attempts() {
    use nexora_auth::rate_limit::{self, CountMode, RateLimitRule};

    let pool = test_pool().await;
    let rule = RateLimitRule {
        max_failures: 2,
        window_secs: 900,
    };
    let key = rate_limit::email_key("brute@example.test");

    for _ in 0..2 {
        rate_limit::check(&pool, "recovery", &key, rule, CountMode::AllAttempts)
            .await
            .unwrap();
        rate_limit::record(&pool, "recovery", &key, "success")
            .await
            .expect("record rate-limit event");
    }
    let retry_after = rate_limit::check(&pool, "recovery", &key, rule, CountMode::AllAttempts)
        .await
        .unwrap_err()
        .retry_after_secs();
    assert_eq!(retry_after, Some(900));
}

#[tokio::test]
#[ignore = "needs an isolated PostgreSQL (NEXORA_AUTH_TEST_DATABASE_URL)"]
async fn registration_login_session_regressions_still_work() {
    // 23C/23D smoke through the full path.
    let pool = test_pool().await;
    let email = create_verified_user(&pool, "regression").await;
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
    session::revoke_family(&pool, &rotated.raw_token, 604_800)
        .await
        .unwrap();
    assert_eq!(live_sessions(&pool).await, 0);
}
