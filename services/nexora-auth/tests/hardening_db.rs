//! REAL PostgreSQL hardening tests for Phase 23G (rate-limit backoff,
//! boundaries, and proxy-header handling). Run ONLY against a dedicated,
//! disposable test database:
//!
//! ```bash
//! createdb nexora_auth_test
//! NEXORA_AUTH_TEST_DATABASE_URL='postgres://user@127.0.0.1:5432/nexora_auth_test?sslmode=disable' \
//!   cargo test --test hardening_db -- --ignored
//! ```
//!
//! Every test is `#[ignore]`d. NEVER point this at production. The URL is read
//! from `NEXORA_AUTH_TEST_DATABASE_URL` only and is never printed.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use nexora_auth::app::{build_router_with_state, AppState};
use nexora_auth::email::UnconfiguredEmailProvider;
use nexora_auth::rate_limit::{
    self, escalated_retry_after, CountMode, RateLimitRule, MAX_BACKOFF_SECS,
};
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use tower::ServiceExt;

static MIGRATIONS: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

const TEST_URL_VAR: &str = "NEXORA_AUTH_TEST_DATABASE_URL";

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
    sqlx::query("TRUNCATE users CASCADE")
        .execute(&pool)
        .await
        .expect("truncate");
    pool
}

#[tokio::test]
#[ignore = "needs an isolated PostgreSQL (NEXORA_AUTH_TEST_DATABASE_URL)"]
async fn exponential_backoff_escalates_for_persistent_offenders() {
    let pool = test_pool().await;
    let rule = RateLimitRule { max_failures: 2, window_secs: 600 };
    let key = rate_limit::email_key("brute@example.test");

    // Exactly one exhausted window → plain window as Retry-After.
    for _ in 0..2 {
        rate_limit::record(&pool, "login", &key, "failure").await;
    }
    assert_eq!(
        rate_limit::check(&pool, "login", &key, rule, CountMode::FailuresOnly)
            .await
            .unwrap_err(),
        600
    );

    // Keep failing: three full windows' worth → exponential backoff
    // (600 << (3-1) = 2400), per plan §4 "exponential lockout".
    for _ in 0..4 {
        rate_limit::record(&pool, "login", &key, "failure").await;
    }
    let retry = rate_limit::check(&pool, "login", &key, rule, CountMode::FailuresOnly)
        .await
        .unwrap_err();
    assert!(retry >= 2_400, "expected escalation, got {retry}");
    assert!(retry <= MAX_BACKOFF_SECS, "backoff must stay bounded, got {retry}");

    // A calm key is unaffected.
    assert!(rate_limit::check(&pool, "login", &rate_limit::email_key("calm@example.test"), rule, CountMode::FailuresOnly)
        .await
        .is_ok());
}

#[tokio::test]
#[ignore = "needs an isolated PostgreSQL (NEXORA_AUTH_TEST_DATABASE_URL)"]
async fn window_boundary_blocks_exactly_at_the_limit() {
    let pool = test_pool().await;
    let rule = RateLimitRule { max_failures: 3, window_secs: 900 };
    let key = rate_limit::ip_key("203.0.113.7");

    rate_limit::record(&pool, "register", &key, "failure").await;
    rate_limit::record(&pool, "register", &key, "failure").await;
    // 2 of 3 → allowed.
    assert!(rate_limit::check(&pool, "register", &key, rule, CountMode::FailuresOnly).await.is_ok());
    rate_limit::record(&pool, "register", &key, "failure").await;
    // 3 of 3 → blocked with a Retry-After the handler turns into a header.
    let retry = rate_limit::check(&pool, "register", &key, rule, CountMode::FailuresOnly)
        .await
        .unwrap_err();
    assert!(retry >= 900);
}

#[tokio::test]
#[ignore = "needs an isolated PostgreSQL (NEXORA_AUTH_TEST_DATABASE_URL)"]
async fn forwarded_headers_are_never_used_for_rate_limit_keys() {
    // The plan defines NO trusted-proxy configuration, so forwarded headers
    // are untrusted by design: the key must come from the socket peer only.
    let pool = test_pool().await;
    let mut state = AppState::unconfigured();
    state.pool = Some(pool.clone());
    state.email = std::sync::Arc::new(UnconfiguredEmailProvider);
    let app = build_router_with_state(Arc::new(state));

    let peer: SocketAddr = "203.0.113.9:52000".parse().unwrap();
    let response = app
        .oneshot({
            let mut request = Request::builder()
                .method("POST")
                .uri("/v1/auth/login")
                .header("content-type", "application/json")
                // SPOOFED client IP — must be ignored.
                .header("x-forwarded-for", "10.99.99.99")
                .header("forwarded", "for=10.99.99.99")
                .body(Body::from(
                    r#"{"email":"ghost@example.test","password":"whatever-value"}"#,
                ))
                .unwrap();
            request.extensions_mut().insert(ConnectInfo(peer));
            request
        })
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    let (spoofed,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM login_attempts WHERE key = $1",
    )
    .bind(rate_limit::ip_key("10.99.99.99"))
    .fetch_one(&pool)
    .await
    .unwrap();
    let (peer_rows,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM login_attempts WHERE key = $1",
    )
    .bind(rate_limit::ip_key("203.0.113.9"))
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(spoofed, 0, "forwarded headers must never key the limiter");
    assert!(peer_rows > 0, "the socket peer must key the limiter");
}

#[tokio::test]
#[ignore = "needs an isolated PostgreSQL (NEXORA_AUTH_TEST_DATABASE_URL)"]
async fn rate_limited_response_carries_retry_after_and_generic_body() {
    let pool = test_pool().await;
    let mut state = AppState::unconfigured();
    state.pool = Some(pool.clone());
    state.email = std::sync::Arc::new(UnconfiguredEmailProvider);
    state.rate_limit_login = RateLimitRule { max_failures: 1, window_secs: 900 };
    let app = build_router_with_state(Arc::new(state));

    let peer: SocketAddr = "203.0.113.10:52000".parse().unwrap();
    let body = Body::from(r#"{"email":"brute@example.test","password":"first-wrong-pass"}"#);
    let mut request = Request::builder()
        .method("POST")
        .uri("/v1/auth/login")
        .header("content-type", "application/json")
        .body(body)
        .unwrap();
    request.extensions_mut().insert(ConnectInfo(peer));
    let first = app.clone().oneshot(request).await.unwrap();
    assert_eq!(first.status(), StatusCode::UNAUTHORIZED);

    let mut request = Request::builder()
        .method("POST")
        .uri("/v1/auth/login")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"email":"brute@example.test","password":"second-wrong-pass"}"#))
        .unwrap();
    request.extensions_mut().insert(ConnectInfo(peer));
    let second = app.oneshot(request).await.unwrap();
    assert_eq!(second.status(), StatusCode::TOO_MANY_REQUESTS);
    let retry_after = second
        .headers()
        .get("retry-after")
        .expect("Retry-After header (plan §4)")
        .to_str()
        .unwrap()
        .parse::<u64>()
        .unwrap();
    assert!(retry_after >= 900);
    let bytes = second.into_body().collect().await.unwrap().to_bytes();
    let body = String::from_utf8(bytes.to_vec()).unwrap();
    assert!(body.contains("rate_limited"), "body: {body}");
    assert!(!body.contains("second-wrong-pass"), "no echo");
}

#[tokio::test]
#[ignore = "needs an isolated PostgreSQL (NEXORA_AUTH_TEST_DATABASE_URL)"]
async fn escalated_retry_after_is_pure_and_bounded() {
    // DB-free logic re-checked in context: monotone growth, hard cap.
    let mut previous = 0;
    for windows in 0..24 {
        let value = escalated_retry_after(600, windows);
        assert!(value >= 600);
        assert!(value <= MAX_BACKOFF_SECS);
        if windows > 0 {
            assert!(value >= previous, "backoff never shrinks with more failures");
        }
        previous = value;
    }
    assert_eq!(escalated_retry_after(600, u64::MAX), MAX_BACKOFF_SECS);
}
