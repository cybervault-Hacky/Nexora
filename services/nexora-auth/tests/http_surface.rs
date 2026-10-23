//! In-process HTTP-surface tests for the nexora-auth service foundation.
//!
//! These use `tower::ServiceExt::oneshot` against the router directly: no
//! sockets, network, database, email, OAuth, or cloud services involved.

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use http_body_util::BodyExt;
use nexora_auth::app::{build_router, HealthResponse};
use tower::ServiceExt;

const HEALTH_BODY: &str = "{\"service\":\"nexora-auth\",\"status\":\"ok\"}";
const NOT_FOUND_BODY: &str =
    "{\"error\":{\"code\":\"not_found\",\"message\":\"No route matches this path.\"}}";
const METHOD_NOT_ALLOWED_BODY: &str =
    "{\"error\":{\"code\":\"method_not_allowed\",\"message\":\"This route does not accept this HTTP method.\"}}";

async fn body_string(response: axum::response::Response) -> String {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    String::from_utf8(bytes.to_vec()).unwrap()
}

async fn get(app: axum::Router, uri: &str) -> axum::response::Response {
    app.oneshot(
        Request::builder()
            .method(Method::GET)
            .uri(uri)
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn health_returns_exact_deterministic_json() {
    let response = get(build_router(), "/health").await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get(header::CONTENT_TYPE).unwrap(),
        "application/json"
    );
    assert_eq!(body_string(response).await, HEALTH_BODY);
}

#[tokio::test]
async fn health_payload_matches_typed_struct() {
    // The exact structure contract: service identity + process status only.
    let value = serde_json::to_value(HealthResponse::healthy()).unwrap();
    assert_eq!(
        value,
        serde_json::json!({ "service": "nexora-auth", "status": "ok" })
    );
    // It must not claim that auth, database, email, or OAuth work.
    let text = value.to_string();
    for forbidden in ["auth_ok", "database", "email", "oauth", "session"] {
        assert!(!text.contains(forbidden), "unexpected claim: {forbidden}");
    }
}

#[tokio::test]
async fn unknown_routes_return_predictable_404() {
    for uri in ["/", "/nope", "/v1/auth/unknown", "/health/extra"] {
        let response = get(build_router(), uri).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND, "uri: {uri}");
        assert_eq!(body_string(response).await, NOT_FOUND_BODY, "uri: {uri}");
    }
}

#[tokio::test]
async fn unsupported_methods_return_predictable_405() {
    let app = build_router();
    for method in [Method::POST, Method::PUT, Method::DELETE, Method::PATCH] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(method.clone())
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            StatusCode::METHOD_NOT_ALLOWED,
            "method: {method}"
        );
        assert_eq!(body_string(response).await, METHOD_NOT_ALLOWED_BODY);
    }
}

#[tokio::test]
async fn error_bodies_are_secret_free_and_generic() {
    let app = build_router();
    let not_found = body_string(get(app.clone(), "/does-not-exist").await).await;
    let method = body_string(
        app.oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap(),
    )
    .await;

    for body in [&not_found, &method] {
        for forbidden in [
            "services/",
            "src/",
            ".rs",
            "NEXORA_",
            "stack",
            "panic",
            "/home/",
            "environment",
            "secret",
            "token",
            "password",
        ] {
            assert!(!body.contains(forbidden), "leaky body: {body}");
        }
    }
}

#[tokio::test]
async fn health_ignores_request_bodies() {
    // Nothing reads request bodies; oversized ones are capped by the body
    // limit and the health route still answers predictably.
    let response = build_router()
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/health")
                .body(Body::from("x".repeat(64)))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(body_string(response).await, HEALTH_BODY);
}
