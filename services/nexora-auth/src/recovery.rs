//! Password recovery & reset (Phase 23E) — plan §6 `POST /v1/auth/recovery-request`
//! and `POST /v1/auth/reset-password`, plus the step-4 `POST /v1/auth/password-change`
//! contract.
//!
//! Security invariants:
//! - Recovery requests are **enumeration-safe**: one constant 202 for every
//!   input; a token is created (and an email queued) only for eligible
//!   accounts; the response is byte-identical either way. Delivery
//!   capability is config-level (23C clarification), never per-account.
//! - Reset tokens are 32 `OsRng` bytes (64-hex), stored **only** as sha256
//!   digests, single-use, short-lived (default 60 min — plan), revocable.
//!   Requesting a newer recovery revokes outstanding reset tokens.
//! - `reset_password` consumes the token and updates the password in ONE
//!   transaction: consume (exactly-once) → account row lock → Argon2id hash
//!   (existing 23B policy) → **revokes every session of the account (all
//!   families)** → invalidates all outstanding reset AND verification tokens.
//!   A concurrent login cannot leave a stale session: `login` verifies the
//!   password while holding the same account row lock, and `reset_password`
//!   holds that lock from before the password update until every session is
//!   revoked — so a session created from a pre-reset credential check is
//!   either revoked by the reset or created strictly after it (and thus
//!   verified against the new password).
//! - `password_change` (authenticated) keeps the caller's session family and
//!   revokes all other sessions; invalidates outstanding reset/verification
//!   tokens.
//! - Errors never leak account existence, database details, credentials, or
//!   token values. Tokens/passwords/hashes are never logged.

use sqlx::PgPool;

use crate::password;
use crate::registration::{self, validate_email};
use crate::session;

/// Hex length of a reset token (32 random bytes).
pub const RESET_TOKEN_HEX_CHARS: usize = 64;

/// Plan §7 `RESET_TTL_SECONDS` default (60 minutes).
pub const DEFAULT_RESET_TTL_SECS: u64 = 3_600;
pub const MIN_RESET_TTL_SECS: u64 = 60;
pub const MAX_RESET_TTL_SECS: u64 = 604_800;

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecoveryRequest {
    pub email: String,
}

#[derive(Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResetPasswordRequest {
    pub token: String,
    pub password: String,
}

/// Redacting `Debug` (Phase 23G): reset tokens and passwords never reach logs.
impl std::fmt::Debug for ResetPasswordRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResetPasswordRequest")
            .field("token", &crate::scrub::REDACTED)
            .field("password", &crate::scrub::REDACTED)
            .finish()
    }
}

#[derive(Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

/// Redacting `Debug` (Phase 23G): passwords never reach logs/traces.
impl std::fmt::Debug for ChangePasswordRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChangePasswordRequest")
            .field("current_password", &crate::scrub::REDACTED)
            .field("new_password", &crate::scrub::REDACTED)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecoveryError {
    /// 400 `validation_failed`.
    Validation(String),
    /// 400 `invalid_or_expired_token` — one generic failure for unknown,
    /// expired, revoked, or already-used tokens (no state distinction).
    InvalidOrExpiredToken,
    /// 401 `invalid_credentials` — wrong current password on change.
    InvalidCredentials,
    /// 503 `dependency_unavailable`.
    DependencyUnavailable,
    /// 500 `internal`.
    Internal,
}

impl RecoveryError {
    pub fn code(&self) -> &'static str {
        match self {
            RecoveryError::Validation(_) => "validation_failed",
            RecoveryError::InvalidOrExpiredToken => "invalid_or_expired_token",
            RecoveryError::InvalidCredentials => "invalid_credentials",
            RecoveryError::DependencyUnavailable => "dependency_unavailable",
            RecoveryError::Internal => "internal",
        }
    }
    pub fn message(&self) -> &str {
        match self {
            RecoveryError::Validation(message) => message,
            RecoveryError::InvalidOrExpiredToken => {
                "This reset link is invalid or has expired. Request a new one."
            }
            RecoveryError::InvalidCredentials => "Current password is incorrect.",
            RecoveryError::DependencyUnavailable => {
                "Password recovery is unavailable: the account database is not configured."
            }
            RecoveryError::Internal => "The request could not be completed.",
        }
    }
    pub fn status_code(&self) -> u16 {
        match self {
            RecoveryError::Validation(_) => 400,
            RecoveryError::InvalidOrExpiredToken => 400,
            RecoveryError::InvalidCredentials => 401,
            RecoveryError::DependencyUnavailable => 503,
            RecoveryError::Internal => 500,
        }
    }
}

/// Cryptographically secure reset token (32 `OsRng` bytes → 64 hex).
pub fn generate_reset_token() -> String {
    registration::generate_verification_token()
}

/// sha256 digest — the only representation ever stored.
pub fn hash_reset_token(raw: &str) -> Vec<u8> {
    session::hash_session_token(raw)
}

pub fn is_valid_reset_token_shape(raw: &str) -> bool {
    raw.len() == RESET_TOKEN_HEX_CHARS && raw.chars().all(|c| c.is_ascii_hexdigit())
}

/// Validate a reset request's shapes/policy WITHOUT touching the token store
/// (invalid input must never consume a token).
pub fn validate_reset_request(request: &ResetPasswordRequest) -> Result<(), RecoveryError> {
    if !is_valid_reset_token_shape(request.token.trim()) {
        return Err(RecoveryError::Validation(
            "Reset token must be 64 hexadecimal characters.".to_string(),
        ));
    }
    password::validate_new_password(&request.password).map_err(|error| {
        RecoveryError::Validation(error.message().to_string())
    })
}

/// Validate a change-password request (pure; no credential checks).
pub fn validate_change_request(request: &ChangePasswordRequest) -> Result<(), RecoveryError> {
    if request.current_password.is_empty() {
        return Err(RecoveryError::Validation(
            "Current password is required.".to_string(),
        ));
    }
    password::validate_new_password(&request.new_password).map_err(|error| {
        RecoveryError::Validation(error.message().to_string())
    })?;
    if request.current_password == request.new_password {
        return Err(RecoveryError::Validation(
            "New password must be different from the current password.".to_string(),
        ));
    }
    Ok(())
}

/// Eligibility: an account can recover when it exists and is `active`.
/// Locked/deleted accounts get no token (they still receive the identical
/// 202 — enumeration defence).
async fn eligible_user_id(pool: &PgPool, email: &str) -> Result<Option<String>, RecoveryError> {
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT id::text FROM users WHERE email = $1 AND status = 'active'",
    )
    .bind(email)
    .fetch_optional(pool)
    .await
    .map_err(|_| RecoveryError::Internal)?;
    Ok(row.map(|(id,)| id))
}

/// `POST /v1/auth/recovery-request` — ALWAYS 202 with a byte-identical body
/// (plan §6: "never confirms existence"). A reset token is created and handed
/// to the email provider only for eligible accounts. With no provider
/// configured the response honestly says no message was sent (23C rules).
pub async fn request_recovery(
    pool: Option<&PgPool>,
    email_provider: &dyn crate::email::EmailProvider,
    delivery: crate::email::EmailDelivery,
    reset_ttl_secs: u64,
    request: &RecoveryRequest,
) -> Result<crate::email::EmailDelivery, RecoveryError> {
    let email = validate_email(&request.email).map_err(RecoveryError::Validation)?;
    let pool = pool.ok_or(RecoveryError::DependencyUnavailable)?;

    if let Some(user_id) = eligible_user_id(pool, &email).await? {
        let raw = generate_reset_token();
        let digest = hash_reset_token(&raw);
        let ttl = reset_ttl_secs.max(60);

        let mut tx = pool.begin().await.map_err(|_| RecoveryError::Internal)?;
        // Revoke any outstanding tokens before minting a new one (plan:
        // invalidation on use; also ensures at most one live reset token).
        sqlx::query(
            "UPDATE password_reset_tokens \
             SET revoked_at = now() \
             WHERE user_id = CAST($1 AS uuid) AND used_at IS NULL AND revoked_at IS NULL",
        )
        .bind(&user_id)
        .execute(&mut *tx)
        .await
        .map_err(|_| RecoveryError::Internal)?;
        sqlx::query(
            "INSERT INTO password_reset_tokens (user_id, token_hash, expires_at) \
             VALUES (CAST($1 AS uuid), $2, now() + make_interval(secs => $3))",
        )
        .bind(&user_id)
        .bind(&digest)
        .bind(ttl as f64)
        .execute(&mut *tx)
        .await
        .map_err(|_| RecoveryError::Internal)?;
        tx.commit().await.map_err(|_| RecoveryError::Internal)?;

        // Outbound email is the ONLY place the raw token appears; it is never
        // stored, logged, or returned to the caller. The body carries the
        // plan's deep-link fragment (`#/reset-password?token=…`) — composing
        // an absolute origin is the future provider/deployment step; no URL
        // origin is fabricated here.
        let message = crate::email::EmailMessage {
            to: email,
            subject: "Reset your Nexora password".to_string(),
            body: format!(
                "A password reset was requested for your Nexora account.\n\n\
                 Open the Nexora password-reset screen and use this reset link \
                 within the reset window:\n\
                 #/reset-password?token={raw}\n\n\
                 If you did not request this, you can ignore this message — \
                 your password has not changed.\n\n\
                 This link can be used once and expires automatically.\n"
            ),
        };
        // Delivery failures never change the response (enumeration) and are
        // never logged (the message contains the raw token).
        let _ = email_provider.send(message).await; // capability reported via `delivery`
    }

    Ok(delivery)
}

/// `POST /v1/auth/reset-password` — consume token + password update + session
/// revocation in ONE transaction. Any token-state failure is the same
/// `invalid_or_expired_token`. Success → 204 (no body, plan §6).
pub async fn reset_password(
    pool: Option<&PgPool>,
    request: &ResetPasswordRequest,
) -> Result<(), RecoveryError> {
    // Cheap shape/policy validation first — invalid input must NOT consume
    // the token.
    validate_reset_request(request)?;
    let pool = pool.ok_or(RecoveryError::DependencyUnavailable)?;
    let digest = hash_reset_token(request.token.trim());

    let mut tx = pool.begin().await.map_err(|_| RecoveryError::Internal)?;

    // Atomic single-use consume: exactly one concurrent request flips used_at.
    let consumed: Option<(String,)> = sqlx::query_as(
        "UPDATE password_reset_tokens \
         SET used_at = now() \
         WHERE token_hash = $1 AND used_at IS NULL AND revoked_at IS NULL \
           AND expires_at > now() \
         RETURNING user_id::text",
    )
    .bind(&digest)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|_| RecoveryError::Internal)?;
    let Some((user_id,)) = consumed else {
        return Err(RecoveryError::InvalidOrExpiredToken);
    };

    // Account row lock — serializes against login/refresh session creation
    // (see module docs). Eligibility re-checked under the lock.
    let locked: Option<(String,)> = sqlx::query_as(
        "SELECT id::text FROM users WHERE id = CAST($1 AS uuid) AND status = 'active' FOR UPDATE",
    )
    .bind(&user_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|_| RecoveryError::Internal)?;
    let Some((user_id,)) = locked else {
        return Err(RecoveryError::InvalidOrExpiredToken);
    };

    // Argon2id via the existing 23B implementation (policy enforced inside).
    let new_hash =
        password::hash_password(&request.password).map_err(|_| RecoveryError::Internal)?;
    sqlx::query("UPDATE users SET password_hash = $2 WHERE id = CAST($1 AS uuid)")
        .bind(&user_id)
        .bind(&new_hash)
        .execute(&mut *tx)
        .await
        .map_err(|_| RecoveryError::Internal)?;

    // Revoke EVERY session of the account — all families, rotated siblings
    // included (plan: "completing a reset revokes all sessions").
    sqlx::query(
        "UPDATE sessions SET revoked_at = COALESCE(revoked_at, now()) \
         WHERE user_id = CAST($1 AS uuid) AND revoked_at IS NULL",
    )
    .bind(&user_id)
    .execute(&mut *tx)
    .await
    .map_err(|_| RecoveryError::Internal)?;

    // Invalidate all outstanding reset AND verification tokens (plan:
    // "invalidates all outstanding reset/verification tokens").
    sqlx::query(
        "UPDATE password_reset_tokens SET revoked_at = COALESCE(revoked_at, now()) \
         WHERE user_id = CAST($1 AS uuid) AND used_at IS NULL AND revoked_at IS NULL",
    )
    .bind(&user_id)
    .execute(&mut *tx)
    .await
    .map_err(|_| RecoveryError::Internal)?;
    sqlx::query(
        "UPDATE email_verification_tokens SET revoked_at = COALESCE(revoked_at, now()) \
         WHERE user_id = CAST($1 AS uuid) AND used_at IS NULL AND revoked_at IS NULL",
    )
    .bind(&user_id)
    .execute(&mut *tx)
    .await
    .map_err(|_| RecoveryError::Internal)?;

    tx.commit().await.map_err(|_| RecoveryError::Internal)?;
    Ok(())
}

/// `POST /v1/auth/password-change` (authenticated) — verify the current
/// password, set the new one (Argon2id), keep the caller's session family,
/// revoke all other sessions, invalidate outstanding reset/verification
/// tokens. Wrong current password → generic `invalid_credentials`.
pub async fn change_password(
    pool: Option<&PgPool>,
    context: &session::SessionContext,
    request: &ChangePasswordRequest,
) -> Result<(), RecoveryError> {
    validate_change_request(request)?;
    let pool = pool.ok_or(RecoveryError::DependencyUnavailable)?;

    let mut tx = pool.begin().await.map_err(|_| RecoveryError::Internal)?;
    let row: Option<(String, String)> = sqlx::query_as(
        "SELECT id::text, password_hash FROM users \
         WHERE id = CAST($1 AS uuid) AND status = 'active' FOR UPDATE",
    )
    .bind(&context.user.id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|_| RecoveryError::Internal)?;
    let Some((_id, password_hash)) = row else {
        return Err(RecoveryError::InvalidCredentials);
    };
    // Generic credential failure for a wrong current password (and for rows
    // without a usable hash) — never distinguishes states.
    if password_hash.is_empty()
        || password::verify_password(&request.current_password, &password_hash).is_err()
    {
        return Err(RecoveryError::InvalidCredentials);
    }

    let new_hash =
        password::hash_password(&request.new_password).map_err(|_| RecoveryError::Internal)?;
    sqlx::query("UPDATE users SET password_hash = $2 WHERE id = CAST($1 AS uuid)")
        .bind(&context.user.id)
        .bind(&new_hash)
        .execute(&mut *tx)
        .await
        .map_err(|_| RecoveryError::Internal)?;

    // "Revokes other sessions" (plan §6): everything outside the caller's
    // session family (the caller's device keeps working, refresh included).
    sqlx::query(
        "UPDATE sessions SET revoked_at = COALESCE(revoked_at, now()) \
         WHERE user_id = CAST($1 AS uuid) AND revoked_at IS NULL \
           AND family_id <> CAST($2 AS uuid)",
    )
    .bind(&context.user.id)
    .bind(&context.family_id)
    .execute(&mut *tx)
    .await
    .map_err(|_| RecoveryError::Internal)?;

    sqlx::query(
        "UPDATE password_reset_tokens SET revoked_at = COALESCE(revoked_at, now()) \
         WHERE user_id = CAST($1 AS uuid) AND used_at IS NULL AND revoked_at IS NULL",
    )
    .bind(&context.user.id)
    .execute(&mut *tx)
    .await
    .map_err(|_| RecoveryError::Internal)?;
    sqlx::query(
        "UPDATE email_verification_tokens SET revoked_at = COALESCE(revoked_at, now()) \
         WHERE user_id = CAST($1 AS uuid) AND used_at IS NULL AND revoked_at IS NULL",
    )
    .bind(&context.user.id)
    .execute(&mut *tx)
    .await
    .map_err(|_| RecoveryError::Internal)?;

    tx.commit().await.map_err(|_| RecoveryError::Internal)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reset_tokens_are_64_hex_like_other_token_families() {
        let token = generate_reset_token();
        assert_eq!(token.len(), RESET_TOKEN_HEX_CHARS);
        assert!(is_valid_reset_token_shape(&token));
        assert_ne!(token, generate_reset_token(), "high entropy");
        assert_eq!(hash_reset_token(&token), session::hash_session_token(&token));
    }

    #[test]
    fn malformed_tokens_are_rejected_by_shape() {
        for bad in [
            "",
            "short",
            "zz",
            &"g".repeat(64),
            &"a".repeat(63),
            &"a".repeat(65),
            "a".repeat(32).as_str(),
        ] {
            assert!(!is_valid_reset_token_shape(bad), "{bad:?}");
        }
    }

    #[test]
    fn token_state_errors_are_one_generic_failure() {
        let error = RecoveryError::InvalidOrExpiredToken;
        assert_eq!(error.code(), "invalid_or_expired_token");
        assert_eq!(error.status_code(), 400);
        // The message describes no specific state (unknown/expired/revoked/used).
        assert!(!error.message().to_lowercase().contains("expired token"));
    }

    #[test]
    fn error_messages_never_echo_values() {
        for error in [
            RecoveryError::InvalidOrExpiredToken,
            RecoveryError::InvalidCredentials,
            RecoveryError::DependencyUnavailable,
        ] {
            assert!(!error.message().contains('@'));
            assert!(!error.message().contains("hash"));
        }
    }

    #[test]
    fn reset_validation_enforces_token_shape_and_password_policy() {
        let bad_token = ResetPasswordRequest {
            token: "not-a-token".to_string(),
            password: "long enough password".to_string(),
        };
        assert_eq!(validate_reset_request(&bad_token).unwrap_err().code(), "validation_failed");

        let short_password = ResetPasswordRequest {
            token: "a".repeat(64),
            password: "short".to_string(),
        };
        let error = validate_reset_request(&short_password).unwrap_err();
        assert_eq!(error.code(), "validation_failed");
        assert!(error.message().contains("at least 8"), "existing policy text");

        let ok = ResetPasswordRequest {
            token: generate_reset_token(),
            password: "long enough password".to_string(),
        };
        assert!(validate_reset_request(&ok).is_ok());
    }

    #[test]
    fn change_validation_covers_empty_and_reused_passwords() {
        let empty_current = ChangePasswordRequest {
            current_password: String::new(),
            new_password: "long enough password".to_string(),
        };
        assert_eq!(
            validate_change_request(&empty_current).unwrap_err().code(),
            "validation_failed"
        );

        let reused = ChangePasswordRequest {
            current_password: "same password value".to_string(),
            new_password: "same password value".to_string(),
        };
        let error = validate_change_request(&reused).unwrap_err();
        assert!(error.message().contains("different"));

        let ok = ChangePasswordRequest {
            current_password: "old password value".to_string(),
            new_password: "new password value".to_string(),
        };
        assert!(validate_change_request(&ok).is_ok());
    }
}
