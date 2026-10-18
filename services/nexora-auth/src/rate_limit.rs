//! DB-backed rate limiting for auth endpoints (Phase 23D v1) — plan §4/§5/§7.
//!
//! Design constraints (brief §4 + plan): the store is the shared PostgreSQL
//! `login_attempts` table, so counts hold **across multiple backend
//! instances** — no in-memory counters. Limits are sliding **windows** over
//! failed attempts only; there is no permanent account lockout (which would
//! let attackers deny service to users). The plan notes this table "can move
//! to Redis without contract changes" — that remains the step-6 hardening
//! track alongside per-IP proxies/`X-Forwarded-For` trust and the
//! secret-leakage test suite.
//!
//! Keys are hashed (`e:` + sha256(email), `i:` + sha256(ip)) — never raw
//! emails, IPs, or credentials. `Retry-After` (seconds) is derived from the
//! window. This is **v1, not production-grade** while the crate remains
//! compiler-unverified (docs/phase-23d/README.md).

use sha2::{Digest, Sha256};
use sqlx::PgPool;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RateLimitRule {
    pub max_failures: u32,
    pub window_secs: u64,
}

/// What a window counts: login brute-force defence counts failures only (a
/// legitimate user signing in repeatedly must never be locked out), while
/// register/resend volume abuse counts every attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CountMode {
    FailuresOnly,
    AllAttempts,
}

/// Plan §7 window syntax: `5/15m`, `3/60m`. Also accepts `s`/`h` suffixes.
pub fn parse_rate_limit(raw: &str) -> Result<RateLimitRule, String> {
    let trimmed = raw.trim();
    let Some((count_part, window_part)) = trimmed.split_once('/') else {
        return Err(format!("{RATE_SYNTAX_HINT} (Value withheld.)"));
    };
    let max_failures: u32 = count_part
        .trim()
        .parse()
        .map_err(|_| format!("{RATE_SYNTAX_HINT} (Value withheld.)"))?;
    if max_failures == 0 {
        return Err(format!("{RATE_SYNTAX_HINT} (Value withheld.)"));
    }
    let window_part = window_part.trim();
    let (number, multiplier) = match window_part.chars().last() {
        Some('s') => (&window_part[..window_part.len() - 1], 1_u64),
        Some('m') => (&window_part[..window_part.len() - 1], 60),
        Some('h') => (&window_part[..window_part.len() - 1], 3_600),
        _ => return Err(format!("{RATE_SYNTAX_HINT} (Value withheld.)")),
    };
    let amount: u64 = number
        .parse()
        .map_err(|_| format!("{RATE_SYNTAX_HINT} (Value withheld.)"))?;
    if amount == 0 {
        return Err(format!("{RATE_SYNTAX_HINT} (Value withheld.)"));
    }
    let window_secs = amount
        .checked_mul(multiplier)
        .ok_or_else(|| format!("{RATE_SYNTAX_HINT} (Value withheld.)"))?;
    Ok(RateLimitRule {
        max_failures,
        window_secs,
    })
}

const RATE_SYNTAX_HINT: &str =
    "Rate-limit values must look like '5/15m' (failures per window; s/m/h suffix)";

/// Plan §7 defaults (conservative; documented in docs/phase-23d/README.md).
pub fn default_login_rule() -> RateLimitRule {
    RateLimitRule { max_failures: 10, window_secs: 900 }
}
pub fn default_register_rule() -> RateLimitRule {
    RateLimitRule { max_failures: 5, window_secs: 3_600 }
}
pub fn default_resend_rule() -> RateLimitRule {
    RateLimitRule { max_failures: 5, window_secs: 900 }
}
/// Plan §7 `AUTH_RATE_LIMIT_RECOVERY` default (5/15m).
pub fn default_recovery_rule() -> RateLimitRule {
    RateLimitRule { max_failures: 5, window_secs: 900 }
}

/// Hashed key derived from a raw reset token (`t:` + sha256) — for
/// rate-limiting reset attempts without storing the token itself.
pub fn token_key(raw_token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"nexora-auth-token:");
    hasher.update(raw_token.as_bytes());
    format!("t:{}", hex(&hasher.finalize()))
}

/// Hashed attempt keys — never raw identifiers.
pub fn email_key(normalized_email: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"nexora-auth-email:");
    hasher.update(normalized_email.as_bytes());
    format!("e:{}", hex(&hasher.finalize()))
}

pub fn ip_key(address: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"nexora-auth-ip:");
    hasher.update(address.as_bytes());
    format!("i:{}", hex(&hasher.finalize()))
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

/// Plan §4: `429` + `Retry-After`, with **exponential lockout backoff** for
/// keys that keep failing across windows (threat row: "lockout backoff").
/// Each additional full window of failures beyond the limit doubles the block
/// time; it is capped (never an indefinite lockout — an attacker must not be
/// able to deny service to a user forever) and decays with the sliding
/// windows themselves.
pub const MAX_BACKOFF_SECS: u64 = 86_400;

/// Retry-After for `excess_windows` additional exhausted windows on top of
/// the base window: `base << (excess-1)`, capped at [`MAX_BACKOFF_SECS`].
pub fn escalated_retry_after(base_window_secs: u64, excess_windows: u64) -> u64 {
    if excess_windows == 0 {
        return base_window_secs;
    }
    let shift = excess_windows.saturating_sub(1).min(20);
    base_window_secs
        .saturating_mul(1u64 << shift)
        .min(MAX_BACKOFF_SECS)
        .max(base_window_secs)
}

/// Check one key against its rule WITHOUT recording. Returns
/// `Err(retry_after_secs)` when the window is exhausted; the retry value
/// escalates exponentially for keys failing repeatedly (plan §4 backoff).
pub async fn check(
    pool: &PgPool,
    scope: &str,
    key: &str,
    rule: RateLimitRule,
    mode: CountMode,
) -> Result<(), u64> {
    let count_sql = match mode {
        CountMode::FailuresOnly => {
            "SELECT count(*) FROM login_attempts \
             WHERE scope = $1 AND key = $2 AND outcome = 'failure' \
               AND created_at > now() - make_interval(secs => $3)"
        }
        CountMode::AllAttempts => {
            "SELECT count(*) FROM login_attempts \
             WHERE scope = $1 AND key = $2 \
               AND created_at > now() - make_interval(secs => $3)"
        }
    };
    let attempts: i64 = sqlx::query_scalar(count_sql)
        .bind(scope)
        .bind(key)
        .bind(rule.window_secs as f64)
        .fetch_one(pool)
        .await
        .map_err(|_| 0_u64)?;

    if attempts >= i64::from(rule.max_failures) {
        // Escalation horizon: 10× the window (bounded by the cap) — how many
        // full windows' worth of failures this key has piled up recently.
        let horizon_secs = (rule.window_secs.saturating_mul(10)).min(MAX_BACKOFF_SECS).max(rule.window_secs);
        let recent: i64 = sqlx::query_scalar(count_sql)
            .bind(scope)
            .bind(key)
            .bind(horizon_secs as f64)
            .fetch_one(pool)
            .await
            .map_err(|_| 0)?;
        let windows_failed = (recent / i64::from(rule.max_failures).max(1)).max(1) as u64;
        return Err(escalated_retry_after(rule.window_secs, windows_failed));
    }
    Ok(())
}

/// Record one attempt outcome (`success` | `failure`) for the sliding window.
pub async fn record(pool: &PgPool, scope: &str, key: &str, outcome: &str) {
    let _ = sqlx::query("INSERT INTO login_attempts (scope, key, outcome) VALUES ($1, $2, $3)")
        .bind(scope)
        .bind(key)
        .bind(outcome)
        .execute(pool)
        .await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plan_window_syntax() {
        let rule = parse_rate_limit("5/15m").unwrap();
        assert_eq!(rule, RateLimitRule { max_failures: 5, window_secs: 900 });
        assert_eq!(
            parse_rate_limit("3/60m").unwrap(),
            RateLimitRule { max_failures: 3, window_secs: 3_600 }
        );
        assert_eq!(
            parse_rate_limit(" 7 / 2h ").unwrap(),
            RateLimitRule { max_failures: 7, window_secs: 7_200 }
        );
        assert_eq!(
            parse_rate_limit("2/30s").unwrap(),
            RateLimitRule { max_failures: 2, window_secs: 30 }
        );
    }

    #[test]
    fn rejects_malformed_windows_without_echoing_values() {
        for bad in ["", "5", "5/15", "x/15m", "0/15m", "5/0m", "5/15x"] {
            let error = parse_rate_limit(bad).unwrap_err();
            assert!(error.contains("withheld"), "{bad:?} → {error}");
        }
    }

    #[test]
    fn attempt_keys_are_hashes_not_raw_identifiers() {
        let key = email_key("ada@example.test");
        assert!(key.starts_with("e:"));
        assert!(!key.contains("ada"));
        assert_eq!(key, email_key("ada@example.test"), "deterministic");
        assert_ne!(key, email_key("other@example.test"));
        let ip = ip_key("203.0.113.9");
        assert!(ip.starts_with("i:"));
        assert!(!ip.contains("203"));
    }

    #[test]
    fn defaults_are_windows_not_lockouts() {
        for rule in [
            default_login_rule(),
            default_register_rule(),
            default_resend_rule(),
            default_recovery_rule(),
        ] {
            assert!(rule.max_failures > 0);
            assert!(rule.window_secs > 0);
        }
    }
}

#[cfg(test)]
mod backoff_tests {
    use super::*;

    #[test]
    fn backoff_doubles_per_excess_window_and_stays_bounded() {
        assert_eq!(escalated_retry_after(900, 0), 900);
        assert_eq!(escalated_retry_after(900, 1), 900);
        assert_eq!(escalated_retry_after(900, 2), 1_800);
        assert_eq!(escalated_retry_after(900, 3), 3_600);
        // Capped at MAX_BACKOFF_SECS — no indefinite lockout.
        assert_eq!(escalated_retry_after(900, 10), MAX_BACKOFF_SECS);
        assert_eq!(escalated_retry_after(3_600, 20), MAX_BACKOFF_SECS);
        // Never below the base window, never panics on huge inputs.
        assert_eq!(escalated_retry_after(60, u64::MAX), MAX_BACKOFF_SECS);
        assert_eq!(escalated_retry_after(1, 1), 1);
    }
}
