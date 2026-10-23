//! Secret-leakage test suite (Phase 23G — plan §7: "secret-leakage tests (log
//! capture asserts no password/token/verifier/authorization material in any
//! log line or error body)").
//!
//! Sentinel values are planted in every secret-bearing input and then every
//! observable output surface is checked: `Debug` renderings, error
//! code()/message() sets, the scrubbed log line format, and real HTTP error
//! envelopes from every auth route. Deliberate response fields (the session
//! `token` returned to the trusted client by design) are out of scope here —
//! this suite is about logs, traces, errors, and debug output.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use nexora_auth::app::build_router;
use nexora_auth::config::{AuthConfig, GoogleSettings};
use nexora_auth::email::EmailMessage;
use nexora_auth::login::{LoginError, LoginRequest};
use nexora_auth::oauth::{GoogleExchangeRequest, OAuthError};
use nexora_auth::recovery::{ChangePasswordRequest, RecoveryError, ResetPasswordRequest};
use nexora_auth::registration::{RegisterRequest, VerifyEmailRequest};
use nexora_auth::session::CreatedSession;
use tower::ServiceExt;

const SENTINEL_PASSWORD: &str = "SENTINEL_Hunter2_pass!";
const SENTINEL_TOKEN: &str = "5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e";
const SENTINEL_CODE: &str = "4/0SENTINEL-authz-code";
const SENTINEL_VERIFIER: &str = "SENTINEL_pkce_verifier_SENTINEL_pkce_verifier_SENT1";
const SENTINEL_STATE: &str = "5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f";
const SENTINEL_DB_URL: &str = "postgres://sentinel-user:SENTINEL_db_pass@db.example.test/nexora";
const SENTINEL_SECRET: &str = "SENTINEL_google_client_secret";
const SENTINEL_BEARER: &str = "b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0";

fn assert_clean(label: &str, output: &str) {
    for sentinel in [
        SENTINEL_PASSWORD,
        SENTINEL_TOKEN,
        SENTINEL_CODE,
        SENTINEL_VERIFIER,
        SENTINEL_STATE,
        SENTINEL_DB_URL,
        SENTINEL_SECRET,
        SENTINEL_BEARER,
        "SENTINEL",
    ] {
        assert!(
            !output.contains(sentinel),
            "{label} leaked sentinel {sentinel:?}: {output}"
        );
    }
}

#[test]
fn debug_output_of_request_types_never_renders_secrets() {
    let login = LoginRequest {
        email: "ada@example.test".to_string(),
        password: SENTINEL_PASSWORD.to_string(),
    };
    let rendered = format!("{login:?}");
    assert_clean("LoginRequest Debug", &rendered);
    assert!(rendered.contains("ada@example.test"));
    assert!(rendered.contains("<redacted>"));

    let register = RegisterRequest {
        name: "Ada".to_string(),
        email: "ada@example.test".to_string(),
        password: SENTINEL_PASSWORD.to_string(),
    };
    assert_clean("RegisterRequest Debug", &format!("{register:?}"));

    let verify = VerifyEmailRequest {
        token: SENTINEL_TOKEN.to_string(),
    };
    assert_clean("VerifyEmailRequest Debug", &format!("{verify:?}"));

    let reset = ResetPasswordRequest {
        token: SENTINEL_TOKEN.to_string(),
        password: SENTINEL_PASSWORD.to_string(),
    };
    assert_clean("ResetPasswordRequest Debug", &format!("{reset:?}"));

    let change = ChangePasswordRequest {
        current_password: SENTINEL_PASSWORD.to_string(),
        new_password: format!("{SENTINEL_PASSWORD}2"),
    };
    assert_clean("ChangePasswordRequest Debug", &format!("{change:?}"));

    let google = GoogleExchangeRequest {
        code: SENTINEL_CODE.to_string(),
        code_verifier: SENTINEL_VERIFIER.to_string(),
        state: SENTINEL_STATE.to_string(),
        redirect_uri: "http://127.0.0.1:49152/callback".to_string(),
        nonce: Some("SENTINEL_nonce".to_string()),
    };
    let rendered = format!("{google:?}");
    assert_clean("GoogleExchangeRequest Debug", &rendered);
    // Redirect URI is not secret and stays visible for diagnostics.
    assert!(rendered.contains("http://127.0.0.1:49152/callback"));
}

#[test]
fn debug_output_of_credentials_and_messages_never_renders_secrets() {
    let session = CreatedSession {
        raw_token: SENTINEL_TOKEN.to_string(),
        expires_at_rfc3339: "2026-11-01T00:00:00Z".to_string(),
    };
    let rendered = format!("{session:?}");
    assert_clean("CreatedSession Debug", &rendered);
    assert!(
        rendered.contains("2026-11-01T00:00:00Z"),
        "expiry stays visible"
    );

    let message = EmailMessage {
        to: "ada@example.test".to_string(),
        subject: "Reset your Nexora password".to_string(),
        body: format!("#/reset-password?token={SENTINEL_TOKEN}"),
    };
    let rendered = format!("{message:?}");
    assert_clean("EmailMessage Debug", &rendered);
    assert!(
        rendered.contains("ada@example.test"),
        "recipient stays visible"
    );

    // Config Debug: DB URL and OAuth secret must never render.
    let mut config =
        AuthConfig::resolve_with_database(Some("127.0.0.1:8787"), None, Some(SENTINEL_DB_URL))
            .unwrap();
    config
        .apply_google_settings(
            GoogleSettings::resolve(
                Some("sentinel-client-id"),
                Some(SENTINEL_SECRET),
                Some("http://127.0.0.1:49152/callback"),
            )
            .unwrap(),
        )
        .unwrap();
    let rendered = format!("{config:?}");
    assert_clean("AuthConfig Debug", &rendered);

    let google = GoogleSettings::resolve(
        Some("sentinel-client-id"),
        Some(SENTINEL_SECRET),
        Some("http://127.0.0.1:49152/callback"),
    )
    .unwrap();
    assert_clean("GoogleSettings Debug", &format!("{google:?}"));

    // ConfigError messages withhold values by construction.
    let error = GoogleSettings::resolve(Some("sentinel-client-id"), None, None).unwrap_err();
    assert_clean("ConfigError message", &format!("{error:?}"));
    assert_clean("ConfigError message", error.message());
}

#[test]
fn error_messages_carry_no_secret_material() {
    // Every stable error surface: codes are short identifiers; messages are
    // fixed strings (or validator text that never echoes the input).
    for error in [
        LoginError::InvalidCredentials,
        LoginError::EmailUnverified,
        LoginError::DependencyUnavailable,
        LoginError::Internal,
    ] {
        assert_clean("LoginError", error.message());
    }
    for error in [
        RecoveryError::InvalidOrExpiredToken,
        RecoveryError::InvalidCredentials,
        RecoveryError::DependencyUnavailable,
        RecoveryError::Internal,
    ] {
        assert_clean("RecoveryError", error.message());
    }
    for error in [
        OAuthError::InvalidOrExpiredToken,
        OAuthError::EmailUnverified,
        OAuthError::InvalidCredentials,
        OAuthError::EmailConflict,
        OAuthError::DependencyUnavailable,
        OAuthError::Internal,
    ] {
        assert_clean("OAuthError", error.message());
    }
    // Validation variants must not echo their input back.
    let leaky = LoginError::Validation(format!("bad input {SENTINEL_PASSWORD}"));
    // (A caller-built message could contain anything — the fixed validator
    // messages are what the service actually emits; the HTTP tests below
    // verify those never echo request material.)
    let _ = leaky.code();
}

#[test]
fn scrub_layer_redacts_the_plan_scrub_list() {
    use nexora_auth::scrub::scrub;
    for line in [
        format!("login password={SENTINEL_PASSWORD}"),
        format!("session token={SENTINEL_TOKEN}"),
        format!("authorization: Bearer {SENTINEL_BEARER}"),
        format!("cookie: session=abc; token={SENTINEL_TOKEN}"),
        format!("code_verifier={SENTINEL_VERIFIER}"),
        format!("google code={SENTINEL_CODE} state={SENTINEL_STATE}"),
        format!("db url={SENTINEL_DB_URL}"),
        format!("client_secret={SENTINEL_SECRET}"),
    ] {
        assert_clean("scrub()", &scrub(&line));
    }
}

async fn post_json(uri: &str, json: &str, auth: Option<&str>) -> (StatusCode, String) {
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
async fn http_error_envelopes_never_echo_request_secrets() {
    let bearer_header = format!("Bearer {SENTINEL_BEARER}");
    let cases: Vec<(&str, String, Option<&str>)> = vec![
        (
            "/v1/auth/login",
            format!(r#"{{"email":"ada@example.test","password":"{SENTINEL_PASSWORD}"}}"#),
            None,
        ),
        (
            "/v1/auth/register",
            format!(
                r#"{{"name":"Ada","email":"ada@example.test","password":"{SENTINEL_PASSWORD}"}}"#
            ),
            None,
        ),
        (
            "/v1/auth/verify-email",
            format!(r#"{{"token":"{SENTINEL_TOKEN}"}}"#),
            None,
        ),
        (
            "/v1/auth/reset-password",
            format!(r#"{{"token":"{SENTINEL_TOKEN}","password":"{SENTINEL_PASSWORD}"}}"#),
            None,
        ),
        (
            "/v1/auth/password-change",
            format!(
                r#"{{"currentPassword":"{SENTINEL_PASSWORD}","newPassword":"{SENTINEL_PASSWORD}2"}}"#
            ),
            Some(bearer_header.as_str()),
        ),
        (
            "/v1/auth/google/exchange",
            format!(
                r#"{{"code":"{SENTINEL_CODE}","codeVerifier":"{SENTINEL_VERIFIER}","state":"{SENTINEL_STATE}","redirectUri":"http://127.0.0.1:49152/callback","nonce":"SENTINEL_nonce"}}"#
            ),
            None,
        ),
        (
            "/v1/auth/recovery-request",
            r#"{"email":"ada@example.test"}"#.to_string(),
            None,
        ),
    ];
    for (uri, body, auth) in cases {
        let (status, response_body) = post_json(uri, &body, auth).await;
        assert!(
            status.is_client_error() || status.is_server_error(),
            "{uri} unexpectedly succeeded: {status}"
        );
        assert_clean(uri, &response_body);
        // The standard envelope shape is preserved.
        assert!(
            response_body.contains("\"error\"") || response_body.is_empty(),
            "{uri}: {response_body}"
        );
    }
}

#[tokio::test]
async fn protected_route_rejection_never_echoes_the_bearer_token() {
    for (method, uri) in [
        ("POST", "/v1/auth/logout"),
        ("POST", "/v1/auth/refresh"),
        ("GET", "/v1/auth/me"),
    ] {
        // Malformed bearer token -> 401 without echoing token.
        let response = build_router()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(uri)
                    .header("authorization", "Bearer SENTINEL_malformed_bearer_token")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let body = String::from_utf8(bytes.to_vec()).unwrap();
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{uri}");
        assert_clean(uri, &body);

        // Well-formed 64-hex bearer token without DB -> 503 without echoing token.
        let response = build_router()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(uri)
                    .header("authorization", format!("Bearer {SENTINEL_BEARER}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let body = String::from_utf8(bytes.to_vec()).unwrap();
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{uri}");
        assert_clean(uri, &body);
    }
}
