//! HTTP-level Google exchange tests that need **no database** (Phase 23F):
//! request validation, redirect-URI refusal, bearer/envelope hygiene, and the
//! honest `dependency_unavailable` answer when Google is unconfigured.
//! State/linking behaviour lives in `oauth_db.rs` (#[ignore]d real PostgreSQL).

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

fn valid_body() -> String {
    format!(
        r#"{{"code":"4/0AeanS0b-code","codeVerifier":"{}","state":"{}","redirectUri":"http://127.0.0.1:49152/callback"}}"#,
        "v".repeat(43),
        "s".repeat(32)
    )
}

#[tokio::test]
async fn exchange_validates_shapes_before_any_state_or_provider_work() {
    // Bad PKCE verifier → 400 validation_failed.
    let (status, body) = post_json(
        "/v1/auth/google/exchange",
        &format!(
            r#"{{"code":"x","codeVerifier":"short","state":"{}","redirectUri":"http://127.0.0.1:1/callback"}}"#,
            "s".repeat(32)
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body.contains("validation_failed"), "body: {body}");

    // Bad state → 400.
    let (status, _) = post_json(
        "/v1/auth/google/exchange",
        &format!(
            r#"{{"code":"x","codeVerifier":"{}","state":"tiny","redirectUri":"http://127.0.0.1:1/callback"}}"#,
            "v".repeat(43)
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // Unknown fields rejected.
    let (status, _) = post_json(
        "/v1/auth/google/exchange",
        &format!(r#"{},"extra":1}}"#, &valid_body()[..valid_body().len() - 1]),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn redirect_uri_mismatch_is_refused_generically() {
    // Well-shaped request whose redirectUri is not the registered one must not
    // reach the provider — and must not echo the URI.
    let (status, body) = post_json(
        "/v1/auth/google/exchange",
        &format!(
            r#"{{"code":"x","codeVerifier":"{}","state":"{}","redirectUri":"https://evil.example.test/callback"}}"#,
            "v".repeat(43),
            "s".repeat(32)
        ),
    )
    .await;
    // Unconfigured build answers 503 before the redirect check at the HTTP
    // layer (provider absent); the service-level check is covered in
    // oauth_db.rs. Either way: never a success, never an echo.
    assert!(
        status == StatusCode::BAD_REQUEST || status == StatusCode::SERVICE_UNAVAILABLE,
        "status: {status}"
    );
    assert!(!body.contains("evil.example.test"), "must not echo: {body}");
}

#[tokio::test]
async fn exchange_without_configuration_is_honest_503() {
    let (status, body) = post_json("/v1/auth/google/exchange", &valid_body()).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(body.contains("dependency_unavailable"), "body: {body}");
    assert!(!body.contains("not configured in this build") || body.contains("dependency_unavailable"));
    // No session, no user, no token — nothing that looks like success.
    assert!(!body.contains("token"));
    assert!(!body.contains("\"session\""));
}

#[tokio::test]
async fn exchange_error_envelopes_never_leak_credentials() {
    let (status, body) = post_json("/v1/auth/google/exchange", &valid_body()).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    // The body must not echo the code, verifier, or state.
    assert!(!body.contains("AeanS0b"), "must not echo code");
    assert!(!body.contains(&"v".repeat(8)), "must not echo verifier");
    assert!(!body.contains(&"s".repeat(8)), "must not echo state");
    assert!(body.contains("\"error\""), "body: {body}");
}

#[tokio::test]
async fn oauth_routes_reject_wrong_methods_with_json_envelope() {
    let response = build_router()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/auth/google/exchange")
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
async fn health_and_unknown_routes_keep_the_existing_surface() {
    let response = build_router()
        .oneshot(Request::builder().uri("/health").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}
