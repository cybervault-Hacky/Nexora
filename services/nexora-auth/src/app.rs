//! HTTP surface of the nexora-auth service.
//!
//! Phase 23A surface: `GET /health` reports **service** health only (process
//! is up and routing works) — it does NOT claim that authentication, a
//! database, email delivery, or OAuth is operational. Phase 23C adds
//! registration + email verification under `/v1/auth/*` (plan §6): endpoints
//! answer honestly (503 `dependency_unavailable`) when no database is
//! configured, and registration responses are enumeration-safe. Login,
//! sessions, recovery, and OAuth remain later steps.

use axum::extract::rejection::JsonRejection;
use axum::extract::{DefaultBodyLimit, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Serialize;

use crate::email::{EmailDelivery, EmailProvider, UnconfiguredEmailProvider};
use crate::registration::{
    self, RegisterRequest, ResendVerificationRequest, VerifyEmailRequest,
    DEFAULT_VERIFICATION_TTL_SECS,
};

/// Defence-in-depth: the health-only service accepts tiny request bodies at
/// most. Nothing reads bodies today; the limit keeps it that way under load.
pub const MAX_REQUEST_BODY_BYTES: usize = 4096;

/// Deterministic health payload. Field order fixes JSON key order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct HealthResponse {
    pub service: &'static str,
    pub status: &'static str,
}

impl HealthResponse {
    pub fn healthy() -> Self {
        Self {
            service: "nexora-auth",
            status: "ok",
        }
    }
}

#[derive(Serialize)]
struct ErrorBody {
    error: ErrorDetail,
}

#[derive(Serialize)]
struct ErrorDetail {
    code: &'static str,
    message: String,
}

/// Predictable error envelope: stable machine code + generic human message.
/// Bodies never include stack traces, filesystem paths, environment values,
/// or secrets.
fn error_response(status: StatusCode, code: &'static str, message: impl Into<String>) -> Response {
    (
        status,
        Json(ErrorBody {
            error: ErrorDetail {
                code,
                message: message.into(),
            },
        }),
    )
        .into_response()
}

/// Shared service state (Phase 23C–23D). `pool: None` means the account
/// database is not configured: routes stay mounted and answer honestly (503)
/// instead of pretending to work.
pub struct AppState {
    pub pool: Option<sqlx::PgPool>,
    pub email: std::sync::Arc<dyn EmailProvider>,
    /// Transport capability reported to clients — config-level, identical for
    /// every account (enumeration defence).
    pub email_delivery: EmailDelivery,
    pub verification_ttl_secs: u64,
    pub session_ttl_secs: u64,
    pub session_idle_ttl_secs: u64,
    pub reset_ttl_secs: u64,
    pub rate_limit_login: crate::rate_limit::RateLimitRule,
    pub rate_limit_register: crate::rate_limit::RateLimitRule,
    pub rate_limit_resend: crate::rate_limit::RateLimitRule,
    pub rate_limit_recovery: crate::rate_limit::RateLimitRule,
    /// Phase 23F: Google identity provider (None = unconfigured → honest 503).
    pub google_provider: Option<std::sync::Arc<dyn crate::oauth::GoogleProvider>>,
    /// Registered loopback redirect; exact-matched on every exchange.
    pub google_redirect_uri: Option<String>,
}

impl AppState {
    /// Honest defaults: no database, no mail. Used by `build_router()` so the
    /// Phase 23A health surface needs no dependencies.
    pub fn unconfigured() -> Self {
        Self {
            pool: None,
            email: std::sync::Arc::new(UnconfiguredEmailProvider),
            email_delivery: EmailDelivery::Unconfigured,
            verification_ttl_secs: DEFAULT_VERIFICATION_TTL_SECS,
            session_ttl_secs: crate::session::DEFAULT_SESSION_TTL_SECS,
            session_idle_ttl_secs: crate::session::DEFAULT_SESSION_IDLE_TTL_SECS,
            reset_ttl_secs: crate::recovery::DEFAULT_RESET_TTL_SECS,
            rate_limit_login: crate::rate_limit::default_login_rule(),
            rate_limit_register: crate::rate_limit::default_register_rule(),
            rate_limit_resend: crate::rate_limit::default_resend_rule(),
            rate_limit_recovery: crate::rate_limit::default_recovery_rule(),
            google_provider: None,
            google_redirect_uri: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Phase 23C — registration & email verification (plan §6 contracts)
// ---------------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RegisterResponse {
    status: &'static str,
    email_delivery: EmailDelivery,
    message: &'static str,
}

#[derive(Serialize)]
struct VerifyResponse {
    status: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ResendResponse {
    email_delivery: EmailDelivery,
    message: &'static str,
}

fn delivery_message(delivery: EmailDelivery) -> &'static str {
    match delivery {
        EmailDelivery::Configured => {
            "If this address can be registered, a verification email is on its way. Check your inbox."
        }
        EmailDelivery::Unconfigured => {
            "Email delivery is not configured on this server; no verification email was sent."
        }
    }
}

fn validation_rejection(rejection: JsonRejection) -> Response {
    let _ = rejection;
    error_response(
        StatusCode::BAD_REQUEST,
        "validation_failed",
        "Request body must be valid JSON with the documented fields.",
    )
}

async fn register_handler(
    State(state): State<std::sync::Arc<AppState>>,
    connect_info: Option<axum::extract::ConnectInfo<std::net::SocketAddr>>,
    body: Result<Json<RegisterRequest>, JsonRejection>,
) -> Response {
    let request = match body {
        Ok(Json(request)) => request,
        Err(rejection) => return validation_rejection(rejection),
    };
    // DB-backed sliding window (plan §4 credential-stuffing control).
    let register_key =
        crate::rate_limit::email_key(&crate::registration::normalize_email(&request.email));
    if let Err(response) = rate_check(
        &state,
        "register",
        &register_key,
        state.rate_limit_register,
        crate::rate_limit::CountMode::AllAttempts,
    )
    .await
    {
        return response;
    }
    if let Some(ip) = client_ip(connect_info) {
        if let Err(response) = rate_check(
            &state,
            "register",
            &crate::rate_limit::ip_key(&ip),
            state.rate_limit_register,
            crate::rate_limit::CountMode::AllAttempts,
        )
        .await
        {
            return response;
        }
    }
    let outcome = registration::register(
        state.pool.as_ref(),
        state.email.as_ref(),
        state.email_delivery,
        state.verification_ttl_secs,
        &request,
    )
    .await;
    match &outcome {
        Ok(_) => rate_record(&state, "register", &register_key, "success").await,
        Err(_) => rate_record(&state, "register", &register_key, "failure").await,
    }
    match outcome {
        Ok(_) => (
            StatusCode::ACCEPTED,
            Json(RegisterResponse {
                status: "verification_pending",
                email_delivery: state.email_delivery,
                message: delivery_message(state.email_delivery),
            }),
        )
            .into_response(),
        Err(error) => error_response(
            status_for(error.code()),
            error.code(),
            error.message().to_string(),
        ),
    }
}

async fn verify_email_handler(
    State(state): State<std::sync::Arc<AppState>>,
    body: Result<Json<VerifyEmailRequest>, JsonRejection>,
) -> Response {
    let request = match body {
        Ok(Json(request)) => request,
        Err(rejection) => return validation_rejection(rejection),
    };
    match registration::verify_email(state.pool.as_ref(), &request.token).await {
        Ok(()) => (StatusCode::OK, Json(VerifyResponse { status: "verified" })).into_response(),
        Err(error) => error_response(
            status_for(error.code()),
            error.code(),
            error.message().to_string(),
        ),
    }
}

async fn resend_verification_handler(
    State(state): State<std::sync::Arc<AppState>>,
    connect_info: Option<axum::extract::ConnectInfo<std::net::SocketAddr>>,
    body: Result<Json<ResendVerificationRequest>, JsonRejection>,
) -> Response {
    let request = match body {
        Ok(Json(request)) => request,
        Err(rejection) => return validation_rejection(rejection),
    };
    // Abuse control on resends (plan: resend is rate-limited).
    let resend_key =
        crate::rate_limit::email_key(&crate::registration::normalize_email(&request.email));
    if let Err(response) = rate_check(
        &state,
        "resend",
        &resend_key,
        state.rate_limit_resend,
        crate::rate_limit::CountMode::AllAttempts,
    )
    .await
    {
        return response;
    }
    if let Some(ip) = client_ip(connect_info) {
        if let Err(response) = rate_check(
            &state,
            "resend",
            &crate::rate_limit::ip_key(&ip),
            state.rate_limit_resend,
            crate::rate_limit::CountMode::AllAttempts,
        )
        .await
        {
            return response;
        }
    }
    match registration::resend_verification(
        state.pool.as_ref(),
        state.email.as_ref(),
        state.email_delivery,
        state.verification_ttl_secs,
        &request.email,
    )
    .await
    {
        Ok(delivery) => {
            rate_record(&state, "resend", &resend_key, "success").await;
            (
                StatusCode::ACCEPTED,
                Json(ResendResponse {
                    email_delivery: delivery,
                    message: delivery_message(delivery),
                }),
            )
                .into_response()
        }
        Err(error) => {
            rate_record(&state, "resend", &resend_key, "failure").await;
            error_response(
                status_for(error.code()),
                error.code(),
                error.message().to_string(),
            )
        }
    }
}

fn status_for(code: &'static str) -> StatusCode {
    match code {
        "validation_failed" | "invalid_or_expired_token" => StatusCode::BAD_REQUEST,
        "dependency_unavailable" => StatusCode::SERVICE_UNAVAILABLE,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse::healthy())
}

async fn not_found() -> Response {
    error_response(
        StatusCode::NOT_FOUND,
        "not_found",
        "No route matches this path.",
    )
}

async fn method_not_allowed() -> Response {
    error_response(
        StatusCode::METHOD_NOT_ALLOWED,
        "method_not_allowed",
        "This route does not accept this HTTP method.",
    )
}

/// Build the service router with no dependencies configured (Phase 23A
/// surface). `/health` works; auth routes answer honestly with 503 until a
/// database and email transport are configured.
pub fn build_router() -> Router {
    build_router_with_state(std::sync::Arc::new(AppState::unconfigured()))
}

/// Build the service router around shared state (Phase 23C–23D).
pub fn build_router_with_state(state: std::sync::Arc<AppState>) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/v1/auth/register", post(register_handler))
        .route("/v1/auth/verify-email", post(verify_email_handler))
        .route(
            "/v1/auth/resend-verification",
            post(resend_verification_handler),
        )
        .route("/v1/auth/login", post(login_handler))
        .route("/v1/auth/logout", post(logout_handler))
        .route("/v1/auth/refresh", post(refresh_handler))
        .route("/v1/auth/me", get(me_handler))
        .route("/v1/auth/recovery-request", post(recovery_request_handler))
        .route("/v1/auth/reset-password", post(reset_password_handler))
        .route("/v1/auth/password-change", post(password_change_handler))
        .route("/v1/auth/google/exchange", post(google_exchange_handler))
        .fallback(not_found)
        .method_not_allowed_fallback(method_not_allowed)
        .layer(DefaultBodyLimit::max(MAX_REQUEST_BODY_BYTES))
        .with_state(state)
}

// ---------------------------------------------------------------------------
// Phase 23D — login, sessions, logout, refresh (plan §6 contracts)
// ---------------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionBody {
    /// Raw session token — returned to the **trusted desktop Rust client**
    /// over the approved transport, which stores it in the OS credential
    /// store (plan §6: never delivered to the WebView). Never logged, never
    /// placed in URLs.
    token: String,
    expires_at: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LoginUserBody {
    id: String,
    name: String,
    email: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LoginResponse {
    user: LoginUserBody,
    session: SessionBody,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RefreshResponse {
    session: SessionBody,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MeResponse {
    user: LoginUserBody,
}

/// Extract `Authorization: Bearer <token>`. Missing/malformed → generic
/// `invalid_or_expired_token` (plan §6 stable code for bearer failures).
fn bearer_token(headers: &axum::http::HeaderMap) -> Result<&str, Response> {
    let value = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(str::trim)
        .filter(|token| !token.is_empty());
    value.ok_or_else(|| {
        error_response(
            StatusCode::UNAUTHORIZED,
            "invalid_or_expired_token",
            "Authentication is required for this request.",
        )
    })
}

fn session_error_response(error: crate::session::SessionError) -> Response {
    let status = match error {
        crate::session::SessionError::DependencyUnavailable => StatusCode::SERVICE_UNAVAILABLE,
        crate::session::SessionError::Internal => StatusCode::INTERNAL_SERVER_ERROR,
        _ => StatusCode::UNAUTHORIZED,
    };
    error_response(status, error.code(), error.message())
}

fn rate_limited_response(retry_after_secs: u64) -> Response {
    (
        StatusCode::TOO_MANY_REQUESTS,
        [(axum::http::header::RETRY_AFTER, retry_after_secs.to_string())],
        Json(ErrorBody {
            error: ErrorDetail {
                code: "rate_limited",
                message: format!(
                    "Too many attempts. Try again in {retry_after_secs} seconds."
                ),
            },
        }),
    )
        .into_response()
}

/// Pre-attempt window check for one hashed key. Only `Err` short-circuits.
async fn rate_check(
    state: &std::sync::Arc<AppState>,
    scope: &str,
    key: &str,
    rule: crate::rate_limit::RateLimitRule,
    mode: crate::rate_limit::CountMode,
) -> Result<(), Response> {
    let Some(pool) = state.pool.as_ref() else {
        return Ok(()); // dependency check happens in the service layer (503).
    };
    crate::rate_limit::check(pool, scope, key, rule, mode)
        .await
        .map_err(rate_limited_response)
}

async fn rate_record(state: &std::sync::Arc<AppState>, scope: &str, key: &str, outcome: &str) {
    if let Some(pool) = state.pool.as_ref() {
        crate::rate_limit::record(pool, scope, key, outcome).await;
    }
}

fn client_ip(connect_info: Option<axum::extract::ConnectInfo<std::net::SocketAddr>>) -> Option<String> {
    connect_info.map(|axum::extract::ConnectInfo(addr)| addr.ip().to_string())
}

async fn login_handler(
    State(state): State<std::sync::Arc<AppState>>,
    connect_info: Option<axum::extract::ConnectInfo<std::net::SocketAddr>>,
    body: Result<Json<crate::login::LoginRequest>, JsonRejection>,
) -> Response {
    let request = match body {
        Ok(Json(request)) => request,
        Err(rejection) => return validation_rejection(rejection),
    };

    let email_key = crate::rate_limit::email_key(&crate::registration::normalize_email(&request.email));
    if let Err(response) = rate_check(
        &state,
        "login",
        &email_key,
        state.rate_limit_login,
        crate::rate_limit::CountMode::FailuresOnly,
    )
    .await
    {
        return response;
    }
    let ip_key = client_ip(connect_info).map(|ip| crate::rate_limit::ip_key(&ip));
    if let Some(key) = &ip_key {
        if let Err(response) = rate_check(
            &state,
            "login",
            key,
            state.rate_limit_login,
            crate::rate_limit::CountMode::FailuresOnly,
        )
        .await
        {
            return response;
        }
    }

    let outcome = crate::login::login(
        state.pool.as_ref(),
        state.session_ttl_secs,
        &request,
        None,
        None,
    )
    .await;

    let result = match outcome {
        Ok(outcome) => {
            rate_record(&state, "login", &email_key, "success").await;
            (
                StatusCode::OK,
                Json(LoginResponse {
                    user: LoginUserBody {
                        id: outcome.user_id,
                        name: outcome.name,
                        email: outcome.email,
                    },
                    session: SessionBody {
                        token: outcome.session.raw_token,
                        expires_at: outcome.session.expires_at_rfc3339,
                    },
                }),
            )
                .into_response()
        }
        Err(error) => {
            if error.code() == "invalid_credentials" {
                rate_record(&state, "login", &email_key, "failure").await;
                if let Some(key) = &ip_key {
                    rate_record(&state, "login", key, "failure").await;
                }
            }
            error_response(
                StatusCode::from_u16(error.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
                error.code(),
                error.message().to_string(),
            )
        }
    };
    result
}

async fn logout_handler(
    State(state): State<std::sync::Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Response {
    let token = match bearer_token(&headers) {
        Ok(token) => token.to_string(),
        Err(response) => return response,
    };
    let Some(pool) = state.pool.as_ref() else {
        return error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "dependency_unavailable",
            "Sign out is unavailable: the account database is not configured.",
        );
    };
    match crate::session::revoke_family(pool, &token, state.session_idle_ttl_secs).await {
        // Plan §6: logout → 204 (no body).
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => session_error_response(error),
    }
}

async fn refresh_handler(
    State(state): State<std::sync::Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Response {
    let token = match bearer_token(&headers) {
        Ok(token) => token.to_string(),
        Err(response) => return response,
    };
    let Some(pool) = state.pool.as_ref() else {
        return error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "dependency_unavailable",
            "Session refresh is unavailable: the account database is not configured.",
        );
    };
    match crate::session::rotate_session(
        pool,
        &token,
        state.session_ttl_secs,
        state.session_idle_ttl_secs,
        None,
        None,
    )
    .await
    {
        Ok((created, _context)) => (
            StatusCode::OK,
            Json(RefreshResponse {
                session: SessionBody {
                    token: created.raw_token,
                    expires_at: created.expires_at_rfc3339,
                },
            }),
        )
            .into_response(),
        Err(error) => session_error_response(error),
    }
}

async fn me_handler(
    State(state): State<std::sync::Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Response {
    let token = match bearer_token(&headers) {
        Ok(token) => token.to_string(),
        Err(response) => return response,
    };
    let Some(pool) = state.pool.as_ref() else {
        return error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "dependency_unavailable",
            "Account lookup is unavailable: the account database is not configured.",
        );
    };
    match crate::session::authenticate(pool, &token, state.session_idle_ttl_secs).await {
        Ok(context) => (
            StatusCode::OK,
            Json(MeResponse {
                user: LoginUserBody {
                    id: context.user.id,
                    name: context.user.name,
                    email: context.user.email,
                },
            }),
        )
            .into_response(),
        Err(error) => session_error_response(error),
    }
}

// ---------------------------------------------------------------------------
// Phase 23E — password recovery, reset, and password change (plan §6)
// ---------------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RecoveryAcceptedBody {
    /// Constant for every input — enumeration defence (plan §6).
    status: &'static str,
    email_delivery: EmailDelivery,
    message: String,
}

fn recovery_message(delivery: EmailDelivery) -> &'static str {
    match delivery {
        EmailDelivery::Configured => "If that address has an account, a reset link is on its way.",
        EmailDelivery::Unconfigured => {
            "Request accepted, but no email was sent: outgoing email is not configured in this build."
        }
    }
}

fn recovery_error_response(error: crate::recovery::RecoveryError) -> Response {
    error_response(
        StatusCode::from_u16(error.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
        error.code(),
        error.message().to_string(),
    )
}

/// `POST /v1/auth/recovery-request` — ALWAYS 202 with a byte-identical body
/// for every input (existing/unknown/verified/unverified/ineligible accounts).
async fn recovery_request_handler(
    State(state): State<std::sync::Arc<AppState>>,
    connect_info: Option<axum::extract::ConnectInfo<std::net::SocketAddr>>,
    body: Result<Json<crate::recovery::RecoveryRequest>, JsonRejection>,
) -> Response {
    let request = match body {
        Ok(Json(request)) => request,
        Err(rejection) => return validation_rejection(rejection),
    };

    let key = crate::rate_limit::email_key(&crate::registration::normalize_email(&request.email));
    if let Err(response) = rate_check(
        &state,
        "recovery",
        &key,
        state.rate_limit_recovery,
        crate::rate_limit::CountMode::AllAttempts,
    )
    .await
    {
        return response;
    }
    if let Some(ip) = client_ip(connect_info) {
        if let Err(response) = rate_check(
            &state,
            "recovery",
            &crate::rate_limit::ip_key(&ip),
            state.rate_limit_recovery,
            crate::rate_limit::CountMode::AllAttempts,
        )
        .await
        {
            return response;
        }
    }

    match crate::recovery::request_recovery(
        state.pool.as_ref(),
        state.email.as_ref(),
        state.email_delivery,
        state.reset_ttl_secs,
        &request,
    )
    .await
    {
        Ok(delivery) => {
            rate_record(&state, "recovery", &key, "success").await;
            (
                StatusCode::ACCEPTED,
                Json(RecoveryAcceptedBody {
                    status: "accepted",
                    email_delivery: delivery,
                    message: recovery_message(delivery),
                }),
            )
                .into_response()
        }
        Err(error) => {
            rate_record(&state, "recovery", &key, "failure").await;
            recovery_error_response(error)
        }
    }
}

/// `POST /v1/auth/reset-password` — consume token + password update +
/// revoke-all-sessions transactionally; success 204 (plan §6).
async fn reset_password_handler(
    State(state): State<std::sync::Arc<AppState>>,
    connect_info: Option<axum::extract::ConnectInfo<std::net::SocketAddr>>,
    body: Result<Json<crate::recovery::ResetPasswordRequest>, JsonRejection>,
) -> Response {
    let request = match body {
        Ok(Json(request)) => request,
        Err(rejection) => return validation_rejection(rejection),
    };

    // Per-IP + per-token windows (plan §4/§5 keys: ip | email-hash | account-id).
    let ip_key = client_ip(connect_info).map(|ip| crate::rate_limit::ip_key(&ip));
    if let Some(key) = &ip_key {
        if let Err(response) = rate_check(
            &state,
            "recovery",
            key,
            state.rate_limit_recovery,
            crate::rate_limit::CountMode::AllAttempts,
        )
        .await
        {
            return response;
        }
    }
    let token_key = crate::rate_limit::token_key(request.token.trim());
    if let Err(response) = rate_check(
        &state,
        "recovery",
        &token_key,
        state.rate_limit_recovery,
        crate::rate_limit::CountMode::AllAttempts,
    )
    .await
    {
        return response;
    }

    match crate::recovery::reset_password(state.pool.as_ref(), &request).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => {
            rate_record(&state, "recovery", &token_key, "failure").await;
            if let Some(key) = &ip_key {
                rate_record(&state, "recovery", key, "failure").await;
            }
            recovery_error_response(error)
        }
    }
}

/// `POST /v1/auth/password-change` (authenticated) — plan §6 contract:
/// `{currentPassword,newPassword}` → 204, revokes other sessions.
async fn password_change_handler(
    State(state): State<std::sync::Arc<AppState>>,
    connect_info: Option<axum::extract::ConnectInfo<std::net::SocketAddr>>,
    headers: axum::http::HeaderMap,
    body: Result<Json<crate::recovery::ChangePasswordRequest>, JsonRejection>,
) -> Response {
    let token = match bearer_token(&headers) {
        Ok(token) => token.to_string(),
        Err(response) => return response,
    };
    let request = match body {
        Ok(Json(request)) => request,
        Err(rejection) => return validation_rejection(rejection),
    };
    // Shape/policy first — bad input answers 400 even without a database.
    if let Err(error) = crate::recovery::validate_change_request(&request) {
        return recovery_error_response(error);
    }
    let Some(pool) = state.pool.as_ref() else {
        return error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "dependency_unavailable",
            "Password change is unavailable: the account database is not configured.",
        );
    };

    let context =
        match crate::session::authenticate(pool, &token, state.session_idle_ttl_secs).await {
            Ok(context) => context,
            Err(error) => return session_error_response(error),
        };

    // Defense-in-depth window on credential guesses against the account.
    let account_key = crate::rate_limit::email_key(&context.user.email);
    if let Err(response) = rate_check(
        &state,
        "recovery",
        &account_key,
        state.rate_limit_recovery,
        crate::rate_limit::CountMode::FailuresOnly,
    )
    .await
    {
        return response;
    }
    if let Some(ip) = client_ip(connect_info) {
        if let Err(response) = rate_check(
            &state,
            "recovery",
            &crate::rate_limit::ip_key(&ip),
            state.rate_limit_recovery,
            crate::rate_limit::CountMode::FailuresOnly,
        )
        .await
        {
            return response;
        }
    }

    match crate::recovery::change_password(Some(pool), &context, &request).await {
        Ok(()) => {
            // Plan §4: password change is audit-logged (metadata only; no
            // email, no credentials — scrubbed via the log layer).
            crate::scrub::audit("password_change", &context.user.id, "ok");
            StatusCode::NO_CONTENT.into_response()
        }
        Err(error) => {
            if error.code() == "invalid_credentials" {
                rate_record(&state, "recovery", &account_key, "failure").await;
                crate::scrub::audit("password_change", &context.user.id, "denied");
            }
            recovery_error_response(error)
        }
    }
}

// ---------------------------------------------------------------------------
// Phase 23F — Google OAuth exchange (plan §6 `POST /v1/auth/google/exchange`)
// ---------------------------------------------------------------------------

fn oauth_error_response(error: crate::oauth::OAuthError) -> Response {
    error_response(
        StatusCode::from_u16(error.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
        error.code(),
        error.message().to_string(),
    )
}

/// `POST /v1/auth/google/exchange {code, codeVerifier, state, redirectUri}`
/// → `200 {user, session}` | `409 email_conflict` (plan §6). The session
/// shape is the plan's `{user, session}` — same token-placement rules as
/// password login (trusted desktop client only; never the WebView).
async fn google_exchange_handler(
    State(state): State<std::sync::Arc<AppState>>,
    connect_info: Option<axum::extract::ConnectInfo<std::net::SocketAddr>>,
    body: Result<Json<crate::oauth::GoogleExchangeRequest>, JsonRejection>,
) -> Response {
    let request = match body {
        Ok(Json(request)) => request,
        Err(rejection) => return validation_rejection(rejection),
    };

    // Plan §4: per-IP windows on `google/exchange` (scope 'google').
    if let Some(ip) = client_ip(connect_info) {
        if let Err(response) = rate_check(
            &state,
            "google",
            &crate::rate_limit::ip_key(&ip),
            state.rate_limit_recovery,
            crate::rate_limit::CountMode::AllAttempts,
        )
        .await
        {
            return response;
        }
    }

    let (Some(provider), Some(expected_redirect)) = (
        state.google_provider.as_ref(),
        state.google_redirect_uri.as_deref(),
    ) else {
        return error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "dependency_unavailable",
            "Google sign-in is unavailable: it is not configured in this build.",
        );
    };

    match crate::oauth::google_exchange(
        state.pool.as_ref(),
        provider.as_ref(),
        expected_redirect,
        &request,
    )
    .await
    {
        Ok(outcome) => (
            StatusCode::OK,
            Json(LoginResponse {
                user: LoginUserBody {
                    id: outcome.user_id,
                    name: outcome.name,
                    email: outcome.email,
                },
                session: SessionBody {
                    token: outcome.session.raw_token,
                    expires_at: outcome.session.expires_at_rfc3339,
                },
            }),
        )
            .into_response(),
        Err(error) => oauth_error_response(error),
    }
}
