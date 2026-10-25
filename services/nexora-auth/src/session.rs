//! Server-managed session lifecycle (Phase 23D) — plan §4/§5/§6.
//!
//! Sessions are opaque 32-byte random tokens stored **only** as sha256(raw)
//! (`sessions.token_hash bytea UNIQUE`). The raw token exists solely in the
//! login/refresh response (to the trusted desktop Rust client over the
//! approved transport — never in URLs, logs, or the WebView) and in the
//! client's OS-backed credential store.
//!
//! Lifecycle invariants:
//! - Absolute expiry (`expires_at`) and idle expiry (`last_used_at`) are both
//!   enforced on **every** authenticated request; `revoked_at` is checked
//!   every time (plan §4 "revocation gap" control).
//! - Refresh rotates the token **within the family** (`family_id`); presenting
//!   an already-revoked token is treated as **reuse** → the entire family is
//!   revoked (plan §4: rotation + family reuse detection). Clients must
//!   single-flight refresh; a concurrent double-refresh revokes the family by
//!   design.
//! - Logout revokes the whole family. Revocation is monotonic
//!   (`revoked_at = COALESCE(revoked_at, now())`) — concurrent requests can
//!   never revive a revoked or expired session.
//! - Session tokens are never logged and never appear in URLs.

use sha2::{Digest, Sha256};
use sqlx::PgPool;

/// Plan §7: `SESSION_TTL_SECONDS` / `SESSION_IDLE_TTL_SECONDS` defaults.
pub const DEFAULT_SESSION_TTL_SECS: u64 = 2_592_000; // 30 days
pub const DEFAULT_SESSION_IDLE_TTL_SECS: u64 = 604_800; // 7 days
pub const MIN_SESSION_TTL_SECS: u64 = 300;
pub const MAX_SESSION_TTL_SECS: u64 = 31_536_000; // 365 days
/// 32 random bytes → 64 hex chars.
pub const SESSION_TOKEN_HEX_CHARS: usize = 64;
/// `last_used_at` writes are throttled to at most one per this interval.
const LAST_USED_THROTTLE_SECS: u64 = 60;

/// 32 cryptographically secure random bytes (OsRng), hex-encoded.
pub fn generate_session_token() -> String {
    use argon2::password_hash::rand_core::{OsRng, RngCore};
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    let mut token = String::with_capacity(SESSION_TOKEN_HEX_CHARS);
    for byte in bytes {
        token.push_str(&format!("{byte:02x}"));
    }
    token
}

/// sha256 of the exact raw token (32-byte digest at rest). Never reversible.
pub fn hash_session_token(raw: &str) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(raw.as_bytes());
    hasher.finalize().to_vec()
}

pub fn is_valid_session_token_shape(raw: &str) -> bool {
    raw.len() == SESSION_TOKEN_HEX_CHARS && raw.chars().all(|c| c.is_ascii_hexdigit())
}

/// The authenticated caller's account (only safe fields).
#[derive(Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionUser {
    pub id: String,
    pub name: String,
    pub email: String,
}

impl std::fmt::Debug for SessionUser {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionUser")
            .field("id", &crate::scrub::REDACTED)
            .field("name", &crate::scrub::REDACTED)
            .field("email", &crate::scrub::REDACTED)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct SessionContext {
    pub user: SessionUser,
    pub session_id: String,
    pub family_id: String,
}

impl std::fmt::Debug for SessionContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionContext")
            .field("user", &self.user)
            .field("session_id", &crate::scrub::REDACTED)
            .field("family_id", &crate::scrub::REDACTED)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionError {
    /// No/invalid/oversized token — one generic failure.
    InvalidToken,
    /// Absolute or idle expiry passed.
    Expired,
    /// Session was revoked; `reuse` marks the family-reuse case (the family
    /// has been revoked as a precaution).
    Revoked {
        reuse: bool,
    },
    DependencyUnavailable,
    Internal,
}

impl SessionError {
    /// Keep ordinary bearer failures generic, but do not make storage failures
    /// look like an invalid credential to desktop clients. In particular, the
    /// desktop IPC layer clears local credentials on `invalid_or_expired_token`.
    pub fn code(&self) -> &'static str {
        match self {
            SessionError::InvalidToken | SessionError::Expired | SessionError::Revoked { .. } => {
                "invalid_or_expired_token"
            }
            SessionError::DependencyUnavailable => "dependency_unavailable",
            SessionError::Internal => "internal",
        }
    }
    pub fn message(&self) -> &str {
        match self {
            SessionError::InvalidToken | SessionError::Expired | SessionError::Revoked { .. } => {
                "This session is invalid or has expired. Sign in again."
            }
            SessionError::DependencyUnavailable => {
                "Sessions are unavailable: the account database is not configured."
            }
            SessionError::Internal => "The request could not be completed.",
        }
    }
}

/// A freshly issued (or rotated) session: raw token returned to the client
/// exactly once, plus its absolute expiry (RFC3339, UTC).
#[derive(Clone, PartialEq, Eq)]
pub struct CreatedSession {
    pub raw_token: String,
    pub expires_at_rfc3339: String,
}

/// Redacting `Debug` (Phase 23G): the raw session token is a credential and
/// must never reach logs/traces/panic messages.
impl std::fmt::Debug for CreatedSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CreatedSession")
            .field("raw_token", &crate::scrub::REDACTED)
            .field("expires_at_rfc3339", &self.expires_at_rfc3339)
            .finish()
    }
}

/// Insert a session row. Takes any executor so callers can run it inside
/// their own transaction (login/reset serialisation — see `create_session`).
pub async fn insert_session<'e, E>(
    db: E,
    user_id: &str,
    session_ttl_secs: u64,
    app_version: Option<&str>,
    platform: Option<&str>,
) -> Result<CreatedSession, SessionError>
where
    E: sqlx::PgExecutor<'e>,
{
    let raw_token = generate_session_token();
    let digest = hash_session_token(&raw_token);
    let ttl = clamp_ttl(session_ttl_secs);

    let row = sqlx::query(
        "INSERT INTO sessions (user_id, token_hash, family_id, expires_at, app_version, platform) \
         VALUES (CAST($1 AS uuid), $2, gen_random_uuid(), \
                 now() + make_interval(secs => $3), $4, $5) \
         RETURNING to_char(expires_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"')",
    )
    .bind(user_id)
    .bind(&digest)
    .bind(ttl as f64)
    .bind(app_version)
    .bind(platform)
    .fetch_one(db)
    .await
    .map_err(|_| SessionError::Internal)?;

    use sqlx::Row;
    let expires_at_rfc3339: String = row.get(0);
    Ok(CreatedSession {
        raw_token,
        expires_at_rfc3339,
    })
}

/// Create a session for an active user, holding the account row lock for the
/// whole operation. The lock is the same one `reset_password`/
/// `change_password` hold while changing the password and revoking sessions:
/// a session can therefore never be created from a credential check that
/// raced a password change (either the password change sees and revokes this
/// session, or this insert happens strictly after it — Phase 23E).
pub async fn create_session(
    pool: &PgPool,
    user_id: &str,
    session_ttl_secs: u64,
    app_version: Option<&str>,
    platform: Option<&str>,
) -> Result<CreatedSession, SessionError> {
    let mut tx = pool.begin().await.map_err(|_| SessionError::Internal)?;
    let locked: Option<(String,)> = sqlx::query_as(
        "SELECT id::text FROM users WHERE id = CAST($1 AS uuid) AND status = 'active' FOR UPDATE",
    )
    .bind(user_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|_| SessionError::Internal)?;
    if locked.is_none() {
        return Err(SessionError::Internal);
    }
    let created =
        insert_session(&mut *tx, user_id, session_ttl_secs, app_version, platform).await?;
    tx.commit().await.map_err(|_| SessionError::Internal)?;
    Ok(created)
}

/// Validate a bearer token for an authenticated request. Enforces revocation,
/// absolute + idle expiry (`idle_ttl_secs`), account eligibility, and
/// refreshes `last_used_at` (throttled). A revoked token triggers
/// family-wide revocation (reuse).
pub async fn authenticate(
    pool: &PgPool,
    raw_token: &str,
    idle_ttl_secs: u64,
) -> Result<SessionContext, SessionError> {
    let raw = raw_token.trim();
    if !is_valid_session_token_shape(raw) {
        return Err(SessionError::InvalidToken);
    }
    let digest = hash_session_token(raw);
    let idle_ttl = clamp_idle_ttl(idle_ttl_secs);

    let row = sqlx::query(
        "SELECT s.id::text AS session_id, s.family_id::text AS family_id, \
                (s.revoked_at IS NULL) AS live, \
                (s.expires_at > now()) AS unexpired, \
                (s.last_used_at > now() - make_interval(secs => $2)) AS idle_ok, \
                u.id::text AS user_id, u.display_name AS name, u.email AS email, \
                (u.status = 'active') AS user_active, \
                (u.email_verified_at IS NOT NULL) AS user_verified \
         FROM sessions s JOIN users u ON u.id = s.user_id \
         WHERE s.token_hash = $1",
    )
    .bind(&digest)
    .bind(idle_ttl as f64)
    .fetch_optional(pool)
    .await
    .map_err(|_| SessionError::Internal)?;

    let Some(row) = row else {
        return Err(SessionError::InvalidToken);
    };

    use sqlx::Row;
    let session_id: String = row.get("session_id");
    let family_id: String = row.get("family_id");
    let live: bool = row.get("live");
    let unexpired: bool = row.get("unexpired");
    let idle_ok: bool = row.get("idle_ok");
    let user_id: String = row.get("user_id");
    let name: String = row.get("name");
    let email: String = row.get("email");
    let user_active: bool = row.get("user_active");
    let user_verified: bool = row.get("user_verified");

    if !live {
        // Family reuse detection: a revoked token is being presented.
        revoke_family_by_id(pool, &family_id).await?;
        return Err(SessionError::Revoked { reuse: true });
    }
    if !unexpired || !idle_ok {
        return Err(SessionError::Expired);
    }
    if !user_active || !user_verified {
        // Locked/deleted/unverified accounts hold no live sessions.
        revoke_family_by_id(pool, &family_id).await?;
        return Err(SessionError::InvalidToken);
    }

    // Throttled idle-anchor refresh; the WHERE clause makes it impossible to
    // resurrect a session that a concurrent logout just revoked.
    let updated = sqlx::query(
        "UPDATE sessions SET last_used_at = now() \
         WHERE id = CAST($1 AS uuid) AND revoked_at IS NULL \
           AND last_used_at < now() - make_interval(secs => $2)",
    )
    .bind(&session_id)
    .bind(LAST_USED_THROTTLE_SECS as f64)
    .execute(pool)
    .await
    .map_err(|_| SessionError::Internal)?;
    let _ = updated;

    Ok(SessionContext {
        user: SessionUser {
            id: user_id,
            name,
            email,
        },
        session_id,
        family_id,
    })
}

/// Rotate: validate the presented token, issue a new one in the same family,
/// and revoke the old one — atomically. Concurrent rotations: the first
/// commit wins; later ones fail without leaving extra live sessions.
pub async fn rotate_session(
    pool: &PgPool,
    raw_token: &str,
    session_ttl_secs: u64,
    idle_ttl_secs: u64,
    app_version: Option<&str>,
    platform: Option<&str>,
) -> Result<(CreatedSession, SessionContext), SessionError> {
    let context = authenticate(pool, raw_token, idle_ttl_secs).await?;

    let mut transaction = pool.begin().await.map_err(|_| SessionError::Internal)?;

    // Serialize refresh with password reset/change. Those operations lock the
    // account before revoking sessions; locking it here means a refresh either
    // commits before that revocation or sees the old session revoked afterward.
    let active_user = sqlx::query_scalar::<_, String>(
        "SELECT id::text FROM users \
         WHERE id = CAST($1 AS uuid) AND status = 'active' \
           AND email_verified_at IS NOT NULL FOR UPDATE",
    )
    .bind(&context.user.id)
    .fetch_optional(&mut *transaction)
    .await
    .map_err(|_| SessionError::Internal)?;
    if active_user.is_none() {
        let _ = transaction.rollback().await;
        return Err(SessionError::InvalidToken);
    }

    let raw_token_new = generate_session_token();
    let digest = hash_session_token(&raw_token_new);
    let ttl = clamp_ttl(session_ttl_secs);

    // Every rotated token keeps the family's original absolute expiry. Without
    // this bound, repeated refreshes would slide `expires_at` forever and turn
    // the configured absolute TTL into an idle-only lifetime.
    let row = sqlx::query(
        "INSERT INTO sessions (user_id, token_hash, family_id, expires_at, app_version, platform) \
         VALUES (CAST($1 AS uuid), $2, CAST($3 AS uuid), \
                 LEAST(
                     now() + make_interval(secs => $4),
                     (SELECT min(expires_at) FROM sessions WHERE family_id = CAST($3 AS uuid))
                 ), $5, $6) \
         RETURNING to_char(expires_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"')",
    )
    .bind(&context.user.id)
    .bind(&digest)
    .bind(&context.family_id)
    .bind(ttl as f64)
    .bind(app_version)
    .bind(platform)
    .fetch_one(&mut *transaction)
    .await
    .map_err(|_| SessionError::Internal)?;

    // Revoke the presented session exactly once; 0 rows = a concurrent
    // rotation won — roll back this one.
    let revoked = sqlx::query(
        "UPDATE sessions SET revoked_at = now() \
         WHERE id = CAST($1 AS uuid) AND revoked_at IS NULL",
    )
    .bind(&context.session_id)
    .execute(&mut *transaction)
    .await
    .map_err(|_| SessionError::Internal)?;
    if revoked.rows_affected() != 1 {
        let _ = transaction.rollback().await;
        return Err(SessionError::Revoked { reuse: false });
    }

    transaction
        .commit()
        .await
        .map_err(|_| SessionError::Internal)?;

    use sqlx::Row;
    let expires_at_rfc3339: String = row.get(0);
    Ok((
        CreatedSession {
            raw_token: raw_token_new,
            expires_at_rfc3339,
        },
        context,
    ))
}

/// Logout: revoke the presented session's entire family (no surviving rotated
/// siblings). Requires a currently valid token; misuse returns the generic
/// token error.
pub async fn revoke_family(
    pool: &PgPool,
    raw_token: &str,
    idle_ttl_secs: u64,
) -> Result<(), SessionError> {
    let context = authenticate(pool, raw_token, idle_ttl_secs).await?;
    revoke_family_by_id(pool, &context.family_id).await
}

/// Monotonic family revocation — safe under concurrency (never un-revokes).
async fn revoke_family_by_id(pool: &PgPool, family_id: &str) -> Result<(), SessionError> {
    sqlx::query(
        "UPDATE sessions SET revoked_at = COALESCE(revoked_at, now()) \
         WHERE family_id = CAST($1 AS uuid)",
    )
    .bind(family_id)
    .execute(pool)
    .await
    .map_err(|_| SessionError::Internal)?;
    Ok(())
}

fn clamp_ttl(secs: u64) -> u64 {
    secs.clamp(MIN_SESSION_TTL_SECS, MAX_SESSION_TTL_SECS)
}

/// Idle lifetimes never exceed the absolute TTL bounds.
fn clamp_idle_ttl(secs: u64) -> u64 {
    secs.clamp(MIN_SESSION_TTL_SECS, MAX_SESSION_TTL_SECS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_tokens_are_64_hex_unique_and_32_bytes_of_entropy() {
        let first = generate_session_token();
        let second = generate_session_token();
        assert_eq!(first.len(), SESSION_TOKEN_HEX_CHARS);
        assert!(is_valid_session_token_shape(&first));
        assert_ne!(first, second);
    }

    #[test]
    fn token_hash_is_sha256_at_rest_not_the_raw_token() {
        let token = generate_session_token();
        let digest = hash_session_token(&token);
        assert_eq!(digest.len(), 32);
        assert_ne!(digest, token.as_bytes());
        assert_eq!(digest, hash_session_token(&token));
    }

    #[test]
    fn token_shape_rejects_wrong_lengths_and_non_hex() {
        assert!(!is_valid_session_token_shape(""));
        assert!(!is_valid_session_token_shape(&"a".repeat(63)));
        assert!(!is_valid_session_token_shape(&"g".repeat(64)));
    }

    #[test]
    fn ttl_clamp_holds_bounds() {
        assert_eq!(clamp_ttl(1), MIN_SESSION_TTL_SECS);
        assert_eq!(clamp_ttl(u64::MAX), MAX_SESSION_TTL_SECS);
        assert_eq!(
            clamp_ttl(DEFAULT_SESSION_TTL_SECS),
            DEFAULT_SESSION_TTL_SECS
        );
    }

    #[test]
    fn session_debug_redacts_user_and_session_identifiers() {
        let context = SessionContext {
            user: SessionUser {
                id: "SENTINEL_user_id".to_string(),
                name: "SENTINEL_person".to_string(),
                email: "SENTINEL_email@example.test".to_string(),
            },
            session_id: "SENTINEL_session_id".to_string(),
            family_id: "SENTINEL_family_id".to_string(),
        };
        assert!(!format!("{context:?}").contains("SENTINEL"));
    }

    #[test]
    fn session_errors_are_generic_and_secret_free() {
        let expired = SessionError::Expired;
        assert_eq!(expired.code(), "invalid_or_expired_token");
        let revoked = SessionError::Revoked { reuse: true };
        assert_eq!(revoked.code(), "invalid_or_expired_token");
        assert_eq!(SessionError::DependencyUnavailable.code(), "dependency_unavailable");
        assert_eq!(SessionError::Internal.code(), "internal");
        for message in [
            expired.message(),
            revoked.message(),
            SessionError::InvalidToken.message(),
            SessionError::DependencyUnavailable.message(),
            SessionError::Internal.message(),
        ] {
            assert!(!message.contains('@'));
            assert!(!message.to_lowercase().contains("token hash"));
        }
        // Revoked and invalid share one message — no state oracle.
        assert_eq!(revoked.message(), SessionError::InvalidToken.message());
    }
}
