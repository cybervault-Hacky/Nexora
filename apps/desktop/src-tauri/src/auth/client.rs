use std::fmt;
use std::time::Duration;

use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use reqwest::redirect::Policy;
use reqwest::{Client, Url};
use serde::{Deserialize, Serialize};

use super::{AuthError, AuthResult, AuthStatusResponse, AuthUser, SessionCredentials};

const COMPILE_TIME_AUTH_API_URL: Option<&'static str> = option_env!("NEXORA_AUTH_API_URL");
const COMPILE_TIME_ENVIRONMENT: Option<&'static str> = option_env!("NEXORA_BUILD_ENV");
const DEFAULT_LOOPBACK_AUTH_URL: &str = "http://127.0.0.1:8787";

#[derive(Deserialize)]
struct ErrorDetail {
    code: String,
    message: String,
}

#[derive(Deserialize)]
struct ErrorEnvelope {
    error: ErrorDetail,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SessionTokenDto {
    token: String,
    #[serde(default)]
    expires_at: Option<String>,
}

#[derive(Deserialize)]
struct LoginResponseBody {
    user: AuthUser,
    session: SessionTokenDto,
}

#[derive(Deserialize)]
struct RefreshResponseBody {
    session: SessionTokenDto,
}

#[derive(Deserialize)]
struct MeResponseBody {
    user: AuthUser,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EmailFlowResponseBody {
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    email_delivery: Option<String>,
    #[serde(default)]
    message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum EndpointState {
    Configured(String),
    Unconfigured,
    Invalid(AuthError),
}

fn is_loopback_host(host: &str) -> bool {
    let address = host
        .strip_prefix('[')
        .and_then(|inner| inner.strip_suffix(']'))
        .unwrap_or(host);
    address.eq_ignore_ascii_case("localhost")
        || address
            .parse::<std::net::IpAddr>()
            .map(|address| address.is_loopback())
            .unwrap_or(false)
}

fn contains_release_placeholder(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    [
        "replace_with",
        "change_me",
        "changeme",
        "placeholder",
        "nexora.local",
        "example.invalid",
        "example.test",
        "example.com",
        "example.org",
        ".example",
        ".invalid",
        ".test",
        "your_",
        "your-",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

/// Validates and normalizes a desktop backend base URL without ever echoing raw
/// configuration input into error strings.
pub(crate) fn validate_backend_base_url(
    raw: &str,
    reject_placeholders: bool,
    allow_loopback_http: bool,
) -> AuthResult<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.len() > 512 {
        return Err(AuthError::new(
            "invalid_backend_url",
            "Backend service URL is malformed.",
        ));
    }

    if trimmed
        .chars()
        .any(|character| {
            character.is_control()
                || character.is_whitespace()
                || matches!(character, '"' | '\'' | '\\')
        })
    {
        return Err(AuthError::new(
            "invalid_backend_url",
            "Backend service URL contains invalid characters.",
        ));
    }

    let parsed = Url::parse(trimmed).map_err(|_| {
        AuthError::new(
            "invalid_backend_url",
            "Backend service URL must be a valid absolute HTTP(S) URL.",
        )
    })?;
    if parsed.query().is_some() || parsed.fragment().is_some() {
        return Err(AuthError::new(
            "invalid_backend_url",
            "Backend service URL must not contain query parameters or fragments.",
        ));
    }

    let raw_authority = trimmed
        .split_once("://")
        .map(|(_, rest)| rest.split('/').next().unwrap_or(""))
        .unwrap_or("");
    if raw_authority.contains('@') || !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(AuthError::new(
            "invalid_backend_url",
            "Backend service URL must not embed credentials.",
        ));
    }

    let host = parsed.host_str().unwrap_or("");
    if host.is_empty() {
        return Err(AuthError::new(
            "invalid_backend_url",
            "Backend service URL must include a host.",
        ));
    }

    if reject_placeholders && contains_release_placeholder(trimmed) {
        return Err(AuthError::new(
            "backend_unconfigured",
            "Nexora online account services use an unconfigured placeholder URL in this build.",
        ));
    }

    let path = parsed.path().trim_matches('/');
    if !path.is_empty()
        && path.split('/').any(|segment| {
            segment.is_empty()
                || segment == "."
                || segment == ".."
                || !segment.chars().all(|character| {
                    character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
                })
        })
    {
        return Err(AuthError::new(
            "invalid_backend_url",
            "Backend service URL base path is invalid.",
        ));
    }

    match parsed.scheme() {
        "https" => {}
        "http" if is_loopback_host(host) && allow_loopback_http => {}
        "http" if is_loopback_host(host) => {
            return Err(AuthError::new(
                "insecure_backend_url",
                "Plaintext HTTP loopback backend URLs are disabled outside development/test.",
            ));
        }
        "http" => {
            return Err(AuthError::new(
                "insecure_backend_url",
                "Nexora requires HTTPS for all non-loopback backend endpoints.",
            ));
        }
        _ => {
            return Err(AuthError::new(
                "invalid_backend_url",
                "Unsupported backend service URL scheme; expected https://.",
            ));
        }
    }

    Ok(parsed.as_str().trim_end_matches('/').to_string())
}

fn resolve_desktop_environment(
    raw: Option<&str>,
    is_debug_build: bool,
) -> AuthResult<&'static str> {
    match raw.map(str::trim).filter(|value| !value.is_empty()) {
        Some("development") => Ok("development"),
        Some("test") => Ok("test"),
        Some("production") => Ok("production"),
        Some(_) => Err(AuthError::new(
            "invalid_environment",
            "NEXORA_ENV must be development, test, or production. (Value withheld.)",
        )),
        None if is_debug_build => Ok("development"),
        None => Ok("production"),
    }
}

fn resolve_runtime_environment(
    build_environment_raw: Option<&str>,
    runtime_environment_raw: Option<&str>,
    is_debug_build: bool,
) -> AuthResult<&'static str> {
    let build_environment = resolve_desktop_environment(build_environment_raw, is_debug_build)?;
    if let Some(runtime_environment_raw) = runtime_environment_raw
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let runtime_environment =
            resolve_desktop_environment(Some(runtime_environment_raw), is_debug_build)?;
        if runtime_environment != build_environment {
            return Err(AuthError::new(
                "environment_override_disabled",
                "Desktop deployment mode is fixed at trusted build time; runtime overrides are disabled.",
            ));
        }
    }

    Ok(build_environment)
}

fn parse_loopback_opt_in(raw: Option<&str>) -> AuthResult<bool> {
    match raw.map(str::trim) {
        None | Some("") | Some("false") => Ok(false),
        Some("true") => Ok(true),
        Some(_) => Err(AuthError::new(
            "invalid_environment",
            "NEXORA_ALLOW_LOOPBACK_HTTP must be exactly true or false. (Value withheld.)",
        )),
    }
}

fn read_runtime_environment(name: &str) -> AuthResult<Option<String>> {
    match std::env::var_os(name) {
        None => Ok(None),
        Some(value) => value.into_string().map(Some).map_err(|_| {
            AuthError::new(
                "invalid_environment",
                format!("{name} must contain valid UTF-8 text. (Value withheld.)"),
            )
        }),
    }
}

fn resolve_backend_endpoint_with(
    compile_url: Option<&str>,
    runtime_primary: Option<&str>,
    runtime_legacy: Option<&str>,
    is_debug_build: bool,
    allow_loopback_flag: bool,
    environment_raw: Option<&str>,
) -> EndpointState {
    let environment = match resolve_desktop_environment(environment_raw, is_debug_build) {
        Ok(environment) => environment,
        Err(error) => return EndpointState::Invalid(error),
    };
    let allow_loopback_http = environment != "production" && (is_debug_build || allow_loopback_flag);
    let reject_placeholders = environment == "production";

    let runtime_override = [runtime_primary, runtime_legacy]
        .into_iter()
        .flatten()
        .map(str::trim)
        .find(|value| !value.is_empty());
    if environment == "production" && runtime_override.is_some() {
        return EndpointState::Invalid(AuthError::new(
            "runtime_endpoint_override_disabled",
            "Production account endpoints are fixed at trusted build time; runtime overrides are disabled.",
        ));
    }

    let selected = if environment == "production" {
        compile_url.map(str::trim).filter(|value| !value.is_empty())
    } else {
        [runtime_primary, compile_url, runtime_legacy]
            .into_iter()
            .flatten()
            .map(str::trim)
            .find(|value| !value.is_empty())
    };

    match selected {
        Some(candidate) => match validate_backend_base_url(
            candidate,
            reject_placeholders,
            allow_loopback_http,
        ) {
            Ok(valid) => EndpointState::Configured(valid),
            Err(error) => EndpointState::Invalid(error),
        },
        None if allow_loopback_http => {
            EndpointState::Configured(DEFAULT_LOOPBACK_AUTH_URL.to_string())
        }
        None => EndpointState::Unconfigured,
    }
}

fn build_http_client() -> Result<Client, AuthError> {
    Client::builder()
        .timeout(Duration::from_secs(10))
        .connect_timeout(Duration::from_secs(5))
        .redirect(Policy::none())
        .build()
        .map_err(|_| {
            AuthError::new(
                "http_client_unavailable",
                "The online account HTTP client could not be initialized; local chat remains available.",
            )
        })
}

pub struct AuthApiClient {
    endpoint: EndpointState,
    client: Result<Client, AuthError>,
}

impl fmt::Debug for AuthApiClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuthApiClient")
            .field("endpoint", &self.base_url())
            .finish()
    }
}

impl Default for AuthApiClient {
    fn default() -> Self {
        Self::new()
    }
}

impl AuthApiClient {
    pub fn new() -> Self {
        let endpoint = (|| -> AuthResult<EndpointState> {
            let runtime_primary = read_runtime_environment("NEXORA_AUTH_API_URL")?;
            let runtime_legacy = read_runtime_environment("AUTH_API_BASE_URL")?;
            let runtime_mode = read_runtime_environment("NEXORA_ENV")?;
            let build_mode = resolve_runtime_environment(
                COMPILE_TIME_ENVIRONMENT,
                runtime_mode.as_deref(),
                cfg!(debug_assertions),
            )?;
            let allow_loopback = read_runtime_environment("NEXORA_ALLOW_LOOPBACK_HTTP")?;
            let allow_loopback_flag = parse_loopback_opt_in(allow_loopback.as_deref())?;
            Ok(resolve_backend_endpoint_with(
                COMPILE_TIME_AUTH_API_URL,
                runtime_primary.as_deref(),
                runtime_legacy.as_deref(),
                cfg!(debug_assertions),
                allow_loopback_flag,
                Some(build_mode),
            ))
        })()
        .unwrap_or_else(EndpointState::Invalid);

        Self {
            endpoint,
            client: build_http_client(),
        }
    }

    #[cfg(test)]
    pub fn with_base_url(base_url: impl Into<String>) -> Self {
        let raw = base_url.into();
        let endpoint = match validate_backend_base_url(&raw, false, true) {
            Ok(valid) => EndpointState::Configured(valid),
            Err(err) => EndpointState::Invalid(err),
        };

        Self {
            endpoint,
            client: build_http_client(),
        }
    }

    #[cfg(test)]
    pub fn unconfigured() -> Self {
        Self::from_endpoint_state(EndpointState::Unconfigured, Duration::from_millis(250))
    }

    #[cfg(test)]
    fn from_endpoint_state(endpoint: EndpointState, timeout: Duration) -> Self {
        let client = Client::builder()
            .timeout(timeout)
            .connect_timeout(timeout)
            .redirect(Policy::none())
            .build()
            .map_err(|_| {
                AuthError::new(
                    "http_client_unavailable",
                    "The online account HTTP client could not be initialized; local chat remains available.",
                )
            });
        Self { endpoint, client }
    }

    pub fn base_url(&self) -> &str {
        match &self.endpoint {
            EndpointState::Configured(url) => url.as_str(),
            EndpointState::Unconfigured => "unconfigured",
            EndpointState::Invalid(_) => "invalid_backend_url",
        }
    }

    pub fn is_configured(&self) -> bool {
        matches!(self.endpoint, EndpointState::Configured(_)) && self.client.is_ok()
    }

    fn http_client(&self) -> AuthResult<&Client> {
        self.client.as_ref().map_err(Clone::clone)
    }

    fn require_base_url(&self) -> AuthResult<&str> {
        match &self.endpoint {
            EndpointState::Configured(url) => Ok(url.as_str()),
            EndpointState::Unconfigured => Err(AuthError::new(
                "backend_unconfigured",
                "Nexora online account services are not configured in this build. Local AI models and workspaces remain available offline.",
            )),
            EndpointState::Invalid(err) => Err(err.clone()),
        }
    }

    fn url(&self, path: &str) -> AuthResult<String> {
        Ok(format!("{}/v1/auth{}", self.require_base_url()?, path))
    }

    fn v1_url(&self, path: &str) -> AuthResult<String> {
        Ok(format!("{}/v1{}", self.require_base_url()?, path))
    }

    pub async fn login(&self, email: &str, password: &str) -> AuthResult<(AuthUser, String)> {
        #[derive(Serialize)]
        struct Body<'a> {
            email: &'a str,
            password: &'a str,
        }

        let resp = self
            .http_client()?
            .post(self.url("/login")?)
            .json(&Body { email, password })
            .send()
            .await
            .map_err(map_transport_error)?;

        let status = resp.status();
        let headers = resp.headers().clone();

        if status.is_success() {
            let body: LoginResponseBody = resp
                .json()
                .await
                .map_err(|_| AuthError::new("parse_error", "Invalid login response payload."))?;
            return Ok((body.user, body.session.token));
        }

        let retry_after = parse_retry_after(&headers);
        let error_body = resp.json::<ErrorEnvelope>().await.ok();

        match status.as_u16() {
            401 => Err(AuthError::invalid_credentials()),
            403 => {
                if let Some(ref err) = error_body {
                    if err.error.code == "email_unverified"
                        || err.error.code == "email_not_verified"
                    {
                        return Err(AuthError::email_unverified());
                    }
                    return Err(AuthError::new(&err.error.code, &err.error.message));
                }
                Err(AuthError::email_unverified())
            }
            429 => {
                let retry = retry_after.unwrap_or(60);
                let msg = error_body
                    .map(|e| e.error.message)
                    .unwrap_or_else(|| "Too many sign-in attempts. Try again shortly.".to_string());
                Err(AuthError::with_retry("rate_limited", msg, retry))
            }
            _ => {
                if let Some(err) = error_body {
                    Err(AuthError::new(err.error.code, err.error.message))
                } else {
                    Err(AuthError::new(
                        "login_failed",
                        "Sign-in could not be completed. Please try again.",
                    ))
                }
            }
        }
    }

    pub async fn register(
        &self,
        name: &str,
        email: &str,
        password: &str,
    ) -> AuthResult<AuthStatusResponse> {
        #[derive(Serialize)]
        struct Body<'a> {
            name: &'a str,
            email: &'a str,
            password: &'a str,
        }

        let resp = self
            .http_client()?
            .post(self.url("/register")?)
            .json(&Body {
                name,
                email,
                password,
            })
            .send()
            .await
            .map_err(map_transport_error)?;

        let status = resp.status();
        let headers = resp.headers().clone();

        if status.is_success() || status.as_u16() == 202 {
            let parsed = resp.json::<EmailFlowResponseBody>().await.ok();
            let message = parsed
                .as_ref()
                .and_then(|b| b.message.clone())
                .unwrap_or_else(|| {
                    "Registration was accepted, but the service did not confirm whether a verification email was sent.".to_string()
                });
            let delivery = parsed.and_then(|b| b.email_delivery);
            return Ok(AuthStatusResponse::with_delivery(
                "verification_pending",
                message,
                delivery,
            ));
        }

        let retry_after = parse_retry_after(&headers);
        let error_body = resp.json::<ErrorEnvelope>().await.ok();

        if status.as_u16() == 409 {
            return Err(AuthError::new(
                "email_exists",
                "An account with this email address already exists.",
            ));
        }

        if status.as_u16() == 429 {
            let retry = retry_after.unwrap_or(60);
            let msg = error_body
                .map(|e| e.error.message)
                .unwrap_or_else(|| "Too many attempts. Try again in a few minutes.".to_string());
            return Err(AuthError::with_retry("rate_limited", msg, retry));
        }

        if let Some(err) = error_body {
            Err(AuthError::new(err.error.code, err.error.message))
        } else {
            Err(AuthError::new(
                "registration_failed",
                "Account registration could not be completed.",
            ))
        }
    }

    pub async fn verify_email(&self, token: &str) -> AuthResult<()> {
        #[derive(Serialize)]
        struct Body<'a> {
            token: &'a str,
        }

        let resp = self
            .http_client()?
            .post(self.url("/verify-email")?)
            .json(&Body { token })
            .send()
            .await
            .map_err(map_transport_error)?;

        if resp.status().is_success() {
            return Ok(());
        }

        handle_api_error(resp, false).await
    }

    pub async fn resend_verification(&self, email: &str) -> AuthResult<AuthStatusResponse> {
        #[derive(Serialize)]
        struct Body<'a> {
            email: &'a str,
        }

        let resp = self
            .http_client()?
            .post(self.url("/resend-verification")?)
            .json(&Body { email })
            .send()
            .await
            .map_err(map_transport_error)?;

        if resp.status().is_success() || resp.status().as_u16() == 202 {
            let parsed = resp.json::<EmailFlowResponseBody>().await.ok();
            let status_label = parsed
                .as_ref()
                .and_then(|b| b.status.clone())
                .unwrap_or_else(|| "accepted".to_string());
            let message = parsed
                .as_ref()
                .and_then(|b| b.message.clone())
                .unwrap_or_else(|| {
                    "If an account exists and email delivery succeeds, verification instructions will be sent."
                        .to_string()
                });
            let delivery = parsed.and_then(|b| b.email_delivery);
            return Ok(AuthStatusResponse::with_delivery(
                status_label,
                message,
                delivery,
            ));
        }

        handle_api_error(resp, false).await
    }

    pub async fn recovery_request(&self, email: &str) -> AuthResult<AuthStatusResponse> {
        #[derive(Serialize)]
        struct Body<'a> {
            email: &'a str,
        }

        let resp = self
            .http_client()?
            .post(self.url("/recovery-request")?)
            .json(&Body { email })
            .send()
            .await
            .map_err(map_transport_error)?;

        if resp.status().is_success() || resp.status().as_u16() == 202 {
            let parsed = resp.json::<EmailFlowResponseBody>().await.ok();
            let status_label = parsed
                .as_ref()
                .and_then(|b| b.status.clone())
                .unwrap_or_else(|| "accepted".to_string());
            let message = parsed
                .as_ref()
                .and_then(|b| b.message.clone())
                .unwrap_or_else(|| {
                    "If an account exists and email delivery succeeds, password-reset instructions will be sent."
                        .to_string()
                });
            let delivery = parsed.and_then(|b| b.email_delivery);
            return Ok(AuthStatusResponse::with_delivery(
                status_label,
                message,
                delivery,
            ));
        }

        handle_api_error(resp, false).await
    }

    pub async fn reset_password(&self, token: &str, password: &str) -> AuthResult<()> {
        #[derive(Serialize)]
        struct Body<'a> {
            token: &'a str,
            password: &'a str,
        }

        let resp = self
            .http_client()?
            .post(self.url("/reset-password")?)
            .json(&Body { token, password })
            .send()
            .await
            .map_err(map_transport_error)?;

        if resp.status().is_success() || resp.status().as_u16() == 204 {
            return Ok(());
        }

        handle_api_error(resp, false).await
    }

    pub async fn get_me(&self, session_token: &str) -> AuthResult<AuthUser> {
        let resp = self
            .http_client()?
            .get(self.url("/me")?)
            .header(AUTHORIZATION, format!("Bearer {session_token}"))
            .send()
            .await
            .map_err(map_transport_error)?;

        if resp.status().is_success() {
            let body: MeResponseBody = resp
                .json()
                .await
                .map_err(|_| AuthError::new("parse_error", "Invalid user session payload."))?;
            return Ok(body.user);
        }

        handle_api_error(resp, true).await
    }

    /// Rotates a live bearer session via `POST /v1/auth/refresh`, returning the
    /// newly issued token and optional server expiry timestamp.
    pub async fn refresh(&self, session_token: &str) -> AuthResult<SessionCredentials> {
        let resp = self
            .http_client()?
            .post(self.url("/refresh")?)
            .header(AUTHORIZATION, format!("Bearer {session_token}"))
            .send()
            .await
            .map_err(map_transport_error)?;

        if resp.status().is_success() {
            let body: RefreshResponseBody = resp
                .json()
                .await
                .map_err(|_| AuthError::new("parse_error", "Invalid session refresh payload."))?;
            if body.session.token.trim().is_empty() {
                return Err(AuthError::new(
                    "parse_error",
                    "Session refresh payload omitted bearer token.",
                ));
            }
            return Ok(SessionCredentials::new(
                body.session.token,
                body.session.expires_at,
            ));
        }

        handle_api_error(resp, true).await
    }

    pub async fn logout(&self, session_token: &str) -> AuthResult<()> {
        if let Ok(target) = self.url("/logout") {
            let _ = self
                .http_client()?
                .post(target)
                .header(AUTHORIZATION, format!("Bearer {session_token}"))
                .send()
                .await;
        }

        Ok(())
    }

    pub async fn password_change(
        &self,
        session_token: &str,
        current_password: &str,
        new_password: &str,
    ) -> AuthResult<()> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Body<'a> {
            current_password: &'a str,
            new_password: &'a str,
        }

        let resp = self
            .http_client()?
            .post(self.url("/password-change")?)
            .header(AUTHORIZATION, format!("Bearer {session_token}"))
            .json(&Body {
                current_password,
                new_password,
            })
            .send()
            .await
            .map_err(map_transport_error)?;

        if resp.status().is_success() || resp.status().as_u16() == 204 {
            return Ok(());
        }

        handle_api_error(resp, true).await
    }

    pub async fn google_exchange(
        &self,
        code: &str,
        code_verifier: &str,
        state: &str,
        redirect_uri: &str,
        nonce: Option<&str>,
    ) -> AuthResult<(AuthUser, String)> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Body<'a> {
            code: &'a str,
            code_verifier: &'a str,
            state: &'a str,
            redirect_uri: &'a str,
            #[serde(skip_serializing_if = "Option::is_none")]
            nonce: Option<&'a str>,
        }

        let resp = self
            .http_client()?
            .post(self.url("/google/exchange")?)
            .json(&Body {
                code,
                code_verifier,
                state,
                redirect_uri,
                nonce,
            })
            .send()
            .await
            .map_err(map_transport_error)?;

        let status = resp.status();
        let headers = resp.headers().clone();

        if status.is_success() {
            let body: LoginResponseBody = resp.json().await.map_err(|_| {
                AuthError::new("parse_error", "Invalid Google authentication payload.")
            })?;
            return Ok((body.user, body.session.token));
        }

        if status.as_u16() == 409 {
            return Err(AuthError::new(
                "email_conflict",
                "A Nexora account with this email already exists. Sign in to that account before linking Google.",
            ));
        }

        let retry_after = parse_retry_after(&headers);
        let error_body = resp.json::<ErrorEnvelope>().await.ok();

        if status.as_u16() == 429 {
            let retry = retry_after.unwrap_or(60);
            return Err(AuthError::with_retry(
                "rate_limited",
                "Too many attempts. Try again in a few minutes.",
                retry,
            ));
        }

        if let Some(err) = error_body {
            Err(AuthError::new(err.error.code, err.error.message))
        } else {
            Err(AuthError::new(
                "google_failed",
                "Google authentication could not be completed.",
            ))
        }
    }

    pub async fn get_account_profile(
        &self,
        session_token: &str,
    ) -> AuthResult<super::AccountProfile> {
        #[derive(Deserialize)]
        struct Envelope {
            profile: super::AccountProfile,
        }
        let resp = self
            .http_client()?
            .get(self.v1_url("/account/profile")?)
            .header(AUTHORIZATION, format!("Bearer {session_token}"))
            .send()
            .await
            .map_err(map_transport_error)?;
        if resp.status().is_success() {
            let env: Envelope = resp
                .json()
                .await
                .map_err(|_| AuthError::new("parse_error", "Invalid profile payload."))?;
            return Ok(env.profile);
        }
        handle_api_error(resp, true).await
    }

    pub async fn update_account_profile(
        &self,
        session_token: &str,
        payload: &super::UpdateProfilePayload,
    ) -> AuthResult<super::AccountProfile> {
        #[derive(Deserialize)]
        struct Envelope {
            profile: super::AccountProfile,
        }
        let resp = self
            .http_client()?
            .patch(self.v1_url("/account/profile")?)
            .header(AUTHORIZATION, format!("Bearer {session_token}"))
            .json(payload)
            .send()
            .await
            .map_err(map_transport_error)?;
        if resp.status().is_success() {
            let env: Envelope = resp
                .json()
                .await
                .map_err(|_| AuthError::new("parse_error", "Invalid profile payload."))?;
            return Ok(env.profile);
        }
        handle_api_error(resp, true).await
    }

    pub async fn link_google(
        &self,
        session_token: &str,
        code: &str,
        code_verifier: &str,
        state: &str,
        redirect_uri: &str,
        nonce: Option<&str>,
    ) -> AuthResult<super::AccountProfile> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Body<'a> {
            code: &'a str,
            code_verifier: &'a str,
            state: &'a str,
            redirect_uri: &'a str,
            #[serde(skip_serializing_if = "Option::is_none")]
            nonce: Option<&'a str>,
        }
        #[derive(Deserialize)]
        struct Envelope {
            profile: super::AccountProfile,
        }
        let resp = self
            .http_client()?
            .post(self.url("/google/link")?)
            .header(AUTHORIZATION, format!("Bearer {session_token}"))
            .json(&Body {
                code,
                code_verifier,
                state,
                redirect_uri,
                nonce,
            })
            .send()
            .await
            .map_err(map_transport_error)?;
        if resp.status().is_success() {
            let env: Envelope = resp
                .json()
                .await
                .map_err(|_| AuthError::new("parse_error", "Invalid profile payload."))?;
            return Ok(env.profile);
        }
        handle_api_error(resp, true).await
    }

    pub async fn export_account_data(&self, session_token: &str) -> AuthResult<serde_json::Value> {
        let resp = self
            .http_client()?
            .get(self.v1_url("/account/export")?)
            .header(AUTHORIZATION, format!("Bearer {session_token}"))
            .send()
            .await
            .map_err(map_transport_error)?;
        if resp.status().is_success() {
            return resp
                .json()
                .await
                .map_err(|_| AuthError::new("parse_error", "Invalid account export payload."));
        }
        handle_api_error(resp, true).await
    }

    pub async fn delete_account(
        &self,
        session_token: &str,
        payload: &super::DeleteAccountPayload,
    ) -> AuthResult<()> {
        let resp = self
            .http_client()?
            .post(self.v1_url("/account/delete")?)
            .header(AUTHORIZATION, format!("Bearer {session_token}"))
            .json(payload)
            .send()
            .await
            .map_err(map_transport_error)?;
        if resp.status().is_success() || resp.status().as_u16() == 204 {
            return Ok(());
        }
        handle_api_error(resp, true).await
    }

    pub async fn list_plans(&self) -> AuthResult<super::PlanCatalogDto> {
        let resp = self
            .http_client()?
            .get(self.v1_url("/plans")?)
            .send()
            .await
            .map_err(map_transport_error)?;
        if resp.status().is_success() {
            return resp
                .json()
                .await
                .map_err(|_| AuthError::new("parse_error", "Invalid plans payload."));
        }
        handle_api_error(resp, false).await
    }

    pub async fn get_billing_status(
        &self,
        session_token: &str,
    ) -> AuthResult<super::BillingStatusDto> {
        let resp = self
            .http_client()?
            .get(self.v1_url("/billing/subscription")?)
            .header(AUTHORIZATION, format!("Bearer {session_token}"))
            .send()
            .await
            .map_err(map_transport_error)?;
        if resp.status().is_success() {
            return resp
                .json()
                .await
                .map_err(|_| AuthError::new("parse_error", "Invalid billing status payload."));
        }
        handle_api_error(resp, true).await
    }

    pub async fn create_checkout(
        &self,
        session_token: &str,
        payload: &super::CheckoutPayload,
    ) -> AuthResult<super::CheckoutSessionDto> {
        let resp = self
            .http_client()?
            .post(self.v1_url("/billing/checkout")?)
            .header(AUTHORIZATION, format!("Bearer {session_token}"))
            .json(payload)
            .send()
            .await
            .map_err(map_transport_error)?;
        if resp.status().is_success() {
            return resp
                .json()
                .await
                .map_err(|_| AuthError::new("parse_error", "Invalid checkout session payload."));
        }
        handle_api_error(resp, true).await
    }

    pub async fn verify_order(
        &self,
        session_token: &str,
        payload: &super::VerifyOrderPayload,
    ) -> AuthResult<super::BillingStatusDto> {
        let resp = self
            .http_client()?
            .post(self.v1_url("/billing/verify")?)
            .header(AUTHORIZATION, format!("Bearer {session_token}"))
            .json(payload)
            .send()
            .await
            .map_err(map_transport_error)?;
        if resp.status().is_success() {
            return resp
                .json()
                .await
                .map_err(|_| AuthError::new("parse_error", "Invalid billing verify payload."));
        }
        handle_api_error(resp, true).await
    }

    pub async fn cancel_subscription(
        &self,
        session_token: &str,
        payload: &super::CancelSubscriptionPayload,
    ) -> AuthResult<super::BillingStatusDto> {
        let resp = self
            .http_client()?
            .post(self.v1_url("/billing/cancel")?)
            .header(AUTHORIZATION, format!("Bearer {session_token}"))
            .json(payload)
            .send()
            .await
            .map_err(map_transport_error)?;
        if resp.status().is_success() {
            return resp.json().await.map_err(|_| {
                AuthError::new("parse_error", "Invalid subscription cancel payload.")
            });
        }
        handle_api_error(resp, true).await
    }

    pub async fn change_plan(
        &self,
        session_token: &str,
        payload: &super::ChangePlanPayload,
    ) -> AuthResult<super::BillingStatusDto> {
        let resp = self
            .http_client()?
            .post(self.v1_url("/billing/change-plan")?)
            .header(AUTHORIZATION, format!("Bearer {session_token}"))
            .json(payload)
            .send()
            .await
            .map_err(map_transport_error)?;
        if resp.status().is_success() {
            return resp
                .json()
                .await
                .map_err(|_| AuthError::new("parse_error", "Invalid plan change payload."));
        }
        handle_api_error(resp, true).await
    }

    pub async fn create_portal(&self, session_token: &str) -> AuthResult<super::PortalSessionDto> {
        let resp = self
            .http_client()?
            .post(self.v1_url("/billing/portal")?)
            .header(AUTHORIZATION, format!("Bearer {session_token}"))
            .send()
            .await
            .map_err(map_transport_error)?;
        if resp.status().is_success() {
            return resp
                .json()
                .await
                .map_err(|_| AuthError::new("parse_error", "Invalid billing portal payload."));
        }
        handle_api_error(resp, true).await
    }

    pub async fn get_usage_summary(
        &self,
        session_token: &str,
    ) -> AuthResult<super::ServerUsageSummaryDto> {
        let resp = self
            .http_client()?
            .get(self.v1_url("/usage")?)
            .header(AUTHORIZATION, format!("Bearer {session_token}"))
            .send()
            .await
            .map_err(map_transport_error)?;
        if resp.status().is_success() {
            return resp
                .json()
                .await
                .map_err(|_| AuthError::new("parse_error", "Invalid usage summary payload."));
        }
        handle_api_error(resp, true).await
    }

    pub async fn reserve_usage(
        &self,
        session_token: &str,
        model_id: &str,
        request_id: &str,
    ) -> AuthResult<super::ReserveQuotaResponseDto> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Body<'a> {
            model_id: &'a str,
            request_id: &'a str,
        }
        let resp = self
            .http_client()?
            .post(self.v1_url("/usage/reserve")?)
            .header(AUTHORIZATION, format!("Bearer {session_token}"))
            .json(&Body {
                model_id,
                request_id,
            })
            .send()
            .await
            .map_err(map_transport_error)?;
        if resp.status().is_success() {
            return resp
                .json()
                .await
                .map_err(|_| AuthError::new("parse_error", "Invalid quota reservation payload."));
        }
        handle_api_error(resp, true).await
    }

    pub async fn settle_usage(
        &self,
        session_token: &str,
        reservation_id: &str,
        commit: bool,
    ) -> AuthResult<super::ServerUsageSummaryDto> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Body<'a> {
            reservation_id: &'a str,
        }
        let endpoint = if commit {
            "/usage/commit"
        } else {
            "/usage/release"
        };
        let resp = self
            .http_client()?
            .post(self.v1_url(endpoint)?)
            .header(AUTHORIZATION, format!("Bearer {session_token}"))
            .json(&Body { reservation_id })
            .send()
            .await
            .map_err(map_transport_error)?;
        if resp.status().is_success() {
            return resp
                .json()
                .await
                .map_err(|_| AuthError::new("parse_error", "Invalid quota settlement payload."));
        }
        handle_api_error(resp, true).await
    }
}

fn map_transport_error(err: reqwest::Error) -> AuthError {
    if err.is_timeout() {
        AuthError::network(
            "The Nexora account service timed out. Check your connection and try again.",
        )
    } else {
        AuthError::network(
            "Can't reach the Nexora account service. Check your connection and try again.",
        )
    }
}

fn parse_retry_after(headers: &HeaderMap<HeaderValue>) -> Option<u64> {
    headers
        .get("retry-after")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
}

async fn handle_api_error<T>(
    resp: reqwest::Response,
    authenticated_request: bool,
) -> AuthResult<T> {
    let status = resp.status();
    let headers = resp.headers().clone();

    let retry_after = parse_retry_after(&headers);
    let error_body = resp.json::<ErrorEnvelope>().await.ok();

    if status.as_u16() == 429 {
        let retry = retry_after.unwrap_or(60);
        let msg = error_body
            .map(|e| e.error.message)
            .unwrap_or_else(|| "Too many requests. Please wait before trying again.".to_string());
        return Err(AuthError::with_retry("rate_limited", msg, retry));
    }

    if let Some(err) = error_body {
        if authenticated_request && err.error.code == "invalid_or_expired_token" {
            return Err(AuthError::session_expired());
        }
        Err(AuthError::new(err.error.code, err.error.message))
    } else {
        Err(AuthError::new(
            format!("http_{}", status.as_u16()),
            format!("Request failed with status {}", status.as_u16()),
        ))
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    use super::{
        parse_loopback_opt_in, resolve_backend_endpoint_with, resolve_desktop_environment,
        resolve_runtime_environment, validate_backend_base_url, AuthApiClient, EndpointState,
    };
    use crate::auth::SessionCredentials;

    fn spawn_single_response_server(
        status_line: &'static str,
        extra_headers: &'static str,
        body: &'static str,
    ) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock listener");
        let addr = listener.local_addr().expect("mock addr");
        std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 4096];
                let _ = stream.read(&mut buf);
                let response = format!(
                    "HTTP/1.1 {status_line}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n{extra_headers}\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes());
            }
        });
        format!("http://{addr}")
    }

    #[test]
    fn release_mode_uses_compile_time_https_url_and_blocks_http() {
        let resolved = resolve_backend_endpoint_with(
            Some("https://auth.nexora-example.net/"),
            None,
            None,
            false,
            false,
            None,
        );
        assert_eq!(
            resolved,
            EndpointState::Configured("https://auth.nexora-example.net".to_string())
        );

        let unconfigured = resolve_backend_endpoint_with(None, None, None, false, false, None);
        assert_eq!(unconfigured, EndpointState::Unconfigured);

        let loopback_in_release = resolve_backend_endpoint_with(
            Some("http://127.0.0.1:8787"),
            None,
            None,
            false,
            false,
            None,
        );
        assert!(
            matches!(loopback_in_release, EndpointState::Invalid(err) if err.code == "insecure_backend_url")
        );
    }

    #[test]
    fn rejects_non_loopback_http_in_all_modes_even_with_loopback_opt_in() {
        for (is_debug, allow_loopback) in
            [(false, false), (false, true), (true, false), (true, true)]
        {
            let res = resolve_backend_endpoint_with(
                None,
                Some("http://203.0.113.42:8787"),
                None,
                is_debug,
                allow_loopback,
                None,
            );
            assert!(
                matches!(res, EndpointState::Invalid(ref err) if err.code == "insecure_backend_url"),
                "expected insecure_backend_url for debug={is_debug}, opt_in={allow_loopback}"
            );
        }

        let explicit_client = AuthApiClient::with_base_url("http://remote-host.org:8787");
        assert!(!explicit_client.is_configured());
    }

    #[test]
    fn allows_loopback_http_only_in_debug_or_explicit_opt_in() {
        let debug_default = resolve_backend_endpoint_with(None, None, None, true, false, None);
        assert_eq!(
            debug_default,
            EndpointState::Configured("http://127.0.0.1:8787".to_string())
        );

        let release_opt_in = resolve_backend_endpoint_with(
            None,
            Some("http://localhost:9898/"),
            None,
            false,
            true,
            Some("test"),
        );
        assert_eq!(
            release_opt_in,
            EndpointState::Configured("http://localhost:9898".to_string())
        );

        let ipv6_loopback = validate_backend_base_url("http://[::1]:8787/", false, true).unwrap();
        assert_eq!(ipv6_loopback, "http://[::1]:8787");
    }

    #[test]
    fn desktop_environment_cannot_be_changed_after_build() {
        let downgrade = resolve_runtime_environment(
            Some("production"),
            Some("development"),
            false,
        )
        .unwrap_err();
        assert_eq!(downgrade.code, "environment_override_disabled");

        let mismatch = resolve_runtime_environment(Some("test"), Some("production"), false)
            .unwrap_err();
        assert_eq!(mismatch.code, "environment_override_disabled");

        assert_eq!(
            resolve_runtime_environment(Some("development"), Some("development"), false)
                .unwrap(),
            "development"
        );
        assert_eq!(
            resolve_runtime_environment(Some("production"), None, false).unwrap(),
            "production"
        );
    }

    #[test]
    fn production_endpoint_is_pinned_to_build_configuration() {
        let runtime_override = resolve_backend_endpoint_with(
            Some("https://auth.nexora-example.net"),
            Some("https://attacker.nexora-example.net"),
            None,
            false,
            false,
            Some("production"),
        );
        assert!(matches!(
            runtime_override,
            EndpointState::Invalid(err) if err.code == "runtime_endpoint_override_disabled"
        ));

        let production_endpoint = resolve_backend_endpoint_with(
            Some("https://auth.nexora-example.net"),
            None,
            None,
            false,
            false,
            Some("production"),
        );
        assert!(matches!(
            production_endpoint,
            EndpointState::Configured(url) if url == "https://auth.nexora-example.net"
        ));
    }

    #[test]
    fn production_mode_cannot_be_overridden_into_plaintext_loopback() {
        let production_loopback = resolve_backend_endpoint_with(
            Some("http://127.0.0.1:8787"),
            None,
            None,
            true,
            true,
            Some("production"),
        );
        assert!(matches!(
            production_loopback,
            EndpointState::Invalid(err) if err.code == "insecure_backend_url"
        ));

        let release_development_without_opt_in = resolve_backend_endpoint_with(
            Some("http://127.0.0.1:8787"),
            None,
            None,
            false,
            false,
            Some("development"),
        );
        assert!(matches!(
            release_development_without_opt_in,
            EndpointState::Invalid(err) if err.code == "insecure_backend_url"
        ));

        let release_test_opt_in = resolve_backend_endpoint_with(
            Some("http://127.0.0.1:8787"),
            None,
            None,
            false,
            true,
            Some("test"),
        );
        assert!(matches!(release_test_opt_in, EndpointState::Configured(_)));
        assert!(parse_loopback_opt_in(Some("yes")).is_err());
        assert!(resolve_desktop_environment(Some("staging"), false).is_err());
    }

    #[test]
    fn validates_url_and_never_leaks_raw_secrets_in_errors() {
        let embedded_secret = "super-secret-inline-password-99182";
        let bad = validate_backend_base_url(
            &format!("https://admin:{embedded_secret}@api.nexora-example.org"),
            true,
            false,
        )
        .unwrap_err();
        assert_eq!(bad.code, "invalid_backend_url");
        assert!(!bad.message.contains(embedded_secret));

        let query_secret = validate_backend_base_url(
            "https://api.nexora-example.org?api_key=leak-me-not",
            true,
            false,
        )
        .unwrap_err();
        assert_eq!(query_secret.code, "invalid_backend_url");
        assert!(!query_secret.message.contains("leak-me-not"));

        let placeholder =
            validate_backend_base_url("https://REPLACE_WITH_PROD_HOST.com", true, false)
                .unwrap_err();
        assert_eq!(placeholder.code, "backend_unconfigured");

        let creds = SessionCredentials::new("raw-bearer-token-should-never-print", None);
        let formatted = format!("{creds:?}");
        assert!(!formatted.contains("raw-bearer-token-should-never-print"));
        assert!(formatted.contains("<redacted>"));
    }

    #[tokio::test]
    async fn unconfigured_release_client_fails_cleanly_without_panicking() {
        let client =
            AuthApiClient::from_endpoint_state(EndpointState::Unconfigured, Duration::from_secs(1));
        let err = client.list_plans().await.unwrap_err();
        assert_eq!(err.code, "backend_unconfigured");
    }

    #[tokio::test]
    async fn authenticated_401_and_429_and_unconfigured_email_map_cleanly() {
        let url_401 = spawn_single_response_server(
            "401 Unauthorized",
            "",
            r#"{"error":{"code":"invalid_or_expired_token","message":"Expired token"}}"#,
        );
        let client_401 = AuthApiClient::with_base_url(url_401);
        let err_401 = client_401.get_account_profile("tok-1").await.unwrap_err();
        assert_eq!(err_401.code, "session_expired");

        // An authenticated password change can also return HTTP 401 for a
        // wrong current password; that must not be mistaken for an expired
        // bearer session or trigger local sign-out.
        let wrong_password_url = spawn_single_response_server(
            "401 Unauthorized",
            "",
            r#"{"error":{"code":"invalid_credentials","message":"Current password is incorrect."}}"#,
        );
        let wrong_password_client = AuthApiClient::with_base_url(wrong_password_url);
        let wrong_password = wrong_password_client
            .password_change("tok-1", "wrong-current-password", "new-password-value")
            .await
            .unwrap_err();
        assert_eq!(wrong_password.code, "invalid_credentials");
        assert!(!wrong_password.is_session_invalid());

        let url_429 = spawn_single_response_server(
            "429 Too Many Requests",
            "Retry-After: 42\r\n",
            r#"{"error":{"code":"rate_limited","message":"Slow down"}}"#,
        );
        let client_429 = AuthApiClient::with_base_url(url_429);
        let err_429 = client_429.get_usage_summary("tok-1").await.unwrap_err();
        assert_eq!(err_429.code, "rate_limited");
        assert_eq!(err_429.retry_after_seconds, Some(42));

        let url_reg = spawn_single_response_server(
            "202 Accepted",
            "",
            r#"{"status":"accepted","emailDelivery":"unconfigured","message":"Account created, email transport is not configured yet."}"#,
        );
        let client_reg = AuthApiClient::with_base_url(url_reg);
        let status = client_reg
            .register("Dev User", "dev@example.org", "Str0ngPass!99")
            .await
            .unwrap();
        assert_eq!(status.status, "verification_pending");
        assert_eq!(status.email_delivery.as_deref(), Some("unconfigured"));
        assert!(status.message.contains("not configured"));
    }

    #[tokio::test]
    async fn email_flow_fallbacks_never_claim_a_message_was_sent() {
        let registration = AuthApiClient::with_base_url(spawn_single_response_server(
            "202 Accepted",
            "",
            "not-json",
        ));
        let status = registration
            .register("Ada", "ada@example.test", "long enough password")
            .await
            .unwrap();
        assert!(status.message.contains("did not confirm"));
        assert!(!status.message.to_lowercase().contains("has been sent"));

        let resend = AuthApiClient::with_base_url(spawn_single_response_server(
            "202 Accepted",
            "",
            "not-json",
        ));
        let status = resend
            .resend_verification("ada@example.test")
            .await
            .unwrap();
        assert_eq!(status.status, "accepted");
        assert!(status.message.contains("delivery succeeds"));
        assert!(!status.message.to_lowercase().contains("has been sent"));

        let recovery = AuthApiClient::with_base_url(spawn_single_response_server(
            "202 Accepted",
            "",
            "not-json",
        ));
        let status = recovery
            .recovery_request("ada@example.test")
            .await
            .unwrap();
        assert_eq!(status.status, "accepted");
        assert!(status.message.contains("delivery succeeds"));
        assert!(!status.message.to_lowercase().contains("has been sent"));
    }

    #[tokio::test]
    async fn bounded_request_timeout_maps_to_network_error_without_hanging() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind stalled listener");
        let addr = listener.local_addr().expect("addr");
        let accepted = Arc::new(AtomicUsize::new(0));
        let accepted_clone = Arc::clone(&accepted);
        std::thread::spawn(move || {
            if let Ok((_stream, _)) = listener.accept() {
                accepted_clone.fetch_add(1, Ordering::SeqCst);
                std::thread::sleep(Duration::from_millis(400));
            }
        });

        let client = AuthApiClient::from_endpoint_state(
            EndpointState::Configured(format!("http://{addr}")),
            Duration::from_millis(80),
        );
        let err = client.list_plans().await.unwrap_err();
        assert_eq!(err.code, "network_error");
        assert_eq!(accepted.load(Ordering::SeqCst), 1);
    }

    /// End-to-end TCP wire verification between `apps/desktop/src-tauri` (`AuthApiClient`)
    /// and a live `nexora-auth` process backed by PostgreSQL 16.
    #[tokio::test]
    #[ignore = "needs live nexora-auth URL in NEXORA_LIVE_AUTH_TEST_URL and pre-verified user token"]
    async fn live_desktop_client_roundtrip_over_tcp() {
        let url = std::env::var("NEXORA_LIVE_AUTH_TEST_URL").expect("NEXORA_LIVE_AUTH_TEST_URL");
        let email = std::env::var("NEXORA_LIVE_TEST_EMAIL").expect("NEXORA_LIVE_TEST_EMAIL");
        let password =
            std::env::var("NEXORA_LIVE_TEST_PASSWORD").expect("NEXORA_LIVE_TEST_PASSWORD");

        let client = AuthApiClient::with_base_url(&url);

        // 1. Public plan catalogue parses cleanly over real HTTP.
        let plans = client.list_plans().await.expect("list_plans over TCP");
        assert_eq!(plans.plans.len(), 3);
        assert_eq!(plans.plans[0].model_quotas.get("nexora-mini"), Some(&25));
        assert_eq!(plans.plans[0].model_quotas.get("nexora-sonnet"), Some(&5));

        // 2. Verify that replaying a rotated token revokes the entire session family in nexora-auth,
        //    then verify that 4 concurrent desktop `AuthState::refresh_session` calls coalesce to a
        //    single network rotation so family theft revocation is never triggered.
        let (_tmp_user, theft_t1) = client
            .login(&email, &password)
            .await
            .expect("theft-probe login over TCP");
        let theft_t2 = client
            .refresh(&theft_t1)
            .await
            .expect("theft-probe initial refresh over TCP");
        let replay_old = client.refresh(&theft_t1).await.unwrap_err();
        assert_eq!(replay_old.code, "session_expired");
        let revoked_family = client.get_me(theft_t2.token()).await.unwrap_err();
        assert_eq!(revoked_family.code, "session_expired");

        let (user, initial_token) = client
            .login(&email, &password)
            .await
            .expect("login over TCP");
        let live_state = std::sync::Arc::new(crate::auth::commands::AuthState::with_vault(
            AuthApiClient::with_base_url(&url),
            crate::auth::commands::tests::TestVault::with_token(&initial_token),
        ));
        let (r1, r2, r3, r4) = tokio::join!(
            live_state.refresh_session(),
            live_state.refresh_session(),
            live_state.refresh_session(),
            live_state.refresh_session(),
        );
        assert_eq!(r1.expect("r1").user.expect("u1").id, user.id);
        assert_eq!(r2.expect("r2").user.expect("u2").id, user.id);
        assert_eq!(r3.expect("r3").user.expect("u3").id, user.id);
        assert_eq!(r4.expect("r4").user.expect("u4").id, user.id);
        let token = live_state
            .read_session_token()
            .expect("vault read")
            .expect("rotated token in vault");
        assert_ne!(token, initial_token);

        let me = client.get_me(&token).await.expect("get_me over TCP");
        assert_eq!(me.id, user.id);

        let prof = client
            .update_account_profile(
                &token,
                &crate::auth::UpdateProfilePayload {
                    name: "Desktop RC Verified".to_string(),
                },
            )
            .await
            .expect("update_account_profile over TCP");
        assert_eq!(prof.name, "Desktop RC Verified");
        assert_eq!(prof.plan_id, "free");

        // 3. Billing status + desktop registry profile ID quota reserve -> release -> reserve -> commit -> summary.
        let bill = client
            .get_billing_status(&token)
            .await
            .expect("get_billing_status over TCP");
        assert_eq!(bill.effective_plan_id, "free");
        assert!(!bill.portal_available);

        let res1 = client
            .reserve_usage(
                &token,
                crate::inference::registry::NEXORA_MINI_ID,
                "rc-wire-req-001",
            )
            .await
            .expect("reserve_usage Mini over TCP");
        assert_eq!(res1.model_id, "nexora-mini");
        assert_eq!(res1.remaining_after, 24);
        assert!(!res1.replayed);

        let replay = client
            .reserve_usage(
                &token,
                crate::inference::registry::NEXORA_MINI_ID,
                "rc-wire-req-001",
            )
            .await
            .expect("replay reserve_usage over TCP");
        assert_eq!(replay.reservation_id, res1.reservation_id);
        assert!(replay.replayed);

        let after_release = client
            .settle_usage(&token, &res1.reservation_id, false)
            .await
            .expect("release usage over TCP");
        assert_eq!(after_release.models[0].remaining, 25);

        let res2 = client
            .reserve_usage(
                &token,
                crate::inference::registry::NEXORA_SONNET_ID,
                "rc-wire-req-002",
            )
            .await
            .expect("reserve_usage Sonnet over TCP");
        assert_eq!(res2.model_id, "nexora-sonnet");
        assert_eq!(res2.remaining_after, 4);
        let after_commit = client
            .settle_usage(&token, &res2.reservation_id, true)
            .await
            .expect("commit usage over TCP");
        assert_eq!(after_commit.models[1].used, 1);
        assert_eq!(after_commit.models[1].remaining, 4);

        let summary = client
            .get_usage_summary(&token)
            .await
            .expect("get_usage_summary over TCP");
        assert_eq!(summary.models[1].remaining, 4);
        assert!(summary.resets_at_utc.ends_with('Z'));

        // 4. Offline network failure maps cleanly to `network_error` without panicking.
        let offline = AuthApiClient::with_base_url("http://127.0.0.1:9");
        let off_err = offline.get_usage_summary(&token).await.unwrap_err();
        assert_eq!(off_err.code, "network_error");

        // 5. GDPR export and account deletion over real HTTP.
        let exported = client
            .export_account_data(&token)
            .await
            .expect("export_account_data over TCP");
        assert_eq!(exported["account"]["name"], "Desktop RC Verified");

        client
            .delete_account(
                &token,
                &crate::auth::DeleteAccountPayload {
                    confirm_email: email,
                    current_password: Some(password),
                },
            )
            .await
            .expect("delete_account over TCP");
        let dead = client.get_me(&token).await.unwrap_err();
        assert_eq!(dead.code, "session_expired");
    }
}
