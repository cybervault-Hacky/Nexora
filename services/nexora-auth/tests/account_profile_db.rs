//! Real PostgreSQL + Axum integration coverage for the authenticated account
//! profile contract. Run only against a dedicated, disposable database whose
//! name ends in `_test`; this test never truncates or deletes existing rows.
//!
//! ```bash
//! createdb nexora_auth_test
//! NEXORA_AUTH_TEST_DATABASE_URL='postgres://user@127.0.0.1:5432/nexora_auth_test?sslmode=disable' \
//!   cargo test --manifest-path services/nexora-auth/Cargo.toml --test account_profile_db -- --ignored
//! ```
//!
//! Never point this test at production or a database containing user data.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use nexora_auth::app::{build_router_with_state, AppState};
use nexora_auth::session;
use serde_json::{json, Value};
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tower::ServiceExt;

const TEST_URL_VAR: &str = "NEXORA_AUTH_TEST_DATABASE_URL";
static UNIQUE: AtomicU64 = AtomicU64::new(0);

fn unique_email() -> String {
    let sequence = UNIQUE.fetch_add(1, Ordering::Relaxed);
    format!(
        "profile.{}.{}@example.test",
        std::process::id(),
        sequence
    )
}

async fn test_pool() -> PgPool {
    let url = std::env::var(TEST_URL_VAR)
        .unwrap_or_else(|_| panic!("{TEST_URL_VAR} must point to a disposable *_test database"));
    let pool = PgPoolOptions::new()
        .max_connections(3)
        .connect(&url)
        .await
        .unwrap_or_else(|_| panic!("could not connect to the dedicated profile test database"));

    // Refuse to apply migrations or write fixtures unless the connected
    // database name is explicitly marked as a test database.
    let database_name: String = sqlx::query_scalar("SELECT current_database()")
        .fetch_one(&pool)
        .await
        .expect("read current database name");
    assert!(
        database_name.ends_with("_test"),
        "account profile integration tests require a database name ending in `_test`"
    );

    nexora_auth::run_migrations(&pool)
        .await
        .expect("migrations must apply cleanly to the disposable test database");
    pool
}

async fn create_verified_fixture(pool: &PgPool, name: &str) -> String {
    let user_id: String = sqlx::query_scalar(
        "INSERT INTO users (display_name, email, password_hash, email_verified_at) \
         VALUES ($1, $2, NULL, now()) RETURNING id::text",
    )
    .bind(name)
    .bind(unique_email())
    .fetch_one(pool)
    .await
    .expect("insert synthetic profile fixture");
    user_id
}

async fn call_json(
    app: &axum::Router,
    method: &str,
    uri: &str,
    bearer: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut request = Request::builder().method(method).uri(uri);
    if let Some(token) = bearer {
        request = request.header("authorization", format!("Bearer {token}"));
    }
    let raw_body = match body {
        Some(value) => {
            request = request.header("content-type", "application/json");
            Body::from(value.to_string())
        }
        None => Body::empty(),
    };
    let response = app
        .clone()
        .oneshot(request.body(raw_body).expect("build test request"))
        .await
        .expect("run in-process router");
    let status = response.status();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("read response body")
        .to_bytes();
    if bytes.is_empty() {
        (status, Value::Null)
    } else {
        (
            status,
            serde_json::from_slice(&bytes).expect("response must be JSON"),
        )
    }
}

#[tokio::test]
#[ignore = "requires an isolated PostgreSQL database whose name ends in _test"]
async fn profile_uses_authenticated_user_and_rejects_unapproved_or_invalid_operations() {
    let pool = test_pool().await;
    let user_id = create_verified_fixture(&pool, "Profile Fixture").await;
    let other_user_id = create_verified_fixture(&pool, "Other Profile Fixture").await;
    sqlx::query(
        "INSERT INTO oauth_accounts (user_id, provider, provider_subject) \
         VALUES (CAST($1 AS uuid), 'google', $2)",
    )
    .bind(&user_id)
    .bind(format!("google-profile-fixture-{}", UNIQUE.fetch_add(1, Ordering::Relaxed)))
    .execute(&pool)
    .await
    .expect("insert synthetic Google provider link");

    let session = session::create_session(
        &pool,
        &user_id,
        session::DEFAULT_SESSION_TTL_SECS,
        None,
        None,
    )
    .await
    .expect("create a Nexora session for the verified fixture");

    let mut state = AppState::unconfigured();
    state.pool = Some(pool.clone());
    let app = build_router_with_state(Arc::new(state));

    // Neither missing credentials nor client-supplied account IDs authorize a profile read.
    let (anonymous_status, _) =
        call_json(&app, "GET", "/v1/account/profile", None, None).await;
    assert_eq!(anonymous_status, StatusCode::UNAUTHORIZED);

    let profile_uri = format!("/v1/account/profile?userId={other_user_id}");
    let (get_status, get_body) =
        call_json(&app, "GET", &profile_uri, Some(&session.raw_token), None).await;
    assert_eq!(get_status, StatusCode::OK);
    let profile = &get_body["profile"];
    assert_eq!(profile["id"], user_id);
    assert_eq!(profile["name"], "Profile Fixture");
    assert_eq!(profile["emailVerified"], true);
    assert_eq!(profile["hasPassword"], false);
    assert_eq!(profile["providers"], json!(["google"]));
    assert!(profile["createdAt"].as_str().is_some());
    assert!(profile.get("passwordHash").is_none());
    assert!(profile.get("sessionToken").is_none());

    // The allowed edit is display name only; the backend normalizes whitespace
    // and returns the authoritative profile, not a client-side identity claim.
    let (anonymous_patch_status, _) = call_json(
        &app,
        "PATCH",
        "/v1/account/profile",
        None,
        Some(json!({ "name": "Should Not Apply" })),
    )
    .await;
    assert_eq!(anonymous_patch_status, StatusCode::UNAUTHORIZED);

    let (patch_status, patched_body) = call_json(
        &app,
        "PATCH",
        "/v1/account/profile",
        Some(&session.raw_token),
        Some(json!({ "name": "  Ada Byron  " })),
    )
    .await;
    assert_eq!(patch_status, StatusCode::OK);
    assert_eq!(patched_body["profile"]["id"], user_id);
    assert_eq!(patched_body["profile"]["name"], "Ada Byron");
    assert_eq!(patched_body["profile"]["email"], profile["email"]);
    assert_eq!(patched_body["profile"]["providers"], json!(["google"]));

    let (forbidden_field_status, forbidden_field_body) = call_json(
        &app,
        "PATCH",
        "/v1/account/profile",
        Some(&session.raw_token),
        Some(json!({
            "name": "Cross-account attempt",
            "userId": other_user_id,
            "email": "attacker@example.test",
            "emailVerified": false,
            "providers": ["password"]
        })),
    )
    .await;
    assert_eq!(forbidden_field_status, StatusCode::BAD_REQUEST);
    assert_eq!(forbidden_field_body["error"]["code"], "validation_failed");

    let too_long_name = "x".repeat(81);
    for invalid_name in ["   ".to_string(), too_long_name, "Ada\nByron".to_string()] {
        let (status, body) = call_json(
            &app,
            "PATCH",
            "/v1/account/profile",
            Some(&session.raw_token),
            Some(json!({ "name": invalid_name })),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"]["code"], "validation_failed");
    }

    let (after_rejections_status, after_rejections_body) =
        call_json(&app, "GET", "/v1/account/profile", Some(&session.raw_token), None).await;
    assert_eq!(after_rejections_status, StatusCode::OK);
    assert_eq!(after_rejections_body["profile"]["id"], user_id);
    assert_eq!(after_rejections_body["profile"]["name"], "Ada Byron");
    assert_eq!(after_rejections_body["profile"]["email"], profile["email"]);

    // An expired bearer session is rejected on the protected profile route.
    let expired_token = session::generate_session_token();
    let expired_digest = session::hash_session_token(&expired_token);
    sqlx::query(
        "INSERT INTO sessions \
         (user_id, token_hash, family_id, created_at, expires_at, last_used_at) \
         VALUES (CAST($1 AS uuid), $2, gen_random_uuid(), \
                 now() - interval '2 days', now() - interval '1 hour', now() - interval '1 hour')",
    )
    .bind(&user_id)
    .bind(expired_digest)
    .execute(&pool)
    .await
    .expect("insert expired synthetic session");
    let (expired_status, expired_body) =
        call_json(&app, "GET", "/v1/account/profile", Some(&expired_token), None).await;
    assert_eq!(expired_status, StatusCode::UNAUTHORIZED);
    assert_eq!(expired_body["error"]["code"], "invalid_or_expired_token");

    // Logout revokes a live session family; that bearer can no longer read a profile.
    let revoked_session = session::create_session(
        &pool,
        &user_id,
        session::DEFAULT_SESSION_TTL_SECS,
        None,
        None,
    )
    .await
    .expect("create session to revoke");
    let (logout_status, _) = call_json(
        &app,
        "POST",
        "/v1/auth/logout",
        Some(&revoked_session.raw_token),
        None,
    )
    .await;
    assert_eq!(logout_status, StatusCode::NO_CONTENT);
    let (revoked_status, revoked_body) = call_json(
        &app,
        "GET",
        "/v1/account/profile",
        Some(&revoked_session.raw_token),
        None,
    )
    .await;
    assert_eq!(revoked_status, StatusCode::UNAUTHORIZED);
    assert_eq!(revoked_body["error"]["code"], "invalid_or_expired_token");

    // Account deletion is bound to the authenticated user, requires explicit
    // identity confirmation, and persists its audit record atomically with the
    // cascade. This fixture has no password hash and exercises that code path.
    let deletion_user_id = create_verified_fixture(&pool, "Disposable Delete Fixture").await;
    let deletion_email: String = sqlx::query_scalar("SELECT email FROM users WHERE id = $1::uuid")
        .bind(&deletion_user_id)
        .fetch_one(&pool)
        .await
        .expect("read disposable deletion fixture email");
    let deletion_session = session::create_session(
        &pool,
        &deletion_user_id,
        session::DEFAULT_SESSION_TTL_SECS,
        None,
        None,
    )
    .await
    .expect("create a session for the disposable deletion fixture");

    let (anonymous_delete_status, _) = call_json(
        &app,
        "POST",
        "/v1/account/delete",
        None,
        Some(json!({ "confirmEmail": deletion_email })),
    )
    .await;
    assert_eq!(anonymous_delete_status, StatusCode::UNAUTHORIZED);

    let (mismatch_status, mismatch_body) = call_json(
        &app,
        "POST",
        "/v1/account/delete",
        Some(&deletion_session.raw_token),
        Some(json!({ "confirmEmail": "different@example.test" })),
    )
    .await;
    assert_eq!(mismatch_status, StatusCode::BAD_REQUEST);
    assert_eq!(mismatch_body["error"]["code"], "validation_failed");
    let still_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM users WHERE id = $1::uuid)",
    )
    .bind(&deletion_user_id)
    .fetch_one(&pool)
    .await
    .expect("check account after rejected confirmation");
    assert!(still_exists);
    let audit_before_delete: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM audit_logs WHERE subject_id = $1 AND action = 'account_deleted'",
    )
    .bind(&deletion_user_id)
    .fetch_one(&pool)
    .await
    .expect("check that rejected deletion did not emit success audit");
    assert_eq!(audit_before_delete, 0);

    let (delete_status, _) = call_json(
        &app,
        "POST",
        "/v1/account/delete",
        Some(&deletion_session.raw_token),
        Some(json!({ "confirmEmail": deletion_email })),
    )
    .await;
    assert_eq!(delete_status, StatusCode::NO_CONTENT);

    let deleted_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM users WHERE id = $1::uuid)",
    )
    .bind(&deletion_user_id)
    .fetch_one(&pool)
    .await
    .expect("confirm account deletion");
    assert!(!deleted_exists);
    let deletion_audit: Option<(Option<String>, String, String)> = sqlx::query_as(
        "SELECT actor_user_id::text, action, outcome FROM audit_logs \
         WHERE subject_id = $1 AND action = 'account_deleted'",
    )
    .bind(&deletion_user_id)
    .fetch_optional(&pool)
    .await
    .expect("read the committed account-deletion audit record");
    assert_eq!(
        deletion_audit,
        Some((None, "account_deleted".to_string(), "ok".to_string()))
    );
    let (deleted_session_status, deleted_session_body) = call_json(
        &app,
        "GET",
        "/v1/account/profile",
        Some(&deletion_session.raw_token),
        None,
    )
    .await;
    assert_eq!(deleted_session_status, StatusCode::UNAUTHORIZED);
    assert_eq!(
        deleted_session_body["error"]["code"],
        "invalid_or_expired_token"
    );
}
