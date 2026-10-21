use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use serde::{Deserialize, Serialize};
use std::time::Duration;

use super::{AuthError, AuthResult, AuthUser};

const DEFAULT_AUTH_URL: &str = "http://127.0.0.1:8787";

#[derive(Clone)]
pub struct AuthApiClient {
    client: reqwest::Client,
    base_url: String,
}

#[derive(Deserialize)]
struct ErrorEnvelope {
    error: ErrorDetails,
}

#[derive(Deserialize)]
struct ErrorDetails {
    code: String,
    message: String,
}

#[derive(Deserialize)]
struct LoginResponseBody {
    user: AuthUser,
    session: SessionTokenField,
}

#[derive(Deserialize)]
struct SessionTokenField {
    token: String,
}

#[derive(Deserialize)]
struct MeResponseBody {
    user: AuthUser,
}

impl AuthApiClient {
    pub fn new() -> Self {
        let base_url = std::env::var("NEXORA_AUTH_API_URL")
            .or_else(|_| std::env::var("AUTH_API_BASE_URL"))
            .unwrap_or_else(|_| DEFAULT_AUTH_URL.to_string());

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_default();

        Self { client, base_url }
    }

    pub fn with_base_url(base_url: impl Into<String>) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_default();

        Self {
            client,
            base_url: base_url.into(),
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}/v1/auth{}", self.base_url.trim_end_matches('/'), path)
    }

    /// The account-service origin this client talks to.
    ///
    /// Exposed read-only so diagnostics can report it without the caller
    /// reaching into the private field (and without letting it be changed).
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub async fn login(&self, email: &str, password: &str) -> AuthResult<(AuthUser, String)> {
        #[derive(Serialize)]
        struct Body<'a> {
            email: &'a str,
            password: &'a str,
        }

        let resp = self
            .client
            .post(self.url("/login"))
            .json(&Body { email, password })
            .send()
            .await
            .map_err(|_| {
                AuthError::network(
                    "Can't reach the Nexora account service. Check your connection and try again.",
                )
            })?;

        let status = resp.status();
        let headers = resp.headers().clone();

        if status.is_success() {
            let body: LoginResponseBody = resp.json().await.map_err(|e| {
                AuthError::new(
                    "parse_error",
                    format!("Failed to parse login response: {e}"),
                )
            })?;
            return Ok((body.user, body.session.token));
        }

        let retry_after = parse_retry_after(&headers);
        let error_body = resp.json::<ErrorEnvelope>().await.ok();

        match status.as_u16() {
            401 => Err(AuthError::invalid_credentials()),
            403 => Err(AuthError::email_unverified()),
            429 => {
                let retry = retry_after.unwrap_or(60);
                Err(AuthError::with_retry(
                    "rate_limited",
                    "Too many attempts. Try again in a few minutes.",
                    retry,
                ))
            }
            _ => {
                if let Some(err) = error_body {
                    Err(AuthError::new(err.error.code, err.error.message))
                } else {
                    Err(AuthError::new(
                        "auth_failed",
                        "Sign-in could not be completed.",
                    ))
                }
            }
        }
    }

    pub async fn register(&self, name: &str, email: &str, password: &str) -> AuthResult<()> {
        #[derive(Serialize)]
        struct Body<'a> {
            name: &'a str,
            email: &'a str,
            password: &'a str,
        }

        let resp = self
            .client
            .post(self.url("/register"))
            .json(&Body {
                name,
                email,
                password,
            })
            .send()
            .await
            .map_err(|_| {
                AuthError::network(
                    "Can't reach the Nexora account service. Check your connection and try again.",
                )
            })?;

        if resp.status().is_success() || resp.status().as_u16() == 202 {
            return Ok(());
        }

        handle_generic_error(resp).await
    }

    pub async fn verify_email(&self, token: &str) -> AuthResult<()> {
        #[derive(Serialize)]
        struct Body<'a> {
            token: &'a str,
        }

        let resp = self
            .client
            .post(self.url("/verify-email"))
            .json(&Body { token })
            .send()
            .await
            .map_err(|_| {
                AuthError::network(
                    "Can't reach the Nexora account service. Check your connection and try again.",
                )
            })?;

        if resp.status().is_success() {
            return Ok(());
        }

        handle_generic_error(resp).await
    }

    pub async fn resend_verification(&self, email: &str) -> AuthResult<()> {
        #[derive(Serialize)]
        struct Body<'a> {
            email: &'a str,
        }

        let resp = self
            .client
            .post(self.url("/resend-verification"))
            .json(&Body { email })
            .send()
            .await
            .map_err(|_| {
                AuthError::network(
                    "Can't reach the Nexora account service. Check your connection and try again.",
                )
            })?;

        if resp.status().is_success() || resp.status().as_u16() == 202 {
            return Ok(());
        }

        handle_generic_error(resp).await
    }

    pub async fn recovery_request(&self, email: &str) -> AuthResult<()> {
        #[derive(Serialize)]
        struct Body<'a> {
            email: &'a str,
        }

        let resp = self
            .client
            .post(self.url("/recovery-request"))
            .json(&Body { email })
            .send()
            .await
            .map_err(|_| {
                AuthError::network(
                    "Can't reach the Nexora account service. Check your connection and try again.",
                )
            })?;

        if resp.status().is_success() || resp.status().as_u16() == 202 {
            return Ok(());
        }

        handle_generic_error(resp).await
    }

    pub async fn reset_password(&self, token: &str, password: &str) -> AuthResult<()> {
        #[derive(Serialize)]
        struct Body<'a> {
            token: &'a str,
            password: &'a str,
        }

        let resp = self
            .client
            .post(self.url("/reset-password"))
            .json(&Body { token, password })
            .send()
            .await
            .map_err(|_| {
                AuthError::network(
                    "Can't reach the Nexora account service. Check your connection and try again.",
                )
            })?;

        if resp.status().is_success() || resp.status().as_u16() == 204 {
            return Ok(());
        }

        handle_generic_error(resp).await
    }

    pub async fn get_me(&self, session_token: &str) -> AuthResult<AuthUser> {
        let resp = self
            .client
            .get(self.url("/me"))
            .header(AUTHORIZATION, format!("Bearer {session_token}"))
            .send()
            .await
            .map_err(|_| {
                AuthError::network(
                    "Can't reach the Nexora account service. Check your connection and try again.",
                )
            })?;

        if resp.status().is_success() {
            let body: MeResponseBody = resp.json().await.map_err(|e| {
                AuthError::new("parse_error", format!("Failed to parse user profile: {e}"))
            })?;
            return Ok(body.user);
        }

        if resp.status().as_u16() == 401 {
            return Err(AuthError::new(
                "session_expired",
                "Your session has expired.",
            ));
        }

        handle_generic_error(resp).await
    }

    pub async fn logout(&self, session_token: &str) -> AuthResult<()> {
        let _ = self
            .client
            .post(self.url("/logout"))
            .header(AUTHORIZATION, format!("Bearer {session_token}"))
            .send()
            .await;

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
            .post(self.url("/password-change"))
            .header(AUTHORIZATION, format!("Bearer {session_token}"))
            .json(&Body {
                current_password,
                new_password,
            })
            .send()
            .await
            .map_err(|_| {
                AuthError::network(
                    "Can't reach the Nexora account service. Check your connection and try again.",
                )
            })?;

        if resp.status().is_success() || resp.status().as_u16() == 204 {
            return Ok(());
        }

        handle_generic_error(resp).await
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
            .post(self.url("/google/exchange"))
            .json(&Body {
                code,
                code_verifier,
                state,
                redirect_uri,
                nonce,
            })
            .send()
            .await
            .map_err(|_| {
                AuthError::network(
                    "Can't reach the Nexora account service. Check your connection and try again.",
                )
            })?;

        let status = resp.status();
        let headers = resp.headers().clone();

        if status.is_success() {
            let body: LoginResponseBody = resp.json().await.map_err(|e| {
                AuthError::new(
                    "parse_error",
                    format!("Failed to parse Google exchange response: {e}"),
                )
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
}

fn parse_retry_after(headers: &HeaderMap<HeaderValue>) -> Option<u64> {
    headers
        .get("retry-after")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
}

async fn handle_generic_error<T>(resp: reqwest::Response) -> AuthResult<T> {
    let status = resp.status();
    let error_body = resp.json::<ErrorEnvelope>().await.ok();

    if let Some(err) = error_body {
        Err(AuthError::new(err.error.code, err.error.message))
    } else {
        Err(AuthError::new(
            format!("http_{}", status.as_u16()),
            format!("Request failed with status {}", status),
        ))
    }
}
