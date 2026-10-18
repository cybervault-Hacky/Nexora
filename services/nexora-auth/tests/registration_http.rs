//! HTTP-level registration/verification tests that need **no database**
//! (Phase 23C). They exercise request validation, the honest
//! `dependency_unavailable` behaviour of the unconfigured service, and the
//! enumeration-safe response shapes. Real-database behaviour lives in
//! `registration_db.rs` (#[ignore]d until an isolated PostgreSQL exists).

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use nexora_auth::app::build_router;
use tower::ServiceExt;

async fn post_json(uri: &str, json: &str) -> (StatusCode, String) {
    let response = build_router()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(uri)
                .header("content-type", "application/json")
                .body(Body::from(json.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8(bytes.to_vec()).unwrap())
}

#[tokio::test]
async fn register_rejects_malformed_json_with_json_envelope() {
    let (status, body) = post_json("/v1/auth/register", "{not json").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body.contains("validation_failed"), "body: {body}");
}

#[tokio::test]
async fn register_rejects_unknown_fields_and_missing_fields() {
    let (status, body) = post_json(
        "/v1/auth/register",
        r#"{"name":"Ada","email":"ada@example.test","password":"longenough1","extra":1}"#,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body.contains("validation_failed"), "body: {body}");

    let (status, body) = post_json("/v1/auth/register", r#"{"name":"Ada"}"#).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body.contains("validation_failed"), "body: {body}");
}

#[tokio::test]
async fn register_rejects_invalid_inputs_without_echoing_them() {
    // Bad email shape.
    let (status, body) = post_json(
        "/v1/auth/register",
        r#"{"name":"Ada","email":"not-an-email","password":"longenough1"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body.contains("validation_failed"));
    assert!(!body.contains("not-an-email"), "must not echo input: {body}");

    // Password policy (too short) — message must not contain the password.
    let (status, body) = post_json(
        "/v1/auth/register",
        r#"{"name":"Ada","email":"ada@example.test","password":"short"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body.contains("validation_failed"));
    assert!(!body.contains("short"), "must not echo password: {body}");

    // Blank name.
    let (status, _) = post_json(
        "/v1/auth/register",
        r#"{"name":"   ","email":"ada@example.test","password":"longenough1"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn register_without_database_is_honest_503() {
    let (status, body) = post_json(
        "/v1/auth/register",
        r#"{"name":"Ada","email":"ada@example.test","password":"longenough1"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(body.contains("dependency_unavailable"), "body: {body}");
    assert!(body.contains("not configured"), "body: {body}");
    // No fake success, no fake verification claim.
    assert!(!body.contains("verification_pending"));
}

#[tokio::test]
async fn verify_email_validates_then_reports_dependency_honestly() {
    // Missing token field.
    let (status, body) = post_json("/v1/auth/verify-email", r#"{}"#).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body.contains("validation_failed"), "body: {body}");

    // Empty token.
    let (status, body) = post_json("/v1/auth/verify-email", r#"{"token":"  "}"#).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body.contains("validation_failed"), "body: {body}");

    // Malformed token shape → generic invalid_or_expired_token (no oracle).
    let (status, body) = post_json("/v1/auth/verify-email", r#"{"token":"garbage"}"#).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body.contains("invalid_or_expired_token"), "body: {body}");
    assert!(!body.contains("garbage"), "must not echo token: {body}");

    // Well-formed token without a database → honest 503.
    let (status, body) = post_json(
        "/v1/auth/verify-email",
        r#"{"token":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(body.contains("dependency_unavailable"), "body: {body}");
}

#[tokio::test]
async fn resend_validates_email_and_reports_dependency_honestly() {
    let (status, body) = post_json("/v1/auth/resend-verification", r#"{"email":"nope"}"#).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body.contains("validation_failed"), "body: {body}");
    assert!(!body.contains("nope"), "must not echo input: {body}");

    let (status, body) = post_json(
        "/v1/auth/resend-verification",
        r#"{"email":"ada@example.test"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(body.contains("dependency_unavailable"), "body: {body}");
}

#[tokio::test]
async fn auth_routes_reject_wrong_methods_with_json_envelope() {
    let response = build_router()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/auth/register")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body = String::from_utf8(bytes.to_vec()).unwrap();
    assert!(body.contains("method_not_allowed"), "body: {body}");
}

#[tokio::test]
async fn health_still_works_alongside_auth_routes() {
    let response = build_router()
        .oneshot(Request::builder().uri("/health").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(
        String::from_utf8(bytes.to_vec()).unwrap(),
        "{\"service\":\"nexora-auth\",\"status\":\"ok\"}"
    );
}
