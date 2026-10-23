//! HTTP-level recovery/reset tests that need **no database** (Phase 23E):
//! request validation, generic token errors, bearer enforcement, and honest
//! `dependency_unavailable` behaviour. Real-database behaviour lives in
//! `recovery_db.rs` (#[ignore]d until an isolated PostgreSQL exists).

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

async fn post_json_auth(uri: &str, json: &str, auth: Option<&str>) -> (StatusCode, String) {
    let mut builder = Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json");
    if let Some(value) = auth {
        builder = builder.header("authorization", value);
    }
    let response = build_router()
        .oneshot(builder.body(Body::from(json.to_string())).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8(bytes.to_vec()).unwrap())
}

#[tokio::test]
async fn recovery_request_validates_email_without_echoing_it() {
    let (status, body) = post_json("/v1/auth/recovery-request", r#"{"email":"nope"}"#).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body.contains("validation_failed"), "body: {body}");
    assert!(!body.contains("nope"), "must not echo input: {body}");

    let (status, _) = post_json("/v1/auth/recovery-request", r#"{"email":123}"#).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (status, _) = post_json(
        "/v1/auth/recovery-request",
        r#"{"email":"a@b.test","extra":true}"#,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn recovery_request_without_database_is_honest_503() {
    let (status, body) = post_json(
        "/v1/auth/recovery-request",
        r#"{"email":"ada@example.test"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(body.contains("dependency_unavailable"), "body: {body}");
    // No delivery claim of any kind in the failure.
    assert!(!body.contains("on its way"));
    assert!(!body.contains("accepted"));
}

#[tokio::test]
async fn reset_password_validates_before_touching_any_token_store() {
    // Malformed token → 400 even with no database (validation runs first).
    let (status, body) = post_json(
        "/v1/auth/reset-password",
        r#"{"token":"not-a-token","password":"long enough password"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body.contains("validation_failed"), "body: {body}");

    // Short password → 400 with the existing policy text.
    let (status, body) = post_json(
        "/v1/auth/reset-password",
        &format!(r#"{{"token":"{}","password":"short"}}"#, "a".repeat(64)),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body.contains("at least 8"), "body: {body}");
}

#[tokio::test]
async fn reset_password_without_database_is_honest_503() {
    let (status, body) = post_json(
        "/v1/auth/reset-password",
        &format!(
            r#"{{"token":"{}","password":"long enough password"}}"#,
            "a".repeat(64)
        ),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(body.contains("dependency_unavailable"), "body: {body}");
}

#[tokio::test]
async fn password_change_requires_a_bearer_token() {
    let (status, body) = post_json_auth(
        "/v1/auth/password-change",
        r#"{"currentPassword":"old password value","newPassword":"new password value"}"#,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(body.contains("invalid_or_expired_token"), "body: {body}");

    let (status, _) = post_json_auth(
        "/v1/auth/password-change",
        r#"{"currentPassword":"old password value","newPassword":"new password value"}"#,
        Some("Basic ZHVtbXk="),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn password_change_validates_input_before_the_database() {
    // Empty current password → 400 (shape validation precedes dependency use).
    let (status, body) = post_json_auth(
        "/v1/auth/password-change",
        r#"{"currentPassword":"","newPassword":"new password value"}"#,
        Some("Bearer aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body.contains("validation_failed"), "body: {body}");

    // Reused password → 400.
    let (status, body) = post_json_auth(
        "/v1/auth/password-change",
        r#"{"currentPassword":"same password value","newPassword":"same password value"}"#,
        Some("Bearer aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body.contains("different"), "body: {body}");
}

#[tokio::test]
async fn password_change_without_database_is_honest_503() {
    let (status, body) = post_json_auth(
        "/v1/auth/password-change",
        r#"{"currentPassword":"old password value","newPassword":"new password value"}"#,
        Some("Bearer aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(body.contains("dependency_unavailable"), "body: {body}");
}

#[tokio::test]
async fn recovery_error_envelopes_never_leak_secrets() {
    for (uri, json) in [
        (
            "/v1/auth/recovery-request",
            r#"{"email":"ada@example.test"}"#,
        ),
        (
            "/v1/auth/reset-password",
            r#"{"token":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","password":"hunter2hunter2"}"#,
        ),
    ] {
        let (status, body) = post_json(uri, json).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{uri}");
        assert!(!body.contains("hunter2"), "{uri} must not echo password");
        assert!(!body.contains("bbbb"), "{uri} must not echo token");
        assert!(body.contains("\"error\""), "{uri}: {body}");
    }
}
