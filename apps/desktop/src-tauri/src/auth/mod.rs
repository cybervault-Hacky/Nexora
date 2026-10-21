pub mod client;
pub mod commands;
pub mod pkce;
pub mod storage;

use serde::{Deserialize, Serialize};

/// Safe user profile data transferred across the IPC boundary to the frontend.
///
/// NOTE: Raw session tokens, password hashes, and secrets are NEVER included
/// in this DTO and never exposed to the webview.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AuthUser {
    pub id: String,
    pub name: String,
    pub email: String,
    pub email_verified: bool,
}

/// Information about the current active session.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AuthSessionInfo {
    pub user: Option<AuthUser>,
    pub authenticated: bool,
}

/// Request payloads for IPC commands.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginPayload {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterPayload {
    pub name: String,
    pub email: String,
    pub password: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifyEmailPayload {
    pub token: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResendVerificationPayload {
    pub email: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryPayload {
    pub email: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResetPasswordPayload {
    pub token: String,
    pub password: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangePasswordPayload {
    pub current_password: String,
    pub new_password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeveloperDiagnostics {
    pub platform: String,
    pub arch: String,
    pub app_version: String,
    pub credential_vault: String,
    pub auth_service_url: String,
    pub csp_enforced: bool,
    pub offline_capable: bool,
}

/// Simple status response for flow steps.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AuthStatusResponse {
    pub status: String,
    pub message: String,
}

/// Structured error returned to the frontend.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AuthError {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_after_seconds: Option<u64>,
}

impl AuthError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            retry_after_seconds: None,
        }
    }

    pub fn with_retry(
        code: impl Into<String>,
        message: impl Into<String>,
        retry_after: u64,
    ) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            retry_after_seconds: Some(retry_after),
        }
    }

    pub fn network(message: impl Into<String>) -> Self {
        Self::new("network_error", message)
    }

    pub fn invalid_credentials() -> Self {
        Self::new("invalid_credentials", "Invalid email or password.")
    }

    pub fn email_unverified() -> Self {
        Self::new(
            "email_unverified",
            "Please verify your email address before signing in.",
        )
    }

    pub fn storage(message: impl Into<String>) -> Self {
        Self::new("storage_error", message)
    }
}

pub type AuthResult<T> = Result<T, AuthError>;
