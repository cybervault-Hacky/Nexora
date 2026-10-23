//! REAL PostgreSQL integration tests for Phase 23F Google OAuth exchange.
//! Run ONLY against a dedicated, disposable test database:
//!
//! ```bash
//! createdb nexora_auth_test
//! NEXORA_AUTH_TEST_DATABASE_URL='postgres://user@127.0.0.1:5432/nexora_auth_test?sslmode=disable' \
//!   cargo test --test oauth_db -- --ignored
//! ```
//!
//! Every test is `#[ignore]`d. NEVER point this at production. The URL is read
//! from `NEXORA_AUTH_TEST_DATABASE_URL` only and is never printed.
//!
//! The tests exercise THIS service's state one-time-ness, redirect/nonce
//! enforcement, and account-linking rules with an injected `GoogleIdentity`
//! (the same seam pattern as the email `CaptureProvider`) — they are real
//! database tests of real logic; the Google network hop itself is the one
//! injected edge (and is unexercised here: no network, no credentials).

use nexora_auth::email::{EmailDelivery, EmailError, EmailMessage, EmailProvider};
use nexora_auth::oauth::{
    self, GoogleExchangeError, GoogleExchangeRequest, GoogleIdentity, GoogleProvider,
};
use nexora_auth::registration::{self, RegisterRequest};
use nexora_auth::session;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::sync::Mutex;

const TEST_URL_VAR: &str = "NEXORA_AUTH_TEST_DATABASE_URL";
const REDIRECT: &str = "http://127.0.0.1:49152/callback";

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

/// The one injected edge: a fixed identity (or refusal) instead of Google.
enum FakeMode {
    Identity(GoogleIdentity),
    Refuse,
    Unconfigured,
}

struct FakeGoogle {
    mode: Mutex<FakeMode>,
}

impl FakeGoogle {
    fn identity(identity: GoogleIdentity) -> Self {
        Self {
            mode: Mutex::new(FakeMode::Identity(identity)),
        }
    }
}

impl GoogleProvider for FakeGoogle {
    fn exchange_code<'a>(
        &'a self,
        _code: &'a str,
        _code_verifier: &'a str,
        _redirect_uri: &'a str,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<Output = Result<GoogleIdentity, GoogleExchangeError>>
                + Send
                + 'a,
        >,
    > {
        let result = match &*self.mode.lock().unwrap() {
            FakeMode::Identity(identity) => Ok(identity.clone()),
            FakeMode::Refuse => Err(GoogleExchangeError::Refused),
            FakeMode::Unconfigured => Err(GoogleExchangeError::NotConfigured),
        };
        Box::pin(async move { result })
    }
}

fn identity(email: &str, verified: bool, subject: &str) -> GoogleIdentity {
    GoogleIdentity {
        subject: subject.to_string(),
        email: email.to_string(),
        email_verified: verified,
        nonce: None,
        name: Some("Ada Lovelace".to_string()),
    }
}

fn request(state: &str) -> GoogleExchangeRequest {
    GoogleExchangeRequest {
        code: "4/0AeanS0b-code".to_string(),
        code_verifier: "v".repeat(43),
        state: state.to_string(),
        redirect_uri: REDIRECT.to_string(),
        nonce: None,
    }
}

fn unique_state(tag: &str) -> String {
    format!("{tag}{}", registration::generate_verification_token())[..64].to_string()
}

fn unique_email(tag: &str) -> String {
    format!(
        "{tag}.{}.{}@example.test",
        std::process::id(),
        registration::generate_verification_token()
    )
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
    sqlx::query("TRUNCATE users CASCADE")
        .execute(&pool)
        .await
        .expect("truncate");
    pool
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
            password: "correct horse battery".to_string(),
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

#[tokio::test]
#[ignore = "needs an isolated PostgreSQL (NEXORA_AUTH_TEST_DATABASE_URL)"]
async fn state_is_single_use_and_replays_fail_generically() {
    let pool = test_pool().await;
    let google = FakeGoogle::identity(identity("ada@example.test", true, "sub-state"));
    let state = unique_state("st");
    let r = request(&state);

    let first = oauth::google_exchange(Some(&pool), &google, REDIRECT, &r)
        .await
        .unwrap();
    assert_eq!(first.email, "ada@example.test");

    // Exact replay of the same state is refused with the generic failure.
    let error = oauth::google_exchange(Some(&pool), &google, REDIRECT, &r)
        .await
        .unwrap_err();
    assert_eq!(error.code(), "invalid_or_expired_token");

    // A fresh state works (the failure was about replay, not the identity).
    let mut r2 = request(&unique_state("st"));
    r2.code = "4/0another-code".to_string();
    assert!(oauth::google_exchange(Some(&pool), &google, REDIRECT, &r2)
        .await
        .is_ok());
}

#[tokio::test]
#[ignore = "needs an isolated PostgreSQL (NEXORA_AUTH_TEST_DATABASE_URL)"]
async fn redirect_uri_mismatch_is_refused_before_any_exchange() {
    let pool = test_pool().await;
    // Provider that PANICS if reached — the mismatch must be refused first.
    struct Bomb;
    impl GoogleProvider for Bomb {
        fn exchange_code<'a>(
            &'a self,
            _code: &'a str,
            _code_verifier: &'a str,
            _redirect_uri: &'a str,
        ) -> std::pin::Pin<
            Box<
                dyn std::future::Future<Output = Result<GoogleIdentity, GoogleExchangeError>>
                    + Send
                    + 'a,
            >,
        > {
            Box::pin(async { panic!("provider must not be reached on redirect mismatch") })
        }
    }

    let mut r = request(&unique_state("rd"));
    r.redirect_uri = "http://127.0.0.1:9999/callback".to_string();
    let error = oauth::google_exchange(Some(&pool), &Bomb, REDIRECT, &r)
        .await
        .unwrap_err();
    assert_eq!(error.code(), "invalid_or_expired_token");

    // A non-loopback URI is refused even if it "matches" nothing configured.
    let mut r2 = request(&unique_state("rd"));
    r2.redirect_uri = "https://accounts.example.test/callback".to_string();
    let error = oauth::google_exchange(Some(&pool), &Bomb, REDIRECT, &r2)
        .await
        .unwrap_err();
    assert_eq!(error.code(), "invalid_or_expired_token");
}

#[tokio::test]
#[ignore = "needs an isolated PostgreSQL (NEXORA_AUTH_TEST_DATABASE_URL)"]
async fn verified_match_auto_links_and_creates_a_session() {
    let pool = test_pool().await;
    let email = create_verified_user(&pool, "link").await;
    let google = FakeGoogle::identity(identity(&email, true, "sub-link"));

    let outcome = oauth::google_exchange(
        Some(&pool),
        &google,
        REDIRECT,
        &request(&unique_state("ln")),
    )
    .await
    .unwrap();
    assert_eq!(outcome.email, email);
    // A real session exists and authenticates.
    assert!(
        session::authenticate(&pool, &outcome.session.raw_token, 604_800)
            .await
            .is_ok()
    );
    // Exactly one link row, keyed by the immutable subject.
    let (links,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM oauth_accounts WHERE provider = 'google' AND provider_subject = 'sub-link'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(links, 1);
}

#[tokio::test]
#[ignore = "needs an isolated PostgreSQL (NEXORA_AUTH_TEST_DATABASE_URL)"]
async fn unverified_account_match_is_a_conflict_not_a_link() {
    let pool = test_pool().await;
    let provider = CaptureProvider::default();
    let email = unique_email("unverified-match");
    registration::register(
        Some(&pool),
        &provider,
        EmailDelivery::Configured,
        3_600,
        &RegisterRequest {
            name: "Unverified".to_string(),
            email: email.clone(),
            password: "correct horse battery".to_string(),
        },
    )
    .await
    .unwrap();
    // Account exists but its email is NOT verified.
    let google = FakeGoogle::identity(identity(&email, true, "sub-unv"));

    let error = oauth::google_exchange(
        Some(&pool),
        &google,
        REDIRECT,
        &request(&unique_state("un")),
    )
    .await
    .unwrap_err();
    assert_eq!(error.code(), "email_conflict");
    assert_eq!(error.status_code(), 409);
    // Plan §2.5: no auto-link on unverified matches.
    let (links,): (i64,) = sqlx::query_as("SELECT count(*) FROM oauth_accounts")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(links, 0);
    let (users,): (i64,) = sqlx::query_as("SELECT count(*) FROM users")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(users, 1, "no account created");
}

#[tokio::test]
#[ignore = "needs an isolated PostgreSQL (NEXORA_AUTH_TEST_DATABASE_URL)"]
async fn unverified_provider_email_is_refused_outright() {
    let pool = test_pool().await;
    let email = unique_email("prov-unv");
    let google = FakeGoogle::identity(identity(&email, false, "sub-pu"));

    let error = oauth::google_exchange(
        Some(&pool),
        &google,
        REDIRECT,
        &request(&unique_state("pu")),
    )
    .await
    .unwrap_err();
    assert_eq!(error.code(), "email_unverified");
    let (users,): (i64,) = sqlx::query_as("SELECT count(*) FROM users")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        users, 0,
        "never auto-link or create on unverified provider emails"
    );
}

#[tokio::test]
#[ignore = "needs an isolated PostgreSQL (NEXORA_AUTH_TEST_DATABASE_URL)"]
async fn new_subject_creates_an_oauth_only_account_and_signs_in() {
    let pool = test_pool().await;
    let email = unique_email("fresh");
    let google = FakeGoogle::identity(identity(&email, true, "sub-fresh"));

    let outcome = oauth::google_exchange(
        Some(&pool),
        &google,
        REDIRECT,
        &request(&unique_state("fr")),
    )
    .await
    .unwrap();
    assert_eq!(outcome.email, email);
    assert_eq!(outcome.name, "Ada Lovelace");

    let (password_hash, verified): (Option<String>, bool) = sqlx::query_as(
        "SELECT password_hash, (email_verified_at IS NOT NULL) FROM users WHERE email = $1",
    )
    .bind(&email)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(password_hash, None, "OAuth-only accounts hold no password");
    assert!(verified, "Google-verified email counts as verified");

    // Identity is keyed by `sub`, not by email.
    let (subject,): (String,) =
        sqlx::query_as("SELECT provider_subject FROM oauth_accounts WHERE provider = 'google'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(subject, "sub-fresh");
}

#[tokio::test]
#[ignore = "needs an isolated PostgreSQL (NEXORA_AUTH_TEST_DATABASE_URL)"]
async fn same_subject_signs_in_regardless_of_email_claim() {
    let pool = test_pool().await;
    let google = FakeGoogle::identity(identity("first@example.test", true, "sub-key"));
    let outcome = oauth::google_exchange(
        Some(&pool),
        &google,
        REDIRECT,
        &request(&unique_state("k1")),
    )
    .await
    .unwrap();

    // Same `sub`, different email claim → same account (sub is the key).
    let google = FakeGoogle::identity(identity("renamed@example.test", true, "sub-key"));
    let second = oauth::google_exchange(
        Some(&pool),
        &google,
        REDIRECT,
        &request(&unique_state("k2")),
    )
    .await
    .unwrap();
    assert_eq!(outcome.user_id, second.user_id);
    let (users,): (i64,) = sqlx::query_as("SELECT count(*) FROM users")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(users, 1, "no second account for the same subject");
}

#[tokio::test]
#[ignore = "needs an isolated PostgreSQL (NEXORA_AUTH_TEST_DATABASE_URL)"]
async fn locked_linked_accounts_are_refused_generically() {
    let pool = test_pool().await;
    let email = create_verified_user(&pool, "locked-oauth").await;
    let google = FakeGoogle::identity(identity(&email, true, "sub-locked"));
    let outcome = oauth::google_exchange(
        Some(&pool),
        &google,
        REDIRECT,
        &request(&unique_state("lk")),
    )
    .await
    .unwrap();

    sqlx::query("UPDATE users SET status = 'locked' WHERE email = $1")
        .bind(&email)
        .execute(&pool)
        .await
        .unwrap();
    let error = oauth::google_exchange(
        Some(&pool),
        &google,
        REDIRECT,
        &request(&unique_state("lk")),
    )
    .await
    .unwrap_err();
    assert_eq!(error.code(), "invalid_credentials");
    // The pre-lock session still exists as a row but the account is locked;
    // nothing here created a new session.
    assert!(
        session::authenticate(&pool, &outcome.session.raw_token, 604_800)
            .await
            .is_err()
    );
}

#[tokio::test]
#[ignore = "needs an isolated PostgreSQL (NEXORA_AUTH_TEST_DATABASE_URL)"]
async fn nonce_mismatches_fail_generically_and_matches_pass() {
    let pool = test_pool().await;
    let mut identified = identity("nonce@example.test", true, "sub-nonce");
    identified.nonce = Some("n-good".to_string());
    let google = FakeGoogle::identity(identified);

    let mut r = request(&unique_state("nc"));
    r.nonce = Some("n-other".to_string());
    let error = oauth::google_exchange(Some(&pool), &google, REDIRECT, &r)
        .await
        .unwrap_err();
    assert_eq!(error.code(), "invalid_or_expired_token");

    // Missing nonce claim with a supplied expectation is also a hard failure.
    let plain = FakeGoogle::identity(identity("nonce2@example.test", true, "sub-n2"));
    let mut r2 = request(&unique_state("nc"));
    r2.nonce = Some("n-good".to_string());
    let error = oauth::google_exchange(Some(&pool), &plain, REDIRECT, &r2)
        .await
        .unwrap_err();
    assert_eq!(error.code(), "invalid_or_expired_token");

    // Matching nonce passes.
    let mut r3 = request(&unique_state("nc"));
    r3.nonce = Some("n-good".to_string());
    assert!(oauth::google_exchange(Some(&pool), &google, REDIRECT, &r3)
        .await
        .is_ok());
}

#[tokio::test]
#[ignore = "needs an isolated PostgreSQL (NEXORA_AUTH_TEST_DATABASE_URL)"]
async fn provider_refusals_and_unconfigured_are_distinct_and_honest() {
    let pool = test_pool().await;
    let refuse = FakeGoogle {
        mode: Mutex::new(FakeMode::Refuse),
    };
    let error = oauth::google_exchange(
        Some(&pool),
        &refuse,
        REDIRECT,
        &request(&unique_state("rf")),
    )
    .await
    .unwrap_err();
    assert_eq!(error.code(), "invalid_or_expired_token");

    let unconfigured = FakeGoogle {
        mode: Mutex::new(FakeMode::Unconfigured),
    };
    let error = oauth::google_exchange(
        Some(&pool),
        &unconfigured,
        REDIRECT,
        &request(&unique_state("uc")),
    )
    .await
    .unwrap_err();
    assert_eq!(error.code(), "dependency_unavailable");

    // Nothing was created by either failure.
    let (users,): (i64,) = sqlx::query_as("SELECT count(*) FROM users")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(users, 0);
}

#[tokio::test]
#[ignore = "needs an isolated PostgreSQL (NEXORA_AUTH_TEST_DATABASE_URL)"]
async fn google_rate_window_blocks_excess_attempts() {
    use nexora_auth::rate_limit::{self, CountMode, RateLimitRule};

    let pool = test_pool().await;
    let rule = RateLimitRule {
        max_failures: 2,
        window_secs: 900,
    };
    let key = rate_limit::ip_key("203.0.113.9");

    for _ in 0..2 {
        rate_limit::check(&pool, "google", &key, rule, CountMode::AllAttempts)
            .await
            .unwrap();
        rate_limit::record(&pool, "google", &key, "failure").await;
    }
    let retry_after = rate_limit::check(&pool, "google", &key, rule, CountMode::AllAttempts)
        .await
        .unwrap_err();
    assert_eq!(retry_after, 900);
}

#[tokio::test]
#[ignore = "needs an isolated PostgreSQL (NEXORA_AUTH_TEST_DATABASE_URL)"]
async fn registration_login_session_regressions_still_work() {
    // 23C–23E smoke through the full path.
    let pool = test_pool().await;
    let email = create_verified_user(&pool, "regression").await;
    let outcome = nexora_auth::login::login(
        Some(&pool),
        session::DEFAULT_SESSION_TTL_SECS,
        &nexora_auth::login::LoginRequest {
            email,
            password: "correct horse battery".to_string(),
        },
        None,
        None,
    )
    .await
    .unwrap();
    nexora_auth::session::revoke_family(&pool, &outcome.session.raw_token, 604_800)
        .await
        .unwrap();
    let (live,): (i64,) = sqlx::query_as("SELECT count(*) FROM sessions WHERE revoked_at IS NULL")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(live, 0);
}
