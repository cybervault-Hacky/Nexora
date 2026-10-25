//! Binary entry point for the standalone nexora-auth service.
//!
//! The service is started manually (see docs/phase-23a/README.md). It never
//! starts with the Nexora desktop app and is not connected to the frontend.

use std::sync::Arc;

use nexora_auth::app::{build_router_with_state, AppState};
use nexora_auth::config::AuthConfig;
use nexora_auth::email::{EmailDelivery, UnconfiguredEmailProvider};

#[tokio::main]
async fn main() {
    let config = AuthConfig::from_env().unwrap_or_else(|error| {
        eprintln!("nexora-auth: configuration error: {error}");
        std::process::exit(2);
    });

    let catalog = nexora_auth::billing::PlanCatalogConfig::from_env().unwrap_or_else(|error| {
        eprintln!("nexora-auth: plan catalogue configuration error: {error}");
        std::process::exit(2);
    });

    // Build the pool lazily from the configured URL (never printed). When a
    // database is configured, verify connectivity at startup so a broken
    // deployment fails loudly instead of serving 503s indefinitely.
    let pool = match config.database_url.as_deref() {
        Some(url) => {
            let pool = sqlx::postgres::PgPoolOptions::new()
                .max_connections(5)
                .connect_lazy(url)
                .unwrap_or_else(|_| {
                    eprintln!(
                        "nexora-auth: {DATABASE_URL_LABEL} could not be used. (Value withheld.)"
                    );
                    std::process::exit(2);
                });
            if let Err(error) = sqlx::query("SELECT 1").execute(&pool).await {
                eprintln!(
                    "nexora-auth: configured database is unreachable ({}).",
                    nexora_auth::scrub::scrub(&error.to_string())
                );
                std::process::exit(1);
            }
            if let Err(error) = nexora_auth::run_migrations(&pool).await {
                eprintln!(
                    "nexora-auth: database migrations failed ({}).",
                    nexora_auth::scrub::scrub(&error.to_string())
                );
                std::process::exit(1);
            }
            if let Err(error) = nexora_auth::billing::sync_plan_catalog(&pool, &catalog).await {
                eprintln!(
                    "nexora-auth: failed to synchronize plan catalogue ({}).",
                    nexora_auth::scrub::scrub(&error.to_string())
                );
                std::process::exit(1);
            }
            Some(pool)
        }
        None => None,
    };

    // Only the honest unconfigured transport exists in this build; no mail is
    // ever sent and the API reports that capability truthfully.
    let email: Arc<dyn nexora_auth::email::EmailProvider> = Arc::new(UnconfiguredEmailProvider);
    let email_delivery = if config.email_provider_configured {
        EmailDelivery::Configured
    } else {
        EmailDelivery::Unconfigured
    };

    // Phase 23F: the Google identity provider. Unconfigured deployments keep
    // `None` and the route answers honestly; configured ones perform the real
    // server-side code exchange (plan §2.5). The client secret stays in
    // process configuration only — never logged.
    let google_provider: Option<Arc<dyn nexora_auth::oauth::GoogleProvider>> =
        match (&config.google_client_id, &config.google_redirect_uri) {
            (Some(client_id), Some(_)) => {
                let provider = match nexora_auth::oauth::HttpGoogleProvider::new(
                    client_id.clone(),
                    config.google_client_secret.clone(),
                ) {
                    Ok(provider) => provider,
                    Err(_) => {
                        eprintln!("nexora-auth: Google HTTP client could not be initialized.");
                        std::process::exit(1);
                    }
                };
                Some(Arc::new(provider))
            }
            _ => None,
        };

    // Payment integration is intentionally disabled until an approved provider
    // implementation and entitlement review exist. Environment credentials are
    // never sufficient to activate checkout, webhooks, or portal operations.
    let (payment_provider, stripe_webhook_secret) = payment_runtime_config(&config);

    let state = Arc::new(AppState {
        pool,
        email,
        email_delivery,
        verification_ttl_secs: config.verification_ttl_secs,
        session_ttl_secs: config.session_ttl_secs,
        session_idle_ttl_secs: config.session_idle_ttl_secs,
        rate_limit_login: config.rate_limit_login,
        rate_limit_register: config.rate_limit_register,
        rate_limit_resend: config.rate_limit_resend,
        reset_ttl_secs: config.reset_ttl_secs,
        rate_limit_recovery: config.rate_limit_recovery,
        google_provider,
        google_redirect_uri: config.google_redirect_uri.clone(),
        payment_provider,
        // Proposed catalogue values are not quota approval. Keep account
        // generation unconfigured until a reviewed policy is explicitly wired.
        quota_policy: nexora_auth::quota::QuotaPolicy::unconfigured(),
        stripe_webhook_secret,
    });

    let listener = tokio::net::TcpListener::bind(config.bind)
        .await
        .unwrap_or_else(|error| {
            eprintln!(
                "nexora-auth: cannot bind {}: {}",
                config.bind,
                nexora_auth::scrub::scrub(&error.to_string())
            );
            std::process::exit(1);
        });

    // Startup output prints the resolved bind address and capability flags
    // only — never environment contents or secret values.
    eprintln!(
        "{}",
        nexora_auth::scrub::scrub(&format!(
        "nexora-auth: listening on http://{} (health + registration/verification/login/sessions/recovery/oauth; \
         database: {}; email delivery: {})",
        config.bind,
        if state.pool.is_some() {
            "configured"
        } else {
            "not configured"
        },
        match email_delivery {
            EmailDelivery::Configured => "configured (provider not included in this build)",
            EmailDelivery::Unconfigured => "not configured",
        }
        ))
    );

    // ConnectInfo feeds the per-IP rate-limit keys (plan §4).
    if let Err(error) = axum::serve(
        listener,
        build_router_with_state(state)
            .into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await
    {
        eprintln!(
            "nexora-auth: server error: {}",
            nexora_auth::scrub::scrub(&error.to_string())
        );
        std::process::exit(1);
    }
}

fn payment_runtime_config(
    _config: &AuthConfig,
) -> (
    Arc<dyn nexora_auth::billing::PaymentProvider>,
    Option<String>,
) {
    (
        Arc::new(nexora_auth::billing::UnconfiguredPaymentProvider),
        None,
    )
}

const DATABASE_URL_LABEL: &str = "NEXORA_AUTH_DATABASE_URL";

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payment_credentials_never_activate_runtime_provider_or_webhooks() {
        let mut config = AuthConfig::resolve(None, None).expect("default config");
        config
            .apply_deployment_and_billing_settings(
                Some("test"),
                Some("sk_test_PHASE17_FIXTURE_ONLY"),
                Some("whsec_PHASE17_FIXTURE_ONLY"),
                Some("https://billing.example.test/success"),
                Some("https://billing.example.test/cancel"),
                Some("https://billing.example.test/return"),
            )
            .expect("synthetic test credentials are accepted only as inert config");

        let (provider, webhook_secret) = payment_runtime_config(&config);
        assert!(!provider.is_configured());
        assert!(webhook_secret.is_none());
        let rendered = format!("{config:?}");
        assert!(!rendered.contains("PHASE17_FIXTURE_ONLY"));
    }
}
