//! HTTP-level login/session tests that need **no database** (Phase 23D):
//! request validation, generic credential errors, bearer enforcement, and the
//! honest `dependency_unavailable` behaviour. Real-database behaviour lives in
//! `login_db.rs` (#[ignore]d until an isolated PostgreSQL exists).

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

async fn request_with_auth(method: &str, uri: &str, auth: Option<&str>) -> (StatusCode, String) {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(value) = auth {
        builder = builder.header("authorization", value);
    }
    let response = build_router()
        .oneshot(builder.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8(bytes.to_vec()).unwrap())
}

#[tokio::test]
async fn login_validates_body_without_echoing_inputs() {
    let (status, body) = post_json("/v1/auth/login", r#"{"email":"ada@example.test"}"#).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body.contains("validation_failed"), "body: {body}");

    let (status, body) = post_json(
        "/v1/auth/login",
        r#"{"email":"not-an-email","password":"whatever"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body.contains("validation_failed"), "body: {body}");
    assert!(
        !body.contains("not-an-email"),
        "must not echo input: {body}"
    );

    // Empty password is validation, not a credential answer.
    let (status, body) = post_json(
        "/v1/auth/login",
        r#"{"email":"ada@example.test","password":""}"#,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body.contains("validation_failed"), "body: {body}");

    // Unknown fields rejected.
    let (status, _) = post_json(
        "/v1/auth/login",
        r#"{"email":"ada@example.test","password":"whatever","admin":true}"#,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn login_without_database_is_honest_503() {
    let (status, body) = post_json(
        "/v1/auth/login",
        r#"{"email":"ada@example.test","password":"whatever"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(body.contains("dependency_unavailable"), "body: {body}");
    // No fake session, no fake login success.
    assert!(!body.contains("token"));
    assert!(!body.contains("verification_pending"));
}

#[tokio::test]
async fn protected_routes_require_a_bearer_token() {
    for (method, uri) in [
        ("POST", "/v1/auth/logout"),
        ("POST", "/v1/auth/refresh"),
        ("GET", "/v1/auth/me"),
    ] {
        let (status, body) = request_with_auth(method, uri, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{uri}");
        assert!(body.contains("invalid_or_expired_token"), "{uri}: {body}");

        // Malformed schemes are rejected the same way.
        let (status, body) =
            request_with_auth(method, uri, Some("Basic YWRhQGV4YW1wbGUudGVzdA==")).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{uri}");
        assert!(body.contains("invalid_or_expired_token"), "{uri}: {body}");

        let (status, _) = request_with_auth(method, uri, Some("Bearer    ")).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{uri}");
    }
}

#[tokio::test]
async fn protected_routes_without_database_are_honest_503() {
    for (method, uri) in [
        ("POST", "/v1/auth/logout"),
        ("POST", "/v1/auth/refresh"),
        ("GET", "/v1/auth/me"),
    ] {
        let (status, body) = request_with_auth(
            method,
            uri,
            Some("Bearer aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
        )
        .await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{uri}");
        assert!(body.contains("dependency_unavailable"), "{uri}: {body}");
    }
}

#[tokio::test]
async fn login_routes_reject_wrong_methods_with_json_envelope() {
    let (status, body) = request_with_auth("GET", "/v1/auth/login", None).await;
    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
    assert!(body.contains("method_not_allowed"), "body: {body}");
}

#[tokio::test]
async fn health_and_unknown_routes_keep_phase23a_surface() {
    let (status, body) = request_with_auth("GET", "/health", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, "{\"service\":\"nexora-auth\",\"status\":\"ok\"}");

    let (status, body) = request_with_auth("GET", "/v1/auth/unknown", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(body.contains("not_found"), "body: {body}");
}

#[tokio::test]
async fn login_error_envelope_never_leaks_credentials_or_hash_material() {
    let (status, body) = post_json(
        "/v1/auth/login",
        r#"{"email":"ada@example.test","password":"hunter2hunter2"}"#,
    )
    .await;
    // 503 here (no DB) — but the shape must be the standard envelope and must
    // not echo the password.
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(!body.contains("hunter2"), "must not echo password: {body}");
    assert!(body.contains("\"error\""), "body: {body}");
}
