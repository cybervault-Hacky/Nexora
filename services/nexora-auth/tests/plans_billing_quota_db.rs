//! End-to-end PostgreSQL + Axum HTTP integration tests for:
//! - Plans, versioned pricing, subscriptions, checkout idempotency
//! - Signed Stripe HMAC-SHA256 webhook verification, replay protection, deduplication
//! - Trusted activation/renewal, failed payment, fail-closed client mutations,
//!   and refund revocation
//! - Backend-authoritative daily per-model quota reservations, concurrency safety,
//!   idempotent retries, release on failed local inference, and stale reservation sweep
//! - Account profile update, safe Google OAuth linking, GDPR data export, account deletion
//! - Production startup environment validation (`NEXORA_ENV=production` fail-fast)

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use nexora_auth::app::{build_router_with_state, AppState};
use nexora_auth::billing::{
    compute_stripe_signature_header, sync_plan_catalog, BillingError, PaymentProvider,
    PlanCatalogConfig, ProviderCheckoutInput, ProviderCheckoutSession, ProviderOrderVerification,
};
use nexora_auth::config::AuthConfig;
use nexora_auth::email::{EmailDelivery, EmailError, EmailMessage, EmailProvider};
use nexora_auth::login::{self, LoginRequest};
use nexora_auth::oauth::{GoogleExchangeError, GoogleIdentity, GoogleProvider};
use nexora_auth::registration::{self, RegisterRequest};
use nexora_auth::session;
use serde_json::{json, Value};
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use tower::ServiceExt;

const TEST_URL_VAR: &str = "NEXORA_AUTH_TEST_DATABASE_URL";

mod support;

const WEBHOOK_SECRET: &str = "whsec_integration_fixture_secret";
const PASSWORD: &str = "correct horse battery staple";
static TEST_WEBHOOK_EVENT_SEQUENCE: AtomicI64 = AtomicI64::new(0);

fn test_unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

fn test_future_unix(days: i64) -> i64 {
    test_unix_now() + days * 24 * 60 * 60
}

#[derive(Default)]
struct CaptureEmail {
    sent: Mutex<Vec<EmailMessage>>,
}

impl EmailProvider for CaptureEmail {
    fn send(
        &self,
        message: EmailMessage,
    ) -> Pin<Box<dyn Future<Output = Result<(), EmailError>> + Send + '_>> {
        self.sent.lock().unwrap().push(message);
        Box::pin(async { Ok(()) })
    }
}

impl CaptureEmail {
    fn last_verification_token(&self) -> String {
        let sent = self.sent.lock().unwrap();
        let body = &sent.last().expect("email sent").body;
        body.lines()
            .map(str::trim)
            .find(|l| l.len() == registration::VERIFICATION_TOKEN_HEX_CHARS)
            .expect("verification hex line")
            .to_string()
    }
}

#[derive(Clone, Default)]
struct ProviderVerificationOverrides {
    checkout_session_id: Option<String>,
    order_id: Option<String>,
    user_id: Option<String>,
    client_reference_id: Option<String>,
    subscription_status: Option<String>,
    subscription_id: Option<String>,
    session_created_at_unix: Option<i64>,
    current_period_end_unix: Option<i64>,
    amount_total_minor: Option<i64>,
    currency: Option<String>,
    omit_payment_intent_id: bool,
}

struct StubPaymentProvider {
    calls: Mutex<usize>,
    paid_on_verify: bool,
    sessions: Mutex<HashMap<String, (String, String)>>,
    verification_overrides: Mutex<ProviderVerificationOverrides>,
}

impl StubPaymentProvider {
    fn new(paid_on_verify: bool) -> Self {
        Self {
            calls: Mutex::new(0),
            paid_on_verify,
            sessions: Mutex::new(HashMap::new()),
            verification_overrides: Mutex::new(ProviderVerificationOverrides::default()),
        }
    }
}

impl PaymentProvider for StubPaymentProvider {
    fn is_configured(&self) -> bool {
        true
    }

    fn create_checkout<'a>(
        &'a self,
        input: &'a ProviderCheckoutInput,
    ) -> Pin<Box<dyn Future<Output = Result<ProviderCheckoutSession, BillingError>> + Send + 'a>>
    {
        let n = {
            let mut guard = self.calls.lock().unwrap();
            *guard += 1;
            *guard
        };
        let session_id = format!("cs_test_{n}_{}", input.order_id);
        self.sessions.lock().unwrap().insert(
            session_id.clone(),
            (input.order_id.clone(), input.user_id.clone()),
        );
        let checkout_url = format!("https://checkout.stripe.test/pay/{session_id}");
        Box::pin(async move {
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
        let paid = self.paid_on_verify;
        let identity = self.sessions.lock().unwrap().get(session_id).cloned();
        let overrides = self.verification_overrides.lock().unwrap().clone();
        Box::pin(async move {
            let now = test_unix_now();
            let (order_id, user_id) = identity.unwrap_or_default();
            let mut verification = ProviderOrderVerification {
                paid: paid && !order_id.is_empty(),
                checkout_session_id: (!order_id.is_empty()).then(|| session_id.to_string()),
                order_id: (!order_id.is_empty()).then_some(order_id.clone()),
                user_id: (!user_id.is_empty()).then_some(user_id.clone()),
                client_reference_id: (!user_id.is_empty()).then_some(user_id),
                session_created_at_unix: (!order_id.is_empty()).then_some(now),
                subscription_status: (!order_id.is_empty()).then(|| "active".to_string()),
                customer_id: (!order_id.is_empty()).then(|| "cus_test_verified".to_string()),
                subscription_id: (!order_id.is_empty()).then(|| format!("sub_test_{order_id}")),
                payment_intent_id: (!order_id.is_empty()).then(|| format!("pi_test_{order_id}")),
                amount_total_minor: (!order_id.is_empty()).then_some(1_900),
                currency: (!order_id.is_empty()).then(|| "usd".to_string()),
                current_period_end_unix: (!order_id.is_empty()).then(|| now + 60 * 24 * 60 * 60),
            };
            if let Some(value) = overrides.checkout_session_id {
                verification.checkout_session_id = Some(value);
            }
            if let Some(value) = overrides.order_id {
                verification.order_id = Some(value);
            }
            if let Some(value) = overrides.user_id {
                verification.user_id = Some(value);
            }
            if let Some(value) = overrides.client_reference_id {
                verification.client_reference_id = Some(value);
            }
            if let Some(value) = overrides.subscription_status {
                verification.subscription_status = Some(value);
            }
            if let Some(value) = overrides.subscription_id {
                verification.subscription_id = Some(value);
            }
            if let Some(value) = overrides.session_created_at_unix {
                verification.session_created_at_unix = Some(value);
            }
            if let Some(value) = overrides.current_period_end_unix {
                verification.current_period_end_unix = Some(value);
            }
            if let Some(value) = overrides.amount_total_minor {
                verification.amount_total_minor = Some(value);
            }
            if let Some(value) = overrides.currency {
                verification.currency = Some(value);
            }
            if overrides.omit_payment_intent_id {
                verification.payment_intent_id = None;
            }
            Ok(verification)
        })
    }

    fn create_billing_portal<'a>(
        &'a self,
        customer_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<String, BillingError>> + Send + 'a>> {
        let url = format!("https://billing.stripe.test/portal/{customer_id}");
        Box::pin(async move { Ok(url) })
    }
}

struct StubGoogleProvider {
    subject: String,
    email: String,
}

impl GoogleProvider for StubGoogleProvider {
    fn exchange_code<'a>(
        &'a self,
        _code: &'a str,
        _code_verifier: &'a str,
        _redirect_uri: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<GoogleIdentity, GoogleExchangeError>> + Send + 'a>>
    {
        let subject = self.subject.clone();
        let email = self.email.clone();
        Box::pin(async move {
            Ok(GoogleIdentity {
                subject,
                email,
                email_verified: true,
                name: Some("Ada Linked".to_string()),
                nonce: Some("google-test-nonce-0001".to_string()),
            })
        })
    }
}

async fn setup_pool() -> PgPool {
    let url = std::env::var(TEST_URL_VAR).expect("NEXORA_AUTH_TEST_DATABASE_URL set");
    let pool = PgPoolOptions::new()
        .max_connections(8)
        .connect(&url)
        .await
        .expect("connect test pg");
    support::assert_test_database(&pool).await;
    nexora_auth::run_migrations(&pool)
        .await
        .expect("run migrations");
    sqlx::query(
        "TRUNCATE users, login_attempts, google_oauth_states, webhook_events, audit_logs CASCADE",
    )
    .execute(&pool)
    .await
    .expect("clean tables");
    sync_plan_catalog(&pool, &PlanCatalogConfig::default())
        .await
        .expect("sync default catalog");
    pool
}

fn make_router(pool: PgPool, payment_provider: Arc<dyn PaymentProvider>) -> axum::Router {
    make_router_with_quota_policy(
        pool,
        payment_provider,
        nexora_auth::quota::QuotaPolicy::configured(),
    )
}

fn make_router_with_quota_policy(
    pool: PgPool,
    payment_provider: Arc<dyn PaymentProvider>,
    quota_policy: nexora_auth::quota::QuotaPolicy,
) -> axum::Router {
    let mut state = AppState::unconfigured();
    state.pool = Some(pool);
    // Tests opt in explicitly using isolated fixture values; this is not
    // production quota or entitlement approval.
    state.quota_policy = quota_policy;
    state.payment_provider = payment_provider;
    state.stripe_webhook_secret = Some(WEBHOOK_SECRET.to_string());
    state.google_provider = Some(Arc::new(StubGoogleProvider {
        subject: "google-sub-9991".to_string(),
        email: "ada.google@example.test".to_string(),
    }));
    state.google_redirect_uri = Some("http://127.0.0.1/callback".to_string());
    build_router_with_state(Arc::new(state))
}

async fn create_logged_in_user(pool: &PgPool, tag: &str) -> (String, String, String) {
    let mailer = CaptureEmail::default();
    let email = format!(
        "{tag}.{}@example.test",
        &registration::generate_verification_token()[..12]
    );
    registration::register(
        Some(pool),
        &mailer,
        EmailDelivery::Configured,
        3600,
        &RegisterRequest {
            name: "Ada Lovelace".to_string(),
            email: email.clone(),
            password: PASSWORD.to_string(),
        },
    )
    .await
    .unwrap();
    let verify_tok = mailer.last_verification_token();
    registration::verify_email(Some(pool), &verify_tok)
        .await
        .unwrap();
    let logged = login::login(
        Some(pool),
        session::DEFAULT_SESSION_TTL_SECS,
        &LoginRequest {
            email: email.clone(),
            password: PASSWORD.to_string(),
        },
        None,
        None,
    )
    .await
    .unwrap();
    (logged.user_id, email, logged.session.raw_token)
}

#[tokio::test]
#[ignore = "needs an isolated PostgreSQL (NEXORA_AUTH_TEST_DATABASE_URL)"]
async fn unconfigured_policy_hides_catalogue_and_returns_no_account_quota_claims() {
    let pool = setup_pool().await;
    let app = make_router_with_quota_policy(
        pool.clone(),
        Arc::new(StubPaymentProvider::new(false)),
        nexora_auth::quota::QuotaPolicy::unconfigured(),
    );
    let (user_id, _email, token) = create_logged_in_user(&pool, "unconfigured-quota").await;
    assert!(!user_id.is_empty());

    let (plans_status, plans) = call_json(&app, "GET", "/v1/plans", None, None).await;
    assert_eq!(plans_status, StatusCode::OK);
    assert!(plans["plans"].as_array().unwrap().is_empty());

    let (summary_status, summary) = call_json(&app, "GET", "/v1/usage", Some(&token), None).await;
    assert_eq!(summary_status, StatusCode::OK);
    assert_eq!(summary["quotaConfigured"], false);
    assert!(summary["planId"].is_null());
    assert!(summary["planName"].is_null());
    assert!(summary["resetAt"].is_null());
    assert!(summary["models"].as_array().unwrap().is_empty());

    let (reserve_status, reserve) = call_json(
        &app,
        "POST",
        "/v1/usage/reserve",
        Some(&token),
        Some(json!({ "modelId": "nexora-mini", "requestId": "unconfigured-usage-01" })),
    )
    .await;
    assert_eq!(reserve_status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(reserve["error"]["code"], "quota_policy_unconfigured");
}

async fn call_json(
    app: &axum::Router,
    method: &str,
    uri: &str,
    bearer: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut req = Request::builder().method(method).uri(uri);
    if let Some(tok) = bearer {
        req = req.header("authorization", format!("Bearer {tok}"));
    }
    let raw = match body {
        Some(v) => {
            req = req.header("content-type", "application/json");
            Body::from(v.to_string())
        }
        None => Body::empty(),
    };
    let resp = app.clone().oneshot(req.body(raw).unwrap()).await.unwrap();
    let status = resp.status();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    if bytes.is_empty() {
        (status, Value::Null)
    } else {
        (status, serde_json::from_slice(&bytes).unwrap())
    }
}

async fn send_webhook(
    app: &axum::Router,
    mut event: Value,
    sig_override: Option<String>,
) -> (StatusCode, Value) {
    let now = test_unix_now();
    if let Some(object) = event.as_object_mut() {
        object.entry("created").or_insert_with(|| {
            json!(now.saturating_add(
                TEST_WEBHOOK_EVENT_SEQUENCE.fetch_add(1, Ordering::SeqCst)
            ))
        });
    }
    let raw = serde_json::to_vec(&event).unwrap();
    let sig =
        sig_override.unwrap_or_else(|| compute_stripe_signature_header(WEBHOOK_SECRET, now, &raw));
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/billing/webhooks/stripe")
                .header("stripe-signature", sig)
                .header("content-type", "application/json")
                .body(Body::from(raw))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = resp.status();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap())
}

#[test]
fn production_startup_validation_refuses_missing_or_placeholder_secrets() {
    let mut missing_db =
        AuthConfig::resolve_with_database(Some("127.0.0.1:8787"), None, None).unwrap();
    let err = missing_db
        .apply_deployment_and_billing_settings(Some("production"), None, None, None, None, None)
        .unwrap_err();
    assert!(err.message().contains("NEXORA_AUTH_DATABASE_URL"));

    let mut placeholder_stripe = AuthConfig::resolve_with_database(
        Some("127.0.0.1:8787"),
        None,
        Some("postgres://prod_user:strong_prod_secret@127.0.0.1:5432/nexora"),
    )
    .unwrap();
    let err = placeholder_stripe
        .apply_deployment_and_billing_settings(
            Some("production"),
            Some("sk_live_CHANGE_ME_SECRET"),
            Some("whsec_real_enough_value"),
            None,
            None,
            None,
        )
        .unwrap_err();
    assert!(err.message().contains("STRIPE_SECRET_KEY"));
    assert!(!err.message().contains("CHANGE_ME_SECRET"));

    let mut test_key_in_prod = AuthConfig::resolve_with_database(
        Some("127.0.0.1:8787"),
        None,
        Some("postgres://prod_user:strong_prod_secret@127.0.0.1:5432/nexora"),
    )
    .unwrap();
    let err = test_key_in_prod
        .apply_deployment_and_billing_settings(
            Some("production"),
            Some("sk_test_9999999999"),
            Some("whsec_9999999999"),
            None,
            None,
            None,
        )
        .unwrap_err();
    assert!(err.message().contains("test key prefix"));
    assert!(!err.message().contains("9999999999"));
}

#[tokio::test]
#[ignore = "needs an isolated PostgreSQL (NEXORA_AUTH_TEST_DATABASE_URL)"]
async fn full_billing_webhook_subscription_and_portal_lifecycle() {
    let pool = setup_pool().await;
    let app = make_router(pool.clone(), Arc::new(StubPaymentProvider::new(false)));
    let (user_id, _email, token) = create_logged_in_user(&pool, "billing").await;

    // 1. Catalogue lists Free, Pro, Team with server prices and model quotas.
    let (status, cat) = call_json(&app, "GET", "/v1/plans", None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(cat["paymentConfigured"], true);
    assert_eq!(cat["plans"].as_array().unwrap().len(), 3);

    // 2. Checkout ignores any client price field (strict deny_unknown_fields) and is idempotent.
    let (bad_status, _) = call_json(
        &app,
        "POST",
        "/v1/billing/checkout",
        Some(&token),
        Some(json!({ "planId": "pro", "interval": "month", "idempotencyKey": "idem-0001", "amount": 1 })),
    )
    .await;
    assert_eq!(bad_status, StatusCode::BAD_REQUEST);

    let (st1, co1) = call_json(
        &app,
        "POST",
        "/v1/billing/checkout",
        Some(&token),
        Some(json!({ "planId": "pro", "interval": "month", "idempotencyKey": "idem-pro-0001" })),
    )
    .await;
    assert_eq!(st1, StatusCode::OK);
    let order_id = co1["orderId"].as_str().unwrap().to_string();

    let (st2, co2) = call_json(
        &app,
        "POST",
        "/v1/billing/checkout",
        Some(&token),
        Some(json!({ "planId": "pro", "interval": "month", "idempotencyKey": "idem-pro-0001" })),
    )
    .await;
    assert_eq!(st2, StatusCode::OK);
    assert_eq!(co1["orderId"], co2["orderId"]);
    assert_eq!(co1["checkoutUrl"], co2["checkoutUrl"]);

    // 3. Invalid webhook signature is rejected before touching DB state.
    let complete_evt = json!({
        "id": "evt_checkout_001",
        "type": "checkout.session.completed",
        // Keep the signed event body byte-identical across duplicate deliveries.
        "created": test_unix_now(),
        "data": {
            "object": {
                "id": co1["providerSessionId"],
                "client_reference_id": user_id,
                "mode": "subscription",
                "status": "complete",
                "payment_status": "paid",
                "amount_total": 1900,
                "currency": "usd",
                "customer": "cus_live_001",
                "subscription": {
                    "id": "sub_live_001",
                    "status": "active",
                    "current_period_end": test_future_unix(30)
                },
                "payment_intent": "pi_live_001",
                "metadata": { "order_id": order_id, "user_id": user_id }
            }
        }
    });
    let (bad_wh, _) =
        send_webhook(&app, complete_evt.clone(), Some("t=100,v1=deadbeef".into())).await;
    assert_eq!(bad_wh, StatusCode::BAD_REQUEST);

    // 4. Valid signed webhook activates Pro subscription & unlocks Pro model quotas.
    let (ok_wh, ack1) = send_webhook(&app, complete_evt.clone(), None).await;
    assert_eq!(ok_wh, StatusCode::OK);
    assert_eq!(ack1["deduplicated"], false);

    // Duplicate delivery of the exact same event ID and payload is idempotent.
    let (dup_wh, ack2) = send_webhook(&app, complete_evt.clone(), None).await;
    assert_eq!(dup_wh, StatusCode::OK);
    assert_eq!(ack2["deduplicated"], true);

    // A validly signed but conflicting payload under an already-seen event ID
    // is not silently acknowledged as a normal duplicate.
    let mut conflicting_evt = complete_evt;
    conflicting_evt["data"]["object"]["amount_total"] = json!(1);
    let (conflict_status, conflict_body) = send_webhook(&app, conflicting_evt, None).await;
    assert_eq!(conflict_status, StatusCode::CONFLICT);
    assert_eq!(conflict_body["error"]["code"], "billing_conflict");

    let (sub_st, billing_now) =
        call_json(&app, "GET", "/v1/billing/subscription", Some(&token), None).await;
    assert_eq!(sub_st, StatusCode::OK);
    assert_eq!(billing_now["effectivePlanId"], "pro");
    assert_eq!(billing_now["subscription"]["status"], "active");

    let (_, usage_pro) = call_json(&app, "GET", "/v1/usage", Some(&token), None).await;
    assert_eq!(usage_pro["planId"], "pro");
    assert_eq!(usage_pro["models"][1]["dailyLimit"], 100); // nexora-sonnet = 100/day on Pro

    // 5. The customer portal remains provider-backed, but this provider trait
    // has no verified plan-change or cancellation operation (G-01 regression).
    let (portal_st, portal) =
        call_json(&app, "POST", "/v1/billing/portal", Some(&token), None).await;
    assert_eq!(portal_st, StatusCode::OK);
    assert!(portal["portalUrl"]
        .as_str()
        .unwrap()
        .contains("cus_live_001"));

    for body in [
        json!({ "planId": "team", "interval": "month" }),
        json!({ "planId": "free", "interval": "month" }),
    ] {
        let (status, error) = call_json(
            &app,
            "POST",
            "/v1/billing/change-plan",
            Some(&token),
            Some(body),
        )
        .await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            error["error"]["code"],
            "payment_provider_operation_unsupported"
        );
    }

    for immediate in [false, true] {
        let (status, error) = call_json(
            &app,
            "POST",
            "/v1/billing/cancel",
            Some(&token),
            Some(json!({ "immediate": immediate })),
        )
        .await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            error["error"]["code"],
            "payment_provider_operation_unsupported"
        );
    }

    for (uri, body) in [
        (
            "/v1/billing/change-plan",
            json!({ "planId": "team", "interval": "month", "status": "active" }),
        ),
        (
            "/v1/billing/change-plan",
            json!({ "planId": "team", "interval": "month", "status": "trialing", "paymentStatus": "paid" }),
        ),
        (
            "/v1/billing/cancel",
            json!({ "immediate": true, "status": "active" }),
        ),
        (
            "/v1/billing/cancel",
            json!({ "immediate": true, "paymentStatus": "paid" }),
        ),
    ] {
        let (status, _) = call_json(&app, "POST", uri, Some(&token), Some(body)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    let (_, unchanged) =
        call_json(&app, "GET", "/v1/billing/subscription", Some(&token), None).await;
    assert_eq!(unchanged["effectivePlanId"], "pro");
    assert_eq!(unchanged["subscription"]["status"], "active");
    assert_eq!(unchanged["subscription"]["cancelAtPeriodEnd"], false);

    // Charge refund immediately revokes entitlement back to Free.
    let refund_evt = json!({
        "id": "evt_refund_001",
        "type": "charge.refunded",
        "data": { "object": { "payment_intent": "pi_live_001", "refunded": true } }
    });
    let (ref_st, _) = send_webhook(&app, refund_evt, None).await;
    assert_eq!(ref_st, StatusCode::OK);

    let (_, after_refund) =
        call_json(&app, "GET", "/v1/billing/subscription", Some(&token), None).await;
    assert_eq!(after_refund["effectivePlanId"], "free");
    assert_eq!(after_refund["subscription"]["status"], "revoked");
    assert_eq!(after_refund["recentOrders"][0]["status"], "refunded");

    // A late/replayed completion for a refunded order is authenticated but no
    // longer in a valid activation state, so it must not restore Pro.
    let late_complete_evt = json!({
        "id": "evt_checkout_late_after_refund_002",
        "type": "checkout.session.completed",
        "data": {
            "object": {
                "id": co1["providerSessionId"],
                "client_reference_id": user_id,
                "mode": "subscription",
                "status": "complete",
                "payment_status": "paid",
                "amount_total": 1900,
                "currency": "usd",
                "customer": "cus_live_001",
                "subscription": {
                    "id": "sub_live_001",
                    "status": "active",
                    "current_period_end": test_future_unix(30)
                },
                "payment_intent": "pi_live_001",
                "metadata": { "order_id": order_id, "user_id": user_id }
            }
        }
    });
    let (late_st, late_ack) = send_webhook(&app, late_complete_evt, None).await;
    assert_eq!(late_st, StatusCode::OK);
    assert_eq!(late_ack["deduplicated"], false);
    let (_, after_late_completion) =
        call_json(&app, "GET", "/v1/billing/subscription", Some(&token), None).await;
    assert_eq!(after_late_completion["effectivePlanId"], "free");
    assert_eq!(after_late_completion["subscription"]["status"], "revoked");
    assert_eq!(after_late_completion["recentOrders"][0]["status"], "refunded");

    // Even a later signed paid invoice cannot reactivate a refund-revoked
    // subscription; terminal lifecycle states are one-way.
    let late_paid_evt = json!({
        "id": "evt_paid_after_refund_003",
        "type": "invoice.paid",
        "data": {
            "object": {
                "subscription": "sub_live_001",
                "status": "paid",
                "paid": true,
                "period_end": test_future_unix(60)
            }
        }
    });
    let (late_paid_status, _) = send_webhook(&app, late_paid_evt, None).await;
    assert_eq!(late_paid_status, StatusCode::OK);
    let (_, after_late_paid) =
        call_json(&app, "GET", "/v1/billing/subscription", Some(&token), None).await;
    assert_eq!(after_late_paid["effectivePlanId"], "free");
    assert_eq!(after_late_paid["subscription"]["status"], "revoked");
}

#[tokio::test]
#[ignore = "needs an isolated PostgreSQL (NEXORA_AUTH_TEST_DATABASE_URL)"]
async fn provider_verified_checkout_activates_pending_order_but_client_plan_change_is_rejected() {
    let pool = setup_pool().await;
    let app = make_router(pool.clone(), Arc::new(StubPaymentProvider::new(true)));
    let (_, _, token) = create_logged_in_user(&pool, "verified-checkout").await;

    let (checkout_status, checkout) = call_json(
        &app,
        "POST",
        "/v1/billing/checkout",
        Some(&token),
        Some(json!({
            "planId": "pro",
            "interval": "month",
            "idempotencyKey": "idem-verified-checkout-01"
        })),
    )
    .await;
    assert_eq!(checkout_status, StatusCode::OK);

    let (verify_status, verified) = call_json(
        &app,
        "POST",
        "/v1/billing/verify",
        Some(&token),
        Some(json!({ "orderId": checkout["orderId"] })),
    )
    .await;
    assert_eq!(verify_status, StatusCode::OK);
    assert_eq!(verified["effectivePlanId"], "pro");
    assert_eq!(verified["subscription"]["status"], "active");

    let (change_status, error) = call_json(
        &app,
        "POST",
        "/v1/billing/change-plan",
        Some(&token),
        Some(json!({ "planId": "team", "interval": "month" })),
    )
    .await;
    assert_eq!(change_status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        error["error"]["code"],
        "payment_provider_operation_unsupported"
    );
    let (_, unchanged) =
        call_json(&app, "GET", "/v1/billing/subscription", Some(&token), None).await;
    assert_eq!(unchanged["effectivePlanId"], "pro");
    assert_eq!(unchanged["subscription"]["status"], "active");
}

#[tokio::test]
#[ignore = "needs an isolated PostgreSQL (NEXORA_AUTH_TEST_DATABASE_URL)"]
async fn checkout_activation_rejects_untrusted_identity_provider_and_lifecycle_state() {
    let pool = setup_pool().await;
    let provider = Arc::new(StubPaymentProvider::new(true));
    let app = make_router(pool.clone(), provider.clone());
    let (owner_id, _owner_email, owner_token) = create_logged_in_user(&pool, "billing-owner").await;
    let (other_id, _other_email, other_token) = create_logged_in_user(&pool, "billing-other").await;

    // Unauthenticated attempts cannot invoke billing mutations.
    for (uri, body) in [
        (
            "/v1/billing/change-plan",
            json!({ "planId": "team", "interval": "month", "status": "active" }),
        ),
        (
            "/v1/billing/cancel",
            json!({ "immediate": true, "status": "active" }),
        ),
    ] {
        let (status, _) = call_json(&app, "POST", uri, None, Some(body)).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    // The client cannot submit a paid status, trial status, or payment claim
    // through any checkout/verify/plan/cancel request shape.
    let (forged_checkout_status, _) = call_json(
        &app,
        "POST",
        "/v1/billing/checkout",
        Some(&owner_token),
        Some(json!({
            "planId": "pro",
            "interval": "month",
            "idempotencyKey": "idem-forged-status-001",
            "status": "active",
            "paymentStatus": "paid"
        })),
    )
    .await;
    assert_eq!(forged_checkout_status, StatusCode::BAD_REQUEST);

    let (owner_checkout_status, owner_checkout) = call_json(
        &app,
        "POST",
        "/v1/billing/checkout",
        Some(&owner_token),
        Some(json!({
            "planId": "pro",
            "interval": "month",
            "idempotencyKey": "idem-identity-owner-001"
        })),
    )
    .await;
    assert_eq!(owner_checkout_status, StatusCode::OK);
    let owner_order_id = owner_checkout["orderId"].as_str().unwrap().to_string();
    let owner_session_id = owner_checkout["providerSessionId"].as_str().unwrap().to_string();
    let (forged_verify_status, _) = call_json(
        &app,
        "POST",
        "/v1/billing/verify",
        Some(&owner_token),
        Some(json!({
            "orderId": owner_order_id,
            "status": "active",
            "paymentStatus": "paid"
        })),
    )
    .await;
    assert_eq!(forged_verify_status, StatusCode::BAD_REQUEST);

    let (other_checkout_status, other_checkout) = call_json(
        &app,
        "POST",
        "/v1/billing/checkout",
        Some(&other_token),
        Some(json!({
            "planId": "pro",
            "interval": "month",
            "idempotencyKey": "idem-identity-other-001"
        })),
    )
    .await;
    assert_eq!(other_checkout_status, StatusCode::OK);
    let other_order_id = other_checkout["orderId"].as_str().unwrap().to_string();

    let (terminal_checkout_status, terminal_checkout) = call_json(
        &app,
        "POST",
        "/v1/billing/checkout",
        Some(&owner_token),
        Some(json!({
            "planId": "pro",
            "interval": "month",
            "idempotencyKey": "idem-terminal-event-001"
        })),
    )
    .await;
    assert_eq!(terminal_checkout_status, StatusCode::OK);
    let terminal_order_id = terminal_checkout["orderId"].as_str().unwrap().to_string();
    let terminal_subscription_id = format!("sub_test_{terminal_order_id}");

    // A signed subscription-deleted event may arrive before the checkout
    // completion/verification path. Its terminal provider identity must be
    // retained so delayed paid evidence cannot recreate that subscription.
    let early_deleted = json!({
        "id": "evt_subscription_deleted_before_activation_001",
        "type": "customer.subscription.deleted",
        "data": { "object": { "id": terminal_subscription_id } }
    });
    let (early_deleted_status, _) = send_webhook(&app, early_deleted, None).await;
    assert_eq!(early_deleted_status, StatusCode::OK);
    let (terminal_verify_status, terminal_verify) = call_json(
        &app,
        "POST",
        "/v1/billing/verify",
        Some(&owner_token),
        Some(json!({ "orderId": terminal_order_id })),
    )
    .await;
    assert_eq!(terminal_verify_status, StatusCode::OK);
    assert_eq!(terminal_verify["effectivePlanId"], "free");
    let terminal_order_status: String = sqlx::query_scalar(
        "SELECT status FROM payment_orders WHERE id = $1::uuid",
    )
    .bind(&terminal_order_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(terminal_order_status, "pending");
    let owner_subscriptions_before_activation: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM subscriptions WHERE user_id = $1::uuid",
    )
    .bind(&owner_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(owner_subscriptions_before_activation, 0);

    // Orders are owner-scoped even when another authenticated account knows
    // the order UUID. The failed lookup leaves the pending order untouched.
    let (wrong_owner_status, _) = call_json(
        &app,
        "POST",
        "/v1/billing/verify",
        Some(&other_token),
        Some(json!({ "orderId": owner_order_id })),
    )
    .await;
    assert_eq!(wrong_owner_status, StatusCode::NOT_FOUND);

    let invalid_evidence_cases = vec![
        ProviderVerificationOverrides {
            checkout_session_id: Some("cs_test_wrong_session".to_string()),
            ..ProviderVerificationOverrides::default()
        },
        ProviderVerificationOverrides {
            order_id: Some(other_order_id.clone()),
            ..ProviderVerificationOverrides::default()
        },
        ProviderVerificationOverrides {
            user_id: Some(other_id.clone()),
            ..ProviderVerificationOverrides::default()
        },
        ProviderVerificationOverrides {
            client_reference_id: Some(other_id.clone()),
            ..ProviderVerificationOverrides::default()
        },
        ProviderVerificationOverrides {
            subscription_status: Some("trialing".to_string()),
            ..ProviderVerificationOverrides::default()
        },
        ProviderVerificationOverrides {
            session_created_at_unix: Some(0),
            ..ProviderVerificationOverrides::default()
        },
        ProviderVerificationOverrides {
            current_period_end_unix: Some(test_unix_now() - 60),
            ..ProviderVerificationOverrides::default()
        },
        ProviderVerificationOverrides {
            amount_total_minor: Some(1),
            ..ProviderVerificationOverrides::default()
        },
        ProviderVerificationOverrides {
            currency: Some("eur".to_string()),
            ..ProviderVerificationOverrides::default()
        },
        ProviderVerificationOverrides {
            omit_payment_intent_id: true,
            ..ProviderVerificationOverrides::default()
        },
    ];
    for overrides in invalid_evidence_cases {
        *provider.verification_overrides.lock().unwrap() = overrides;
        let (verify_status, status) = call_json(
            &app,
            "POST",
            "/v1/billing/verify",
            Some(&owner_token),
            Some(json!({ "orderId": owner_order_id })),
        )
        .await;
        assert_eq!(verify_status, StatusCode::OK);
        assert_eq!(status["effectivePlanId"], "free");

        let (order_status, stored_session_id): (String, Option<String>) = sqlx::query_as(
            "SELECT status, provider_checkout_session_id FROM payment_orders WHERE id = $1::uuid",
        )
        .bind(&owner_order_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(order_status, "pending");
        assert_eq!(stored_session_id.as_deref(), Some(owner_session_id.as_str()));
        let subscriptions: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM subscriptions WHERE user_id = $1::uuid",
        )
        .bind(&owner_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(subscriptions, 0);
    }

    // A full refund can arrive before the checkout/verification event. Its
    // signed payment-intent evidence is retained so a later successful-looking
    // verification cannot activate the refunded pending order.
    *provider.verification_overrides.lock().unwrap() =
        ProviderVerificationOverrides::default();
    let refunded_payment_intent = format!("pi_test_{other_order_id}");
    let early_refund = json!({
        "id": "evt_refund_before_activation_001",
        "type": "charge.refunded",
        "data": {
            "object": {
                "payment_intent": refunded_payment_intent,
                "refunded": true
            }
        }
    });
    let (early_refund_status, _) = send_webhook(&app, early_refund, None).await;
    assert_eq!(early_refund_status, StatusCode::OK);
    let (refunded_verify_status, refunded_verify) = call_json(
        &app,
        "POST",
        "/v1/billing/verify",
        Some(&other_token),
        Some(json!({ "orderId": other_order_id })),
    )
    .await;
    assert_eq!(refunded_verify_status, StatusCode::OK);
    assert_eq!(refunded_verify["effectivePlanId"], "free");
    let refunded_order_status: String = sqlx::query_scalar(
        "SELECT status FROM payment_orders WHERE id = $1::uuid",
    )
    .bind(&other_order_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(refunded_order_status, "pending");
    let other_subscriptions: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM subscriptions WHERE user_id = $1::uuid",
    )
    .bind(&other_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(other_subscriptions, 0);

    // A checkout row naming a different provider is not verified by Stripe
    // and cannot transition, even with an otherwise valid provider response.
    *provider.verification_overrides.lock().unwrap() =
        ProviderVerificationOverrides::default();
    sqlx::query("UPDATE payment_orders SET provider = 'wrong_provider' WHERE id = $1::uuid")
        .bind(&owner_order_id)
        .execute(&pool)
        .await
        .unwrap();
    let (wrong_provider_status, wrong_provider_body) = call_json(
        &app,
        "POST",
        "/v1/billing/verify",
        Some(&owner_token),
        Some(json!({ "orderId": owner_order_id })),
    )
    .await;
    assert_eq!(wrong_provider_status, StatusCode::OK);
    assert_eq!(wrong_provider_body["effectivePlanId"], "free");
    let stored_status: String = sqlx::query_scalar(
        "SELECT status FROM payment_orders WHERE id = $1::uuid",
    )
    .bind(&owner_order_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(stored_status, "pending");

    // A trusted provider response for the exact pending owner/session/order,
    // with a paid active subscription and provider period end, is the sole
    // positive path in this fixture.
    sqlx::query("UPDATE payment_orders SET provider = 'stripe' WHERE id = $1::uuid")
        .bind(&owner_order_id)
        .execute(&pool)
        .await
        .unwrap();
    let mismatched_amount_event = json!({
        "id": "evt_checkout_wrong_amount_001",
        "type": "checkout.session.completed",
        "data": {
            "object": {
                "id": owner_session_id,
                "client_reference_id": owner_id.clone(),
                "mode": "subscription",
                "status": "complete",
                "payment_status": "paid",
                "amount_total": 1,
                "currency": "usd",
                "payment_intent": format!("pi_test_{owner_order_id}"),
                "subscription": {
                    "id": format!("sub_test_{owner_order_id}"),
                    "status": "active",
                    "current_period_end": test_future_unix(30)
                },
                "metadata": {
                    "order_id": owner_order_id,
                    "user_id": owner_id.clone()
                }
            }
        }
    });
    let (wrong_amount_webhook_status, _) = send_webhook(&app, mismatched_amount_event, None).await;
    assert_eq!(wrong_amount_webhook_status, StatusCode::OK);
    let wrong_amount_order_status: String = sqlx::query_scalar(
        "SELECT status FROM payment_orders WHERE id = $1::uuid",
    )
    .bind(&owner_order_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(wrong_amount_order_status, "pending");
    let subscriptions_after_wrong_amount: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM subscriptions WHERE user_id = $1::uuid",
    )
    .bind(&owner_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(subscriptions_after_wrong_amount, 0);

    let (trusted_status, trusted_body) = call_json(
        &app,
        "POST",
        "/v1/billing/verify",
        Some(&owner_token),
        Some(json!({ "orderId": owner_order_id })),
    )
    .await;
    assert_eq!(trusted_status, StatusCode::OK);
    assert_eq!(trusted_body["effectivePlanId"], "pro");
    assert_eq!(trusted_body["subscription"]["status"], "active");

    let (order_status, linked_subscription_id): (String, Option<String>) = sqlx::query_as(
        "SELECT status, provider_subscription_id FROM payment_orders WHERE id = $1::uuid",
    )
    .bind(&owner_order_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(order_status, "completed");
    let linked_subscription_id = linked_subscription_id.expect("trusted transition links subscription");
    let stored_subscription: (String, String, Option<i64>) = sqlx::query_as(
        "SELECT user_id::text, status, last_provider_event_created_at
         FROM subscriptions WHERE provider_subscription_id = $1",
    )
    .bind(&linked_subscription_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(stored_subscription.0, owner_id);
    assert_eq!(stored_subscription.1, "active");
    assert!(stored_subscription.2.is_some());

    let (collision_checkout_status, collision_checkout) = call_json(
        &app,
        "POST",
        "/v1/billing/checkout",
        Some(&other_token),
        Some(json!({
            "planId": "pro",
            "interval": "month",
            "idempotencyKey": "idem-subscription-collision-001"
        })),
    )
    .await;
    assert_eq!(collision_checkout_status, StatusCode::OK);
    let collision_order_id = collision_checkout["orderId"].as_str().unwrap().to_string();
    *provider.verification_overrides.lock().unwrap() = ProviderVerificationOverrides {
        subscription_id: Some(linked_subscription_id.clone()),
        ..ProviderVerificationOverrides::default()
    };
    let (collision_verify_status, collision_verify) = call_json(
        &app,
        "POST",
        "/v1/billing/verify",
        Some(&other_token),
        Some(json!({ "orderId": collision_order_id })),
    )
    .await;
    assert_eq!(collision_verify_status, StatusCode::OK);
    assert_eq!(collision_verify["effectivePlanId"], "free");
    let collision_order_status: String = sqlx::query_scalar(
        "SELECT status FROM payment_orders WHERE id = $1::uuid",
    )
    .bind(&collision_order_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(collision_order_status, "pending");
    let original_subscription: (String, String, String) = sqlx::query_as(
        "SELECT user_id::text, status, plan_id FROM subscriptions WHERE provider_subscription_id = $1",
    )
    .bind(&linked_subscription_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(original_subscription, (owner_id.clone(), "active".to_string(), "pro".to_string()));
    let other_subscription_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM subscriptions WHERE user_id = $1::uuid",
    )
    .bind(&other_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(other_subscription_count, 0);

    *provider.verification_overrides.lock().unwrap() =
        ProviderVerificationOverrides::default();
    let other_order_status: String = sqlx::query_scalar(
        "SELECT status FROM payment_orders WHERE id = $1::uuid",
    )
    .bind(other_checkout["orderId"].as_str().unwrap())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(other_order_status, "pending");
}

#[tokio::test]
#[ignore = "needs an isolated PostgreSQL (NEXORA_AUTH_TEST_DATABASE_URL)"]
async fn concurrent_quota_reservations_release_on_failure_and_account_privacy_controls() {
    let pool = setup_pool().await;
    let app = make_router(pool.clone(), Arc::new(StubPaymentProvider::new(true)));
    let (user_id, email, token) = create_logged_in_user(&pool, "quota").await;

    // 1. Concurrent burst of 10 reservations against Free plan's 5/day Sonnet cap -> exactly 5 succeed, 5 return 429!
    let mut handles = Vec::new();
    for i in 0..10 {
        let app_clone = app.clone();
        let tok_clone = token.clone();
        handles.push(tokio::spawn(async move {
            let req_id = format!("req-burst-{i:04}");
            let (st, body) = call_json(
                &app_clone,
                "POST",
                "/v1/usage/reserve",
                Some(&tok_clone),
                Some(json!({
                    "modelId": "nexora-sonnet",
                    "requestId": req_id,
                })),
            )
            .await;
            (req_id, st, body)
        }));
    }
    let mut accepted: Vec<(String, String)> = Vec::new();
    let mut rejected = 0usize;
    for h in handles {
        let (req_id, st, body) = h.await.unwrap();
        if st == StatusCode::OK {
            accepted.push((req_id, body["reservationId"].as_str().unwrap().to_string()));
        } else {
            assert_eq!(st, StatusCode::TOO_MANY_REQUESTS);
            assert_eq!(body["error"]["code"], "quota_exceeded");
            assert_eq!(body["quota"]["remaining"], 0);
            rejected += 1;
        }
    }
    assert_eq!(accepted.len(), 5);
    assert_eq!(rejected, 5);

    // 2. Retrying an already-accepted requestId is idempotent even when remaining == 0!
    let (winner_req_id, winner_res_id) = &accepted[0];
    let (retry_st, retry_body) = call_json(
        &app,
        "POST",
        "/v1/usage/reserve",
        Some(&token),
        Some(json!({ "modelId": "nexora-sonnet", "requestId": winner_req_id })),
    )
    .await;
    assert_eq!(retry_st, StatusCode::OK);
    assert_eq!(
        retry_body["reservationId"].as_str().unwrap(),
        winner_res_id.as_str()
    );

    // 3. Releasing one reservation (e.g. cancelled local generation) frees a slot;
    // the same outcome is idempotent and the opposite outcome conflicts.
    let (_, released_id) = accepted.pop().unwrap();
    let (rel_st, after_rel) = call_json(
        &app,
        "POST",
        "/v1/usage/release",
        Some(&token),
        Some(json!({ "reservationId": released_id.clone() })),
    )
    .await;
    assert_eq!(rel_st, StatusCode::OK);
    assert_eq!(after_rel["quotaConfigured"], true);
    assert_eq!(after_rel["resetTimezone"], "UTC");
    assert_eq!(after_rel["models"][1]["remaining"], 1);
    assert!(after_rel["periodDayUtc"].as_str().unwrap().len() == 10);
    assert!(after_rel["resetsAtUtc"].as_str().unwrap().ends_with("T00:00:00Z"));

    let (repeat_release_st, _) = call_json(
        &app,
        "POST",
        "/v1/usage/release",
        Some(&token),
        Some(json!({ "reservationId": released_id.clone() })),
    )
    .await;
    assert_eq!(repeat_release_st, StatusCode::OK);
    let (release_as_commit_st, release_as_commit) = call_json(
        &app,
        "POST",
        "/v1/usage/commit",
        Some(&token),
        Some(json!({ "reservationId": released_id })),
    )
    .await;
    assert_eq!(release_as_commit_st, StatusCode::CONFLICT);
    assert_eq!(release_as_commit["error"]["code"], "reservation_conflict");

    // Commit increments the counter once, repeats idempotently, and cannot be
    // replayed through the reservation endpoint as a fresh generation.
    let (committed_req_id, committed_id) = accepted.pop().unwrap();
    let (com_st, _) = call_json(
        &app,
        "POST",
        "/v1/usage/commit",
        Some(&token),
        Some(json!({ "reservationId": committed_id.clone() })),
    )
    .await;
    assert_eq!(com_st, StatusCode::OK);
    let (repeat_commit_st, _) = call_json(
        &app,
        "POST",
        "/v1/usage/commit",
        Some(&token),
        Some(json!({ "reservationId": committed_id.clone() })),
    )
    .await;
    assert_eq!(repeat_commit_st, StatusCode::OK);
    let (commit_as_release_st, commit_as_release) = call_json(
        &app,
        "POST",
        "/v1/usage/release",
        Some(&token),
        Some(json!({ "reservationId": committed_id })),
    )
    .await;
    assert_eq!(commit_as_release_st, StatusCode::CONFLICT);
    assert_eq!(commit_as_release["error"]["code"], "reservation_conflict");
    let (replay_settled_st, replay_settled) = call_json(
        &app,
        "POST",
        "/v1/usage/reserve",
        Some(&token),
        Some(json!({ "modelId": "nexora-sonnet", "requestId": committed_req_id })),
    )
    .await;
    assert_eq!(replay_settled_st, StatusCode::CONFLICT);
    assert_eq!(replay_settled["error"]["code"], "reservation_conflict");

    // An expired reservation is swept before settlement: it cannot commit,
    // its slot is released once, and repeated release remains idempotent.
    let (stale_st, stale) = call_json(
        &app,
        "POST",
        "/v1/usage/reserve",
        Some(&token),
        Some(json!({ "modelId": "nexora-mini", "requestId": "req-expire-0001" })),
    )
    .await;
    assert_eq!(stale_st, StatusCode::OK);
    let stale_id = stale["reservationId"].as_str().unwrap().to_string();
    sqlx::query(
        "UPDATE quota_reservations
         SET expires_at = now() - interval '1 second'
         WHERE id = $1::uuid",
    )
    .bind(&stale_id)
    .execute(&pool)
    .await
    .expect("expire isolated fixture reservation");

    let (stale_commit_st, stale_commit) = call_json(
        &app,
        "POST",
        "/v1/usage/commit",
        Some(&token),
        Some(json!({ "reservationId": stale_id.clone() })),
    )
    .await;
    assert_eq!(stale_commit_st, StatusCode::CONFLICT);
    assert_eq!(stale_commit["error"]["code"], "reservation_expired");
    let (stale_release_st, _) = call_json(
        &app,
        "POST",
        "/v1/usage/release",
        Some(&token),
        Some(json!({ "reservationId": stale_id.clone() })),
    )
    .await;
    assert_eq!(stale_release_st, StatusCode::OK);
    let (stale_replay_st, stale_replay) = call_json(
        &app,
        "POST",
        "/v1/usage/reserve",
        Some(&token),
        Some(json!({ "modelId": "nexora-mini", "requestId": "req-expire-0001" })),
    )
    .await;
    assert_eq!(stale_replay_st, StatusCode::CONFLICT);
    assert_eq!(stale_replay["error"]["code"], "reservation_expired");
    let (mini_reserved, mini_committed): (i32, i32) = sqlx::query_as(
        "SELECT reserved_count, committed_count FROM daily_usage_counters
         WHERE user_id = $1::uuid
           AND period_day = (now() AT TIME ZONE 'UTC')::date
           AND model_id = 'nexora-mini'",
    )
    .bind(&user_id)
    .fetch_optional(&pool)
    .await
    .expect("query isolated mini counter")
    .unwrap_or((0, 0));
    // The stale reservation must be expired and its reserved count released.
    assert_eq!(mini_reserved, 0);
    assert_eq!(mini_committed, 0);

    // 4. Profile update + safe Google linking (dynamic loopback port!) + GDPR export + account deletion.
    let (prof_st, prof) = call_json(
        &app,
        "PATCH",
        "/v1/account/profile",
        Some(&token),
        Some(json!({ "name": "Ada Byron" })),
    )
    .await;
    assert_eq!(prof_st, StatusCode::OK);
    assert_eq!(prof["profile"]["name"], "Ada Byron");

    let (link_st, linked) = call_json(
        &app,
        "POST",
        "/v1/auth/google/link",
        Some(&token),
        Some(json!({
            "code": "valid-oauth-authz-code",
            "codeVerifier": "a_valid_rfc7636_pkce_verifier_with_more_than_43_chars",
            "state": "1111111111111111111111111111111111111111111111111111111111111111",
            "redirectUri": "http://127.0.0.1:54321/callback",
            "nonce": "google-test-nonce-0001"
        })),
    )
    .await;
    assert_eq!(link_st, StatusCode::OK);
    assert_eq!(
        linked["profile"]["providers"],
        json!(["password", "google"])
    );

    let (exp_st, exported) = call_json(&app, "GET", "/v1/account/export", Some(&token), None).await;
    assert_eq!(exp_st, StatusCode::OK);
    assert_eq!(exported["account"]["name"], "Ada Byron");
    assert!(!exported.to_string().contains("$argon2id$"));

    // Deletion with wrong password fails; deletion with valid password wipes account & sessions cleanly.
    let (bad_del, _) = call_json(
        &app,
        "POST",
        "/v1/account/delete",
        Some(&token),
        Some(json!({ "confirmEmail": email, "currentPassword": "wrong-password" })),
    )
    .await;
    assert_eq!(bad_del, StatusCode::UNAUTHORIZED);

    let (ok_del, _) = call_json(
        &app,
        "POST",
        "/v1/account/delete",
        Some(&token),
        Some(json!({ "confirmEmail": email, "currentPassword": PASSWORD })),
    )
    .await;
    assert_eq!(ok_del, StatusCode::NO_CONTENT);

    // Session is dead immediately after account deletion.
    let (after_del_me, _) = call_json(&app, "GET", "/v1/auth/me", Some(&token), None).await;
    assert_eq!(after_del_me, StatusCode::UNAUTHORIZED);
}

#[test]
fn dot_env_example_is_complete_placeholder_only_and_fails_fast_in_production() {
    let raw = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../.env.example"))
        .expect("read repo root .env.example");

    let mut pairs = std::collections::BTreeMap::new();
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let (k, v) = trimmed
            .split_once('=')
            .unwrap_or_else(|| panic!("malformed .env.example line: {trimmed}"));
        assert!(
            !k.starts_with("VITE_"),
            ".env.example must never expose variables with VITE_ prefix: {k}"
        );
        pairs.insert(k.trim().to_string(), v.trim().to_string());
    }

    for required in [
        "NEXORA_ENV",
        "NEXORA_AUTH_BIND",
        "NEXORA_AUTH_ALLOW_NON_LOOPBACK",
        "NEXORA_AUTH_DATABASE_URL",
        "NEXORA_AUTH_TEST_DATABASE_URL",
        "NEXORA_AUTH_VERIFICATION_TTL_SECONDS",
        "NEXORA_AUTH_SESSION_TTL_SECONDS",
        "NEXORA_AUTH_SESSION_IDLE_TTL_SECONDS",
        "NEXORA_AUTH_RESET_TTL_SECONDS",
        "NEXORA_AUTH_RATE_LIMIT_LOGIN",
        "NEXORA_AUTH_RATE_LIMIT_REGISTER",
        "NEXORA_AUTH_RATE_LIMIT_RESEND",
        "NEXORA_AUTH_RATE_LIMIT_RECOVERY",
        "NEXORA_AUTH_EMAIL_PROVIDER",
        "GOOGLE_OAUTH_CLIENT_ID",
        "NEXORA_AUTH_GOOGLE_CLIENT_ID",
        "NEXORA_AUTH_GOOGLE_CLIENT_SECRET",
        "NEXORA_AUTH_GOOGLE_REDIRECT_URI",
        "STRIPE_SECRET_KEY",
        "STRIPE_WEBHOOK_SECRET",
        "STRIPE_SUCCESS_URL",
        "STRIPE_CANCEL_URL",
        "STRIPE_PORTAL_RETURN_URL",
        "NEXORA_BILLING_CURRENCY",
        "NEXORA_FREE_MINI_DAILY_LIMIT",
        "NEXORA_FREE_SONNET_DAILY_LIMIT",
        "NEXORA_PRO_MONTHLY_PRICE_CENTS",
        "NEXORA_PRO_YEARLY_PRICE_CENTS",
        "NEXORA_PRO_MINI_DAILY_LIMIT",
        "NEXORA_PRO_SONNET_DAILY_LIMIT",
        "NEXORA_TEAM_MONTHLY_PRICE_CENTS",
        "NEXORA_TEAM_YEARLY_PRICE_CENTS",
        "NEXORA_TEAM_MINI_DAILY_LIMIT",
        "NEXORA_TEAM_SONNET_DAILY_LIMIT",
        "NEXORA_AUTH_API_URL",
    ] {
        assert!(
            pairs.contains_key(required),
            "missing required documented variable in .env.example: {required}"
        );
    }

    // Development resolution using `.env.example` values succeeds cleanly.
    let mut dev_cfg = AuthConfig::resolve_with_database(
        pairs.get("NEXORA_AUTH_BIND").map(String::as_str),
        pairs
            .get("NEXORA_AUTH_ALLOW_NON_LOOPBACK")
            .map(String::as_str),
        pairs.get("NEXORA_AUTH_DATABASE_URL").map(String::as_str),
    )
    .expect("development bind + db url parse");
    dev_cfg
        .apply_deployment_and_billing_settings(
            pairs.get("NEXORA_ENV").map(String::as_str),
            pairs.get("STRIPE_SECRET_KEY").map(String::as_str),
            pairs.get("STRIPE_WEBHOOK_SECRET").map(String::as_str),
            pairs.get("STRIPE_SUCCESS_URL").map(String::as_str),
            pairs.get("STRIPE_CANCEL_URL").map(String::as_str),
            pairs.get("STRIPE_PORTAL_RETURN_URL").map(String::as_str),
        )
        .expect("development mode accepts non-production placeholders");

    // Flipping `NEXORA_ENV=production` without replacing `.env.example` placeholders fails fast
    // and never echoes the raw secret string.
    let prod_err = dev_cfg
        .apply_deployment_and_billing_settings(
            Some("production"),
            pairs.get("STRIPE_SECRET_KEY").map(String::as_str),
            pairs.get("STRIPE_WEBHOOK_SECRET").map(String::as_str),
            pairs.get("STRIPE_SUCCESS_URL").map(String::as_str),
            pairs.get("STRIPE_CANCEL_URL").map(String::as_str),
            pairs.get("STRIPE_PORTAL_RETURN_URL").map(String::as_str),
        )
        .expect_err("production mode must refuse unedited .env.example placeholders");
    assert!(prod_err.message().contains("Production startup refused"));
    assert!(!prod_err
        .message()
        .contains("REPLACE_WITH_STRONG_DB_PASSWORD"));
}

#[tokio::test]
#[ignore = "needs an isolated PostgreSQL (NEXORA_AUTH_TEST_DATABASE_URL)"]
async fn unpaid_verification_expired_and_failed_renewals_and_desktop_profile_ids() {
    let pool = setup_pool().await;
    let app = make_router(pool.clone(), Arc::new(StubPaymentProvider::new(false)));
    let (user_id, _email, token) = create_logged_in_user(&pool, "expiry").await;

    // 1. Frontend alone cannot activate Pro: creating checkout + calling verify while unpaid keeps Free.
    let (co_st, co) = call_json(
        &app,
        "POST",
        "/v1/billing/checkout",
        Some(&token),
        Some(json!({ "planId": "pro", "interval": "month", "idempotencyKey": "idem-unpaid-0001" })),
    )
    .await;
    assert_eq!(co_st, StatusCode::OK);
    let order_id = co["orderId"].as_str().unwrap().to_string();

    let (ver_st, ver_unpaid) = call_json(
        &app,
        "POST",
        "/v1/billing/verify",
        Some(&token),
        Some(json!({ "orderId": order_id })),
    )
    .await;
    assert_eq!(ver_st, StatusCode::OK);
    assert_eq!(ver_unpaid["effectivePlanId"], "free");
    assert_eq!(ver_unpaid["portalAvailable"], false);

    let unpaid_complete_evt = json!({
        "id": "evt_exp_checkout_unpaid_000",
        "type": "checkout.session.completed",
        "data": {
            "object": {
                "mode": "subscription",
                "status": "complete",
                "payment_status": "unpaid",
                "customer": "cus_exp_001",
                "subscription": "sub_exp_001",
                "payment_intent": "pi_exp_001",
                "metadata": { "order_id": order_id }
            }
        }
    });
    let (unpaid_wh, _) = send_webhook(&app, unpaid_complete_evt, None).await;
    assert_eq!(unpaid_wh, StatusCode::OK);
    let (_, after_unpaid_webhook) =
        call_json(&app, "GET", "/v1/billing/subscription", Some(&token), None).await;
    assert_eq!(after_unpaid_webhook["effectivePlanId"], "free");
    assert_eq!(after_unpaid_webhook["recentOrders"][0]["status"], "pending");

    // 2. A signed paid completion activates Pro. Failed renewal
    // (`invoice.payment_failed`) drops to Free; paid invoice (`invoice.paid`)
    // restores Pro; elapsed `current_period_end` auto-expires back to Free.
    let activate_evt = json!({
        "id": "evt_exp_001",
        "type": "checkout.session.completed",
        "data": {
            "object": {
                "id": co["providerSessionId"],
                "client_reference_id": user_id,
                "mode": "subscription",
                "status": "complete",
                "payment_status": "paid",
                "amount_total": 1900,
                "currency": "usd",
                "customer": "cus_exp_001",
                "subscription": {
                    "id": "sub_exp_001",
                    "status": "active",
                    "current_period_end": test_future_unix(30)
                },
                "payment_intent": "pi_exp_001",
                "metadata": { "order_id": order_id, "user_id": user_id }
            }
        }
    });
    let (wh1, _) = send_webhook(&app, activate_evt, None).await;
    assert_eq!(wh1, StatusCode::OK);

    // A signed provider transition to `unpaid` must remove a previously active
    // entitlement even though Stripe's raw state is not a DB enum value.
    let subscription_unpaid_evt = json!({
        "id": "evt_exp_subscription_unpaid_002",
        "type": "customer.subscription.updated",
        "data": {
            "object": {
                "id": "sub_exp_001",
                "status": "unpaid",
                "current_period_end": test_future_unix(30)
            }
        }
    });
    let (unpaid_transition_status, _) = send_webhook(&app, subscription_unpaid_evt, None).await;
    assert_eq!(unpaid_transition_status, StatusCode::OK);
    let (_, after_unpaid_transition) =
        call_json(&app, "GET", "/v1/billing/subscription", Some(&token), None).await;
    assert_eq!(after_unpaid_transition["subscription"]["status"], "past_due");
    assert_eq!(after_unpaid_transition["effectivePlanId"], "free");

    let fail_evt = json!({
        "id": "evt_exp_fail_002",
        "type": "invoice.payment_failed",
        "data": { "object": { "subscription": "sub_exp_001", "status": "open", "paid": false } }
    });
    let (wh2, _) = send_webhook(&app, fail_evt, None).await;
    assert_eq!(wh2, StatusCode::OK);

    let (_, after_fail) =
        call_json(&app, "GET", "/v1/billing/subscription", Some(&token), None).await;
    assert_eq!(after_fail["subscription"]["status"], "past_due");
    assert_eq!(after_fail["effectivePlanId"], "free");

    let renew_evt = json!({
        "id": "evt_exp_paid_003",
        "type": "invoice.paid",
        "data": {
            "object": {
                "subscription": "sub_exp_001",
                "payment_intent": "pi_exp_renewal_001",
                "status": "paid",
                "paid": true,
                "lines": { "data": [{ "period": { "end": test_future_unix(60) } }] }
            }
        }
    });
    let (wh3, _) = send_webhook(&app, renew_evt, None).await;
    assert_eq!(wh3, StatusCode::OK);
    let (_, after_renew) =
        call_json(&app, "GET", "/v1/billing/subscription", Some(&token), None).await;
    assert_eq!(after_renew["effectivePlanId"], "pro");
    assert_eq!(after_renew["portalAvailable"], true);

    // A correctly signed but older payment-failed event arriving after the
    // successful renewal cannot roll the newer active state back to past_due.
    let stale_fail_evt = json!({
        "id": "evt_exp_stale_fail_004",
        "created": test_unix_now() - 60,
        "type": "invoice.payment_failed",
        "data": { "object": { "subscription": "sub_exp_001", "status": "open", "paid": false } }
    });
    let (stale_fail_status, _) = send_webhook(&app, stale_fail_evt, None).await;
    assert_eq!(stale_fail_status, StatusCode::OK);
    let (_, after_stale_fail) =
        call_json(&app, "GET", "/v1/billing/subscription", Some(&token), None).await;
    assert_eq!(after_stale_fail["subscription"]["status"], "active");
    assert_eq!(after_stale_fail["effectivePlanId"], "pro");

    // Even a validly signed payload cannot supply an event timestamp far in
    // the future to bypass lifecycle ordering or extend an entitlement.
    let future_fail_evt = json!({
        "id": "evt_exp_future_fail_005",
        "created": test_unix_now() + 3_600,
        "type": "invoice.payment_failed",
        "data": { "object": { "subscription": "sub_exp_001", "status": "open", "paid": false } }
    });
    let (future_fail_status, _) = send_webhook(&app, future_fail_evt, None).await;
    assert_eq!(future_fail_status, StatusCode::OK);
    let (_, after_future_fail) =
        call_json(&app, "GET", "/v1/billing/subscription", Some(&token), None).await;
    assert_eq!(after_future_fail["subscription"]["status"], "active");
    assert_eq!(after_future_fail["effectivePlanId"], "pro");

    // Backdate billing period (`current_period_start` and `current_period_end`) into the past
    // to verify automatic time-based expiration enforcement (`CHECK (current_period_end >= current_period_start)`).
    sqlx::query(
        "UPDATE subscriptions
         SET current_period_start = now() - interval '31 days',
             current_period_end = now() - interval '2 minutes'
         WHERE user_id = $1::uuid",
    )
    .bind(&user_id)
    .execute(&pool)
    .await
    .unwrap();

    let (_, after_expiry) =
        call_json(&app, "GET", "/v1/billing/subscription", Some(&token), None).await;
    assert_eq!(after_expiry["subscription"]["status"], "expired");
    assert_eq!(after_expiry["effectivePlanId"], "free");

    // 3. Full desktop registry model IDs enforce model-specific limits & accurate 00:00 UTC message.
    for i in 0..5 {
        let (st, res) = call_json(
            &app,
            "POST",
            "/v1/usage/reserve",
            Some(&token),
            Some(json!({
                "modelId": "nexora-sonnet-qwen2.5-coder-14b-instruct-q4-k-m",
                "requestId": format!("desk-sonnet-{i:02}")
            })),
        )
        .await;
        assert_eq!(st, StatusCode::OK);
        assert_eq!(res["modelId"], "nexora-sonnet");
    }
    let (sixth_st, sixth_body) = call_json(
        &app,
        "POST",
        "/v1/usage/reserve",
        Some(&token),
        Some(json!({
            "modelId": "nexora-sonnet-qwen2.5-coder-14b-instruct-q4-k-m",
            "requestId": "desk-sonnet-06"
        })),
    )
    .await;
    assert_eq!(sixth_st, StatusCode::TOO_MANY_REQUESTS);
    assert!(sixth_body["error"]["message"]
        .as_str()
        .unwrap()
        .contains("00:00 UTC"));

    // Meanwhile Nexora Mini still has its separate 25/day allowance open on the exact same account!
    let (mini_st, mini_body) = call_json(
        &app,
        "POST",
        "/v1/usage/reserve",
        Some(&token),
        Some(json!({
            "modelId": "nexora-mini-qwen2.5-coder-7b-instruct-q4-k-m",
            "requestId": "desk-mini-01"
        })),
    )
    .await;
    assert_eq!(mini_st, StatusCode::OK);
    assert_eq!(mini_body["modelId"], "nexora-mini");
    assert_eq!(mini_body["remainingAfter"], 24);
}
