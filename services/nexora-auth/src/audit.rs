//! Persistent, secret-safe security & billing audit log.
//!
//! Records high-value account, identity, and subscription lifecycle events in
//! `audit_logs` alongside scrubbed structured stderr metadata. Any metadata
//! field whose key matches sensitive terminology is stripped before persistence.

use serde_json::{Map, Value};
use sqlx::PgPool;

const FORBIDDEN_METADATA_KEYS: &[&str] = &[
    "password",
    "passwd",
    "token",
    "secret",
    "client_secret",
    "authorization",
    "cookie",
    "code_verifier",
    "hash",
    "session",
];

/// Strip any forbidden secret-bearing keys recursively before persisting audit JSON.
pub fn sanitize_metadata(input: Value) -> Value {
    match input {
        Value::Object(map) => {
            let mut clean = Map::new();
            for (k, v) in map {
                let lowered = k.to_ascii_lowercase();
                if FORBIDDEN_METADATA_KEYS
                    .iter()
                    .any(|forbidden| lowered.contains(forbidden))
                {
                    continue;
                }
                clean.insert(k, sanitize_metadata(v));
            }
            Value::Object(clean)
        }
        Value::Array(items) => Value::Array(items.into_iter().map(sanitize_metadata).collect()),
        Value::String(s) => Value::String(crate::scrub::scrub(&s)),
        other => other,
    }
}

/// Write a scrubbed metadata record to `audit_logs` (best-effort on `pool`) and
/// emit the standard scrubbed audit line.
pub async fn record_event(
    pool: &PgPool,
    actor_user_id: Option<&str>,
    subject_id: &str,
    action: &str,
    outcome: &str,
    metadata: Value,
) {
    crate::scrub::audit(action, subject_id, outcome);
    let sanitized = sanitize_metadata(metadata);
    let _ = sqlx::query(
        "INSERT INTO audit_logs (actor_user_id, subject_id, action, outcome, metadata)
         VALUES ($1::uuid, $2, $3, $4, $5)",
    )
    .bind(actor_user_id)
    .bind(subject_id)
    .bind(action)
    .bind(outcome)
    .bind(sanitized)
    .execute(pool)
    .await;
}

/// Persists an audit record inside a caller-owned transaction. Unlike
/// `record_event`, this does not emit a success log before commit and does not
/// swallow storage errors; callers can make the audited mutation atomic.
pub async fn record_event_in_transaction(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    actor_user_id: Option<&str>,
    subject_id: &str,
    action: &str,
    outcome: &str,
    metadata: Value,
) -> Result<(), sqlx::Error> {
    let sanitized = sanitize_metadata(metadata);
    sqlx::query(
        "INSERT INTO audit_logs (actor_user_id, subject_id, action, outcome, metadata)
         VALUES ($1::uuid, $2, $3, $4, $5)",
    )
    .bind(actor_user_id)
    .bind(subject_id)
    .bind(action)
    .bind(outcome)
    .bind(sanitized)
    .execute(&mut **transaction)
    .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn sanitize_metadata_strips_secret_keys_and_scrubs_strings() {
        let raw = json!({
            "planId": "pro",
            "password": "SENTINEL_password",
            "session_token": "SENTINEL_token",
            "nested": {
                "client_secret": "SENTINEL_secret",
                "note": "Authorization: Bearer SENTINEL_bearer"
            }
        });
        let cleaned = sanitize_metadata(raw);
        let text = cleaned.to_string();
        assert!(!text.contains("SENTINEL"));
        assert_eq!(cleaned["planId"], "pro");
    }
}
