//! Plans, versioned pricing, subscriptions, modular Stripe Checkout & Billing
//! Portal integration, and signed HMAC-SHA256 webhook processing (Phases 5–6).
//!
//! Architecture & safety guarantees:
//! - Single selected payment provider (`stripe`) behind [`PaymentProvider`],
//!   cleanly replaceable in tests and future deployments without rewriting
//!   plan, entitlement, or quota logic.
//! - Prices, currencies, billing intervals, and per-model daily limits come
//!   exclusively from server configuration synchronized into `plans`,
//!   `plan_versions`, and `plan_model_quotas` — **client-submitted prices or
//!   entitlement flags are never accepted**.
//! - Webhook verification computes RFC 2104 HMAC-SHA256 over
//!   `"{timestamp}.{raw_body}"` against `Stripe-Signature` (`t=...,v1=...`),
//!   enforces a bounded clock-skew window to block replay attacks, and records
//!   `(provider, provider_event_id)` in `webhook_events` so duplicate deliveries
//!   are strictly idempotent.

use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Row};
use std::future::Future;
use std::pin::Pin;

use crate::audit;

/// Default maximum age/skew accepted on `Stripe-Signature` timestamps.
pub const DEFAULT_WEBHOOK_TOLERANCE_SECS: i64 = 300;
/// Checkout order TTL before uncompleted sessions expire.
const CHECKOUT_ORDER_TTL_SECS: i64 = 86_400;

// ---------------------------------------------------------------------------
// Configurable plan & pricing catalogue (documented proposed defaults)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TierConfig {
    pub id: String,
    pub display_name: String,
    pub description: String,
    pub tier_rank: i32,
    pub monthly_amount_minor: Option<i64>,
    pub yearly_amount_minor: Option<i64>,
    pub stripe_price_monthly: Option<String>,
    pub stripe_price_yearly: Option<String>,
    pub mini_daily_limit: i32,
    pub sonnet_daily_limit: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanCatalogConfig {
    pub currency: String,
    pub tiers: Vec<TierConfig>,
}

impl Default for PlanCatalogConfig {
    /// Proposed default catalogue for engineering review — every price and
    /// daily model allowance can be overridden through environment variables.
    fn default() -> Self {
        Self {
            currency: "USD".to_string(),
            tiers: vec![
                TierConfig {
                    id: "free".to_string(),
                    display_name: "Free".to_string(),
                    description:
                        "On-device coding assistant with daily starter allowances for Nexora Mini and Sonnet."
                            .to_string(),
                    tier_rank: 0,
                    monthly_amount_minor: None,
                    yearly_amount_minor: None,
                    stripe_price_monthly: None,
                    stripe_price_yearly: None,
                    mini_daily_limit: 25,
                    sonnet_daily_limit: 5,
                },
                TierConfig {
                    id: "pro".to_string(),
                    display_name: "Pro".to_string(),
                    description:
                        "Expanded daily allowances for sustained software engineering across Nexora Mini and Sonnet."
                            .to_string(),
                    tier_rank: 1,
                    monthly_amount_minor: Some(1_900),
                    yearly_amount_minor: Some(19_000),
                    stripe_price_monthly: None,
                    stripe_price_yearly: None,
                    mini_daily_limit: 300,
                    sonnet_daily_limit: 100,
                },
                TierConfig {
                    id: "team".to_string(),
                    display_name: "Team".to_string(),
                    description:
                        "High-throughput daily allowances for intensive multi-repository engineering."
                            .to_string(),
                    tier_rank: 2,
                    monthly_amount_minor: Some(3_900),
                    yearly_amount_minor: Some(39_000),
                    stripe_price_monthly: None,
                    stripe_price_yearly: None,
                    mini_daily_limit: 1_000,
                    sonnet_daily_limit: 300,
                },
            ],
        }
    }
}

fn opt_env_any(names: &[&str]) -> Option<String> {
    for name in names {
        if let Ok(raw) = std::env::var(name) {
            let trimmed = raw.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

fn env_i32_any(names: &[&str], default: i32) -> Result<i32, String> {
    if let Some(raw) = opt_env_any(names) {
        return raw
            .parse::<i32>()
            .ok()
            .filter(|v| *v >= 0)
            .ok_or_else(|| format!("{} must be a non-negative integer.", names[0]));
    }
    Ok(default)
}

fn env_i64_any(names: &[&str], default: i64) -> Result<i64, String> {
    if let Some(raw) = opt_env_any(names) {
        return raw.parse::<i64>().ok().filter(|v| *v >= 0).ok_or_else(|| {
            format!(
                "{} must be a non-negative integer amount in minor currency units.",
                names[0]
            )
        });
    }
    Ok(default)
}

impl PlanCatalogConfig {
    pub fn from_env() -> Result<Self, String> {
        let mut cfg = Self::default();
        if let Some(curr) = opt_env_any(&["NEXORA_BILLING_CURRENCY"]) {
            let upper = curr.to_ascii_uppercase();
            if upper.len() != 3 || !upper.chars().all(|c| c.is_ascii_alphabetic()) {
                return Err("NEXORA_BILLING_CURRENCY must be a 3-letter ISO-4217 code.".to_string());
            }
            cfg.currency = upper;
        }
        // Free overrides
        cfg.tiers[0].mini_daily_limit = env_i32_any(
            &[
                "NEXORA_FREE_MINI_DAILY_LIMIT",
                "NEXORA_QUOTA_FREE_MINI_DAILY",
            ],
            25,
        )?;
        cfg.tiers[0].sonnet_daily_limit = env_i32_any(
            &[
                "NEXORA_FREE_SONNET_DAILY_LIMIT",
                "NEXORA_QUOTA_FREE_SONNET_DAILY",
            ],
            5,
        )?;
        // Pro overrides
        cfg.tiers[1].monthly_amount_minor = Some(env_i64_any(
            &[
                "NEXORA_PRO_MONTHLY_PRICE_CENTS",
                "NEXORA_PLAN_PRO_MONTHLY_CENTS",
            ],
            1_900,
        )?);
        cfg.tiers[1].yearly_amount_minor = Some(env_i64_any(
            &[
                "NEXORA_PRO_YEARLY_PRICE_CENTS",
                "NEXORA_PLAN_PRO_YEARLY_CENTS",
            ],
            19_000,
        )?);
        cfg.tiers[1].mini_daily_limit = env_i32_any(
            &["NEXORA_PRO_MINI_DAILY_LIMIT", "NEXORA_QUOTA_PRO_MINI_DAILY"],
            300,
        )?;
        cfg.tiers[1].sonnet_daily_limit = env_i32_any(
            &[
                "NEXORA_PRO_SONNET_DAILY_LIMIT",
                "NEXORA_QUOTA_PRO_SONNET_DAILY",
            ],
            100,
        )?;
        cfg.tiers[1].stripe_price_monthly = opt_env_any(&["STRIPE_PRICE_PRO_MONTHLY"]);
        cfg.tiers[1].stripe_price_yearly = opt_env_any(&["STRIPE_PRICE_PRO_YEARLY"]);
        // Team overrides
        cfg.tiers[2].monthly_amount_minor = Some(env_i64_any(
            &[
                "NEXORA_TEAM_MONTHLY_PRICE_CENTS",
                "NEXORA_PLAN_TEAM_MONTHLY_CENTS",
            ],
            3_900,
        )?);
        cfg.tiers[2].yearly_amount_minor = Some(env_i64_any(
            &[
                "NEXORA_TEAM_YEARLY_PRICE_CENTS",
                "NEXORA_PLAN_TEAM_YEARLY_CENTS",
            ],
            39_000,
        )?);
        cfg.tiers[2].mini_daily_limit = env_i32_any(
            &[
                "NEXORA_TEAM_MINI_DAILY_LIMIT",
                "NEXORA_QUOTA_TEAM_MINI_DAILY",
            ],
            1_000,
        )?;
        cfg.tiers[2].sonnet_daily_limit = env_i32_any(
            &[
                "NEXORA_TEAM_SONNET_DAILY_LIMIT",
                "NEXORA_QUOTA_TEAM_SONNET_DAILY",
            ],
            300,
        )?;
        cfg.tiers[2].stripe_price_monthly = opt_env_any(&["STRIPE_PRICE_TEAM_MONTHLY"]);
        cfg.tiers[2].stripe_price_yearly = opt_env_any(&["STRIPE_PRICE_TEAM_YEARLY"]);
        Ok(cfg)
    }
}

/// Synchronize configured plans, prices, and daily model limits into PostgreSQL.
pub async fn sync_plan_catalog(
    pool: &PgPool,
    catalog: &PlanCatalogConfig,
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    for tier in &catalog.tiers {
        sqlx::query(
            "INSERT INTO plans (id, display_name, description, tier_rank, active, updated_at)
             VALUES ($1, $2, $3, $4, true, now())
             ON CONFLICT (id) DO UPDATE
               SET display_name = EXCLUDED.display_name,
                   description = EXCLUDED.description,
                   tier_rank = EXCLUDED.tier_rank,
                   updated_at = now()",
        )
        .bind(&tier.id)
        .bind(&tier.display_name)
        .bind(&tier.description)
        .bind(tier.tier_rank)
        .execute(&mut *tx)
        .await?;

        if tier.monthly_amount_minor.is_none() && tier.yearly_amount_minor.is_none() {
            upsert_plan_version(&mut tx, &tier.id, "none", &catalog.currency, 0, None).await?;
        }
        if let Some(monthly) = tier.monthly_amount_minor {
            upsert_plan_version(
                &mut tx,
                &tier.id,
                "month",
                &catalog.currency,
                monthly,
                tier.stripe_price_monthly.as_deref(),
            )
            .await?;
        }
        if let Some(yearly) = tier.yearly_amount_minor {
            upsert_plan_version(
                &mut tx,
                &tier.id,
                "year",
                &catalog.currency,
                yearly,
                tier.stripe_price_yearly.as_deref(),
            )
            .await?;
        }

        for (model_id, limit) in [
            ("nexora-mini", tier.mini_daily_limit),
            ("nexora-sonnet", tier.sonnet_daily_limit),
        ] {
            sqlx::query(
                "INSERT INTO plan_model_quotas (plan_id, model_id, daily_limit, updated_at)
                 VALUES ($1, $2, $3, now())
                 ON CONFLICT (plan_id, model_id) DO UPDATE
                   SET daily_limit = EXCLUDED.daily_limit,
                       updated_at = now()",
            )
            .bind(&tier.id)
            .bind(model_id)
            .bind(limit)
            .execute(&mut *tx)
            .await?;
        }
    }
    tx.commit().await
}

async fn upsert_plan_version(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    plan_id: &str,
    interval: &str,
    currency: &str,
    amount_minor: i64,
    provider_price_id: Option<&str>,
) -> Result<(), sqlx::Error> {
    let existing: Option<(i32, String, i64, Option<String>)> = sqlx::query_as(
        "SELECT version_number, currency, amount_minor, provider_price_id
         FROM plan_versions
         WHERE plan_id = $1 AND billing_interval = $2 AND active = true
         ORDER BY version_number DESC
         LIMIT 1",
    )
    .bind(plan_id)
    .bind(interval)
    .fetch_optional(&mut **tx)
    .await?;

    match existing {
        Some((_ver, prev_curr, prev_amt, prev_price))
            if prev_curr == currency
                && prev_amt == amount_minor
                && prev_price.as_deref() == provider_price_id => {}
        Some((prev_ver, _, _, _)) => {
            sqlx::query(
                "UPDATE plan_versions SET active = false
                 WHERE plan_id = $1 AND billing_interval = $2 AND active = true",
            )
            .bind(plan_id)
            .bind(interval)
            .execute(&mut **tx)
            .await?;
            sqlx::query(
                "INSERT INTO plan_versions
                 (plan_id, version_number, billing_interval, currency, amount_minor, provider_price_id, active)
                 VALUES ($1, $2, $3, $4, $5, $6, true)",
            )
            .bind(plan_id)
            .bind(prev_ver + 1)
            .bind(interval)
            .bind(currency)
            .bind(amount_minor)
            .bind(provider_price_id)
            .execute(&mut **tx)
            .await?;
        }
        None => {
            sqlx::query(
                "INSERT INTO plan_versions
                 (plan_id, version_number, billing_interval, currency, amount_minor, provider_price_id, active)
                 VALUES ($1, 1, $2, $3, $4, $5, true)",
            )
            .bind(plan_id)
            .bind(interval)
            .bind(currency)
            .bind(amount_minor)
            .bind(provider_price_id)
            .execute(&mut **tx)
            .await?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Payment provider abstraction (Stripe Checkout + Billing Portal)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ProviderCheckoutInput {
    pub order_id: String,
    pub user_id: String,
    pub user_email: String,
    pub plan_id: String,
    pub plan_display_name: String,
    pub billing_interval: String,
    pub currency: String,
    pub amount_minor: i64,
    pub provider_price_id: Option<String>,
    pub idempotency_key: String,
}

#[derive(Debug, Clone)]
pub struct ProviderCheckoutSession {
    pub session_id: String,
    pub checkout_url: String,
}

#[derive(Debug, Clone)]
pub struct ProviderOrderVerification {
    pub paid: bool,
    pub checkout_session_id: Option<String>,
    pub order_id: Option<String>,
    pub user_id: Option<String>,
    pub client_reference_id: Option<String>,
    pub session_created_at_unix: Option<i64>,
    pub subscription_status: Option<String>,
    pub customer_id: Option<String>,
    pub subscription_id: Option<String>,
    pub payment_intent_id: Option<String>,
    pub amount_total_minor: Option<i64>,
    pub currency: Option<String>,
    pub current_period_end_unix: Option<i64>,
}

/// Identity, amount, and provider-state evidence required before a pending
/// order can create a paid subscription. Values are checked against the
/// server-stored payment order inside the activation transaction.
struct CheckoutActivationEvidence<'a> {
    checkout_session_id: &'a str,
    provider_order_id: &'a str,
    metadata_user_id: &'a str,
    client_reference_id: &'a str,
    customer_id: Option<&'a str>,
    subscription_id: &'a str,
    payment_intent_id: Option<&'a str>,
    amount_total_minor: i64,
    currency: &'a str,
    current_period_end_unix: i64,
    provider_event_created_unix: i64,
}

/// Serialize checkout activation and webhook processing for shared provider
/// identifiers. Keys are sorted before locking so a checkout's payment intent
/// and subscription cannot race a refund or lifecycle event.
async fn lock_provider_transition_keys(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    subscription_id: Option<&str>,
    payment_intent_id: Option<&str>,
) -> Result<(), BillingError> {
    let mut keys = Vec::with_capacity(2);
    if let Some(id) = subscription_id.filter(|id| !id.trim().is_empty()) {
        keys.push(format!("stripe:subscription:{id}"));
    }
    if let Some(id) = payment_intent_id.filter(|id| !id.trim().is_empty()) {
        keys.push(format!("stripe:payment-intent:{id}"));
    }
    keys.sort_unstable();
    keys.dedup();

    for key in keys {
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(key)
            .fetch_one(&mut **tx)
            .await
            .map_err(|_| BillingError::Internal)?;
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BillingError {
    Validation(String),
    PaymentProviderUnconfigured,
    ProviderOperationUnsupported,
    DependencyUnavailable,
    NotFound(String),
    Conflict(String),
    SignatureInvalid,
    ProviderFailure,
    Internal,
}

impl BillingError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Validation(_) => "validation_failed",
            Self::PaymentProviderUnconfigured => "payment_provider_unconfigured",
            Self::ProviderOperationUnsupported => "payment_provider_operation_unsupported",
            Self::DependencyUnavailable => "dependency_unavailable",
            Self::NotFound(_) => "not_found",
            Self::Conflict(_) => "billing_conflict",
            Self::SignatureInvalid => "invalid_webhook_signature",
            Self::ProviderFailure => "payment_provider_error",
            Self::Internal => "internal_error",
        }
    }

    pub fn status_code(&self) -> u16 {
        match self {
            Self::Validation(_) => 400,
            Self::SignatureInvalid => 400,
            Self::NotFound(_) => 404,
            Self::Conflict(_) => 409,
            Self::ProviderFailure => 502,
            Self::PaymentProviderUnconfigured
            | Self::ProviderOperationUnsupported
            | Self::DependencyUnavailable => 503,
            Self::Internal => 500,
        }
    }

    pub fn message(&self) -> String {
        match self {
            Self::Validation(msg) => msg.clone(),
            Self::PaymentProviderUnconfigured => {
                "Online payments are disabled in this build pending approved provider and entitlement review; credentials do not enable checkout.".to_string()
            }
            Self::ProviderOperationUnsupported => {
                "Plan changes and subscription cancellation are disabled because no trusted provider lifecycle operation is implemented; no entitlement state was changed.".to_string()
            }
            Self::DependencyUnavailable => {
                "Billing service is unavailable: the account database is not configured.".to_string()
            }
            Self::NotFound(msg) => msg.clone(),
            Self::Conflict(msg) => msg.clone(),
            Self::SignatureInvalid => "Webhook signature verification failed.".to_string(),
            Self::ProviderFailure => {
                "The payment provider could not complete the request. Please try again shortly.".to_string()
            }
            Self::Internal => "An unexpected billing error occurred.".to_string(),
        }
    }
}

pub trait PaymentProvider: Send + Sync {
    fn is_configured(&self) -> bool;

    fn create_checkout<'a>(
        &'a self,
        input: &'a ProviderCheckoutInput,
    ) -> Pin<Box<dyn Future<Output = Result<ProviderCheckoutSession, BillingError>> + Send + 'a>>;

    fn verify_checkout<'a>(
        &'a self,
        session_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<ProviderOrderVerification, BillingError>> + Send + 'a>>;

    fn create_billing_portal<'a>(
        &'a self,
        customer_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<String, BillingError>> + Send + 'a>>;
}

pub struct UnconfiguredPaymentProvider;

impl PaymentProvider for UnconfiguredPaymentProvider {
    fn is_configured(&self) -> bool {
        false
    }

    fn create_checkout<'a>(
        &'a self,
        _input: &'a ProviderCheckoutInput,
    ) -> Pin<Box<dyn Future<Output = Result<ProviderCheckoutSession, BillingError>> + Send + 'a>>
    {
        Box::pin(async { Err(BillingError::PaymentProviderUnconfigured) })
    }

    fn verify_checkout<'a>(
        &'a self,
        _session_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<ProviderOrderVerification, BillingError>> + Send + 'a>>
    {
        Box::pin(async { Err(BillingError::PaymentProviderUnconfigured) })
    }

    fn create_billing_portal<'a>(
        &'a self,
        _customer_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<String, BillingError>> + Send + 'a>> {
        Box::pin(async { Err(BillingError::PaymentProviderUnconfigured) })
    }
}

pub struct StripeHttpProvider {
    secret_key: String,
    success_url: String,
    cancel_url: String,
    portal_return_url: String,
    api_base: String,
    client: reqwest::Client,
}

impl std::fmt::Debug for StripeHttpProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StripeHttpProvider")
            .field("secret_key", &"<redacted>")
            .field("success_url", &self.success_url)
            .field("cancel_url", &self.cancel_url)
            .field("portal_return_url", &self.portal_return_url)
            .field("api_base", &self.api_base)
            .finish()
    }
}

impl StripeHttpProvider {
    pub fn new(
        secret_key: String,
        success_url: String,
        cancel_url: String,
        portal_return_url: String,
    ) -> Self {
        Self {
            secret_key,
            success_url,
            cancel_url,
            portal_return_url,
            api_base: "https://api.stripe.com".to_string(),
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(15))
                .build()
                .unwrap_or_default(),
        }
    }
}

/// Basic payment-state gate for Checkout. A completed session must be a
/// subscription session and explicitly paid; callers still require exact
/// stored order/session/account identity, an active expanded subscription,
/// provider period end, and a refund-trackable payment intent before activation.
fn stripe_object_id<'a>(value: Option<&'a serde_json::Value>) -> Option<&'a str> {
    match value? {
        serde_json::Value::String(id) => Some(id.as_str()),
        serde_json::Value::Object(object) => object
            .get("id")
            .and_then(serde_json::Value::as_str),
        _ => None,
    }
}

fn active_subscription_details(body: &serde_json::Value) -> Option<(String, i64)> {
    let subscription = body.get("subscription")?.as_object()?;
    let id = subscription.get("id")?.as_str()?.trim();
    let status = subscription.get("status")?.as_str()?;
    let period_end = subscription.get("current_period_end")?.as_i64()?;
    if id.is_empty() || status != "active" || period_end <= current_unix_seconds() {
        return None;
    }
    Some((id.to_string(), period_end))
}

fn current_unix_seconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

/// Stripe Checkout completion is a payment signal, but activation additionally
/// requires the expanded active subscription and exact stored order/account
/// identity. Unexpanded sessions cannot establish a billing period and fail
/// closed rather than receiving a guessed 30/365-day entitlement.
fn is_completed_paid_subscription_session(body: &serde_json::Value) -> bool {
    body.get("mode").and_then(serde_json::Value::as_str) == Some("subscription")
        && body.get("status").and_then(serde_json::Value::as_str) == Some("complete")
        && body.get("payment_status").and_then(serde_json::Value::as_str) == Some("paid")
        && stripe_object_id(body.get("subscription"))
            .is_some_and(|id| !id.trim().is_empty())
}

impl PaymentProvider for StripeHttpProvider {
    fn is_configured(&self) -> bool {
        true
    }

    fn create_checkout<'a>(
        &'a self,
        input: &'a ProviderCheckoutInput,
    ) -> Pin<Box<dyn Future<Output = Result<ProviderCheckoutSession, BillingError>> + Send + 'a>>
    {
        Box::pin(async move {
            let mut form: Vec<(String, String)> = vec![
                ("mode".into(), "subscription".into()),
                ("customer_email".into(), input.user_email.clone()),
                ("client_reference_id".into(), input.user_id.clone()),
                ("success_url".into(), self.success_url.clone()),
                ("cancel_url".into(), self.cancel_url.clone()),
                ("metadata[order_id]".into(), input.order_id.clone()),
                ("metadata[user_id]".into(), input.user_id.clone()),
                ("metadata[plan_id]".into(), input.plan_id.clone()),
                (
                    "metadata[billing_interval]".into(),
                    input.billing_interval.clone(),
                ),
                (
                    "subscription_data[metadata][user_id]".into(),
                    input.user_id.clone(),
                ),
                (
                    "subscription_data[metadata][plan_id]".into(),
                    input.plan_id.clone(),
                ),
                ("line_items[0][quantity]".into(), "1".into()),
            ];
            if let Some(price_id) = &input.provider_price_id {
                form.push(("line_items[0][price]".into(), price_id.clone()));
            } else {
                form.push((
                    "line_items[0][price_data][currency]".into(),
                    input.currency.to_ascii_lowercase(),
                ));
                form.push((
                    "line_items[0][price_data][unit_amount]".into(),
                    input.amount_minor.to_string(),
                ));
                form.push((
                    "line_items[0][price_data][recurring][interval]".into(),
                    input.billing_interval.clone(),
                ));
                form.push((
                    "line_items[0][price_data][product_data][name]".into(),
                    format!("Nexora {}", input.plan_display_name),
                ));
            }

            let resp = self
                .client
                .post(format!("{}/v1/checkout/sessions", self.api_base))
                .bearer_auth(&self.secret_key)
                .header("Idempotency-Key", &input.idempotency_key)
                .form(&form)
                .send()
                .await
                .map_err(|_| BillingError::ProviderFailure)?;
            if !resp.status().is_success() {
                return Err(BillingError::ProviderFailure);
            }
            let body: serde_json::Value = resp
                .json()
                .await
                .map_err(|_| BillingError::ProviderFailure)?;
            let session_id = body
                .get("id")
                .and_then(|v| v.as_str())
                .ok_or(BillingError::ProviderFailure)?
                .to_string();
            let checkout_url = body
                .get("url")
                .and_then(|v| v.as_str())
                .ok_or(BillingError::ProviderFailure)?
                .to_string();
            Ok(ProviderCheckoutSession {
                session_id,
                checkout_url,
            })
        })
    }

    fn verify_checkout<'a>(
        &'a self,
        session_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<ProviderOrderVerification, BillingError>> + Send + 'a>>
    {
        Box::pin(async move {
            let resp = self
                .client
                .get(format!(
                    "{}/v1/checkout/sessions/{session_id}",
                    self.api_base
                ))
                .bearer_auth(&self.secret_key)
                .query(&[
                    ("expand[]", "subscription"),
                    ("expand[]", "subscription.latest_invoice.payment_intent"),
                ])
                .send()
                .await
                .map_err(|_| BillingError::ProviderFailure)?;
            if !resp.status().is_success() {
                return Err(BillingError::ProviderFailure);
            }
            let body: serde_json::Value = resp
                .json()
                .await
                .map_err(|_| BillingError::ProviderFailure)?;
            let checkout_session_id = body
                .get("id")
                .and_then(|v| v.as_str())
                .map(ToString::to_string);
            let order_id = body
                .pointer("/metadata/order_id")
                .and_then(|v| v.as_str())
                .map(ToString::to_string);
            let user_id = body
                .pointer("/metadata/user_id")
                .and_then(|v| v.as_str())
                .map(ToString::to_string);
            let client_reference_id = body
                .get("client_reference_id")
                .and_then(|v| v.as_str())
                .map(ToString::to_string);
            let session_created_at_unix = body.get("created").and_then(|v| v.as_i64());
            let customer_id = stripe_object_id(body.get("customer")).map(ToString::to_string);
            let payment_intent_id = stripe_object_id(body.get("payment_intent"))
                .or_else(|| {
                    stripe_object_id(body.pointer("/subscription/latest_invoice/payment_intent"))
                })
                .map(ToString::to_string);
            let amount_total_minor = body.get("amount_total").and_then(|v| v.as_i64());
            let currency = body
                .get("currency")
                .and_then(|v| v.as_str())
                .map(ToString::to_string);
            let (subscription_id, subscription_status, current_period_end_unix) =
                match body.get("subscription") {
                    Some(serde_json::Value::String(id)) => {
                        (Some(id.clone()), None, None)
                    }
                    Some(serde_json::Value::Object(sub)) => (
                        sub.get("id")
                            .and_then(|v| v.as_str())
                            .map(ToString::to_string),
                        sub.get("status")
                            .and_then(|v| v.as_str())
                            .map(ToString::to_string),
                        sub.get("current_period_end").and_then(|v| v.as_i64()),
                    ),
                    _ => (None, None, None),
                };
            Ok(ProviderOrderVerification {
                paid: is_completed_paid_subscription_session(&body),
                checkout_session_id,
                order_id,
                user_id,
                client_reference_id,
                session_created_at_unix,
                subscription_status,
                customer_id,
                subscription_id,
                payment_intent_id,
                amount_total_minor,
                currency,
                current_period_end_unix,
            })
        })
    }

    fn create_billing_portal<'a>(
        &'a self,
        customer_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<String, BillingError>> + Send + 'a>> {
        Box::pin(async move {
            let resp = self
                .client
                .post(format!("{}/v1/billing_portal/sessions", self.api_base))
                .bearer_auth(&self.secret_key)
                .form(&[
                    ("customer", customer_id),
                    ("return_url", self.portal_return_url.as_str()),
                ])
                .send()
                .await
                .map_err(|_| BillingError::ProviderFailure)?;
            if !resp.status().is_success() {
                return Err(BillingError::ProviderFailure);
            }
            let body: serde_json::Value = resp
                .json()
                .await
                .map_err(|_| BillingError::ProviderFailure)?;
            body.get("url")
                .and_then(|v| v.as_str())
                .map(ToString::to_string)
                .ok_or(BillingError::ProviderFailure)
        })
    }
}

// ---------------------------------------------------------------------------
// RFC 2104 HMAC-SHA256 & constant-time Stripe webhook verification
// ---------------------------------------------------------------------------

pub fn hmac_sha256(key: &[u8], message: &[u8]) -> [u8; 32] {
    const BLOCK: usize = 64;
    let mut normalized_key = [0u8; BLOCK];
    if key.len() > BLOCK {
        let digest = Sha256::digest(key);
        normalized_key[..32].copy_from_slice(&digest);
    } else {
        normalized_key[..key.len()].copy_from_slice(key);
    }
    let mut ipad = [0x36u8; BLOCK];
    let mut opad = [0x5cu8; BLOCK];
    for i in 0..BLOCK {
        ipad[i] ^= normalized_key[i];
        opad[i] ^= normalized_key[i];
    }
    let mut inner = Sha256::new();
    inner.update(ipad);
    inner.update(message);
    let inner_digest = inner.finalize();

    let mut outer = Sha256::new();
    outer.update(opad);
    outer.update(inner_digest);
    let final_digest = outer.finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(&final_digest);
    out
}

fn decode_hex_32(hex: &str) -> Option<[u8; 32]> {
    if hex.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    let bytes = hex.as_bytes();
    for i in 0..32 {
        let hi = (bytes[2 * i] as char).to_digit(16)?;
        let lo = (bytes[2 * i + 1] as char).to_digit(16)?;
        out[i] = ((hi << 4) | lo) as u8;
    }
    Some(out)
}

fn constant_time_eq_32(a: &[u8; 32], b: &[u8; 32]) -> bool {
    let mut diff = 0u8;
    for i in 0..32 {
        diff |= a[i] ^ b[i];
    }
    diff == 0
}

pub fn compute_stripe_signature_header(secret: &str, timestamp: i64, payload: &[u8]) -> String {
    let mut signed = Vec::with_capacity(24 + payload.len());
    signed.extend_from_slice(timestamp.to_string().as_bytes());
    signed.push(b'.');
    signed.extend_from_slice(payload);
    let mac = hmac_sha256(secret.as_bytes(), &signed);
    let mut hex = String::with_capacity(64);
    for b in mac {
        hex.push_str(&format!("{b:02x}"));
    }
    format!("t={timestamp},v1={hex}")
}

pub fn verify_stripe_webhook_signature(
    payload: &[u8],
    header: &str,
    secret: &str,
    now_unix: i64,
    tolerance_secs: i64,
) -> Result<(), BillingError> {
    if secret.trim().is_empty() {
        return Err(BillingError::PaymentProviderUnconfigured);
    }
    let mut timestamp: Option<i64> = None;
    let mut signatures: Vec<[u8; 32]> = Vec::new();
    for part in header.split(',') {
        let Some((k, v)) = part.trim().split_once('=') else {
            continue;
        };
        match k.trim() {
            "t" => {
                timestamp = v.trim().parse::<i64>().ok();
            }
            "v1" => {
                if let Some(sig) = decode_hex_32(v.trim()) {
                    signatures.push(sig);
                }
            }
            _ => {}
        }
    }
    let Some(ts) = timestamp else {
        return Err(BillingError::SignatureInvalid);
    };
    if signatures.is_empty() {
        return Err(BillingError::SignatureInvalid);
    }
    // Both fields are attacker-controlled (timestamp from the header, and the
    // tolerance is supplied by the caller). Avoid signed subtraction/`abs`,
    // which can overflow for i64::MIN/MAX and panic or wrap in release builds.
    if tolerance_secs < 0 || now_unix.abs_diff(ts) > tolerance_secs as u64 {
        return Err(BillingError::SignatureInvalid);
    }
    let mut signed = Vec::with_capacity(24 + payload.len());
    signed.extend_from_slice(ts.to_string().as_bytes());
    signed.push(b'.');
    signed.extend_from_slice(payload);
    let expected = hmac_sha256(secret.as_bytes(), &signed);
    let matched = signatures.iter().fold(false, |acc, candidate| {
        acc | constant_time_eq_32(&expected, candidate)
    });
    if matched {
        Ok(())
    } else {
        Err(BillingError::SignatureInvalid)
    }
}

// ---------------------------------------------------------------------------
// DTOs & PostgreSQL domain operations
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PlanPriceView {
    pub interval: String,
    pub currency: String,
    pub amount_minor: i64,
    pub version_number: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PlanModelLimitView {
    pub model_id: String,
    pub daily_limit: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PlanView {
    pub id: String,
    pub display_name: String,
    pub description: String,
    pub tier_rank: i32,
    pub prices: Vec<PlanPriceView>,
    pub model_quotas: Vec<PlanModelLimitView>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlansCatalogueResponse {
    pub provider: &'static str,
    pub payment_configured: bool,
    pub plans: Vec<PlanView>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubscriptionView {
    pub id: String,
    pub plan_id: String,
    pub status: String,
    pub provider: String,
    pub cancel_at_period_end: bool,
    pub canceled_at: Option<String>,
    pub current_period_start: String,
    pub current_period_end: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentOrderSummary {
    pub id: String,
    pub plan_id: String,
    pub billing_interval: String,
    pub currency: String,
    pub amount_minor: i64,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BillingStatusResponse {
    pub effective_plan_id: String,
    pub effective_plan_name: String,
    pub payment_configured: bool,
    pub portal_available: bool,
    pub subscription: Option<SubscriptionView>,
    pub recent_orders: Vec<PaymentOrderSummary>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CheckoutRequest {
    pub plan_id: String,
    pub interval: String,
    pub idempotency_key: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckoutResponse {
    pub order_id: String,
    pub checkout_url: String,
    pub provider_session_id: String,
    pub expires_at: String,
    pub expires_note: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VerifyOrderRequest {
    pub order_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CancelSubscriptionRequest {
    #[serde(default)]
    pub immediate: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChangePlanRequest {
    pub plan_id: String,
    pub interval: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PortalResponse {
    pub portal_url: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebhookAckResponse {
    pub received: bool,
    pub deduplicated: bool,
}

pub async fn list_plans(
    pool: Option<&PgPool>,
    payment_configured: bool,
) -> Result<PlansCatalogueResponse, BillingError> {
    let Some(pool) = pool else {
        return Err(BillingError::DependencyUnavailable);
    };
    let plan_rows = sqlx::query(
        "SELECT id, display_name, description, tier_rank
         FROM plans
         WHERE active = true
         ORDER BY tier_rank ASC",
    )
    .fetch_all(pool)
    .await
    .map_err(|_| BillingError::Internal)?;

    let version_rows = sqlx::query(
        "SELECT plan_id, billing_interval, currency, amount_minor, version_number
         FROM plan_versions
         WHERE active = true
         ORDER BY billing_interval ASC",
    )
    .fetch_all(pool)
    .await
    .map_err(|_| BillingError::Internal)?;

    let quota_rows = sqlx::query(
        "SELECT plan_id, model_id, daily_limit
         FROM plan_model_quotas
         ORDER BY model_id ASC",
    )
    .fetch_all(pool)
    .await
    .map_err(|_| BillingError::Internal)?;

    let mut plans = Vec::with_capacity(plan_rows.len());
    for row in plan_rows {
        let id: String = row.get("id");
        let prices = version_rows
            .iter()
            .filter(|r| r.get::<String, _>("plan_id") == id)
            .map(|r| PlanPriceView {
                interval: r.get("billing_interval"),
                currency: r.get("currency"),
                amount_minor: r.get("amount_minor"),
                version_number: r.get("version_number"),
            })
            .collect();
        let model_quotas = quota_rows
            .iter()
            .filter(|r| r.get::<String, _>("plan_id") == id)
            .map(|r| PlanModelLimitView {
                model_id: r.get("model_id"),
                daily_limit: r.get("daily_limit"),
            })
            .collect();
        plans.push(PlanView {
            id,
            display_name: row.get("display_name"),
            description: row.get("description"),
            tier_rank: row.get("tier_rank"),
            prices,
            model_quotas,
        });
    }

    Ok(PlansCatalogueResponse {
        provider: "stripe",
        payment_configured,
        plans,
    })
}

/// Reconcile any elapsed active/trialing/canceled subscriptions whose billing
/// period has now ended before reading entitlements.
pub async fn expire_elapsed_subscriptions(
    pool: &PgPool,
    user_id: &str,
) -> Result<(), BillingError> {
    sqlx::query(
        "UPDATE subscriptions
         SET status = 'expired',
             ended_at = COALESCE(ended_at, now()),
             updated_at = now()
         WHERE user_id = $1::uuid
           AND status IN ('active', 'trialing', 'canceled')
           AND current_period_end <= now()",
    )
    .bind(user_id)
    .execute(pool)
    .await
    .map_err(|_| BillingError::Internal)?;
    Ok(())
}

pub async fn resolve_effective_plan(
    pool: &PgPool,
    user_id: &str,
) -> Result<(String, String), BillingError> {
    expire_elapsed_subscriptions(pool, user_id).await?;
    let active: Option<(String, String)> = sqlx::query_as(
        "SELECT p.id, p.display_name
         FROM subscriptions s
         JOIN plans p ON p.id = s.plan_id
         WHERE s.user_id = $1::uuid
           AND s.provider = 'stripe'
           AND s.last_provider_event_created_at IS NOT NULL
           AND s.status = 'active'
           AND s.current_period_end > now()
         ORDER BY p.tier_rank DESC, s.current_period_end DESC
         LIMIT 1",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .map_err(|_| BillingError::Internal)?;

    if let Some(pair) = active {
        return Ok(pair);
    }
    let free: (String, String) =
        sqlx::query_as("SELECT id, display_name FROM plans WHERE id = 'free'")
            .fetch_one(pool)
            .await
            .map_err(|_| BillingError::Internal)?;
    Ok(free)
}

pub async fn get_billing_status(
    pool: Option<&PgPool>,
    user_id: &str,
    payment_configured: bool,
) -> Result<BillingStatusResponse, BillingError> {
    let Some(pool) = pool else {
        return Err(BillingError::DependencyUnavailable);
    };
    let (effective_plan_id, effective_plan_name) = resolve_effective_plan(pool, user_id).await?;

    let sub_row = sqlx::query(
        "SELECT id::text AS id, plan_id, status, provider, cancel_at_period_end,
                provider_customer_id,
                CASE WHEN canceled_at IS NOT NULL
                     THEN to_char(canceled_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"')
                     ELSE NULL END AS canceled_at,
                to_char(current_period_start AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS period_start,
                to_char(current_period_end AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS period_end
         FROM subscriptions
         WHERE user_id = $1::uuid
         ORDER BY updated_at DESC
         LIMIT 1",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .map_err(|_| BillingError::Internal)?;

    let mut has_customer = false;
    let subscription = sub_row.map(|r| {
        has_customer = r.get::<Option<String>, _>("provider_customer_id").is_some();
        SubscriptionView {
            id: r.get("id"),
            plan_id: r.get("plan_id"),
            status: r.get("status"),
            provider: r.get("provider"),
            cancel_at_period_end: r.get("cancel_at_period_end"),
            canceled_at: r.get("canceled_at"),
            current_period_start: r.get("period_start"),
            current_period_end: r.get("period_end"),
        }
    });

    let order_rows = sqlx::query(
        "SELECT id::text AS id, plan_id, billing_interval, currency, amount_minor, status,
                to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at
         FROM payment_orders
         WHERE user_id = $1::uuid
         ORDER BY created_at DESC
         LIMIT 5",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
    .map_err(|_| BillingError::Internal)?;

    let recent_orders = order_rows
        .into_iter()
        .map(|r| PaymentOrderSummary {
            id: r.get("id"),
            plan_id: r.get("plan_id"),
            billing_interval: r.get("billing_interval"),
            currency: r.get("currency"),
            amount_minor: r.get("amount_minor"),
            status: r.get("status"),
            created_at: r.get("created_at"),
        })
        .collect();

    Ok(BillingStatusResponse {
        effective_plan_id,
        effective_plan_name,
        payment_configured,
        portal_available: payment_configured && has_customer,
        subscription,
        recent_orders,
    })
}

pub async fn create_checkout_session(
    pool: Option<&PgPool>,
    provider: &dyn PaymentProvider,
    user_id: &str,
    user_email: &str,
    request: &CheckoutRequest,
) -> Result<CheckoutResponse, BillingError> {
    let plan_id = request.plan_id.trim();
    let interval = request.interval.trim();
    let idem_key = request.idempotency_key.trim();

    if plan_id.is_empty() || plan_id == "free" {
        return Err(BillingError::Validation(
            "Select a paid plan ('pro' or 'team') to start checkout.".to_string(),
        ));
    }
    if interval != "month" && interval != "year" {
        return Err(BillingError::Validation(
            "Billing interval must be 'month' or 'year'.".to_string(),
        ));
    }
    if idem_key.len() < 8
        || idem_key.len() > 128
        || !idem_key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | ':'))
    {
        return Err(BillingError::Validation(
            "Idempotency key must be 8–128 URL-safe characters.".to_string(),
        ));
    }
    let Some(pool) = pool else {
        return Err(BillingError::DependencyUnavailable);
    };
    if !provider.is_configured() {
        return Err(BillingError::PaymentProviderUnconfigured);
    }

    // Idempotent replay check before creating a new provider session.
    if let Some(existing) = sqlx::query(
        "SELECT id::text AS id, checkout_url, provider_checkout_session_id,
                to_char(expires_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS expires_at
         FROM payment_orders
         WHERE user_id = $1::uuid AND idempotency_key = $2",
    )
    .bind(user_id)
    .bind(idem_key)
    .fetch_optional(pool)
    .await
    .map_err(|_| BillingError::Internal)?
    {
        if let Some(url) = existing.get::<Option<String>, _>("checkout_url") {
            let expires_at: String = existing.get("expires_at");
            let session_id: String = existing
                .get::<Option<String>, _>("provider_checkout_session_id")
                .unwrap_or_default();
            return Ok(CheckoutResponse {
                order_id: existing.get("id"),
                checkout_url: url,
                provider_session_id: session_id,
                expires_note: format!("Expires at {expires_at}"),
                expires_at,
            });
        }
    }

    let version_row = sqlx::query(
        "SELECT pv.id::text AS version_id, pv.currency, pv.amount_minor, pv.provider_price_id, p.display_name
         FROM plan_versions pv
         JOIN plans p ON p.id = pv.plan_id
         WHERE pv.plan_id = $1 AND pv.billing_interval = $2 AND pv.active = true AND p.active = true
         ORDER BY pv.version_number DESC
         LIMIT 1",
    )
    .bind(plan_id)
    .bind(interval)
    .fetch_optional(pool)
    .await
    .map_err(|_| BillingError::Internal)?
    .ok_or_else(|| BillingError::Validation("Unknown plan or billing interval.".to_string()))?;

    let version_id: String = version_row.get("version_id");
    let currency: String = version_row.get("currency");
    let amount_minor: i64 = version_row.get("amount_minor");
    let provider_price_id: Option<String> = version_row.get("provider_price_id");
    let display_name: String = version_row.get("display_name");

    let order_row = sqlx::query(
        "INSERT INTO payment_orders
         (user_id, plan_id, plan_version_id, billing_interval, currency, amount_minor, provider, idempotency_key, status, expires_at)
         VALUES ($1::uuid, $2, $3::uuid, $4, $5, $6, 'stripe', $7, 'pending', now() + make_interval(secs => $8))
         RETURNING id::text AS id,
                   to_char(expires_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS expires_at",
    )
    .bind(user_id)
    .bind(plan_id)
    .bind(&version_id)
    .bind(interval)
    .bind(&currency)
    .bind(amount_minor)
    .bind(idem_key)
    .bind(CHECKOUT_ORDER_TTL_SECS as f64)
    .fetch_one(pool)
    .await
    .map_err(|_| BillingError::Conflict("Duplicate checkout order request.".to_string()))?;

    let order_id: String = order_row.get("id");
    let expires_at: String = order_row.get("expires_at");

    let created = provider
        .create_checkout(&ProviderCheckoutInput {
            order_id: order_id.clone(),
            user_id: user_id.to_string(),
            user_email: user_email.to_string(),
            plan_id: plan_id.to_string(),
            plan_display_name: display_name,
            billing_interval: interval.to_string(),
            currency,
            amount_minor,
            provider_price_id,
            idempotency_key: idem_key.to_string(),
        })
        .await;

    let session = match created {
        Ok(session) => session,
        Err(err) => {
            let _ = sqlx::query(
                "UPDATE payment_orders SET status = 'failed', updated_at = now() WHERE id = $1::uuid",
            )
            .bind(&order_id)
            .execute(pool)
            .await;
            return Err(err);
        }
    };

    sqlx::query(
        "UPDATE payment_orders
         SET provider_checkout_session_id = $1,
             checkout_url = $2,
             updated_at = now()
         WHERE id = $3::uuid",
    )
    .bind(&session.session_id)
    .bind(&session.checkout_url)
    .bind(&order_id)
    .execute(pool)
    .await
    .map_err(|_| BillingError::Internal)?;

    audit::record_event(
        pool,
        Some(user_id),
        user_id,
        "billing_checkout_created",
        "ok",
        json!({ "orderId": order_id, "planId": plan_id, "interval": interval }),
    )
    .await;

    Ok(CheckoutResponse {
        order_id,
        checkout_url: session.checkout_url,
        provider_session_id: session.session_id,
        expires_note: format!("Expires at {expires_at}"),
        expires_at,
    })
}

async fn activate_order_in_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    order_id: &str,
    evidence: CheckoutActivationEvidence<'_>,
) -> Result<Option<(String, String)>, BillingError> {
    let subscription_id = evidence.subscription_id.trim();
    let payment_intent_id = evidence
        .payment_intent_id
        .map(str::trim)
        .filter(|id| !id.is_empty());
    let Some(payment_intent_id) = payment_intent_id else {
        // Without a provider payment-intent identity, later full-refund events
        // cannot be safely joined to this order; do not activate it.
        return Ok(None);
    };
    if subscription_id.is_empty()
        || evidence.current_period_end_unix <= current_unix_seconds()
        || evidence.provider_event_created_unix <= 0
        || evidence.provider_event_created_unix
            > current_unix_seconds().saturating_add(DEFAULT_WEBHOOK_TOLERANCE_SECS)
    {
        return Ok(None);
    }
    lock_provider_transition_keys(tx, Some(subscription_id), Some(payment_intent_id)).await?;

    let row = sqlx::query(
        "SELECT user_id::text AS user_id, plan_id, plan_version_id::text AS plan_version_id,
                currency, amount_minor, status, provider, provider_checkout_session_id,
                provider_payment_intent_id, provider_subscription_id,
                (expires_at > now()) AS order_is_unexpired
         FROM payment_orders
         WHERE id = $1::uuid
         FOR UPDATE",
    )
    .bind(order_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|_| BillingError::Internal)?
    .ok_or_else(|| BillingError::NotFound("Payment order not found.".to_string()))?;

    let status: String = row.get("status");
    let provider: String = row.get("provider");
    let stored_session_id: Option<String> = row.get("provider_checkout_session_id");
    let linked_payment_intent_id: Option<String> = row.get("provider_payment_intent_id");
    let linked_subscription_id: Option<String> = row.get("provider_subscription_id");
    let expected_currency: String = row.get("currency");
    let expected_amount_minor: i64 = row.get("amount_minor");
    let user_id: String = row.get("user_id");
    if status != "pending"
        || provider != "stripe"
        || !row.get::<bool, _>("order_is_unexpired")
        || stored_session_id.as_deref() != Some(evidence.checkout_session_id)
        || evidence.provider_order_id != order_id
        || evidence.metadata_user_id != user_id.as_str()
        || evidence.client_reference_id != user_id.as_str()
        || evidence.checkout_session_id.trim().is_empty()
        || evidence.amount_total_minor != expected_amount_minor
        || !expected_currency.eq_ignore_ascii_case(evidence.currency)
        || subscription_id.is_empty()
        || linked_subscription_id
            .as_deref()
            .is_some_and(|linked| linked != subscription_id)
        || linked_payment_intent_id
            .as_deref()
            .is_some_and(|linked| linked != payment_intent_id)
    {
        // Only a fresh pending order for its stored provider/session/account
        // can transition. Stale, mismatched, expired, or terminal orders are
        // inert and remain available for audit.
        return Ok(None);
    }

    let conflicting_provider_state: bool = sqlx::query_scalar(
        "SELECT EXISTS (
             SELECT 1 FROM webhook_events
             WHERE provider = 'stripe'
               AND (
                   (
                       is_full_refund
                       AND (
                           provider_payment_intent_id = $1
                           OR provider_payment_intent_id IN (
                               SELECT provider_payment_intent_id FROM webhook_events
                               WHERE provider = 'stripe'
                                 AND provider_subscription_id = $2
                                 AND provider_payment_intent_id IS NOT NULL
                           )
                       )
                   )
                   OR (
                       provider_subscription_id = $2
                       AND (
                           event_type = 'customer.subscription.deleted'
                           OR provider_subscription_status IN ('canceled', 'expired', 'revoked')
                           OR (
                               provider_event_created_at >= $3
                               AND (
                                   event_type = 'invoice.payment_failed'
                                   OR (
                                       event_type = 'customer.subscription.updated'
                                       AND COALESCE(provider_subscription_status, '') <> 'active'
                                   )
                               )
                           )
                       )
                   )
               )
         )",
    )
    .bind(payment_intent_id)
    .bind(subscription_id)
    .bind(evidence.provider_event_created_unix)
    .fetch_one(&mut **tx)
    .await
    .map_err(|_| BillingError::Internal)?;
    if conflicting_provider_state {
        return Ok(None);
    }

    let plan_id: String = row.get("plan_id");
    let plan_version_id: String = row.get("plan_version_id");
    let customer_id = evidence
        .customer_id
        .map(str::trim)
        .filter(|id| !id.is_empty());

    // A provider subscription identifier is globally unique in our schema.
    // Never upsert it: a collision must not rewrite another account's plan or
    // reactivate a terminal subscription.
    let inserted = sqlx::query(
        "INSERT INTO subscriptions
         (user_id, plan_id, plan_version_id, provider, provider_customer_id, provider_subscription_id,
          status, cancel_at_period_end, current_period_start, current_period_end,
          last_provider_event_created_at, updated_at)
         VALUES (
            $1::uuid, $2, $3::uuid, 'stripe', $4, $5,
            'active', false, now(), to_timestamp($6::double precision),
            GREATEST($7::bigint, floor(EXTRACT(EPOCH FROM now())::numeric)::bigint), now()
         )
         ON CONFLICT (provider_subscription_id) DO NOTHING",
    )
    .bind(&user_id)
    .bind(&plan_id)
    .bind(&plan_version_id)
    .bind(customer_id)
    .bind(subscription_id)
    .bind(evidence.current_period_end_unix as f64)
    .bind(evidence.provider_event_created_unix)
    .execute(&mut **tx)
    .await
    .map_err(|_| BillingError::Internal)?;
    if inserted.rows_affected() != 1 {
        return Ok(None);
    }

    let completed = sqlx::query(
        "UPDATE payment_orders
         SET status = 'completed',
             provider_payment_intent_id = COALESCE($1, provider_payment_intent_id),
             provider_subscription_id = $2,
             completed_at = COALESCE(completed_at, now()),
             updated_at = now()
         WHERE id = $3::uuid AND status = 'pending' AND provider = 'stripe'
           AND provider_checkout_session_id = $4 AND expires_at > now()",
    )
    .bind(payment_intent_id)
    .bind(subscription_id)
    .bind(order_id)
    .bind(evidence.checkout_session_id)
    .execute(&mut **tx)
    .await
    .map_err(|_| BillingError::Internal)?;
    if completed.rows_affected() != 1 {
        // This should be impossible while the order row is locked, but retain
        // transaction safety if a future caller changes that invariant.
        return Err(BillingError::Conflict(
            "Payment order changed during trusted activation.".to_string(),
        ));
    }

    Ok(Some((user_id, plan_id)))
}

pub async fn verify_checkout_order(
    pool: Option<&PgPool>,
    provider: &dyn PaymentProvider,
    user_id: &str,
    request: &VerifyOrderRequest,
) -> Result<BillingStatusResponse, BillingError> {
    let order_id = request.order_id.trim();
    if order_id.len() != 36 {
        return Err(BillingError::Validation(
            "Malformed order identifier.".to_string(),
        ));
    }
    let Some(pool) = pool else {
        return Err(BillingError::DependencyUnavailable);
    };
    let row = sqlx::query(
        "SELECT status, provider, provider_checkout_session_id,
                (expires_at > now()) AS order_is_unexpired
         FROM payment_orders
         WHERE id = $1::uuid AND user_id = $2::uuid",
    )
    .bind(order_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .map_err(|_| BillingError::Internal)?
    .ok_or_else(|| BillingError::NotFound("Payment order not found.".to_string()))?;

    let status: String = row.get("status");
    let order_provider: String = row.get("provider");
    let session_id: Option<String> = row.get("provider_checkout_session_id");
    let order_is_unexpired: bool = row.get("order_is_unexpired");

    if status == "pending"
        && order_provider == "stripe"
        && order_is_unexpired
        && provider.is_configured()
    {
        if let Some(session_id) = session_id {
            let verified = provider.verify_checkout(&session_id).await?;
            let activation_evidence = match (
                verified.checkout_session_id.as_deref(),
                verified.order_id.as_deref(),
                verified.user_id.as_deref(),
                verified.client_reference_id.as_deref(),
                verified.subscription_status.as_deref(),
                verified.subscription_id.as_deref(),
                verified.amount_total_minor,
                verified.currency.as_deref(),
                verified.current_period_end_unix,
                verified.session_created_at_unix,
            ) {
                (
                    Some(checkout_session_id),
                    Some(provider_order_id),
                    Some(metadata_user_id),
                    Some(client_reference_id),
                    Some("active"),
                    Some(subscription_id),
                    Some(amount_total_minor),
                    Some(currency),
                    Some(current_period_end_unix),
                    Some(provider_event_created_unix),
                ) if verified.paid
                    && checkout_session_id == session_id
                    && provider_order_id == order_id
                    && metadata_user_id == user_id
                    && client_reference_id == user_id
                    && !subscription_id.trim().is_empty()
                    && current_period_end_unix > current_unix_seconds()
                    && provider_event_created_unix > 0
                    && provider_event_created_unix
                        <= current_unix_seconds().saturating_add(DEFAULT_WEBHOOK_TOLERANCE_SECS) =>
                {
                    Some(CheckoutActivationEvidence {
                        checkout_session_id,
                        provider_order_id,
                        metadata_user_id,
                        client_reference_id,
                        customer_id: verified.customer_id.as_deref(),
                        subscription_id,
                        payment_intent_id: verified.payment_intent_id.as_deref(),
                        amount_total_minor,
                        currency,
                        current_period_end_unix,
                        provider_event_created_unix,
                    })
                }
                _ => None,
            };

            if let Some(evidence) = activation_evidence {
                let mut tx = pool.begin().await.map_err(|_| BillingError::Internal)?;
                let activation = activate_order_in_tx(&mut tx, order_id, evidence).await?;
                tx.commit().await.map_err(|_| BillingError::Internal)?;
                if let Some((_, plan_id)) = activation {
                    audit::record_event(
                        pool,
                        Some(user_id),
                        user_id,
                        "billing_subscription_activated",
                        "ok",
                        json!({ "orderId": order_id, "planId": plan_id }),
                    )
                    .await;
                }
            }
        }
    }

    get_billing_status(Some(pool), user_id, provider.is_configured()).await
}

/// Keep lifecycle requests closed until the provider abstraction can perform
/// and verify them; configured credentials alone do not add those operations.
fn unsupported_subscription_lifecycle_error(provider: &dyn PaymentProvider) -> BillingError {
    if provider.is_configured() {
        BillingError::ProviderOperationUnsupported
    } else {
        BillingError::PaymentProviderUnconfigured
    }
}

/// Subscription cancellation must not be represented as a local database-only
/// transition. The provider abstraction currently has no verified cancellation
/// operation, so both scheduled and immediate requests fail without mutation.
pub async fn cancel_subscription(
    _pool: Option<&PgPool>,
    provider: &dyn PaymentProvider,
    _user_id: &str,
    _immediate: bool,
) -> Result<BillingStatusResponse, BillingError> {
    Err(unsupported_subscription_lifecycle_error(provider))
}

/// Client-selected plan identifiers are never provider evidence. Keep this
/// endpoint disabled until the configured provider supports a verified plan
/// transition; `free` routes through the same fail-closed cancellation path.
pub async fn change_subscription_plan(
    pool: Option<&PgPool>,
    provider: &dyn PaymentProvider,
    user_id: &str,
    request: &ChangePlanRequest,
) -> Result<BillingStatusResponse, BillingError> {
    let plan_id = request.plan_id.trim();
    if plan_id == "free" {
        return cancel_subscription(pool, provider, user_id, false).await;
    }
    let interval = request.interval.trim();
    if interval != "month" && interval != "year" {
        return Err(BillingError::Validation(
            "Billing interval must be 'month' or 'year'.".to_string(),
        ));
    }

    Err(unsupported_subscription_lifecycle_error(provider))
}

pub async fn create_portal_session(
    pool: Option<&PgPool>,
    provider: &dyn PaymentProvider,
    user_id: &str,
) -> Result<PortalResponse, BillingError> {
    let Some(pool) = pool else {
        return Err(BillingError::DependencyUnavailable);
    };
    if !provider.is_configured() {
        return Err(BillingError::PaymentProviderUnconfigured);
    }
    let customer: Option<(Option<String>,)> = sqlx::query_as(
        "SELECT provider_customer_id
         FROM subscriptions
         WHERE user_id = $1::uuid AND provider_customer_id IS NOT NULL
         ORDER BY updated_at DESC
         LIMIT 1",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .map_err(|_| BillingError::Internal)?;

    let Some((Some(customer_id),)) = customer else {
        return Err(BillingError::NotFound(
            "No billing profile exists for this account yet.".to_string(),
        ));
    };
    let portal_url = provider.create_billing_portal(&customer_id).await?;
    Ok(PortalResponse { portal_url })
}

// ---------------------------------------------------------------------------
// Signed webhook ingestion & idempotent state transitions
// ---------------------------------------------------------------------------

/// Map every known non-entitling Stripe state, and any future unknown state,
/// to an allowed inactive local state instead of leaving a stale `active` row.
fn normalize_subscription_status(status: &str) -> &'static str {
    match status {
        "active" => "active",
        "trialing" => "trialing",
        "past_due" | "unpaid" | "paused" => "past_due",
        "canceled" => "canceled",
        "incomplete" => "incomplete",
        "incomplete_expired" => "expired",
        _ => "past_due",
    }
}

pub async fn process_signed_webhook(
    pool: Option<&PgPool>,
    payload: &[u8],
    signature_header: &str,
    webhook_secret: Option<&str>,
    now_unix: i64,
) -> Result<WebhookAckResponse, BillingError> {
    let Some(secret) = webhook_secret else {
        return Err(BillingError::PaymentProviderUnconfigured);
    };
    verify_stripe_webhook_signature(
        payload,
        signature_header,
        secret,
        now_unix,
        DEFAULT_WEBHOOK_TOLERANCE_SECS,
    )?;

    let Some(pool) = pool else {
        return Err(BillingError::DependencyUnavailable);
    };

    let parsed: serde_json::Value = serde_json::from_slice(payload)
        .map_err(|_| BillingError::Validation("Invalid webhook JSON.".to_string()))?;
    let event_id = parsed
        .get("id")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| BillingError::Validation("Missing webhook event id.".to_string()))?;
    let event_type = parsed
        .get("type")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| BillingError::Validation("Missing webhook event type.".to_string()))?;
    let event_created_unix = parsed
        .get("created")
        .and_then(serde_json::Value::as_i64)
        .filter(|created| {
            *created > 0 && *created <= now_unix.saturating_add(DEFAULT_WEBHOOK_TOLERANCE_SECS)
        });

    let obj = parsed
        .pointer("/data/object")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let provider_payment_intent_id = stripe_object_id(obj.get("payment_intent"))
        .or_else(|| stripe_object_id(obj.pointer("/subscription/latest_invoice/payment_intent")));
    let provider_subscription_id = match event_type {
        "checkout.session.completed" => stripe_object_id(obj.get("subscription")),
        "customer.subscription.updated" | "customer.subscription.deleted" => {
            obj.get("id").and_then(|value| value.as_str())
        }
        "invoice.paid" | "invoice.payment_failed" => stripe_object_id(obj.get("subscription")),
        _ => None,
    };
    let is_full_refund = event_type == "charge.refunded"
        && obj.get("refunded").and_then(|value| value.as_bool()) == Some(true);
    let provider_subscription_status = match event_type {
        "checkout.session.completed" => obj.pointer("/subscription/status").and_then(|v| v.as_str()),
        "customer.subscription.deleted" => Some("canceled"),
        "customer.subscription.updated" => obj.get("status").and_then(|v| v.as_str()),
        "invoice.paid"
            if obj.get("paid").and_then(|v| v.as_bool()) == Some(true)
                && obj.get("status").and_then(|v| v.as_str()) == Some("paid") =>
        {
            Some("active")
        }
        "invoice.payment_failed"
            if obj.get("paid").and_then(|v| v.as_bool()) == Some(false) =>
        {
            Some("past_due")
        }
        "charge.refunded" if is_full_refund => Some("revoked"),
        _ => None,
    };

    let payload_digest = Sha256::digest(payload).to_vec();

    let mut tx = pool.begin().await.map_err(|_| BillingError::Internal)?;
    lock_provider_transition_keys(
        &mut tx,
        provider_subscription_id,
        provider_payment_intent_id,
    )
    .await?;
    let claim = sqlx::query(
        "INSERT INTO webhook_events
         (provider, provider_event_id, event_type, payload_sha256, provider_event_created_at,
          provider_payment_intent_id, provider_subscription_id, provider_subscription_status,
          is_full_refund, status)
         VALUES ('stripe', $1, $2, $3, $4, $5, $6, $7, $8, 'processing')
         ON CONFLICT (provider, provider_event_id) DO NOTHING",
    )
    .bind(event_id)
    .bind(event_type)
    .bind(&payload_digest)
    .bind(event_created_unix)
    .bind(provider_payment_intent_id)
    .bind(provider_subscription_id)
    .bind(provider_subscription_status)
    .bind(is_full_refund)
    .execute(&mut *tx)
    .await
    .map_err(|_| BillingError::Internal)?;

    if claim.rows_affected() == 0 {
        // A repeated provider event ID is only an idempotent replay when the
        // authenticated payload bytes match the event we already processed.
        // Do not silently acknowledge a conflicting body under a reused ID.
        let existing_digest: Option<Vec<u8>> = sqlx::query_scalar(
            "SELECT payload_sha256 FROM webhook_events
             WHERE provider = 'stripe' AND provider_event_id = $1",
        )
        .bind(event_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|_| BillingError::Internal)?;
        let same_payload = existing_digest
            .as_deref()
            .is_some_and(|existing| existing == payload_digest.as_slice());
        tx.rollback().await.map_err(|_| BillingError::Internal)?;
        if !same_payload {
            return Err(BillingError::Conflict(
                "A provider event ID was received with different signed content.".to_string(),
            ));
        }
        return Ok(WebhookAckResponse {
            received: true,
            deduplicated: true,
        });
    }

    let mut outcome_status = "processed";
    match event_type {
        "checkout.session.completed" => {
            let active_subscription = active_subscription_details(&obj);
            if !is_completed_paid_subscription_session(&obj) {
                outcome_status = "ignored";
            } else if let (
                Some(event_created),
                Some(session_id),
                Some(order_id),
                Some(metadata_user_id),
                Some(client_reference_id),
                Some(amount_total_minor),
                Some(currency),
                Some((subscription_id, period_end)),
            ) = (
                event_created_unix,
                obj.get("id").and_then(|v| v.as_str()),
                obj.pointer("/metadata/order_id").and_then(|v| v.as_str()),
                obj.pointer("/metadata/user_id").and_then(|v| v.as_str()),
                obj.get("client_reference_id").and_then(|v| v.as_str()),
                obj.get("amount_total").and_then(|v| v.as_i64()),
                obj.get("currency").and_then(|v| v.as_str()),
                active_subscription,
            ) {
                if activate_order_in_tx(
                    &mut tx,
                    order_id,
                    CheckoutActivationEvidence {
                        checkout_session_id: session_id,
                        provider_order_id: order_id,
                        metadata_user_id,
                        client_reference_id,
                        customer_id: stripe_object_id(obj.get("customer")),
                        subscription_id: &subscription_id,
                        payment_intent_id: provider_payment_intent_id,
                        amount_total_minor,
                        currency,
                        current_period_end_unix: period_end,
                        provider_event_created_unix: event_created,
                    },
                )
                .await?
                .is_none()
                {
                    outcome_status = "ignored";
                }
            } else {
                // Stripe's ordinary unexpanded checkout event does not carry a
                // trusted subscription period. It is recorded but cannot grant
                // access; the authenticated verify endpoint retrieves the full
                // subscription state before activation.
                outcome_status = "ignored";
            }
        }
        "customer.subscription.updated" | "customer.subscription.deleted" => {
            let sub_id = obj.get("id").and_then(|v| v.as_str());
            let raw_status = if event_type == "customer.subscription.deleted" {
                Some("canceled")
            } else {
                obj.get("status").and_then(|v| v.as_str())
            };
            let mut normalized_status = raw_status.map(normalize_subscription_status);
            let cancel_at_end = obj
                .get("cancel_at_period_end")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let period_end = obj.get("current_period_end").and_then(|v| v.as_i64());
            if normalized_status == Some("active")
                && !period_end.is_some_and(|end| end > current_unix_seconds())
            {
                // Do not leave a prior paid status active when the provider's
                // latest event cannot establish a current paid period.
                normalized_status = Some("past_due");
            }

            if let (Some(sub_id), Some(status), Some(event_created)) =
                (sub_id, normalized_status, event_created_unix)
            {
                let updated = sqlx::query(
                    "UPDATE subscriptions
                     SET status = $1,
                         cancel_at_period_end = $2,
                         current_period_end = COALESCE(to_timestamp($3::double precision), current_period_end),
                         ended_at = CASE WHEN $1 IN ('canceled', 'expired') THEN COALESCE(ended_at, now()) ELSE ended_at END,
                         last_provider_event_created_at = $4,
                         updated_at = now()
                     WHERE provider = 'stripe'
                       AND provider_subscription_id = $5
                       AND last_provider_event_created_at IS NOT NULL
                       AND $4 > last_provider_event_created_at
                       AND status NOT IN ('canceled', 'expired', 'revoked')",
                )
                .bind(status)
                .bind(cancel_at_end)
                .bind(period_end.map(|end| end as f64))
                .bind(event_created)
                .bind(sub_id)
                .execute(&mut *tx)
                .await
                .map_err(|_| BillingError::Internal)?;
                if updated.rows_affected() == 0 {
                    outcome_status = "ignored";
                }
            } else {
                outcome_status = "ignored";
            }
        }
        "invoice.paid" => {
            let invoice_is_paid = obj.get("paid").and_then(|v| v.as_bool()) == Some(true)
                && obj.get("status").and_then(|v| v.as_str()) == Some("paid");
            let period_end = obj
                .pointer("/lines/data/0/period/end")
                .and_then(|v| v.as_i64())
                .or_else(|| obj.get("period_end").and_then(|v| v.as_i64()));
            if let (
                true,
                Some(sub_id),
                Some(payment_intent_id),
                Some(period_end),
                Some(event_created),
            ) = (
                invoice_is_paid,
                stripe_object_id(obj.get("subscription")),
                provider_payment_intent_id,
                period_end,
                event_created_unix,
            ) {
                let previously_refunded: bool = sqlx::query_scalar(
                    "SELECT EXISTS (
                         SELECT 1 FROM webhook_events
                         WHERE provider = 'stripe'
                           AND is_full_refund = true
                           AND provider_payment_intent_id = $1
                     )",
                )
                .bind(payment_intent_id)
                .fetch_one(&mut *tx)
                .await
                .map_err(|_| BillingError::Internal)?;
                if previously_refunded {
                    outcome_status = "ignored";
                } else {
                    let updated = sqlx::query(
                        "UPDATE subscriptions
                         SET status = 'active',
                             current_period_end = to_timestamp($1::double precision),
                             last_provider_event_created_at = $2,
                             updated_at = now()
                         WHERE provider = 'stripe'
                           AND provider_subscription_id = $3
                           AND status IN ('active', 'past_due')
                           AND last_provider_event_created_at IS NOT NULL
                           AND $2 > last_provider_event_created_at
                           AND to_timestamp($1::double precision) > now()
                           AND to_timestamp($1::double precision) >= current_period_end",
                    )
                    .bind(period_end as f64)
                    .bind(event_created)
                    .bind(sub_id)
                    .execute(&mut *tx)
                    .await
                    .map_err(|_| BillingError::Internal)?;
                    if updated.rows_affected() == 0 {
                        outcome_status = "ignored";
                    }
                }
            } else {
                // A paid invoice without provider-reported period end is not
                // enough evidence to extend an entitlement; never invent 30 days.
                outcome_status = "ignored";
            }
        }
        "invoice.payment_failed" => {
            let invoice_is_unpaid = obj.get("paid").and_then(|v| v.as_bool()) == Some(false);
            if let (true, Some(sub_id), Some(event_created)) = (
                invoice_is_unpaid,
                stripe_object_id(obj.get("subscription")),
                event_created_unix,
            ) {
                let updated = sqlx::query(
                    "UPDATE subscriptions
                     SET status = 'past_due',
                         last_provider_event_created_at = $1,
                         updated_at = now()
                     WHERE provider = 'stripe'
                       AND provider_subscription_id = $2
                       AND status IN ('active', 'past_due')
                       AND last_provider_event_created_at IS NOT NULL
                       AND $1 > last_provider_event_created_at",
                )
                .bind(event_created)
                .bind(sub_id)
                .execute(&mut *tx)
                .await
                .map_err(|_| BillingError::Internal)?;
                if updated.rows_affected() == 0 {
                    outcome_status = "ignored";
                }
            } else {
                outcome_status = "ignored";
            }
        }
        "charge.refunded" => {
            let fully_refunded = obj.get("refunded").and_then(|v| v.as_bool()) == Some(true);
            if let (true, Some(payment_intent), Some(event_created)) = (
                fully_refunded,
                stripe_object_id(obj.get("payment_intent")),
                event_created_unix,
            ) {
                let refunded_orders = sqlx::query(
                    "UPDATE payment_orders
                     SET status = 'refunded', updated_at = now()
                     WHERE provider = 'stripe'
                       AND status = 'completed'
                       AND provider_payment_intent_id = $1",
                )
                .bind(payment_intent)
                .execute(&mut *tx)
                .await
                .map_err(|_| BillingError::Internal)?;

                let revoked_subscriptions = sqlx::query(
                    "UPDATE subscriptions
                     SET status = 'revoked',
                         ended_at = COALESCE(ended_at, now()),
                         last_provider_event_created_at = $2,
                         updated_at = now()
                     WHERE provider = 'stripe'
                       AND status NOT IN ('canceled', 'expired', 'revoked')
                       AND (last_provider_event_created_at IS NULL OR $2 > last_provider_event_created_at)
                       AND provider_subscription_id IN (
                           SELECT provider_subscription_id FROM payment_orders
                           WHERE provider = 'stripe'
                             AND status = 'refunded'
                             AND provider_payment_intent_id = $1
                             AND provider_subscription_id IS NOT NULL
                           UNION
                           SELECT provider_subscription_id FROM webhook_events
                           WHERE provider = 'stripe'
                             AND provider_payment_intent_id = $1
                             AND provider_subscription_id IS NOT NULL
                       )",
                )
                .bind(payment_intent)
                .bind(event_created)
                .execute(&mut *tx)
                .await
                .map_err(|_| BillingError::Internal)?;
                if refunded_orders.rows_affected() == 0 && revoked_subscriptions.rows_affected() == 0 {
                    outcome_status = "ignored";
                }
            } else {
                // Partial refunds and charges without an order-linked payment
                // intent never revoke an entire subscription.
                outcome_status = "ignored";
            }
        }
        _ => {
            outcome_status = "ignored";
        }
    }

    sqlx::query(
        "UPDATE webhook_events
         SET status = $1, processed_at = now()
         WHERE provider = 'stripe' AND provider_event_id = $2",
    )
    .bind(outcome_status)
    .bind(event_id)
    .execute(&mut *tx)
    .await
    .map_err(|_| BillingError::Internal)?;

    tx.commit().await.map_err(|_| BillingError::Internal)?;

    audit::record_event(
        pool,
        None,
        event_id,
        "billing_webhook_processed",
        outcome_status,
        json!({ "eventType": event_type }),
    )
    .await;

    Ok(WebhookAckResponse {
        received: true,
        deduplicated: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unconfigured_payment_message_does_not_invite_credentials_to_enable_billing() {
        let message = BillingError::PaymentProviderUnconfigured.message();
        assert!(message.contains("disabled"));
        assert!(message.contains("credentials do not enable checkout"));
        assert!(!message.contains("Set STRIPE_SECRET_KEY"));
    }

    #[test]
    fn provider_subscription_statuses_fail_closed_for_non_active_states() {
        for status in ["unpaid", "paused", "past_due", "unknown_future_status"] {
            assert_eq!(normalize_subscription_status(status), "past_due");
        }
        assert_eq!(normalize_subscription_status("trialing"), "trialing");
        assert_eq!(normalize_subscription_status("incomplete"), "incomplete");
        assert_eq!(normalize_subscription_status("incomplete_expired"), "expired");
        assert_eq!(normalize_subscription_status("canceled"), "canceled");
        assert_eq!(normalize_subscription_status("active"), "active");
    }

    #[test]
    fn checkout_activation_requires_completed_paid_subscription_state() {
        assert!(is_completed_paid_subscription_session(&json!({
            "mode": "subscription",
            "status": "complete",
            "payment_status": "paid",
            "subscription": "sub_test_verified"
        })));
        assert!(is_completed_paid_subscription_session(&json!({
            "mode": "subscription",
            "status": "complete",
            "payment_status": "paid",
            "subscription": { "id": "sub_test_verified" }
        })));
        let period_end = current_unix_seconds() + 60;
        assert_eq!(
            active_subscription_details(&json!({
                "subscription": {
                    "id": "sub_test_verified",
                    "status": "active",
                    "current_period_end": period_end
                }
            })),
            Some(("sub_test_verified".to_string(), period_end))
        );
        assert!(active_subscription_details(&json!({
            "subscription": {
                "id": "sub_test_trial",
                "status": "trialing",
                "current_period_end": period_end
            }
        }))
        .is_none());
        assert!(active_subscription_details(&json!({
            "subscription": { "id": "sub_test_no_period", "status": "active" }
        }))
        .is_none());
        assert!(!is_completed_paid_subscription_session(&json!({
            "mode": "subscription",
            "status": "complete",
            "payment_status": "paid"
        })));
        assert!(!is_completed_paid_subscription_session(&json!({
            "mode": "subscription",
            "status": "complete",
            "payment_status": "unpaid"
        })));
        assert!(!is_completed_paid_subscription_session(&json!({
            "mode": "subscription",
            "status": "open",
            "payment_status": "paid"
        })));
        assert!(!is_completed_paid_subscription_session(&json!({
            "mode": "payment",
            "status": "complete",
            "payment_status": "paid"
        })));
    }

    #[tokio::test]
    async fn client_plan_changes_and_cancellation_fail_closed_when_provider_is_unconfigured() {
        let provider = UnconfiguredPaymentProvider;
        for plan_id in ["pro", "free"] {
            let error = change_subscription_plan(
                None,
                &provider,
                "00000000-0000-0000-0000-000000000001",
                &ChangePlanRequest {
                    plan_id: plan_id.to_string(),
                    interval: "month".to_string(),
                },
            )
            .await
            .unwrap_err();
            assert_eq!(error, BillingError::PaymentProviderUnconfigured);
        }
        for immediate in [false, true] {
            let error = cancel_subscription(
                None,
                &provider,
                "00000000-0000-0000-0000-000000000001",
                immediate,
            )
            .await
            .unwrap_err();
            assert_eq!(error, BillingError::PaymentProviderUnconfigured);
        }
    }

    #[tokio::test]
    async fn configured_provider_without_lifecycle_methods_cannot_mutate_subscriptions() {
        // Constructing the HTTP provider performs no request; the plan/cancel
        // methods remain unsupported even when `is_configured()` is true.
        let provider = StripeHttpProvider::new(
            "fixture-only-not-a-secret".to_string(),
            "https://billing.example.test/success".to_string(),
            "https://billing.example.test/cancel".to_string(),
            "https://billing.example.test/return".to_string(),
        );
        for plan_id in ["pro", "free"] {
            let error = change_subscription_plan(
                None,
                &provider,
                "00000000-0000-0000-0000-000000000001",
                &ChangePlanRequest {
                    plan_id: plan_id.to_string(),
                    interval: "month".to_string(),
                },
            )
            .await
            .unwrap_err();
            assert_eq!(error, BillingError::ProviderOperationUnsupported);
            assert_eq!(error.status_code(), 503);
        }
        for immediate in [false, true] {
            let error = cancel_subscription(
                None,
                &provider,
                "00000000-0000-0000-0000-000000000001",
                immediate,
            )
            .await
            .unwrap_err();
            assert_eq!(error, BillingError::ProviderOperationUnsupported);
        }
    }

    #[test]
    fn hmac_sha256_matches_rfc_4231_test_vector_2() {
        let mac = hmac_sha256(b"Jefe", b"what do ya want for nothing?");
        let hex: String = mac.iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(
            hex,
            "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
        );
    }

    #[test]
    fn stripe_signature_verifies_valid_header_and_rejects_tampering_or_replay() {
        let secret = "whsec_test_secret_fixture_only";
        let payload = br#"{"id":"evt_1","type":"invoice.paid"}"#;
        let now = 1_700_000_000;
        let sig = compute_stripe_signature_header(secret, now - 10, payload);
        assert!(verify_stripe_webhook_signature(payload, &sig, secret, now, 300).is_ok());

        // Wrong secret
        assert_eq!(
            verify_stripe_webhook_signature(payload, &sig, "wrong", now, 300),
            Err(BillingError::SignatureInvalid)
        );
        // Modified payload
        assert_eq!(
            verify_stripe_webhook_signature(br#"{"id":"evt_2"}"#, &sig, secret, now, 300),
            Err(BillingError::SignatureInvalid)
        );
        // Stale timestamp outside tolerance window
        assert_eq!(
            verify_stripe_webhook_signature(payload, &sig, secret, now + 999, 300),
            Err(BillingError::SignatureInvalid)
        );
    }

    #[test]
    fn stripe_signature_timestamp_extremes_and_negative_tolerance_fail_closed() {
        let payload = b"{}";
        let secret = "whsec_test_secret_fixture_only";
        let now = 1_700_000_000;
        let zero_signature = "00".repeat(32);

        for timestamp in [i64::MIN, i64::MAX] {
            let header = format!("t={timestamp},v1={zero_signature}");
            assert_eq!(
                verify_stripe_webhook_signature(payload, &header, secret, now, 300),
                Err(BillingError::SignatureInvalid)
            );
        }

        let valid_header = compute_stripe_signature_header(secret, now, payload);
        assert_eq!(
            verify_stripe_webhook_signature(payload, &valid_header, secret, now, -1),
            Err(BillingError::SignatureInvalid)
        );
    }
}
