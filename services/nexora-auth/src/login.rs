//! Login service (Phase 23D) — plan §6 `POST /v1/auth/login`.
//!
//! Security invariants:
//! - Unknown accounts and wrong passwords return the **same generic**
//!   `invalid_credentials` failure (no enumeration). Unknown-account attempts
//!   run a dummy Argon2id verification so response timing does not separate
//!   "no such account" from "wrong password".
//! - Plaintext passwords are never stored or logged; verification goes
//!   through the Phase 23B Argon2id module only.
//! - Ineligible accounts (status `locked`/`deleted`, or email unverified) are
//!   refused. Unverified-with-correct-password returns the plan's
//!   `email_unverified` code (the caller proved the credential; the account
//!   model requires verification-first — plan §9 decision 4).
//! - Responses never contain password hashes, raw credentials, database
//!   errors, or secret configuration values.

use std::sync::OnceLock;

use sqlx::PgPool;

use crate::password;
use crate::registration::validate_email;
use crate::session::{self, CreatedSession, SessionError};

#[derive(Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

/// Redacting `Debug` (Phase 23G): passwords must never reach logs/traces.
impl std::fmt::Debug for LoginRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoginRequest")
            .field("email", &self.email)
            .field("password", &crate::scrub::REDACTED)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoginError {
    /// 400 `validation_failed`.
    Validation(String),
    /// 401 `invalid_credentials` — unknown account, wrong password, or an
    /// ineligible (locked/deleted) account. One generic failure.
    InvalidCredentials,
    /// 403 `email_unverified` — password correct, account not yet verified.
    EmailUnverified,
    /// 503 `dependency_unavailable`.
    DependencyUnavailable,
    /// 500 `internal`.
    Internal,
}

impl LoginError {
    pub fn code(&self) -> &'static str {
        match self {
            LoginError::Validation(_) => "validation_failed",
            LoginError::InvalidCredentials => "invalid_credentials",
            LoginError::EmailUnverified => "email_unverified",
            LoginError::DependencyUnavailable => "dependency_unavailable",
            LoginError::Internal => "internal",
        }
    }
    pub fn message(&self) -> &str {
        match self {
            LoginError::Validation(message) => message,
            LoginError::InvalidCredentials => "Email or password is incorrect.",
            LoginError::EmailUnverified => {
                "Verify your email address before signing in. Request a new code from the sign-up screen."
            }
            LoginError::DependencyUnavailable => {
                "Sign in is unavailable: the account database is not configured."
            }
            LoginError::Internal => "The request could not be completed.",
        }
    }
    pub fn status_code(&self) -> u16 {
        match self {
            LoginError::Validation(_) => 400,
            LoginError::InvalidCredentials => 401,
            LoginError::EmailUnverified => 403,
            LoginError::DependencyUnavailable => 503,
            LoginError::Internal => 500,
        }
    }
}

/// Authenticated login result: user identity + a fresh session (raw token
/// returned exactly once, to the trusted client over the approved transport).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginOutcome {
    pub user_id: String,
    pub name: String,
    pub email: String,
    pub session: CreatedSession,
}

/// A dummy PHC hash used to equalize timing for unknown accounts (the
/// password is irrelevant — only the verification cost matters).
fn dummy_hash() -> &'static str {
    static DUMMY: OnceLock<String> = OnceLock::new();
    DUMMY.get_or_init(|| {
        password::hash_password("nexora-timing-equalizer-not-a-real-password")
            .expect("dummy hash always succeeds")
    })
}

fn verify_any(password_input: &str, password_hash: Option<&str>) -> Result<(), ()> {
    match password_hash {
        Some(hash) if !hash.is_empty() => {
            password::verify_password(password_input, hash).map_err(|_| ())
        }
        // Unknown account or OAuth-only row: burn the same Argon2id cost.
        _ => password::verify_password(password_input, dummy_hash()).map_err(|_| ()),
    }
}

/// Validate the request shape without leaking password policy (a short
/// password is simply an invalid credential at login time).
fn validate_login_request(request: &LoginRequest) -> Result<String, LoginError> {
    let email = validate_email(&request.email).map_err(LoginError::Validation)?;
    if request.password.is_empty() {
        return Err(LoginError::Validation("Password is required.".to_string()));
    }
    Ok(email)
}

/// Full login flow: normalize → **transactional** lookup with the account row
/// lock → Argon2id verify (timing equalized) → account-status model → create a
/// session row → commit. The row lock is held across verification and insert
/// so a concurrent `reset_password`/`change_password` (which holds the same
/// lock while changing the password and revoking sessions) can never coexist
/// with a session created from a stale credential check (Phase 23E).
pub async fn login(
    pool: Option<&PgPool>,
    session_ttl_secs: u64,
    request: &LoginRequest,
    app_version: Option<&str>,
    platform: Option<&str>,
) -> Result<LoginOutcome, LoginError> {
    let email = validate_login_request(request)?;
    let pool = pool.ok_or(LoginError::DependencyUnavailable)?;

    let mut tx = pool.begin().await.map_err(|_| LoginError::Internal)?;
    let row = sqlx::query(
        "SELECT id::text AS id, display_name AS name, email, password_hash, \
                (email_verified_at IS NOT NULL) AS verified, status \
         FROM users WHERE email = $1 FOR UPDATE",
    )
    .bind(&email)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|_| LoginError::Internal)?;

    let (user_id, name, email_out, verified, status): (String, String, String, bool, String) =
        match row {
            Some(row) => {
                use sqlx::Row;
                let id: String = row.get("id");
                let name: String = row.get("name");
                let email: String = row.get("email");
                let password_hash: Option<String> = row.get("password_hash");
                let verified: bool = row.get("verified");
                let status: String = row.get("status");
                if verify_any(&request.password, password_hash.as_deref()).is_err() {
                    return Err(LoginError::InvalidCredentials);
                }
                (id, name, email, verified, status)
            }
            None => {
                // Unknown account: same generic failure after equal work.
                let _ = verify_any(&request.password, None);
                return Err(LoginError::InvalidCredentials);
            }
        };

    // Account-status model (Phase 23B: active | locked | deleted).
    if status != "active" {
        return Err(LoginError::InvalidCredentials);
    }
    // Verification-first policy (plan §9 decision 4).
    if !verified {
        return Err(LoginError::EmailUnverified);
    }

    let session =
        session::insert_session(&mut *tx, &user_id, session_ttl_secs, app_version, platform)
            .await
            .map_err(|error| match error {
                SessionError::DependencyUnavailable => LoginError::DependencyUnavailable,
                SessionError::Internal => LoginError::Internal,
                other => {
                    let _ = other;
                    LoginError::Internal
                }
            })?;
    tx.commit().await.map_err(|_| LoginError::Internal)?;

    Ok(LoginOutcome {
        user_id,
        name,
        email: email_out,
        session,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn login_request_normalizes_email_like_registration() {
        let request = LoginRequest {
            email: "  Ada@Example.Test  ".to_string(),
            password: "whatever".to_string(),
        };
        assert_eq!(
            validate_login_request(&request).unwrap(),
            "ada@example.test"
        );
    }

    #[test]
    fn empty_password_is_validation_not_credential_leak() {
        let request = LoginRequest {
            email: "ada@example.test".to_string(),
            password: String::new(),
        };
        let error = validate_login_request(&request).unwrap_err();
        assert_eq!(error.code(), "validation_failed");
    }

    #[test]
    fn bad_email_shape_is_validation_failed() {
        let request = LoginRequest {
            email: "nope".to_string(),
            password: "whatever".to_string(),
        };
        let error = validate_login_request(&request).unwrap_err();
        assert_eq!(error.code(), "validation_failed");
        assert!(!error.message().contains("nope"));
    }

    #[test]
    fn credential_errors_share_one_generic_message() {
        assert_eq!(
            LoginError::InvalidCredentials.message(),
            "Email or password is incorrect."
        );
        assert_eq!(LoginError::InvalidCredentials.status_code(), 401);
        assert_eq!(LoginError::EmailUnverified.status_code(), 403);
        assert_ne!(
            LoginError::InvalidCredentials.code(),
            LoginError::EmailUnverified.code()
        );
    }

    #[test]
    fn dummy_hash_is_a_valid_phc_string_for_timing_equalization() {
        assert!(dummy_hash().starts_with("$argon2id$"));
        // Verifying against it behaves like a real (mismatching) verify.
        assert!(password::verify_password("anything", dummy_hash()).is_err());
    }

    #[test]
    fn error_messages_never_contain_credentials() {
        for error in [
            LoginError::InvalidCredentials,
            LoginError::EmailUnverified,
            LoginError::DependencyUnavailable,
        ] {
            let message = error.message();
            assert!(!message.contains('@'));
            assert!(!message.to_lowercase().contains("password hash"));
        }
    }
}
