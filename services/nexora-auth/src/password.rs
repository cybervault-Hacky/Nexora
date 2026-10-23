//! Argon2id password hashing and verification (Phase 23B).
//!
//! Self-contained crypto module, deliberately independent of HTTP routes and
//! the frontend so it can be tested in isolation. It is the single policy
//! point for password hashing:
//!
//! - Algorithm: **Argon2id** (version 0x13) via the maintained `argon2` crate
//!   (RustCrypto `password-hashes` family). No custom cryptography and no
//!   custom hash format — encoded hashes use the library's PHC string format,
//!   which carries algorithm, parameters, and salt.
//! - Parameters: m = 19,456 KiB (19 MiB), t = 2, p = 1, 32-byte output — the
//!   OWASP recommended Argon2id baseline. Explicit, not library defaults.
//! - Salt: a fresh cryptographically secure random salt (16 bytes via
//!   `SaltString::generate` + `OsRng`) for **every** hash, so identical
//!   passwords never produce identical hashes.
//!
//! Security rules enforced here:
//! - Plaintext passwords are never logged, displayed, or persisted; only the
//!   PHC string leaves this module (`hash_password` return value).
//! - `verify_password` returns one **generic** failure for every outcome:
//!   wrong password and malformed encoded hash are indistinguishable.
//! - No hardcoded credentials, no reversible encryption, no unsalted or fast
//!   hashes (MD5/SHA-1/SHA-256) — those are categorically wrong for passwords.

use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::{Algorithm, Argon2, Params, Version};

/// New-password policy, matching `apps/desktop/ui/src/lib/formValidation.ts`.
pub const PASSWORD_MIN_LENGTH: usize = 8;

/// Upper bound so hostile input cannot inflate hashing cost (argon2 DoS cap).
pub const PASSWORD_MAX_LENGTH: usize = 128;

/// Argon2id memory cost in KiB (OWASP baseline).
pub const ARGON2_M_COST_KIB: u32 = 19_456;

/// Argon2id time cost (OWASP baseline).
pub const ARGON2_T_COST: u32 = 2;

/// Argon2id parallelism (OWASP baseline).
pub const ARGON2_P_COST: u32 = 1;

/// Derived-key length in bytes.
pub const ARGON2_OUTPUT_BYTES: usize = 32;

/// Failure modes. `Display` messages are fixed strings — they never include
/// any part of a password or hash.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasswordError {
    TooShort,
    TooLong,
    HashingFailed,
    /// Generic: wrong password, malformed PHC string, and oversized candidate
    /// all produce this exact error.
    VerificationFailed,
}

impl PasswordError {
    pub fn message(&self) -> &'static str {
        match self {
            Self::TooShort => "Password must be at least 8 characters.",
            Self::TooLong => "Password must be at most 128 characters.",
            Self::HashingFailed => "Password hashing failed.",
            Self::VerificationFailed => "Password verification failed.",
        }
    }
}

impl std::fmt::Display for PasswordError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.message())
    }
}

impl std::error::Error for PasswordError {}

fn argon2_context() -> Result<Argon2<'static>, ()> {
    let params = Params::new(
        ARGON2_M_COST_KIB,
        ARGON2_T_COST,
        ARGON2_P_COST,
        Some(ARGON2_OUTPUT_BYTES),
    )
    .map_err(|_| ())?;
    Ok(Argon2::new(Algorithm::Argon2id, Version::V0x13, params))
}

/// New-password policy (single source of truth — exported in Phase 23E so
/// reset/change can validate WITHOUT hashing or consuming tokens).
pub fn validate_new_password(password: &str) -> Result<(), PasswordError> {
    let length = password.chars().count();
    if length < PASSWORD_MIN_LENGTH {
        return Err(PasswordError::TooShort);
    }
    if length > PASSWORD_MAX_LENGTH || password.len() > PASSWORD_MAX_LENGTH {
        return Err(PasswordError::TooLong);
    }
    Ok(())
}

/// Hash a new password with Argon2id and a unique random salt.
///
/// Returns the PHC encoded string (e.g. `$argon2id$v=19$m=19456,t=2,p=1$…`),
/// the only representation callers may store. Plaintext is never retained.
pub fn hash_password(password: &str) -> Result<String, PasswordError> {
    validate_new_password(password)?;
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = argon2_context().map_err(|_| PasswordError::HashingFailed)?;
    argon2
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|_| PasswordError::HashingFailed)
}

/// Verify a candidate password against a stored PHC encoded hash.
///
/// Every failure — wrong password, malformed/foreign encoded hash, oversized
/// candidate — returns the same generic `VerificationFailed`. Callers must
/// surface only that generic failure.
pub fn verify_password(password: &str, encoded_hash: &str) -> Result<(), PasswordError> {
    if password.chars().count() > PASSWORD_MAX_LENGTH || password.len() > PASSWORD_MAX_LENGTH {
        return Err(PasswordError::VerificationFailed);
    }
    let parsed = PasswordHash::new(encoded_hash).map_err(|_| PasswordError::VerificationFailed)?;
    let argon2 = argon2_context().map_err(|_| PasswordError::VerificationFailed)?;
    argon2
        .verify_password(password.as_bytes(), &parsed)
        .map_err(|_| PasswordError::VerificationFailed)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Synthetic fixture only — never a real credential.
    const FIXTURE: &str = "correct-horse-battery-staple";
    const OTHER: &str = "an entirely different password";

    #[test]
    fn hash_succeeds_for_valid_input() {
        let encoded = hash_password(FIXTURE).unwrap();
        assert!(
            encoded.starts_with("$argon2id$"),
            "PHC algorithm tag: {encoded}"
        );
        assert!(
            encoded.contains("m=19456"),
            "encoded params record m: {encoded}"
        );
        assert!(
            encoded.contains("t=2"),
            "encoded params record t: {encoded}"
        );
        assert!(
            encoded.contains("p=1"),
            "encoded params record p: {encoded}"
        );
    }

    #[test]
    fn identical_passwords_produce_distinct_hashes() {
        let first = hash_password(FIXTURE).unwrap();
        let second = hash_password(FIXTURE).unwrap();
        assert_ne!(first, second, "unique salts must make hashes distinct");
        // Both still verify against the original password.
        verify_password(FIXTURE, &first).unwrap();
        verify_password(FIXTURE, &second).unwrap();
    }

    #[test]
    fn correct_password_verifies() {
        let encoded = hash_password(FIXTURE).unwrap();
        assert_eq!(verify_password(FIXTURE, &encoded), Ok(()));
    }

    #[test]
    fn incorrect_password_fails_generically() {
        let encoded = hash_password(FIXTURE).unwrap();
        assert_eq!(
            verify_password(OTHER, &encoded),
            Err(PasswordError::VerificationFailed)
        );
    }

    #[test]
    fn malformed_encoded_hashes_fail_safely() {
        for bad in ["", "not-a-phc-string", "$argon2id$broken", "$bcrypt$x"] {
            assert_eq!(
                verify_password(FIXTURE, bad),
                Err(PasswordError::VerificationFailed),
                "case: {bad}"
            );
        }
    }

    #[test]
    fn wrong_password_and_malformed_hash_are_indistinguishable() {
        let encoded = hash_password(FIXTURE).unwrap();
        let wrong = verify_password(OTHER, &encoded).unwrap_err();
        let malformed = verify_password(FIXTURE, "garbage").unwrap_err();
        assert_eq!(wrong, malformed);
        assert_eq!(wrong.to_string(), malformed.to_string());
    }

    #[test]
    fn encoded_hash_never_contains_plaintext() {
        let encoded = hash_password(FIXTURE).unwrap();
        assert!(!encoded.contains(FIXTURE));
        assert!(!encoded.contains("correct-horse"));
    }

    #[test]
    fn rejects_passwords_outside_policy() {
        assert_eq!(hash_password("short"), Err(PasswordError::TooShort));
        let long = "x".repeat(PASSWORD_MAX_LENGTH + 1);
        assert_eq!(hash_password(&long), Err(PasswordError::TooLong));
    }

    #[test]
    fn verify_rejects_oversized_candidates_generically() {
        let encoded = hash_password(FIXTURE).unwrap();
        let long = "x".repeat(PASSWORD_MAX_LENGTH + 1);
        assert_eq!(
            verify_password(&long, &encoded),
            Err(PasswordError::VerificationFailed)
        );
    }

    #[test]
    fn error_messages_never_reveal_input() {
        let long = format!("hunter2-{}", "x".repeat(PASSWORD_MAX_LENGTH));
        for error in [
            hash_password("abc").unwrap_err(), // too short
            hash_password(&long).unwrap_err(), // too long
        ] {
            let rendered = error.to_string();
            assert!(!rendered.contains("hunter2"));
            assert!(!rendered.contains("abc"));
            assert!(!rendered.contains('x'));
        }
    }
}
