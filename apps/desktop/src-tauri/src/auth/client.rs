use std::fmt;
use std::time::Duration;

use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use reqwest::redirect::Policy;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use super::{AuthError, AuthResult, AuthStatusResponse, AuthUser, SessionCredentials};

const COMPILE_TIME_AUTH_API_URL: Option<&'static str> = option_env!("NEXORA_AUTH_API_URL");
const DEFAULT_LOOPBACK_AUTH_URL: &str = "http://127.0.0.1:8787";

#[derive(Debug, Deserialize)]
struct ErrorDetail {
    code: String,
    message: String,
}

#[derive(Debug, Deserialize)]
struct ErrorEnvelope {
    error: ErrorDetail,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SessionTokenDto {
    token: String,
    #[serde(default)]
    expires_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LoginResponseBody {
    user: AuthUser,
    session: SessionTokenDto,
}

#[derive(Debug, Deserialize)]
struct RefreshResponseBody {
    session: SessionTokenDto,
}

#[derive(Debug, Deserialize)]
struct MeResponseBody {
    user: AuthUser,
}

#[derive(Debug, Deserialize)]
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
    matches!(
        host.to_ascii_lowercase().as_str(),
        "127.0.0.1" | "localhost" | "[::1]"
    )
}

fn contains_release_placeholder(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    lower.contains("replace_with")
        || lower.contains("change_me")
        || lower.contains("nexora.local")
        || lower.contains("example.invalid")
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
        .any(|c| c.is_control() || c.is_whitespace() || matches!(c, '"' | '\'' | '\\'))
    {
        return Err(AuthError::new(
            "invalid_backend_url",
            "Backend service URL contains invalid characters.",
        ));
    }

    let Some((scheme_raw, rest)) = trimmed.split_once("://") else {
        return Err(AuthError::new(
            "invalid_backend_url",
            "Backend service URL must declare an explicit https:// scheme.",
        ));
    };

    if rest.contains('?') || rest.contains('#') {
        return Err(AuthError::new(
            "invalid_backend_url",
            "Backend service URL must not contain query parameters or fragments.",
        ));
    }

    let (authority, path_tail) = match rest.split_once('/') {
        Some((auth, tail)) => (auth, tail.trim_end_matches('/')),
        None => (rest, ""),
    };

    if authority.is_empty() || authority.contains('@') {
        return Err(AuthError::new(
            "invalid_backend_url",
            "Backend service URL authority is invalid or embeds credentials.",
        ));
    }

    if reject_placeholders && contains_release_placeholder(trimmed) {
        return Err(AuthError::new(
            "backend_unconfigured",
            "Nexora online account services use an unconfigured placeholder URL in this build.",
        ));
    }

    let (host, port_str) = if authority.starts_with('[') {
        let Some(close_idx) = authority.find(']') else {
            return Err(AuthError::new(
                "invalid_backend_url",
                "Backend service URL contains a malformed IPv6 host.",
            ));
        };
        let ipv6_host = &authority[..=close_idx];
        let inner = &ipv6_host[1..ipv6_host.len() - 1];
        if inner.is_empty()
            || !inner.contains(':')
            || !inner
                .chars()
                .all(|c| c.is_ascii_hexdigit() || c == ':' || c == '.')
        {
            return Err(AuthError::new(
                "invalid_backend_url",
                "Backend service URL contains an invalid IPv6 address.",
            ));
        }
        let remainder = &authority[close_idx + 1..];
        if remainder.is_empty() {
            (ipv6_host, None)
        } else if let Some(p) = remainder.strip_prefix(':') {
            (ipv6_host, Some(p))
        } else {
            return Err(AuthError::new(
                "invalid_backend_url",
                "Backend service URL authority is malformed.",
            ));
        }
    } else {
        let mut parts = authority.split(':');
        let host_part = parts.next().unwrap_or("");
        let port_part = parts.next();
        if parts.next().is_some() {
            return Err(AuthError::new(
                "invalid_backend_url",
                "Backend service URL host or port is malformed.",
            ));
        }
        if host_part.is_empty()
            || host_part.starts_with('.')
            || host_part.ends_with('.')
            || host_part.contains("..")
            || !host_part
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '.'))
        {
            return Err(AuthError::new(
                "invalid_backend_url",
                "Backend service URL hostname is invalid.",
            ));
        }
        (host_part, port_part)
    };

    if let Some(p) = port_str {
        match p.parse::<u16>() {
            Ok(port) if port > 0 => {}
            _ => {
                return Err(AuthError::new(
                    "invalid_backend_url",
                    "Backend service URL port must be between 1 and 65535.",
                ));
            }
        }
    }

    if !path_tail.is_empty()
        && path_tail.split('/').any(|seg| {
            seg.is_empty()
                || seg == "."
                || seg == ".."
                || !seg
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        })
    {
        return Err(AuthError::new(
            "invalid_backend_url",
            "Backend service URL base path is invalid.",
        ));
    }

    let scheme = scheme_raw.to_ascii_lowercase();
    match scheme.as_str() {
        "https" => {}
        "http" if is_loopback_host(host) => {
            if !allow_loopback_http {
                return Err(AuthError::new(
                    "insecure_backend_url",
                    "Plaintext HTTP loopback backend URLs are disabled in release builds.",
                ));
            }
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

    let normalized = if path_tail.is_empty() {
        format!("{scheme}://{authority}")
    } else {
        format!("{scheme}://{authority}/{path_tail}")
    };

    Ok(normalized)
}

fn resolve_backend_endpoint_with(
    compile_url: Option<&str>,
    runtime_primary: Option<&str>,
    runtime_legacy: Option<&str>,
    is_debug_build: bool,
    allow_loopback_flag: bool,
) -> EndpointState {
    let allow_loopback_http = is_debug_build || allow_loopback_flag;

    let selected = [runtime_primary, compile_url, runtime_legacy]
        .into_iter()
        .flatten()
        .map(str::trim)
        .find(|value| !value.is_empty());

    match selected {
        Some(candidate) => {
            match validate_backend_base_url(candidate, !allow_loopback_http, allow_loopback_http) {
                Ok(valid) => EndpointState::Configured(valid),
                Err(err) => EndpointState::Invalid(err),
            }
        }
        None if allow_loopback_http => {
            EndpointState::Configured(DEFAULT_LOOPBACK_AUTH_URL.to_string())
        }
        None => EndpointState::Unconfigured,
    }
}

fn build_http_client() -> Client {
    Client::builder()
        .timeout(Duration::from_secs(10))
        .connect_timeout(Duration::from_secs(5))
        .redirect(Policy::none())
        .build()
        .unwrap_or_else(|_| Client::new())
}

pub struct AuthApiClient {
    endpoint: EndpointState,
    client: Client,
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
        let runtime_primary = std::env::var("NEXORA_AUTH_API_URL").ok();
        let runtime_legacy = std::env::var("AUTH_API_BASE_URL").ok();
        let allow_loopback_flag = std::env::var("NEXORA_ALLOW_LOOPBACK_HTTP")
            .map(|v| v.trim() == "true")
            .unwrap_or(false);

        let endpoint = resolve_backend_endpoint_with(
            COMPILE_TIME_AUTH_API_URL,
            runtime_primary.as_deref(),
            runtime_legacy.as_deref(),
            cfg!(debug_assertions),
            allow_loopback_flag,
        );

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
            .unwrap_or_else(|_| Client::new());
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
        matches!(self.endpoint, EndpointState::Configured(_))
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
            .client
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
            .client
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
                    "Account created. Please check your email to verify your address.".to_string()
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
            .client
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
            .client
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
                .unwrap_or_else(|| "sent".to_string());
            let message = parsed
                .as_ref()
                .and_then(|b| b.message.clone())
                .unwrap_or_else(|| {
                    "If an account exists for that email, a verification link has been sent."
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
            .client
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
                .unwrap_or_else(|| "sent".to_string());
            let message = parsed
                .as_ref()
                .and_then(|b| b.message.clone())
                .unwrap_or_else(|| {
                    "If an account exists for that email, password reset instructions have been sent."
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
            .client
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
            .client
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
            .client
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
                .client
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
            .client
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
            .client
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
                "An account with this email already exists. Sign in with your password to link Google.",
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
            .client
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
            .client
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
            .client
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
            .client
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
            .client
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
            .client
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
            .client
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
            .client
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
            .client
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
            .client
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
            .client
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
            .client
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
            .client
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
            .client
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
            .client
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

    if authenticated_request && status.as_u16() == 401 {
        return Err(AuthError::session_expired());
    }

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
        resolve_backend_endpoint_with, validate_backend_base_url, AuthApiClient, EndpointState,
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
            Some("https://auth.nexora-example.org/"),
            None,
            None,
            false,
            false,
        );
        assert_eq!(
            resolved,
            EndpointState::Configured("https://auth.nexora-example.org".to_string())
        );

        let unconfigured = resolve_backend_endpoint_with(None, None, None, false, false);
        assert_eq!(unconfigured, EndpointState::Unconfigured);

        let loopback_in_release =
            resolve_backend_endpoint_with(Some("http://127.0.0.1:8787"), None, None, false, false);
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
        let debug_default = resolve_backend_endpoint_with(None, None, None, true, false);
        assert_eq!(
            debug_default,
            EndpointState::Configured("http://127.0.0.1:8787".to_string())
        );

        let release_opt_in =
            resolve_backend_endpoint_with(None, Some("http://localhost:9898/"), None, false, true);
        assert_eq!(
            release_opt_in,
            EndpointState::Configured("http://localhost:9898".to_string())
        );

        let ipv6_loopback = validate_backend_base_url("http://[::1]:8787/", false, true).unwrap();
        assert_eq!(ipv6_loopback, "http://[::1]:8787");
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
