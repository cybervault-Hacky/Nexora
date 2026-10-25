pub mod client;
pub mod commands;
pub mod pkce;
pub mod storage;

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

fn default_true() -> bool {
    true
}

/// Safe user profile data transferred across the IPC boundary to the frontend.
///
/// NOTE: Raw session tokens, password hashes, and secrets are NEVER included
/// in this DTO and never exposed to the webview.
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AuthUser {
    pub id: String,
    pub name: String,
    pub email: String,
    #[serde(default = "default_true")]
    pub email_verified: bool,
}

impl std::fmt::Debug for AuthUser {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthUser")
            .field("id", &"<redacted>")
            .field("name", &"<redacted>")
            .field("email", &"<redacted>")
            .field("email_verified", &self.email_verified)
            .finish()
    }
}

/// Enriched account profile including linked authentication methods and active plan.
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AccountProfile {
    pub id: String,
    pub name: String,
    pub email: String,
    pub email_verified: bool,
    pub has_password: bool,
    pub providers: Vec<String>,
    pub plan_id: String,
    pub plan_name: String,
    pub created_at: String,
}

impl std::fmt::Debug for AccountProfile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AccountProfile")
            .field("id", &"<redacted>")
            .field("name", &"<redacted>")
            .field("email", &"<redacted>")
            .field("email_verified", &self.email_verified)
            .field("has_password", &self.has_password)
            .field("providers", &"<redacted>")
            .field("plan_id", &"<redacted>")
            .field("plan_name", &"<redacted>")
            .field("created_at", &"<redacted>")
            .finish()
    }
}

/// Information about the current active session.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AuthSessionInfo {
    pub user: Option<AuthUser>,
    pub authenticated: bool,
}

/// Request payloads for IPC commands.
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginPayload {
    pub email: String,
    pub password: String,
}

impl std::fmt::Debug for LoginPayload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoginPayload")
            .field("email", &"<redacted>")
            .field("password", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterPayload {
    pub name: String,
    pub email: String,
    pub password: String,
}

impl std::fmt::Debug for RegisterPayload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RegisterPayload")
            .field("name", &"<redacted>")
            .field("email", &"<redacted>")
            .field("password", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifyEmailPayload {
    pub token: String,
}

impl std::fmt::Debug for VerifyEmailPayload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VerifyEmailPayload")
            .field("token", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResendVerificationPayload {
    pub email: String,
}

impl std::fmt::Debug for ResendVerificationPayload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResendVerificationPayload")
            .field("email", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryPayload {
    pub email: String,
}

impl std::fmt::Debug for RecoveryPayload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RecoveryPayload")
            .field("email", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResetPasswordPayload {
    pub token: String,
    pub password: String,
}

impl std::fmt::Debug for ResetPasswordPayload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResetPasswordPayload")
            .field("token", &"<redacted>")
            .field("password", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangePasswordPayload {
    pub current_password: String,
    pub new_password: String,
}

impl std::fmt::Debug for ChangePasswordPayload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChangePasswordPayload")
            .field("current_password", &"<redacted>")
            .field("new_password", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProfilePayload {
    pub name: String,
}

impl std::fmt::Debug for UpdateProfilePayload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UpdateProfilePayload")
            .field("name", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteAccountPayload {
    pub confirm_email: String,
    #[serde(default)]
    pub current_password: Option<String>,
}

impl std::fmt::Debug for DeleteAccountPayload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeleteAccountPayload")
            .field("confirm_email", &"<redacted>")
            .field("current_password", &"<redacted>")
            .finish()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PlanPriceViewDto {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub plan_id: String,
    pub interval: String,
    pub currency: String,
    pub amount_minor: i32,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum ModelQuotasWire {
    Map(BTreeMap<String, i32>),
    List(Vec<ModelLimitPairWire>),
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ModelLimitPairWire {
    model_id: String,
    daily_limit: i32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawPlanViewWire {
    id: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    display_name: Option<String>,
    description: String,
    #[serde(default)]
    features: Vec<String>,
    model_quotas: ModelQuotasWire,
    prices: Vec<PlanPriceViewDto>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PlanViewDto {
    pub id: String,
    pub name: String,
    pub description: String,
    pub features: Vec<String>,
    pub model_quotas: BTreeMap<String, i32>,
    pub prices: Vec<PlanPriceViewDto>,
}

impl<'de> Deserialize<'de> for PlanViewDto {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = RawPlanViewWire::deserialize(deserializer)?;
        let plan_id = raw.id;
        let name = raw
            .name
            .or(raw.display_name)
            .unwrap_or_else(|| plan_id.clone());
        let model_quotas = match raw.model_quotas {
            ModelQuotasWire::Map(m) => m,
            ModelQuotasWire::List(list) => list
                .into_iter()
                .map(|item| (item.model_id, item.daily_limit))
                .collect(),
        };
        let features = if raw.features.is_empty() {
            let mut synthesized = vec!["Local GGUF inference on your hardware".to_string()];
            if let Some(mini) = model_quotas.get("nexora-mini") {
                synthesized.push(format!("{mini} Nexora Mini messages per day"));
            }
            if let Some(sonnet) = model_quotas.get("nexora-sonnet") {
                synthesized.push(format!("{sonnet} Nexora Sonnet messages per day"));
            }
            synthesized
        } else {
            raw.features
        };
        let prices = raw
            .prices
            .into_iter()
            .map(|mut p| {
                if p.plan_id.is_empty() {
                    p.plan_id = plan_id.clone();
                }
                if p.id.is_empty() {
                    p.id = format!("{}-{}", plan_id, p.interval);
                }
                p
            })
            .collect();
        Ok(Self {
            id: plan_id,
            name,
            description: raw.description,
            features,
            model_quotas,
            prices,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PlanCatalogDto {
    pub plans: Vec<PlanViewDto>,
    pub payment_configured: bool,
    pub provider: String,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SubscriptionViewDto {
    pub id: String,
    pub plan_id: String,
    pub status: String,
    pub current_period_start: String,
    pub current_period_end: String,
    pub cancel_at_period_end: bool,
    pub canceled_at: Option<String>,
    pub provider: String,
}

impl std::fmt::Debug for SubscriptionViewDto {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SubscriptionViewDto")
            .field("id", &"<redacted>")
            .field("plan_id", &"<redacted>")
            .field("status", &"<redacted>")
            .field("current_period_start", &"<redacted>")
            .field("current_period_end", &"<redacted>")
            .field("cancel_at_period_end", &self.cancel_at_period_end)
            .field("canceled_at", &self.canceled_at.as_ref().map(|_| "<redacted>"))
            .field("provider", &self.provider)
            .finish()
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PaymentOrderViewDto {
    pub id: String,
    pub plan_id: String,
    pub amount_minor: i32,
    pub currency: String,
    pub status: String,
    pub created_at: String,
}

impl std::fmt::Debug for PaymentOrderViewDto {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PaymentOrderViewDto")
            .field("id", &"<redacted>")
            .field("plan_id", &"<redacted>")
            .field("amount_minor", &"<redacted>")
            .field("currency", &"<redacted>")
            .field("status", &"<redacted>")
            .field("created_at", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BillingStatusDto {
    pub effective_plan_id: String,
    pub effective_plan_name: String,
    pub subscription: Option<SubscriptionViewDto>,
    pub recent_orders: Vec<PaymentOrderViewDto>,
    pub payment_configured: bool,
    pub portal_available: bool,
}

impl std::fmt::Debug for BillingStatusDto {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BillingStatusDto")
            .field("effective_plan_id", &"<redacted>")
            .field("effective_plan_name", &"<redacted>")
            .field("subscription", &self.subscription)
            .field("recent_orders", &self.recent_orders)
            .field("payment_configured", &self.payment_configured)
            .field("portal_available", &self.portal_available)
            .finish()
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckoutPayload {
    pub plan_id: String,
    pub interval: String,
    pub idempotency_key: String,
}

impl std::fmt::Debug for CheckoutPayload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CheckoutPayload")
            .field("plan_id", &"<redacted>")
            .field("interval", &"<redacted>")
            .field("idempotency_key", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CheckoutSessionDto {
    pub order_id: String,
    pub checkout_url: String,
    pub provider_session_id: String,
    pub expires_note: String,
}

impl std::fmt::Debug for CheckoutSessionDto {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CheckoutSessionDto")
            .field("order_id", &"<redacted>")
            .field("checkout_url", &"<redacted>")
            .field("provider_session_id", &"<redacted>")
            .field("expires_note", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifyOrderPayload {
    pub order_id: String,
}

impl std::fmt::Debug for VerifyOrderPayload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VerifyOrderPayload")
            .field("order_id", &"<redacted>")
            .finish()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelSubscriptionPayload {
    #[serde(default)]
    pub immediate: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangePlanPayload {
    pub plan_id: String,
    pub interval: String,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PortalSessionDto {
    pub portal_url: String,
}

impl std::fmt::Debug for PortalSessionDto {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PortalSessionDto")
            .field("portal_url", &"<redacted>")
            .finish()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ModelUsageItemDto {
    pub model_id: String,
    pub daily_limit: i32,
    pub used: i32,
    pub reserved: i32,
    pub remaining: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ServerUsageSummaryDto {
    #[serde(default)]
    pub quota_configured: bool,
    pub plan_id: Option<String>,
    pub plan_name: Option<String>,
    pub period_day_utc: Option<String>,
    pub resets_at_utc: Option<String>,
    pub models: Vec<ModelUsageItemDto>,
    #[serde(default)]
    pub offline_fallback: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ReserveQuotaResponseDto {
    pub reservation_id: String,
    pub plan_id: String,
    pub model_id: String,
    pub daily_limit: i32,
    pub remaining_after: i32,
    pub resets_at_utc: String,
    pub replayed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeveloperDiagnostics {
    pub platform: String,
    pub arch: String,
    pub app_version: String,
    pub credential_vault: String,
    pub auth_service_url: String,
    pub google_oauth_configured: bool,
    pub csp_enforced: bool,
    pub offline_capable: bool,
}

/// Internal session credential wrapper for desktop-side rotation and storage.
///
/// Redacts the bearer token in `Debug` output so credentials can never leak
/// into logs, panic traces, or diagnostics. Never serialized across IPC.
#[derive(Clone, PartialEq, Eq)]
pub struct SessionCredentials {
    token: String,
    pub expires_at: Option<String>,
}

impl SessionCredentials {
    pub fn new(token: impl Into<String>, expires_at: Option<String>) -> Self {
        Self {
            token: token.into(),
            expires_at,
        }
    }

    pub fn token(&self) -> &str {
        &self.token
    }
}

impl std::fmt::Debug for SessionCredentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionCredentials")
            .field("token", &"<redacted>")
            .field("expires_at", &self.expires_at)
            .finish()
    }
}

/// Simple status response for flow steps.
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AuthStatusResponse {
    pub status: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email_delivery: Option<String>,
}

impl std::fmt::Debug for AuthStatusResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthStatusResponse")
            .field("status", &self.status)
            .field("message", &"<redacted>")
            .field("email_delivery", &self.email_delivery)
            .finish()
    }
}

impl AuthStatusResponse {
    pub fn new(status: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            status: status.into(),
            message: message.into(),
            email_delivery: None,
        }
    }

    pub fn with_delivery(
        status: impl Into<String>,
        message: impl Into<String>,
        email_delivery: Option<String>,
    ) -> Self {
        Self {
            status: status.into(),
            message: message.into(),
            email_delivery,
        }
    }
}

fn is_ascii_word_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric()
}

fn redact_uri_user_info(input: &str) -> String {
    let mut output = input.to_string();
    let mut cursor = 0usize;
    loop {
        let Some(relative_scheme_end) = output[cursor..].find("://") else {
            break;
        };
        let authority_start = cursor + relative_scheme_end + 3;
        let authority_end = output[authority_start..]
            .find(['/', '?', '#', ' ', '\t', '\n', '\r', '"', '\''])
            .map(|offset| authority_start + offset)
            .unwrap_or(output.len());
        let Some(at_offset) = output[authority_start..authority_end].rfind('@') else {
            cursor = authority_end;
            continue;
        };
        let at = authority_start + at_offset;
        if at > authority_start && &output[authority_start..at] != "<redacted>" {
            output.replace_range(authority_start..at, "<redacted>");
            cursor = authority_start + "<redacted>".len() + 1;
        } else {
            cursor = authority_end;
        }
    }
    output
}

fn redact_bearer_values(input: &str) -> String {
    let mut output = input.to_string();
    let mut cursor = 0usize;
    loop {
        let lowered = output.to_ascii_lowercase();
        let Some(relative) = lowered[cursor..].find("bearer ") else {
            break;
        };
        let bearer_start = cursor + relative;
        if bearer_start > 0 && is_ascii_word_byte(output.as_bytes()[bearer_start - 1]) {
            cursor = bearer_start + 7;
            continue;
        }
        let token_start = bearer_start + 7;
        let token_end = output[token_start..]
            .find(|character: char| {
                character.is_whitespace()
                    || matches!(character, '"' | '\'' | ',' | ';' | ')' | '}' | ']')
            })
            .map(|offset| token_start + offset)
            .unwrap_or(output.len());
        if token_end > token_start && &output[token_start..token_end] != "<redacted>" {
            output.replace_range(token_start..token_end, "<redacted>");
            cursor = token_start + "<redacted>".len();
        } else {
            cursor = token_end;
        }
    }
    output
}

fn next_sensitive_assignment(input: &str, search_from: usize) -> Option<(usize, usize)> {
    const KEYS: &[&str] = &[
        "access_token",
        "refresh_token",
        "id_token",
        "code_verifier",
        "client_secret",
        "api_key",
        "apikey",
        "password",
        "passwd",
        "authorization",
        "cookie",
        "signature",
        "private_key",
        "database_url",
        "connection_string",
        "secret",
        "token",
        "code",
        "state",
        "nonce",
        "key",
    ];
    let lowered = input.to_ascii_lowercase();
    let mut earliest: Option<(usize, usize, usize)> = None;

    for key in KEYS {
        let mut cursor = search_from;
        while cursor < lowered.len() {
            let Some(relative) = lowered[cursor..].find(key) else {
                break;
            };
            let key_start = cursor + relative;
            let after_key = key_start + key.len();
            let boundary_before = key_start == 0
                || !is_ascii_word_byte(lowered.as_bytes()[key_start - 1]);
            let boundary_after = after_key == lowered.len()
                || !is_ascii_word_byte(lowered.as_bytes()[after_key]);
            if boundary_before && boundary_after {
                let mut separator = after_key;
                while separator < input.len()
                    && matches!(input.as_bytes()[separator], b' ' | b'\t')
                {
                    separator += 1;
                }
                if separator < input.len()
                    && matches!(input.as_bytes()[separator], b'=' | b':')
                {
                    let mut value_start = separator + 1;
                    while value_start < input.len()
                        && matches!(input.as_bytes()[value_start], b' ' | b'\t')
                    {
                        value_start += 1;
                    }
                    if value_start < input.len()
                        && matches!(input.as_bytes()[value_start], b'"' | b'\'')
                    {
                        value_start += 1;
                    }
                    let value_end = input[value_start..]
                        .find(|character: char| {
                            matches!(character, '&' | '\n' | '\r' | '"' | '\'' | '}' | ']' | ')')
                        })
                        .map(|offset| value_start + offset)
                        .unwrap_or(input.len());
                    if value_end > value_start
                        && earliest.is_none_or(|(current, _, _)| key_start < current)
                    {
                        earliest = Some((key_start, value_start, value_end));
                    }
                }
            }
            cursor = after_key.max(key_start + 1);
        }
    }

    earliest.map(|(_, value_start, value_end)| (value_start, value_end))
}

fn scrub_client_message(raw: &str) -> String {
    let mut output = redact_uri_user_info(raw);
    output = redact_bearer_values(&output);

    let mut cursor = 0usize;
    while let Some((value_start, value_end)) = next_sensitive_assignment(&output, cursor) {
        if output.get(value_start..value_end) == Some("<redacted>") {
            cursor = value_end;
        } else {
            output.replace_range(value_start..value_end, "<redacted>");
            cursor = value_start + "<redacted>".len();
        }
    }

    output.chars().take(512).collect()
}

/// Structured error returned to the frontend.
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AuthError {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_after_seconds: Option<u64>,
}

impl std::fmt::Debug for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthError")
            .field("code", &self.code)
            .field("message", &"<redacted>")
            .field("retry_after_seconds", &self.retry_after_seconds)
            .finish()
    }
}

impl AuthError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        let raw_msg = message.into();
        Self {
            code: code.into(),
            message: scrub_client_message(&raw_msg),
            retry_after_seconds: None,
        }
    }

    pub fn with_retry(
        code: impl Into<String>,
        message: impl Into<String>,
        retry_after: u64,
    ) -> Self {
        let raw_msg = message.into();
        Self {
            code: code.into(),
            message: scrub_client_message(&raw_msg),
            retry_after_seconds: Some(retry_after),
        }
    }

    pub fn is_session_invalid(&self) -> bool {
        matches!(
            self.code.as_str(),
            "session_expired" | "invalid_or_expired_token"
        )
    }

    pub fn session_expired() -> Self {
        Self::new(
            "session_expired",
            "Your session has expired. Please sign in again.",
        )
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

#[cfg(test)]
mod secret_debug_tests {
    use super::*;

    #[test]
    fn auth_user_and_ipc_credentials_have_redacted_debug_output() {
        let user = AuthUser {
            id: "SENTINEL_id".to_string(),
            name: "SENTINEL_name".to_string(),
            email: "SENTINEL_email@example.test".to_string(),
            email_verified: true,
        };
        assert!(!format!("{user:?}").contains("SENTINEL"));

        let error = AuthError::new("internal", "SENTINEL_email@example.test failed.");
        assert!(!format!("{error:?}").contains("SENTINEL"));
        let status = AuthStatusResponse::new(
            "accepted",
            "SENTINEL_email@example.test verification instructions will be sent.",
        );
        assert!(!format!("{status:?}").contains("SENTINEL"));

        let rendered = [
            format!("{:?}", LoginPayload {
                email: "SENTINEL_email@example.test".to_string(),
                password: "SENTINEL_password".to_string(),
            }),
            format!("{:?}", RegisterPayload {
                name: "SENTINEL_name".to_string(),
                email: "SENTINEL_email@example.test".to_string(),
                password: "SENTINEL_password".to_string(),
            }),
            format!("{:?}", VerifyEmailPayload {
                token: "SENTINEL_token".to_string(),
            }),
            format!("{:?}", ResendVerificationPayload {
                email: "SENTINEL_email@example.test".to_string(),
            }),
            format!("{:?}", RecoveryPayload {
                email: "SENTINEL_email@example.test".to_string(),
            }),
            format!("{:?}", ResetPasswordPayload {
                token: "SENTINEL_token".to_string(),
                password: "SENTINEL_password".to_string(),
            }),
            format!("{:?}", ChangePasswordPayload {
                current_password: "SENTINEL_old_password".to_string(),
                new_password: "SENTINEL_new_password".to_string(),
            }),
        ];
        assert!(rendered.iter().all(|line| !line.contains("SENTINEL")));
    }

    #[test]
    fn client_errors_redact_unicode_secrets_oauth_queries_and_database_user_info() {
        let oauth = AuthError::new(
            "network_error",
            "Réponse refusée: https://auth.example.test/callback?code=SENTINEL_code&state=SENTINEL_state",
        );
        assert!(oauth.message.contains("Réponse refusée"));
        assert!(!oauth.message.contains("SENTINEL_code"));
        assert!(!oauth.message.contains("SENTINEL_state"));

        let database = AuthError::new(
            "network_error",
            "connect failed: postgres://db_user:SENTINEL_db_password@db.internal/nexora",
        );
        assert!(!database.message.contains("db_user"));
        assert!(!database.message.contains("SENTINEL_db_password"));

        let signature = AuthError::new(
            "network_error",
            "Stripe-Signature: t=123,v1=SENTINEL_payment_signature",
        );
        assert!(!signature.message.contains("SENTINEL_payment_signature"));
    }

    #[test]
    fn sensitive_account_and_payment_dto_debug_is_redacted() {
        let profile = AccountProfile {
            id: "SENTINEL_user_id".to_string(),
            name: "SENTINEL_name".to_string(),
            email: "SENTINEL_email@example.test".to_string(),
            email_verified: true,
            has_password: true,
            providers: vec!["SENTINEL_provider".to_string()],
            plan_id: "SENTINEL_plan".to_string(),
            plan_name: "SENTINEL_plan_name".to_string(),
            created_at: "SENTINEL_timestamp".to_string(),
        };
        let deletion = DeleteAccountPayload {
            confirm_email: "SENTINEL_email@example.test".to_string(),
            current_password: Some("SENTINEL_password".to_string()),
        };
        let checkout = CheckoutSessionDto {
            order_id: "SENTINEL_order".to_string(),
            checkout_url: "https://checkout.example.test/SENTINEL_capability".to_string(),
            provider_session_id: "SENTINEL_session".to_string(),
            expires_note: "SENTINEL_note".to_string(),
        };
        let portal = PortalSessionDto {
            portal_url: "https://portal.example.test/SENTINEL_capability".to_string(),
        };
        let rendered = format!("{profile:?} {deletion:?} {checkout:?} {portal:?}");
        assert!(!rendered.contains("SENTINEL"));
    }
}
