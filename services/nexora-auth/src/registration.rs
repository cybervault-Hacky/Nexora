//! Registration, email verification, and resend (Phase 23C).
//!
//! Scope follows the plan's sequence step 2 (docs/phase-23/README.md §9):
//! `POST /v1/auth/register`, `POST /v1/auth/verify-email`,
//! `POST /v1/auth/resend-verification`. Login/sessions, recovery/reset, and
//! OAuth are later steps and are NOT implemented here.
//!
//! Security invariants enforced here:
//! - Plaintext passwords are never stored or logged; only Argon2id PHC strings
//!   (via [`crate::password`], policy 8–128 chars).
//! - Verification tokens are 32 random bytes, hex-encoded for the email; the
//!   database stores **only** sha256(raw) (plan §4/§5), single-use
//!   (`used_at`), expiring, revocable (`revoked_at` on resend).
//! - Registration is enumeration-safe: the response is identical whether or
//!   not the email already existed (plan §6: 202 `verification_pending`).
//! - All SQL is parameterized (`sqlx` bind arguments).
//! - Errors never echo passwords, tokens, hashes, connection strings, or
//!   provider credentials.

use sha2::{Digest, Sha256};
use sqlx::postgres::PgDatabaseError;
use sqlx::PgPool;

use crate::email::{EmailDelivery, EmailMessage, EmailProvider};
use crate::password;

/// Plan §7: `VERIFICATION_TTL_SECONDS` default 86400 (24 h).
pub const DEFAULT_VERIFICATION_TTL_SECS: u64 = 86_400;
pub const MIN_VERIFICATION_TTL_SECS: u64 = 300;
pub const MAX_VERIFICATION_TTL_SECS: u64 = 604_800;

pub const MAX_NAME_CHARS: usize = 128;
pub const MAX_EMAIL_CHARS: usize = 254;
/// 32 random bytes → 64 hex chars.
pub const VERIFICATION_TOKEN_HEX_CHARS: usize = 64;

// ---------------------------------------------------------------------------
// Request schema (documented; `deny_unknown_fields` keeps bodies strict)
// ---------------------------------------------------------------------------

#[derive(Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RegisterRequest {
    pub name: String,
    pub email: String,
    pub password: String,
}

/// Redacting `Debug` (Phase 23G): passwords must never reach logs/traces.
impl std::fmt::Debug for RegisterRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RegisterRequest")
            .field("name", &self.name)
            .field("email", &self.email)
            .field("password", &crate::scrub::REDACTED)
            .finish()
    }
}

#[derive(Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VerifyEmailRequest {
    pub token: String,
}

/// Redacting `Debug` (Phase 23G): verification tokens must never reach logs.
impl std::fmt::Debug for VerifyEmailRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VerifyEmailRequest")
            .field("token", &crate::scrub::REDACTED)
            .finish()
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResendVerificationRequest {
    pub email: String,
}

// ---------------------------------------------------------------------------
// Outcomes / errors (generic messages; no secret material)
// ---------------------------------------------------------------------------

/// Enumeration-safe registration outcome. `email_delivery` reports transport
/// capability (config-level, identical for every account) — it never reveals
/// whether this particular address was new.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterOutcome {
    pub email_delivery: EmailDelivery,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegisterError {
    /// 400 — request failed validation (message is input-free).
    Validation(String),
    /// 503 — the database is not configured/reachable.
    DependencyUnavailable,
    /// 500 — unexpected storage failure (generic).
    Internal,
}

impl RegisterError {
    pub fn code(&self) -> &'static str {
        match self {
            RegisterError::Validation(_) => "validation_failed",
            RegisterError::DependencyUnavailable => "dependency_unavailable",
            RegisterError::Internal => "internal",
        }
    }
    pub fn message(&self) -> &str {
        match self {
            RegisterError::Validation(message) => message,
            RegisterError::DependencyUnavailable => {
                "Registration is unavailable: the account database is not configured."
            }
            RegisterError::Internal => "The request could not be completed.",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerifyError {
    /// 400 — missing/empty token field.
    Validation(String),
    /// 400 — unknown, malformed, expired, used, or revoked token. One
    /// generic failure for all of these (no oracle).
    InvalidOrExpired,
    /// 503 — database not configured/reachable.
    DependencyUnavailable,
    /// 500.
    Internal,
}

impl VerifyError {
    pub fn code(&self) -> &'static str {
        match self {
            VerifyError::Validation(_) => "validation_failed",
            VerifyError::InvalidOrExpired => "invalid_or_expired_token",
            VerifyError::DependencyUnavailable => "dependency_unavailable",
            VerifyError::Internal => "internal",
        }
    }
    pub fn message(&self) -> &str {
        match self {
            VerifyError::Validation(message) => message,
            VerifyError::InvalidOrExpired => "This verification code is invalid or has expired.",
            VerifyError::DependencyUnavailable => {
                "Email verification is unavailable: the account database is not configured."
            }
            VerifyError::Internal => "The request could not be completed.",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResendError {
    Validation(String),
    DependencyUnavailable,
    Internal,
}

impl ResendError {
    pub fn code(&self) -> &'static str {
        match self {
            ResendError::Validation(_) => "validation_failed",
            ResendError::DependencyUnavailable => "dependency_unavailable",
            ResendError::Internal => "internal",
        }
    }
    pub fn message(&self) -> &str {
        match self {
            ResendError::Validation(message) => message,
            ResendError::DependencyUnavailable => {
                "Verification email resend is unavailable: the account database is not configured."
            }
            ResendError::Internal => "The request could not be completed.",
        }
    }
}

// ---------------------------------------------------------------------------
// Validation (pure; mirrors apps/desktop/ui/src/lib/formValidation.ts shapes)
// ---------------------------------------------------------------------------

/// Canonical form: trimmed + lowercased. Matches the database CHECK
/// (`email = lower(btrim(email))`) from Phase 23B.
pub fn normalize_email(raw: &str) -> String {
    raw.trim().to_lowercase()
}

/// Shape check consistent with the frontend (`local@domain.tld`, no spaces).
pub fn validate_email(raw: &str) -> Result<String, String> {
    let normalized = normalize_email(raw);
    if normalized.is_empty() {
        return Err("Email is required.".to_string());
    }
    if normalized.chars().count() > MAX_EMAIL_CHARS {
        return Err("Email is too long.".to_string());
    }
    if normalized.split_whitespace().count() != 1 {
        return Err("Email must not contain spaces.".to_string());
    }
    let mut parts = normalized.split('@');
    let local = parts.next().unwrap_or("");
    let domain = parts.next().unwrap_or("");
    if local.is_empty() || domain.is_empty() || parts.next().is_some() {
        return Err("Email must look like name@example.com.".to_string());
    }
    if !domain.contains('.') || domain.starts_with('.') || domain.ends_with('.') {
        return Err("Email must look like name@example.com.".to_string());
    }
    Ok(normalized)
}

pub fn validate_name(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("Name is required.".to_string());
    }
    if trimmed.chars().count() > MAX_NAME_CHARS {
        return Err("Name is too long (max 128 characters).".to_string());
    }
    Ok(trimmed.to_string())
}

/// Token strings are 64 lowercase/uppercase hex chars (32 bytes). Malformed
/// tokens are rejected by callers as generic `InvalidOrExpired`.
pub fn is_valid_token_shape(raw: &str) -> bool {
    raw.len() == VERIFICATION_TOKEN_HEX_CHARS && raw.chars().all(|c| c.is_ascii_hexdigit())
}

pub fn clamp_verification_ttl(secs: u64) -> u64 {
    secs.clamp(MIN_VERIFICATION_TTL_SECS, MAX_VERIFICATION_TTL_SECS)
}

// ---------------------------------------------------------------------------
// Token generation / hashing (raw token never stored)
// ---------------------------------------------------------------------------

/// 32 cryptographically secure random bytes (OsRng), hex-encoded for email.
pub fn generate_verification_token() -> String {
    use argon2::password_hash::rand_core::{OsRng, RngCore};
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    let mut token = String::with_capacity(VERIFICATION_TOKEN_HEX_CHARS);
    for byte in bytes {
        token.push_str(&format!("{byte:02x}"));
    }
    token
}

/// sha256 of the exact token string sent to the user (32-byte digest at rest).
pub fn hash_verification_token(raw: &str) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(raw.as_bytes());
    hasher.finalize().to_vec()
}

// ---------------------------------------------------------------------------
// Service functions
// ---------------------------------------------------------------------------

/// Build the verification email. The raw token appears ONLY here (body), never
/// in logs, errors, URLs-in-logs, or database rows.
fn verification_message(to: &str, name: &str, token: &str) -> EmailMessage {
    EmailMessage {
        to: to.to_string(),
        subject: "Verify your email for Nexora".to_string(),
        body: format!(
            "Hi {name},\n\n\
             Use this code to verify your email address for Nexora:\n\n\
             {token}\n\n\
             The code can be used once and expires in 24 hours. If you did not \
             create a Nexora account, you can ignore this message — nothing has \
             been verified.\n"
        ),
    }
}

/// Registration: validate → hash → insert user → store token digest → attempt
/// delivery. Duplicate normalized emails return the SAME outcome as a fresh
/// registration (enumeration defence, plan §6); the UNIQUE constraint makes
/// that safe under concurrency.
///
/// `delivery` is the deployment's transport capability (config-level). It is
/// identical in every outcome branch — the response can never reveal whether
/// the address was new. Per-attempt send failures are server-side only.
pub async fn register(
    pool: Option<&PgPool>,
    provider: &dyn EmailProvider,
    delivery: EmailDelivery,
    verification_ttl_secs: u64,
    request: &RegisterRequest,
) -> Result<RegisterOutcome, RegisterError> {
    let name = validate_name(&request.name).map_err(RegisterError::Validation)?;
    let email = validate_email(&request.email).map_err(RegisterError::Validation)?;
    let password_hash = password::hash_password(&request.password)
        .map_err(|error| RegisterError::Validation(error.to_string()))?;

    let pool = pool.ok_or(RegisterError::DependencyUnavailable)?;

    let inserted = sqlx::query(
        "INSERT INTO users (display_name, email, password_hash) \
         VALUES ($1, $2, $3) RETURNING id::text",
    )
    .bind(&name)
    .bind(&email)
    .bind(&password_hash)
    .fetch_optional(pool)
    .await;

    let user_id: Option<String> = match inserted {
        Ok(row) => row.map(|row: sqlx::postgres::PgRow| {
            use sqlx::Row;
            row.get("id")
        }),
        Err(error) if is_unique_violation(&error) => {
            // Address already registered: return the identical generic outcome
            // and create nothing (no token, no email).
            return Ok(RegisterOutcome {
                email_delivery: delivery,
            });
        }
        Err(_) => return Err(RegisterError::Internal),
    };

    let Some(user_id) = user_id else {
        return Err(RegisterError::Internal);
    };

    // Fresh token for the new account. Raw token goes to the email only.
    let raw_token = generate_verification_token();
    let token_digest = hash_verification_token(&raw_token);
    let ttl = clamp_verification_ttl(verification_ttl_secs);

    let stored = sqlx::query(
        "INSERT INTO email_verification_tokens (user_id, token_hash, expires_at) \
         VALUES (CAST($1 AS uuid), $2, now() + make_interval(secs => $3))",
    )
    .bind(&user_id)
    .bind(&token_digest)
    .bind(ttl as f64)
    .execute(pool)
    .await;

    if stored.is_err() {
        return Err(RegisterError::Internal);
    }

    // Attempt delivery for the new account only. Failures are honest: the
    // response reports capability (identical for every account), never a send
    // receipt, so a failed send cannot leak account existence.
    let message = verification_message(&email, &name, &raw_token);
    let _ = provider.send(message).await;

    Ok(RegisterOutcome {
        email_delivery: delivery,
    })
}

/// Verify: atomically consume the token (single-use) then mark the user
/// verified. All token failures collapse into one generic error.
pub async fn verify_email(pool: Option<&PgPool>, raw_token: &str) -> Result<(), VerifyError> {
    if raw_token.trim().is_empty() {
        return Err(VerifyError::Validation(
            "Verification code is required.".to_string(),
        ));
    }
    if !is_valid_token_shape(raw_token.trim()) {
        return Err(VerifyError::InvalidOrExpired);
    }
    let pool = pool.ok_or(VerifyError::DependencyUnavailable)?;
    let digest = hash_verification_token(raw_token.trim());

    let mut transaction = pool.begin().await.map_err(|_| VerifyError::Internal)?;

    // Atomic consume: only one concurrent request can flip used_at.
    let consumed = sqlx::query_scalar::<_, String>(
        "UPDATE email_verification_tokens \
         SET used_at = now() \
         WHERE token_hash = $1 \
           AND used_at IS NULL \
           AND revoked_at IS NULL \
           AND expires_at > now() \
         RETURNING user_id::text",
    )
    .bind(&digest)
    .fetch_optional(&mut *transaction)
    .await
    .map_err(|_| VerifyError::Internal)?;

    let Some(user_id) = consumed else {
        // Roll back (no writes happened) and report one generic failure.
        let _ = transaction.rollback().await;
        return Err(VerifyError::InvalidOrExpired);
    };

    let marked = sqlx::query(
        "UPDATE users SET email_verified_at = COALESCE(email_verified_at, now()) \
         WHERE id = CAST($1 AS uuid)",
    )
    .bind(&user_id)
    .execute(&mut *transaction)
    .await
    .map_err(|_| VerifyError::Internal)?;

    if marked.rows_affected() != 1 {
        let _ = transaction.rollback().await;
        return Err(VerifyError::InvalidOrExpired);
    }

    transaction.commit().await.map_err(|_| VerifyError::Internal)?;
    Ok(())
}

/// Resend: always safe to call; never reveals whether the address exists.
/// Prior live tokens for an unverified account are revoked and a fresh token
/// issued. Returns transport capability only (config-level, identical for
/// every address).
pub async fn resend_verification(
    pool: Option<&PgPool>,
    provider: &dyn EmailProvider,
    delivery: EmailDelivery,
    verification_ttl_secs: u64,
    raw_email: &str,
) -> Result<EmailDelivery, ResendError> {
    let email = validate_email(raw_email).map_err(ResendError::Validation)?;
    let pool = pool.ok_or(ResendError::DependencyUnavailable)?;

    let existing = sqlx::query_scalar::<_, String>(
        "SELECT id::text FROM users \
         WHERE email = $1 AND email_verified_at IS NULL AND status = 'active'",
    )
    .bind(&email)
    .fetch_optional(pool)
    .await
    .map_err(|_| ResendError::Internal)?;

    if let Some(user_id) = existing {
        // Invalidate earlier live tokens (resend invalidation).
        sqlx::query(
            "UPDATE email_verification_tokens \
             SET revoked_at = now() \
             WHERE user_id = CAST($1 AS uuid) AND used_at IS NULL AND revoked_at IS NULL",
        )
        .bind(&user_id)
        .execute(pool)
        .await
        .map_err(|_| ResendError::Internal)?;

        let raw_token = generate_verification_token();
        let token_digest = hash_verification_token(&raw_token);
        let ttl = clamp_verification_ttl(verification_ttl_secs);
        sqlx::query(
            "INSERT INTO email_verification_tokens (user_id, token_hash, expires_at) \
             VALUES (CAST($1 AS uuid), $2, now() + make_interval(secs => $3))",
        )
        .bind(&user_id)
        .bind(&token_digest)
        .bind(ttl as f64)
        .execute(pool)
        .await
        .map_err(|_| ResendError::Internal)?;

        let message = verification_message(&email, "there", &raw_token);
        let _ = provider.send(message).await;
    }

    Ok(delivery)
}

/// PostgreSQL unique_violation (23505) — the concurrent-duplicate path.
fn is_unique_violation(error: &sqlx::Error) -> bool {
    match error {
        sqlx::Error::Database(database_error) => {
            if let Some(pg) = database_error.downcast_ref::<PgDatabaseError>() {
                pg.code() == "23505"
            } else {
                database_error.code().as_deref() == Some("23505")
            }
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_email_trims_and_lowercases() {
        assert_eq!(normalize_email("  Ada@Example.Test  "), "ada@example.test");
    }

    #[test]
    fn validate_email_accepts_shaped_addresses() {
        for good in ["ada@example.test", "a.b+c@sub.example.test", "x@y.io"] {
            assert!(validate_email(good).is_ok(), "{good} should pass");
        }
    }

    #[test]
    fn validate_email_rejects_bad_shapes_without_echoing_long_input() {
        for bad in ["", "   ", "nope", "a@b", "a@@b.com", "a b@c.com", "@b.com", "a@.com", "a@b."] {
            assert!(validate_email(bad).is_err(), "{bad:?} should fail");
        }
        let long = format!("{}@example.test", "a".repeat(300));
        let error = validate_email(&long).unwrap_err();
        assert!(!error.contains("aaa"), "error must not echo the input");
    }

    #[test]
    fn validate_name_bounds() {
        assert!(validate_name("  Ada  ").is_ok());
        assert!(validate_name("").is_err());
        assert!(validate_name("   ").is_err());
        assert!(validate_name(&"x".repeat(MAX_NAME_CHARS)).is_ok());
        assert!(validate_name(&"x".repeat(MAX_NAME_CHARS + 1)).is_err());
    }

    #[test]
    fn token_shape_is_64_hex() {
        assert!(is_valid_token_shape(&"a1".repeat(32)));
        assert!(!is_valid_token_shape(&"a1".repeat(31)));
        assert!(!is_valid_token_shape(&"g".repeat(64)));
        assert!(!is_valid_token_shape(""));
    }

    #[test]
    fn generated_tokens_are_64_hex_and_unique() {
        let first = generate_verification_token();
        let second = generate_verification_token();
        assert_eq!(first.len(), VERIFICATION_TOKEN_HEX_CHARS);
        assert!(is_valid_token_shape(&first));
        assert_ne!(first, second, "tokens must be unique");
    }

    #[test]
    fn token_hash_is_32_bytes_and_never_the_raw_token() {
        let token = generate_verification_token();
        let digest = hash_verification_token(&token);
        assert_eq!(digest.len(), 32);
        assert_ne!(digest, token.as_bytes());
        assert_eq!(digest, hash_verification_token(&token), "hash is deterministic");
    }

    #[test]
    fn ttl_clamp_holds_bounds() {
        assert_eq!(clamp_verification_ttl(1), MIN_VERIFICATION_TTL_SECS);
        assert_eq!(clamp_verification_ttl(999_999), MAX_VERIFICATION_TTL_SECS);
        assert_eq!(clamp_verification_ttl(3_600), 3_600);
    }

    #[test]
    fn error_messages_never_contain_secrets() {
        let register = RegisterError::Validation("bad".to_string());
        assert_eq!(register.code(), "validation_failed");
        for error in [
            VerifyError::InvalidOrExpired.message().to_string(),
            VerifyError::DependencyUnavailable.message().to_string(),
            ResendError::DependencyUnavailable.message().to_string(),
        ] {
            assert!(!error.contains('@'));
            assert!(!error.to_lowercase().contains("token hash"));
        }
    }

    #[test]
    fn verification_message_contains_token_only_in_body() {
        let message = verification_message("ada@example.test", "Ada", "deadbeef");
        assert_eq!(message.to, "ada@example.test");
        assert!(message.body.contains("deadbeef"));
        assert!(!message.subject.contains("deadbeef"));
    }
}
