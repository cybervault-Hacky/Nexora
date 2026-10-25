//! HTTP surface of the nexora-auth service.
//!
//! Phase 23A surface: `GET /health` reports **service** health only (process
//! is up and routing works) — it does NOT claim that authentication, a
//! database, email delivery, or OAuth is operational. Phase 23C adds
//! registration + email verification under `/v1/auth/*` (plan §6): endpoints
//! answer honestly (503 `dependency_unavailable`) when no database is
//! configured, and registration responses are enumeration-safe. Login,
//! sessions, recovery, and OAuth remain later steps.

use axum::body::Bytes;
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

/// Defence-in-depth cap for normal JSON auth/billing/quota requests. Signed
/// provider webhooks mount on their own sub-router with a 64 KiB cap.
pub const MAX_REQUEST_BODY_BYTES: usize = 4096;
pub const MAX_WEBHOOK_BODY_BYTES: usize = 65_536;

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
    /// Injected payment seam. The service binary deliberately keeps this
    /// unconfigured until provider and entitlement review is approved.
    pub payment_provider: std::sync::Arc<dyn crate::billing::PaymentProvider>,
    /// Quota enforcement is fail-closed until an approved policy is supplied.
    pub quota_policy: crate::quota::QuotaPolicy,
    /// Optional test/provider webhook secret; current binary composition passes `None`.
    pub stripe_webhook_secret: Option<String>,
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
            payment_provider: std::sync::Arc::new(crate::billing::UnconfiguredPaymentProvider),
            quota_policy: crate::quota::QuotaPolicy::unconfigured(),
            stripe_webhook_secret: None,
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
            "If this address can be registered and delivery succeeds, a verification email will be sent."
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
    connect_info: MaybeConnectInfo,
    body: Result<Json<RegisterRequest>, JsonRejection>,
) -> Response {
    let request = match body {
        Ok(Json(request)) => request,
        Err(rejection) => return validation_rejection(rejection),
    };
    // DB-backed sliding windows cover both normalized address and peer IP.
    let register_key =
        crate::rate_limit::email_key(&crate::registration::normalize_email(&request.email));
    let ip_key = client_ip(connect_info).map(|ip| crate::rate_limit::ip_key(&ip));
    let mut keys = vec![register_key.as_str()];
    if let Some(ip_key) = ip_key.as_deref() {
        keys.push(ip_key);
    }
    if let Err(response) = rate_attempts(
        &state,
        "register",
        &keys,
        state.rate_limit_register,
        crate::rate_limit::CountMode::AllAttempts,
    )
    .await
    {
        return response;
    }
    let outcome = registration::register(
        state.pool.as_ref(),
        state.email.as_ref(),
        state.email_delivery,
        state.verification_ttl_secs,
        &request,
    )
    .await;
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
    connect_info: MaybeConnectInfo,
    body: Result<Json<ResendVerificationRequest>, JsonRejection>,
) -> Response {
    let request = match body {
        Ok(Json(request)) => request,
        Err(rejection) => return validation_rejection(rejection),
    };
    // Abuse control on resends covers both normalized address and peer IP.
    let resend_key =
        crate::rate_limit::email_key(&crate::registration::normalize_email(&request.email));
    let ip_key = client_ip(connect_info).map(|ip| crate::rate_limit::ip_key(&ip));
    let mut keys = vec![resend_key.as_str()];
    if let Some(ip_key) = ip_key.as_deref() {
        keys.push(ip_key);
    }
    if let Err(response) = rate_attempts(
        &state,
        "resend",
        &keys,
        state.rate_limit_resend,
        crate::rate_limit::CountMode::AllAttempts,
    )
    .await
    {
        return response;
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
    let json_routes = Router::new()
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
        .route("/v1/auth/google/link", post(google_link_handler))
        .route(
            "/v1/account/profile",
            get(account_profile_get_handler).patch(account_profile_patch_handler),
        )
        .route("/v1/account/export", get(account_export_handler))
        .route("/v1/account/delete", post(account_delete_handler))
        .route("/v1/plans", get(list_plans_handler))
        .route("/v1/billing/subscription", get(billing_status_handler))
        .route("/v1/billing/checkout", post(billing_checkout_handler))
        .route("/v1/billing/verify", post(billing_verify_handler))
        .route("/v1/billing/cancel", post(billing_cancel_handler))
        .route("/v1/billing/change-plan", post(billing_change_plan_handler))
        .route("/v1/billing/portal", post(billing_portal_handler))
        .route("/v1/usage", get(usage_summary_handler))
        .route("/v1/usage/reserve", post(usage_reserve_handler))
        .route("/v1/usage/commit", post(usage_commit_handler))
        .route("/v1/usage/release", post(usage_release_handler))
        .layer(DefaultBodyLimit::max(MAX_REQUEST_BODY_BYTES));

    let webhook_routes = Router::new()
        .route("/v1/billing/webhooks/stripe", post(stripe_webhook_handler))
        .layer(DefaultBodyLimit::max(MAX_WEBHOOK_BODY_BYTES));

    json_routes
        .merge(webhook_routes)
        .fallback(not_found)
        .method_not_allowed_fallback(method_not_allowed)
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
#[allow(clippy::result_large_err)]
fn bearer_token(headers: &axum::http::HeaderMap) -> Result<&str, Response> {
    let value = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(str::trim)
        .filter(|token| crate::session::is_valid_session_token_shape(token));
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
        [(
            axum::http::header::RETRY_AFTER,
            retry_after_secs.to_string(),
        )],
        Json(ErrorBody {
            error: ErrorDetail {
                code: "rate_limited",
                message: format!("Too many attempts. Try again in {retry_after_secs} seconds."),
            },
        }),
    )
        .into_response()
}

fn rate_error_response(error: crate::rate_limit::RateLimitError) -> Response {
    match error {
        crate::rate_limit::RateLimitError::Limited(retry_after) => {
            rate_limited_response(retry_after)
        }
        crate::rate_limit::RateLimitError::StoreUnavailable => error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "dependency_unavailable",
            "Request protection is temporarily unavailable. Try again later.",
        ),
    }
}

/// Pre-attempt window check for one hashed key. Storage failures fail closed
/// with 503; only an exhausted window becomes `429 rate_limited`.
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
        .map_err(rate_error_response)
}

async fn rate_record(
    state: &std::sync::Arc<AppState>,
    scope: &str,
    key: &str,
    outcome: &str,
) -> Result<(), Response> {
    if let Some(pool) = state.pool.as_ref() {
        crate::rate_limit::record(pool, scope, key, outcome)
            .await
            .map_err(rate_error_response)?;
    }
    Ok(())
}

/// Check every key before recording any, then persist all-attempt windows
/// before the handler performs its database or email side effects. This makes
/// a rate-store failure fail closed and keeps peer-IP limits effective.
async fn rate_attempts(
    state: &std::sync::Arc<AppState>,
    scope: &str,
    keys: &[&str],
    rule: crate::rate_limit::RateLimitRule,
    mode: crate::rate_limit::CountMode,
) -> Result<(), Response> {
    for key in keys {
        rate_check(state, scope, key, rule, mode).await?;
    }
    for key in keys {
        rate_record(state, scope, key, "attempt").await?;
    }
    Ok(())
}

#[derive(Clone, Copy, Debug)]
struct MaybeConnectInfo(Option<std::net::SocketAddr>);

impl<S: Send + Sync> axum::extract::FromRequestParts<S> for MaybeConnectInfo {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        Ok(Self(
            parts
                .extensions
                .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
                .map(|axum::extract::ConnectInfo(addr)| *addr),
        ))
    }
}

fn client_ip(connect_info: MaybeConnectInfo) -> Option<String> {
    connect_info.0.map(|addr| addr.ip().to_string())
}

async fn login_handler(
    State(state): State<std::sync::Arc<AppState>>,
    connect_info: MaybeConnectInfo,
    body: Result<Json<crate::login::LoginRequest>, JsonRejection>,
) -> Response {
    let request = match body {
        Ok(Json(request)) => request,
        Err(rejection) => return validation_rejection(rejection),
    };

    let email_key =
        crate::rate_limit::email_key(&crate::registration::normalize_email(&request.email));
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
                if let Err(response) =
                    rate_record(&state, "login", &email_key, "failure").await
                {
                    return response;
                }
                if let Some(key) = &ip_key {
                    if let Err(response) =
                        rate_record(&state, "login", key, "failure").await
                    {
                        return response;
                    }
                }
            }
            error_response(
                StatusCode::from_u16(error.status_code())
                    .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
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
        EmailDelivery::Configured => {
            "If that address has an account and delivery succeeds, reset instructions will be sent."
        },
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
    connect_info: MaybeConnectInfo,
    body: Result<Json<crate::recovery::RecoveryRequest>, JsonRejection>,
) -> Response {
    let request = match body {
        Ok(Json(request)) => request,
        Err(rejection) => return validation_rejection(rejection),
    };

    let key = crate::rate_limit::email_key(&crate::registration::normalize_email(&request.email));
    let ip_key = client_ip(connect_info).map(|ip| crate::rate_limit::ip_key(&ip));
    let mut keys = vec![key.as_str()];
    if let Some(ip_key) = ip_key.as_deref() {
        keys.push(ip_key);
    }
    if let Err(response) = rate_attempts(
        &state,
        "recovery",
        &keys,
        state.rate_limit_recovery,
        crate::rate_limit::CountMode::AllAttempts,
    )
    .await
    {
        return response;
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
            (
                StatusCode::ACCEPTED,
                Json(RecoveryAcceptedBody {
                    status: "accepted",
                    email_delivery: delivery,
                    message: recovery_message(delivery).to_string(),
                }),
            )
                .into_response()
        }
        Err(error) => recovery_error_response(error),
    }
}

/// `POST /v1/auth/reset-password` — consume token + password update +
/// revoke-all-sessions transactionally; success 204 (plan §6).
async fn reset_password_handler(
    State(state): State<std::sync::Arc<AppState>>,
    connect_info: MaybeConnectInfo,
    body: Result<Json<crate::recovery::ResetPasswordRequest>, JsonRejection>,
) -> Response {
    let request = match body {
        Ok(Json(request)) => request,
        Err(rejection) => return validation_rejection(rejection),
    };

    // Per-IP + per-token windows count every attempt, including malformed or
    // replayed tokens, before the password-reset transaction begins.
    let ip_key = client_ip(connect_info).map(|ip| crate::rate_limit::ip_key(&ip));
    let token_key = crate::rate_limit::token_key(request.token.trim());
    let mut keys = vec![token_key.as_str()];
    if let Some(ip_key) = ip_key.as_deref() {
        keys.push(ip_key);
    }
    if let Err(response) = rate_attempts(
        &state,
        "recovery",
        &keys,
        state.rate_limit_recovery,
        crate::rate_limit::CountMode::AllAttempts,
    )
    .await
    {
        return response;
    }

    match crate::recovery::reset_password(state.pool.as_ref(), &request).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => recovery_error_response(error),
    }
}

/// `POST /v1/auth/password-change` (authenticated) — plan §6 contract:
/// `{currentPassword,newPassword}` → 204, revokes other sessions.
async fn password_change_handler(
    State(state): State<std::sync::Arc<AppState>>,
    connect_info: MaybeConnectInfo,
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
    let ip_key = client_ip(connect_info).map(|ip| crate::rate_limit::ip_key(&ip));
    if let Some(key) = &ip_key {
        if let Err(response) = rate_check(
            &state,
            "recovery",
            key,
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
                if let Err(response) =
                    rate_record(&state, "recovery", &account_key, "failure").await
                {
                    return response;
                }
                if let Some(key) = &ip_key {
                    if let Err(response) =
                        rate_record(&state, "recovery", key, "failure").await
                    {
                        return response;
                    }
                }
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
    connect_info: MaybeConnectInfo,
    body: Result<Json<crate::oauth::GoogleExchangeRequest>, JsonRejection>,
) -> Response {
    let request = match body {
        Ok(Json(request)) => request,
        Err(rejection) => return validation_rejection(rejection),
    };

    if let Err(error) = crate::oauth::validate_exchange_shapes_only(&request) {
        return oauth_error_response(error);
    }

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

// ---------------------------------------------------------------------------
// Account profile, linking, export, deletion, plans, billing, and usage handlers
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct QuotaExceededEnvelope {
    error: ErrorDetail,
    quota: crate::quota::QuotaExceededDetail,
}

fn account_error_response(error: crate::account::AccountError) -> Response {
    error_response(
        StatusCode::from_u16(error.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
        error.code(),
        error.message(),
    )
}

fn billing_error_response(error: crate::billing::BillingError) -> Response {
    error_response(
        StatusCode::from_u16(error.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
        error.code(),
        error.message(),
    )
}

fn quota_error_response(error: crate::quota::QuotaError) -> Response {
    let status =
        StatusCode::from_u16(error.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    let code = error.code();
    let message = error.message();
    match error {
        crate::quota::QuotaError::Exceeded(detail) => (
            status,
            Json(QuotaExceededEnvelope {
                error: ErrorDetail { code, message },
                quota: detail,
            }),
        )
            .into_response(),
        _ => error_response(status, code, message),
    }
}

#[allow(clippy::result_large_err)]
async fn require_authenticated_session(
    state: &AppState,
    headers: &axum::http::HeaderMap,
) -> Result<crate::session::SessionContext, Response> {
    let token = bearer_token(headers)?;
    let Some(pool) = state.pool.as_ref() else {
        return Err(error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "dependency_unavailable",
            "Service is unavailable: the account database is not configured.",
        ));
    };
    crate::session::authenticate(pool, token, state.session_idle_ttl_secs)
        .await
        .map_err(session_error_response)
}

#[derive(Serialize)]
struct ProfileEnvelope {
    profile: crate::account::AccountProfileView,
}

async fn account_profile_get_handler(
    State(state): State<std::sync::Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Response {
    let context = match require_authenticated_session(&state, &headers).await {
        Ok(ctx) => ctx,
        Err(resp) => return resp,
    };
    match crate::account::get_account_profile(state.pool.as_ref(), &context.user.id).await {
        Ok(profile) => (StatusCode::OK, Json(ProfileEnvelope { profile })).into_response(),
        Err(err) => account_error_response(err),
    }
}

async fn account_profile_patch_handler(
    State(state): State<std::sync::Arc<AppState>>,
    headers: axum::http::HeaderMap,
    body: Result<Json<crate::account::UpdateProfileRequest>, JsonRejection>,
) -> Response {
    let token = match bearer_token(&headers) {
        Ok(t) => t.to_string(),
        Err(resp) => return resp,
    };
    let request = match body {
        Ok(Json(req)) => req,
        Err(rej) => return validation_rejection(rej),
    };
    let Some(pool) = state.pool.as_ref() else {
        return error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "dependency_unavailable",
            "Profile update is unavailable: the account database is not configured.",
        );
    };
    let context =
        match crate::session::authenticate(pool, &token, state.session_idle_ttl_secs).await {
            Ok(ctx) => ctx,
            Err(err) => return session_error_response(err),
        };
    match crate::account::update_account_profile(Some(pool), &context.user.id, &request).await {
        Ok(profile) => (StatusCode::OK, Json(ProfileEnvelope { profile })).into_response(),
        Err(err) => account_error_response(err),
    }
}

async fn account_export_handler(
    State(state): State<std::sync::Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Response {
    let context = match require_authenticated_session(&state, &headers).await {
        Ok(ctx) => ctx,
        Err(resp) => return resp,
    };
    match crate::account::export_account_data(state.pool.as_ref(), &context.user.id).await {
        Ok(payload) => (StatusCode::OK, Json(payload)).into_response(),
        Err(err) => account_error_response(err),
    }
}

async fn account_delete_handler(
    State(state): State<std::sync::Arc<AppState>>,
    headers: axum::http::HeaderMap,
    body: Result<Json<crate::account::DeleteAccountRequest>, JsonRejection>,
) -> Response {
    let token = match bearer_token(&headers) {
        Ok(t) => t.to_string(),
        Err(resp) => return resp,
    };
    let request = match body {
        Ok(Json(req)) => req,
        Err(rej) => return validation_rejection(rej),
    };
    let Some(pool) = state.pool.as_ref() else {
        return error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "dependency_unavailable",
            "Account deletion is unavailable: the account database is not configured.",
        );
    };
    let context =
        match crate::session::authenticate(pool, &token, state.session_idle_ttl_secs).await {
            Ok(ctx) => ctx,
            Err(err) => return session_error_response(err),
        };
    match crate::account::delete_account(Some(pool), &context.user.id, &request).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(err) => account_error_response(err),
    }
}

async fn google_link_handler(
    State(state): State<std::sync::Arc<AppState>>,
    headers: axum::http::HeaderMap,
    body: Result<Json<crate::oauth::GoogleExchangeRequest>, JsonRejection>,
) -> Response {
    let token = match bearer_token(&headers) {
        Ok(t) => t.to_string(),
        Err(resp) => return resp,
    };
    let request = match body {
        Ok(Json(req)) => req,
        Err(rej) => return validation_rejection(rej),
    };
    if let Err(err) = crate::oauth::validate_exchange_shapes_only(&request) {
        return oauth_error_response(err);
    }
    let Some(pool) = state.pool.as_ref() else {
        return error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "dependency_unavailable",
            "Account linking is unavailable: the account database is not configured.",
        );
    };
    let (Some(provider), Some(expected_redirect)) = (
        state.google_provider.as_ref(),
        state.google_redirect_uri.as_deref(),
    ) else {
        return error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "dependency_unavailable",
            "Google account linking is not configured on this server.",
        );
    };
    let context =
        match crate::session::authenticate(pool, &token, state.session_idle_ttl_secs).await {
            Ok(ctx) => ctx,
            Err(err) => return session_error_response(err),
        };
    match crate::account::link_google_identity(
        Some(pool),
        provider.as_ref(),
        expected_redirect,
        &context.user.id,
        &request,
    )
    .await
    {
        Ok(profile) => (StatusCode::OK, Json(ProfileEnvelope { profile })).into_response(),
        Err(err) => account_error_response(err),
    }
}

async fn list_plans_handler(State(state): State<std::sync::Arc<AppState>>) -> Response {
    match crate::billing::list_plans(state.pool.as_ref(), state.payment_provider.is_configured())
        .await
    {
        Ok(mut catalogue) => {
            if !state.quota_policy.is_configured() {
                // Database plan rows may contain historical or proposed terms;
                // do not present them as an approved catalogue or entitlement.
                catalogue.plans.clear();
            } else if !catalogue.payment_configured {
                // Quotas can be explicitly configured independently from a
                // payment provider. Do not expose stored prices until payment
                // configuration is present.
                for plan in &mut catalogue.plans {
                    plan.prices.clear();
                }
            }
            (StatusCode::OK, Json(catalogue)).into_response()
        }
        Err(err) => billing_error_response(err),
    }
}

async fn billing_status_handler(
    State(state): State<std::sync::Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Response {
    let context = match require_authenticated_session(&state, &headers).await {
        Ok(ctx) => ctx,
        Err(resp) => return resp,
    };
    match crate::billing::get_billing_status(
        state.pool.as_ref(),
        &context.user.id,
        state.payment_provider.is_configured(),
    )
    .await
    {
        Ok(status) => (StatusCode::OK, Json(status)).into_response(),
        Err(err) => billing_error_response(err),
    }
}

async fn billing_checkout_handler(
    State(state): State<std::sync::Arc<AppState>>,
    headers: axum::http::HeaderMap,
    body: Result<Json<crate::billing::CheckoutRequest>, JsonRejection>,
) -> Response {
    let token = match bearer_token(&headers) {
        Ok(t) => t.to_string(),
        Err(resp) => return resp,
    };
    let request = match body {
        Ok(Json(req)) => req,
        Err(rej) => return validation_rejection(rej),
    };
    let Some(pool) = state.pool.as_ref() else {
        return billing_error_response(crate::billing::BillingError::DependencyUnavailable);
    };
    let context =
        match crate::session::authenticate(pool, &token, state.session_idle_ttl_secs).await {
            Ok(ctx) => ctx,
            Err(err) => return session_error_response(err),
        };
    match crate::billing::create_checkout_session(
        Some(pool),
        state.payment_provider.as_ref(),
        &context.user.id,
        &context.user.email,
        &request,
    )
    .await
    {
        Ok(resp) => (StatusCode::OK, Json(resp)).into_response(),
        Err(err) => billing_error_response(err),
    }
}

async fn billing_verify_handler(
    State(state): State<std::sync::Arc<AppState>>,
    headers: axum::http::HeaderMap,
    body: Result<Json<crate::billing::VerifyOrderRequest>, JsonRejection>,
) -> Response {
    let token = match bearer_token(&headers) {
        Ok(t) => t.to_string(),
        Err(resp) => return resp,
    };
    let request = match body {
        Ok(Json(req)) => req,
        Err(rej) => return validation_rejection(rej),
    };
    let Some(pool) = state.pool.as_ref() else {
        return billing_error_response(crate::billing::BillingError::DependencyUnavailable);
    };
    let context =
        match crate::session::authenticate(pool, &token, state.session_idle_ttl_secs).await {
            Ok(ctx) => ctx,
            Err(err) => return session_error_response(err),
        };
    match crate::billing::verify_checkout_order(
        Some(pool),
        state.payment_provider.as_ref(),
        &context.user.id,
        &request,
    )
    .await
    {
        Ok(resp) => (StatusCode::OK, Json(resp)).into_response(),
        Err(err) => billing_error_response(err),
    }
}

async fn billing_cancel_handler(
    State(state): State<std::sync::Arc<AppState>>,
    headers: axum::http::HeaderMap,
    body: Result<Json<crate::billing::CancelSubscriptionRequest>, JsonRejection>,
) -> Response {
    let token = match bearer_token(&headers) {
        Ok(t) => t.to_string(),
        Err(resp) => return resp,
    };
    let request = match body {
        Ok(Json(req)) => req,
        Err(rej) => return validation_rejection(rej),
    };
    let Some(pool) = state.pool.as_ref() else {
        return billing_error_response(crate::billing::BillingError::DependencyUnavailable);
    };
    let context =
        match crate::session::authenticate(pool, &token, state.session_idle_ttl_secs).await {
            Ok(ctx) => ctx,
            Err(err) => return session_error_response(err),
        };
    match crate::billing::cancel_subscription(
        Some(pool),
        state.payment_provider.as_ref(),
        &context.user.id,
        request.immediate,
    )
    .await
    {
        Ok(resp) => (StatusCode::OK, Json(resp)).into_response(),
        Err(err) => billing_error_response(err),
    }
}

async fn billing_change_plan_handler(
    State(state): State<std::sync::Arc<AppState>>,
    headers: axum::http::HeaderMap,
    body: Result<Json<crate::billing::ChangePlanRequest>, JsonRejection>,
) -> Response {
    let token = match bearer_token(&headers) {
        Ok(t) => t.to_string(),
        Err(resp) => return resp,
    };
    let request = match body {
        Ok(Json(req)) => req,
        Err(rej) => return validation_rejection(rej),
    };
    let Some(pool) = state.pool.as_ref() else {
        return billing_error_response(crate::billing::BillingError::DependencyUnavailable);
    };
    let context =
        match crate::session::authenticate(pool, &token, state.session_idle_ttl_secs).await {
            Ok(ctx) => ctx,
            Err(err) => return session_error_response(err),
        };
    match crate::billing::change_subscription_plan(
        Some(pool),
        state.payment_provider.as_ref(),
        &context.user.id,
        &request,
    )
    .await
    {
        Ok(resp) => (StatusCode::OK, Json(resp)).into_response(),
        Err(err) => billing_error_response(err),
    }
}

async fn billing_portal_handler(
    State(state): State<std::sync::Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Response {
    let context = match require_authenticated_session(&state, &headers).await {
        Ok(ctx) => ctx,
        Err(resp) => return resp,
    };
    match crate::billing::create_portal_session(
        state.pool.as_ref(),
        state.payment_provider.as_ref(),
        &context.user.id,
    )
    .await
    {
        Ok(resp) => (StatusCode::OK, Json(resp)).into_response(),
        Err(err) => billing_error_response(err),
    }
}

async fn stripe_webhook_handler(
    State(state): State<std::sync::Arc<AppState>>,
    headers: axum::http::HeaderMap,
    payload: Bytes,
) -> Response {
    let sig_header = headers
        .get("stripe-signature")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let now_unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    match crate::billing::process_signed_webhook(
        state.pool.as_ref(),
        &payload,
        sig_header,
        state.stripe_webhook_secret.as_deref(),
        now_unix,
    )
    .await
    {
        Ok(ack) => (StatusCode::OK, Json(ack)).into_response(),
        Err(err) => billing_error_response(err),
    }
}

async fn usage_summary_handler(
    State(state): State<std::sync::Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Response {
    let context = match require_authenticated_session(&state, &headers).await {
        Ok(ctx) => ctx,
        Err(resp) => return resp,
    };
    match crate::quota::get_usage_summary(
        state.pool.as_ref(),
        &context.user.id,
        state.quota_policy,
    )
    .await
    {
        Ok(summary) => (StatusCode::OK, Json(summary)).into_response(),
        Err(err) => quota_error_response(err),
    }
}

async fn usage_reserve_handler(
    State(state): State<std::sync::Arc<AppState>>,
    headers: axum::http::HeaderMap,
    body: Result<Json<crate::quota::ReserveQuotaRequest>, JsonRejection>,
) -> Response {
    let token = match bearer_token(&headers) {
        Ok(t) => t.to_string(),
        Err(resp) => return resp,
    };
    let request = match body {
        Ok(Json(req)) => req,
        Err(rej) => return validation_rejection(rej),
    };
    let Some(pool) = state.pool.as_ref() else {
        return quota_error_response(crate::quota::QuotaError::DependencyUnavailable);
    };
    let context =
        match crate::session::authenticate(pool, &token, state.session_idle_ttl_secs).await {
            Ok(ctx) => ctx,
            Err(err) => return session_error_response(err),
        };
    match crate::quota::reserve_model_quota(
        Some(pool),
        &context.user.id,
        &request,
        state.quota_policy,
    )
    .await
    {
        Ok(reserved) => (StatusCode::OK, Json(reserved)).into_response(),
        Err(err) => quota_error_response(err),
    }
}

async fn usage_commit_handler(
    State(state): State<std::sync::Arc<AppState>>,
    headers: axum::http::HeaderMap,
    body: Result<Json<crate::quota::SettleReservationRequest>, JsonRejection>,
) -> Response {
    let token = match bearer_token(&headers) {
        Ok(t) => t.to_string(),
        Err(resp) => return resp,
    };
    let request = match body {
        Ok(Json(req)) => req,
        Err(rej) => return validation_rejection(rej),
    };
    let Some(pool) = state.pool.as_ref() else {
        return quota_error_response(crate::quota::QuotaError::DependencyUnavailable);
    };
    let context =
        match crate::session::authenticate(pool, &token, state.session_idle_ttl_secs).await {
            Ok(ctx) => ctx,
            Err(err) => return session_error_response(err),
        };
    match crate::quota::settle_reservation(
        Some(pool),
        &context.user.id,
        &request.reservation_id,
        true,
        state.quota_policy,
    )
    .await
    {
        Ok(summary) => (StatusCode::OK, Json(summary)).into_response(),
        Err(err) => quota_error_response(err),
    }
}

async fn usage_release_handler(
    State(state): State<std::sync::Arc<AppState>>,
    headers: axum::http::HeaderMap,
    body: Result<Json<crate::quota::SettleReservationRequest>, JsonRejection>,
) -> Response {
    let token = match bearer_token(&headers) {
        Ok(t) => t.to_string(),
        Err(resp) => return resp,
    };
    let request = match body {
        Ok(Json(req)) => req,
        Err(rej) => return validation_rejection(rej),
    };
    let Some(pool) = state.pool.as_ref() else {
        return quota_error_response(crate::quota::QuotaError::DependencyUnavailable);
    };
    let context =
        match crate::session::authenticate(pool, &token, state.session_idle_ttl_secs).await {
            Ok(ctx) => ctx,
            Err(err) => return session_error_response(err),
        };
    match crate::quota::settle_reservation(
        Some(pool),
        &context.user.id,
        &request.reservation_id,
        false,
        state.quota_policy,
    )
    .await
    {
        Ok(summary) => (StatusCode::OK, Json(summary)).into_response(),
        Err(err) => quota_error_response(err),
    }
}

#[cfg(test)]
mod rate_limit_tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn rate_limit_store_failure_is_unavailable_not_zero_second_limit() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0")
            .expect("reserve an unused local port for the offline test");
        let unused_addr = listener.local_addr().expect("local listener address");
        drop(listener);
        let database_url = format!(
            "postgres://nexora_test:nexora_test@{unused_addr}/nexora_test"
        );
        let pool = sqlx::postgres::PgPoolOptions::new()
            .acquire_timeout(Duration::from_millis(250))
            .connect_lazy(&database_url)
            .expect("valid lazy PostgreSQL URL");
        let mut app_state = AppState::unconfigured();
        app_state.pool = Some(pool);
        let state = std::sync::Arc::new(app_state);

        let response = rate_check(
            &state,
            "register",
            "e:hashed-test-key",
            crate::rate_limit::RateLimitRule {
                max_failures: 1,
                window_secs: 60,
            },
            crate::rate_limit::CountMode::AllAttempts,
        )
        .await
        .expect_err("unreachable limiter store must fail closed");

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert!(response.headers().get("retry-after").is_none());

        let write_error = rate_record(&state, "register", "e:hashed-test-key", "attempt")
            .await
            .expect_err("rate-limit write failure must be surfaced");
        assert_eq!(write_error.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert!(write_error.headers().get("retry-after").is_none());
    }
}
