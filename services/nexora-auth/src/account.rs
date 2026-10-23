//! User profile management, safe Google identity linking, GDPR data export,
//! and explicit account deletion (Phases 4 & 8).

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{PgPool, Row};

use crate::audit;
use crate::billing;
use crate::oauth::{self, GoogleExchangeRequest, GoogleProvider, OAuthError};
use crate::password;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountProfileView {
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateProfileRequest {
    pub name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeleteAccountRequest {
    pub confirm_email: String,
    #[serde(default)]
    pub current_password: Option<String>,
}

impl std::fmt::Debug for DeleteAccountRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeleteAccountRequest")
            .field("confirm_email", &self.confirm_email)
            .field("current_password", &"<redacted>")
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccountError {
    Validation(String),
    InvalidCredentials,
    EmailConflict,
    IdentityAlreadyLinked,
    DependencyUnavailable,
    NotFound,
    Internal,
}

impl AccountError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Validation(_) => "validation_failed",
            Self::InvalidCredentials => "invalid_credentials",
            Self::EmailConflict => "email_conflict",
            Self::IdentityAlreadyLinked => "identity_already_linked",
            Self::DependencyUnavailable => "dependency_unavailable",
            Self::NotFound => "not_found",
            Self::Internal => "internal_error",
        }
    }

    pub fn status_code(&self) -> u16 {
        match self {
            Self::Validation(_) => 400,
            Self::InvalidCredentials => 401,
            Self::NotFound => 404,
            Self::EmailConflict | Self::IdentityAlreadyLinked => 409,
            Self::DependencyUnavailable => 503,
            Self::Internal => 500,
        }
    }

    pub fn message(&self) -> String {
        match self {
            Self::Validation(msg) => msg.clone(),
            Self::InvalidCredentials => {
                "Account confirmation credentials were invalid.".to_string()
            }
            Self::EmailConflict => {
                "That Google identity is already linked to a different account.".to_string()
            }
            Self::IdentityAlreadyLinked => {
                "A Google account is already linked to your profile.".to_string()
            }
            Self::DependencyUnavailable => {
                "Account service is unavailable: required dependencies are not configured."
                    .to_string()
            }
            Self::NotFound => "Account not found.".to_string(),
            Self::Internal => "An unexpected account error occurred.".to_string(),
        }
    }
}

pub async fn get_account_profile(
    pool: Option<&PgPool>,
    user_id: &str,
) -> Result<AccountProfileView, AccountError> {
    let Some(pool) = pool else {
        return Err(AccountError::DependencyUnavailable);
    };
    let row = sqlx::query(
        "SELECT id::text AS id, display_name, email,
                email_verified_at IS NOT NULL AS email_verified,
                password_hash IS NOT NULL AS has_password,
                to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at
         FROM users
         WHERE id = $1::uuid",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .map_err(|_| AccountError::Internal)?
    .ok_or(AccountError::NotFound)?;

    let has_password: bool = row.get("has_password");
    let mut providers = Vec::new();
    if has_password {
        providers.push("password".to_string());
    }
    let oauth_providers: Vec<String> = sqlx::query_scalar(
        "SELECT provider FROM oauth_accounts WHERE user_id = $1::uuid ORDER BY provider ASC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
    .map_err(|_| AccountError::Internal)?;
    providers.extend(oauth_providers);

    let (plan_id, plan_name) = billing::resolve_effective_plan(pool, user_id)
        .await
        .unwrap_or_else(|_| ("free".to_string(), "Free".to_string()));

    Ok(AccountProfileView {
        id: row.get("id"),
        name: row.get("display_name"),
        email: row.get("email"),
        email_verified: row.get("email_verified"),
        has_password,
        providers,
        plan_id,
        plan_name,
        created_at: row.get("created_at"),
    })
}

pub async fn update_account_profile(
    pool: Option<&PgPool>,
    user_id: &str,
    request: &UpdateProfileRequest,
) -> Result<AccountProfileView, AccountError> {
    let trimmed = request.name.trim();
    let count = trimmed.chars().count();
    if count == 0 || count > 80 {
        return Err(AccountError::Validation(
            "Display name must be between 1 and 80 characters.".to_string(),
        ));
    }
    if trimmed.chars().any(char::is_control) {
        return Err(AccountError::Validation(
            "Display name must not contain control characters.".to_string(),
        ));
    }
    let Some(pool) = pool else {
        return Err(AccountError::DependencyUnavailable);
    };

    sqlx::query("UPDATE users SET display_name = $1, updated_at = now() WHERE id = $2::uuid")
        .bind(trimmed)
        .bind(user_id)
        .execute(pool)
        .await
        .map_err(|_| AccountError::Internal)?;

    audit::record_event(
        pool,
        Some(user_id),
        user_id,
        "account_profile_updated",
        "ok",
        json!({}),
    )
    .await;

    get_account_profile(Some(pool), user_id).await
}

/// Safe authenticated linking of a Google OAuth identity onto the signed-in account.
pub async fn link_google_identity(
    pool: Option<&PgPool>,
    provider: &dyn GoogleProvider,
    expected_redirect_uri: &str,
    user_id: &str,
    request: &GoogleExchangeRequest,
) -> Result<AccountProfileView, AccountError> {
    oauth::validate_exchange_request(request, expected_redirect_uri).map_err(|e| match e {
        OAuthError::Validation(msg) => AccountError::Validation(msg),
        _ => AccountError::InvalidCredentials,
    })?;
    let Some(pool) = pool else {
        return Err(AccountError::DependencyUnavailable);
    };

    let claims = oauth::verify_code_with_provider(pool, provider, request)
        .await
        .map_err(|e| match e {
            OAuthError::Validation(msg) => AccountError::Validation(msg),
            OAuthError::DependencyUnavailable => AccountError::DependencyUnavailable,
            _ => AccountError::InvalidCredentials,
        })?;

    let mut tx = pool.begin().await.map_err(|_| AccountError::Internal)?;

    // Refuse if this Google subject already belongs to another user.
    if let Some((owner_id,)) = sqlx::query_as::<_, (String,)>(
        "SELECT user_id::text FROM oauth_accounts
         WHERE provider = 'google' AND provider_subject = $1
         FOR UPDATE",
    )
    .bind(claims.subject.trim())
    .fetch_optional(&mut *tx)
    .await
    .map_err(|_| AccountError::Internal)?
    {
        if owner_id != user_id {
            return Err(AccountError::EmailConflict);
        }
        tx.commit().await.map_err(|_| AccountError::Internal)?;
        return get_account_profile(Some(pool), user_id).await;
    }

    // Refuse if another Google identity is already linked to this user.
    let existing_for_user: Option<(String,)> = sqlx::query_as(
        "SELECT provider_subject FROM oauth_accounts
         WHERE user_id = $1::uuid AND provider = 'google'
         FOR UPDATE",
    )
    .bind(user_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|_| AccountError::Internal)?;
    if existing_for_user.is_some() {
        return Err(AccountError::IdentityAlreadyLinked);
    }

    sqlx::query(
        "INSERT INTO oauth_accounts (user_id, provider, provider_subject, provider_email)
         VALUES ($1::uuid, 'google', $2, $3)",
    )
    .bind(user_id)
    .bind(claims.subject.trim())
    .bind(crate::registration::normalize_email(&claims.email))
    .execute(&mut *tx)
    .await
    .map_err(|_| AccountError::EmailConflict)?;

    tx.commit().await.map_err(|_| AccountError::Internal)?;

    audit::record_event(
        pool,
        Some(user_id),
        user_id,
        "account_google_linked",
        "ok",
        json!({ "provider": "google" }),
    )
    .await;

    get_account_profile(Some(pool), user_id).await
}

/// Export all non-secret user account, identity, subscription, order, and usage records.
pub async fn export_account_data(
    pool: Option<&PgPool>,
    user_id: &str,
) -> Result<Value, AccountError> {
    let Some(pool) = pool else {
        return Err(AccountError::DependencyUnavailable);
    };
    let profile = get_account_profile(Some(pool), user_id).await?;

    let identities = sqlx::query(
        "SELECT provider, provider_email,
                to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS linked_at
         FROM oauth_accounts
         WHERE user_id = $1::uuid
         ORDER BY created_at ASC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
    .map_err(|_| AccountError::Internal)?
    .into_iter()
    .map(|r| {
        json!({
            "provider": r.get::<String, _>("provider"),
            "providerEmail": r.get::<Option<String>, _>("provider_email"),
            "linkedAt": r.get::<String, _>("linked_at"),
        })
    })
    .collect::<Vec<_>>();

    let billing_status = billing::get_billing_status(Some(pool), user_id, false)
        .await
        .map_err(|_| AccountError::Internal)?;

    let usage_rows = sqlx::query(
        "SELECT period_day::text AS period_day, model_id, committed_count
         FROM daily_usage_counters
         WHERE user_id = $1::uuid
         ORDER BY period_day DESC, model_id ASC
         LIMIT 90",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
    .map_err(|_| AccountError::Internal)?
    .into_iter()
    .map(|r| {
        json!({
            "periodDayUtc": r.get::<String, _>("period_day"),
            "modelId": r.get::<String, _>("model_id"),
            "messagesUsed": r.get::<i32, _>("committed_count"),
        })
    })
    .collect::<Vec<_>>();

    audit::record_event(
        pool,
        Some(user_id),
        user_id,
        "account_data_exported",
        "ok",
        json!({}),
    )
    .await;

    Ok(json!({
        "schemaVersion": 1,
        "exportedAtUtc": sqlx::query_scalar::<_, String>(
            "SELECT to_char(now() AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"')"
        )
        .fetch_one(pool)
        .await
        .unwrap_or_default(),
        "account": profile,
        "linkedIdentities": identities,
        "billing": {
            "effectivePlanId": billing_status.effective_plan_id,
            "subscription": billing_status.subscription,
            "recentOrders": billing_status.recent_orders,
        },
        "dailyUsageHistory": usage_rows,
        "localNote": "Conversations, prompts, and workspace files remain stored only on your local device unless exported separately from the desktop app."
    }))
}

pub async fn delete_account(
    pool: Option<&PgPool>,
    user_id: &str,
    request: &DeleteAccountRequest,
) -> Result<(), AccountError> {
    let confirm = crate::registration::normalize_email(&request.confirm_email);
    if confirm.is_empty() {
        return Err(AccountError::Validation(
            "Enter your account email address to confirm permanent deletion.".to_string(),
        ));
    }
    let Some(pool) = pool else {
        return Err(AccountError::DependencyUnavailable);
    };

    let row = sqlx::query("SELECT email, password_hash FROM users WHERE id = $1::uuid FOR UPDATE")
        .bind(user_id)
        .fetch_optional(pool)
        .await
        .map_err(|_| AccountError::Internal)?
        .ok_or(AccountError::NotFound)?;

    let email: String = row.get("email");
    let password_hash: Option<String> = row.get("password_hash");

    if confirm != email {
        return Err(AccountError::Validation(
            "Confirmation email does not match the signed-in account.".to_string(),
        ));
    }
    if let Some(hash) = password_hash {
        let supplied = request.current_password.as_deref().unwrap_or("");
        if supplied.is_empty() || password::verify_password(supplied, &hash).is_err() {
            return Err(AccountError::InvalidCredentials);
        }
    }

    audit::record_event(
        pool,
        Some(user_id),
        user_id,
        "account_deleted",
        "ok",
        json!({}),
    )
    .await;

    sqlx::query("DELETE FROM users WHERE id = $1::uuid")
        .bind(user_id)
        .execute(pool)
        .await
        .map_err(|_| AccountError::Internal)?;

    Ok(())
}
