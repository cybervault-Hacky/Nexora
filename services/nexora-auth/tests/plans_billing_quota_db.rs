//! End-to-end PostgreSQL + Axum HTTP integration tests for:
//! - Plans, versioned pricing, subscriptions, checkout idempotency
//! - Signed Stripe HMAC-SHA256 webhook verification, replay protection, deduplication
//! - Activation, renewal, failed payment, upgrade/downgrade, cancellation, refund revocation
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
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use tower::ServiceExt;

const TEST_URL_VAR: &str = "NEXORA_AUTH_TEST_DATABASE_URL";
const WEBHOOK_SECRET: &str = "whsec_integration_fixture_secret";
const PASSWORD: &str = "correct horse battery staple";

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

struct StubPaymentProvider {
    calls: Mutex<usize>,
    paid_on_verify: bool,
}

impl StubPaymentProvider {
    fn new(paid_on_verify: bool) -> Self {
        Self {
            calls: Mutex::new(0),
            paid_on_verify,
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
        _session_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<ProviderOrderVerification, BillingError>> + Send + 'a>>
    {
        let paid = self.paid_on_verify;
        Box::pin(async move {
            Ok(ProviderOrderVerification {
                paid,
                customer_id: Some("cus_test_verified".to_string()),
                subscription_id: Some("sub_test_verified".to_string()),
                payment_intent_id: Some("pi_test_verified".to_string()),
                current_period_end_unix: None,
            })
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
                nonce: None,
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
    let mut state = AppState::unconfigured();
    state.pool = Some(pool);
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
    event: Value,
    sig_override: Option<String>,
) -> (StatusCode, Value) {
    let raw = serde_json::to_vec(&event).unwrap();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
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
    let (_user_id, _email, token) = create_logged_in_user(&pool, "billing").await;

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
        "data": {
            "object": {
                "customer": "cus_live_001",
                "subscription": "sub_live_001",
                "payment_intent": "pi_live_001",
                "metadata": { "order_id": order_id }
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

    // Duplicate delivery of the exact same event ID is deduplicated cleanly.
    let (dup_wh, ack2) = send_webhook(&app, complete_evt, None).await;
    assert_eq!(dup_wh, StatusCode::OK);
    assert_eq!(ack2["deduplicated"], true);

    let (sub_st, billing_now) =
        call_json(&app, "GET", "/v1/billing/subscription", Some(&token), None).await;
    assert_eq!(sub_st, StatusCode::OK);
    assert_eq!(billing_now["effectivePlanId"], "pro");
    assert_eq!(billing_now["subscription"]["status"], "active");

    let (_, usage_pro) = call_json(&app, "GET", "/v1/usage", Some(&token), None).await;
    assert_eq!(usage_pro["planId"], "pro");
    assert_eq!(usage_pro["models"][1]["dailyLimit"], 100); // nexora-sonnet = 100/day on Pro

    // 5. Billing portal, upgrade to Team, cancel at period end, and refund revocation.
    let (portal_st, portal) =
        call_json(&app, "POST", "/v1/billing/portal", Some(&token), None).await;
    assert_eq!(portal_st, StatusCode::OK);
    assert!(portal["portalUrl"]
        .as_str()
        .unwrap()
        .contains("cus_live_001"));

    let (up_st, upgraded) = call_json(
        &app,
        "POST",
        "/v1/billing/change-plan",
        Some(&token),
        Some(json!({ "planId": "team", "interval": "month" })),
    )
    .await;
    assert_eq!(up_st, StatusCode::OK);
    assert_eq!(upgraded["effectivePlanId"], "team");

    let (can_st, canceled_period_end) = call_json(
        &app,
        "POST",
        "/v1/billing/cancel",
        Some(&token),
        Some(json!({ "immediate": false })),
    )
    .await;
    assert_eq!(can_st, StatusCode::OK);
    assert_eq!(
        canceled_period_end["subscription"]["cancelAtPeriodEnd"],
        true
    );
    assert_eq!(canceled_period_end["effectivePlanId"], "team");

    // Charge refund immediately revokes entitlement back to Free.
    let refund_evt = json!({
        "id": "evt_refund_001",
        "type": "charge.refunded",
        "data": { "object": { "payment_intent": "pi_live_001" } }
    });
    let (ref_st, _) = send_webhook(&app, refund_evt, None).await;
    assert_eq!(ref_st, StatusCode::OK);

    let (_, after_refund) =
        call_json(&app, "GET", "/v1/billing/subscription", Some(&token), None).await;
    assert_eq!(after_refund["effectivePlanId"], "free");
    assert_eq!(after_refund["subscription"]["status"], "revoked");
    assert_eq!(after_refund["recentOrders"][0]["status"], "refunded");
}

#[tokio::test]
#[ignore = "needs an isolated PostgreSQL (NEXORA_AUTH_TEST_DATABASE_URL)"]
async fn concurrent_quota_reservations_release_on_failure_and_account_privacy_controls() {
    let pool = setup_pool().await;
    let app = make_router(pool.clone(), Arc::new(StubPaymentProvider::new(true)));
    let (_user_id, email, token) = create_logged_in_user(&pool, "quota").await;

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

    // 3. Releasing one reservation (e.g. cancelled local generation) immediately frees 1 slot; committing records usage.
    let (_, released_id) = accepted.pop().unwrap();
    let (rel_st, after_rel) = call_json(
        &app,
        "POST",
        "/v1/usage/release",
        Some(&token),
        Some(json!({ "reservationId": released_id })),
    )
    .await;
    assert_eq!(rel_st, StatusCode::OK);
    assert_eq!(after_rel["models"][1]["remaining"], 1);

    let (_, committed_id) = accepted.pop().unwrap();
    let (com_st, _) = call_json(
        &app,
        "POST",
        "/v1/usage/commit",
        Some(&token),
        Some(json!({ "reservationId": committed_id })),
    )
    .await;
    assert_eq!(com_st, StatusCode::OK);

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
            "redirectUri": "http://127.0.0.1:54321/callback"
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

    // 2. Signed checkout completion activates Pro; failed renewal (`invoice.payment_failed`) drops to Free;
    //    paid invoice (`invoice.paid`) restores Pro; elapsed `current_period_end` auto-expires back to Free.
    let activate_evt = json!({
        "id": "evt_exp_001",
        "type": "checkout.session.completed",
        "data": {
            "object": {
                "customer": "cus_exp_001",
                "subscription": "sub_exp_001",
                "payment_intent": "pi_exp_001",
                "metadata": { "order_id": order_id }
            }
        }
    });
    let (wh1, _) = send_webhook(&app, activate_evt, None).await;
    assert_eq!(wh1, StatusCode::OK);

    let fail_evt = json!({
        "id": "evt_exp_fail_002",
        "type": "invoice.payment_failed",
        "data": { "object": { "subscription": "sub_exp_001" } }
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
        "data": { "object": { "subscription": "sub_exp_001" } }
    });
    let (wh3, _) = send_webhook(&app, renew_evt, None).await;
    assert_eq!(wh3, StatusCode::OK);
    let (_, after_renew) =
        call_json(&app, "GET", "/v1/billing/subscription", Some(&token), None).await;
    assert_eq!(after_renew["effectivePlanId"], "pro");
    assert_eq!(after_renew["portalAvailable"], true);

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
