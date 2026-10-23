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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AuthUser {
    pub id: String,
    pub name: String,
    pub email: String,
    #[serde(default = "default_true")]
    pub email_verified: bool,
}

/// Enriched account profile including linked authentication methods and active plan.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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
pub struct UpdateProfilePayload {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteAccountPayload {
    pub confirm_email: String,
    #[serde(default)]
    pub current_password: Option<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PaymentOrderViewDto {
    pub id: String,
    pub plan_id: String,
    pub amount_minor: i32,
    pub currency: String,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BillingStatusDto {
    pub effective_plan_id: String,
    pub effective_plan_name: String,
    pub subscription: Option<SubscriptionViewDto>,
    pub recent_orders: Vec<PaymentOrderViewDto>,
    pub payment_configured: bool,
    pub portal_available: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckoutPayload {
    pub plan_id: String,
    pub interval: String,
    pub idempotency_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CheckoutSessionDto {
    pub order_id: String,
    pub checkout_url: String,
    pub provider_session_id: String,
    pub expires_note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifyOrderPayload {
    pub order_id: String,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PortalSessionDto {
    pub portal_url: String,
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
    pub plan_id: String,
    pub plan_name: String,
    pub period_day_utc: String,
    pub resets_at_utc: String,
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AuthStatusResponse {
    pub status: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email_delivery: Option<String>,
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

fn scrub_client_message(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let lower = raw.to_ascii_lowercase();
    let bytes = raw.as_bytes();
    let lower_bytes = lower.as_bytes();
    let mut i = 0usize;

    while i < bytes.len() {
        if lower_bytes[i..].starts_with(b"bearer ") {
            out.push_str("Bearer <redacted>");
            i += 7;
            while i < bytes.len() && !bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            continue;
        }
        out.push(bytes[i] as char);
        i += 1;
    }

    if out.len() > 512 {
        out.truncate(512);
    }
    out
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
