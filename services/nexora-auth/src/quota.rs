//! Backend-authoritative daily per-plan, per-model usage limits (Phase 7).
//!
//! Reset rule:
//! - Periods reset strictly at **`00:00:00 UTC`** each calendar day using the
//!   PostgreSQL database clock (`(now() AT TIME ZONE 'UTC')::date`).
//! - Client timestamps, local counters, and client-claimed plan tiers are never
//!   trusted.
//! - Reservations use PostgreSQL row-level locking (`FOR UPDATE`) and unique
//!   `(user_id, request_id)` idempotency keys so concurrent requests or network
//!   retries can never overshoot `daily_limit` or double-charge a single prompt.
//! - Failed or cancelled local generations release their reservation cleanly;
//!   completed generations commit it once. Stale reservations older than
//!   [`RESERVATION_TTL_SECS`] are swept automatically before each check.

use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};

use crate::billing;

/// Unsettled reservations older than 15 minutes are swept automatically so a
/// crashed desktop process cannot hold a slot forever.
pub const RESERVATION_TTL_SECS: i64 = 900;

/// Whether a reviewed server quota policy is active. The database migration
/// and plan catalogue contain proposed values, not approval; production starts
/// unconfigured until an operator-approved policy is wired in deliberately.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuotaPolicy {
    configured: bool,
}

impl QuotaPolicy {
    pub const fn unconfigured() -> Self {
        Self { configured: false }
    }

    /// Intended for a deployment only after quota amounts and entitlement
    /// evidence have been reviewed and approved. Integration tests also use it
    /// with isolated test fixtures.
    pub const fn configured() -> Self {
        Self { configured: true }
    }

    pub const fn is_configured(self) -> bool {
        self.configured
    }
}

impl Default for QuotaPolicy {
    fn default() -> Self {
        Self::unconfigured()
    }
}

/// Canonicalizes desktop model profile identifiers (`nexora-mini-qwen2.5-...` or
/// `nexora-sonnet-qwen2.5-...`) to their canonical quota bucket (`nexora-mini` /
/// `nexora-sonnet`), rejecting any unknown model profile.
pub fn canonical_model_id(raw: &str) -> Option<&'static str> {
    match raw.trim() {
        "nexora-mini" | "nexora-mini-qwen2.5-coder-7b-instruct-q4-k-m" => Some("nexora-mini"),
        "nexora-sonnet" | "nexora-sonnet-qwen2.5-coder-14b-instruct-q4-k-m" => {
            Some("nexora-sonnet")
        }
        _ => None,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ModelQuotaView {
    pub model_id: String,
    pub daily_limit: i32,
    pub used: i32,
    pub reserved: i32,
    pub remaining: i32,
    pub exhausted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UsageSummaryResponse {
    /// False means no usage policy or plan entitlement is being asserted by
    /// this response. Optional values remain absent until policy is configured.
    pub quota_configured: bool,
    pub plan_id: Option<String>,
    pub plan_name: Option<String>,
    pub reset_timezone: Option<&'static str>,
    pub period_day_utc: Option<String>,
    pub reset_at: Option<String>,
    pub resets_at_utc: Option<String>,
    pub reset_at_ms: Option<i64>,
    pub models: Vec<ModelQuotaView>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReserveQuotaRequest {
    pub model_id: String,
    pub request_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct QuotaReservationResponse {
    pub reservation_id: String,
    pub plan_id: String,
    pub model_id: String,
    pub daily_limit: i32,
    pub used: i32,
    pub reserved: i32,
    pub remaining: i32,
    pub remaining_after: i32,
    pub reset_at: String,
    pub resets_at_utc: String,
    pub reset_at_ms: i64,
    pub replayed: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SettleReservationRequest {
    pub reservation_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct QuotaExceededDetail {
    pub plan_id: String,
    pub model_id: String,
    pub daily_limit: i32,
    pub used: i32,
    pub remaining: i32,
    pub reset_at: String,
    pub reset_at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuotaError {
    Validation(String),
    UnknownModel,
    Exceeded(QuotaExceededDetail),
    PolicyUnconfigured,
    IdempotencyConflict,
    ReservationConflict(&'static str),
    ReservationExpired,
    NotFound,
    DependencyUnavailable,
    Internal,
}

impl QuotaError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Validation(_) => "validation_failed",
            Self::UnknownModel => "unknown_model",
            Self::Exceeded(_) => "quota_exceeded",
            Self::PolicyUnconfigured => "quota_policy_unconfigured",
            Self::IdempotencyConflict => "idempotency_conflict",
            Self::ReservationConflict(_) => "reservation_conflict",
            Self::ReservationExpired => "reservation_expired",
            Self::NotFound => "reservation_not_found",
            Self::DependencyUnavailable => "dependency_unavailable",
            Self::Internal => "internal_error",
        }
    }

    pub fn status_code(&self) -> u16 {
        match self {
            Self::Validation(_) | Self::UnknownModel => 400,
            Self::NotFound => 404,
            Self::Exceeded(_) => 429,
            Self::IdempotencyConflict | Self::ReservationConflict(_) | Self::ReservationExpired => 409,
            Self::PolicyUnconfigured | Self::DependencyUnavailable => 503,
            Self::Internal => 500,
        }
    }

    pub fn message(&self) -> String {
        match self {
            Self::Validation(msg) => msg.clone(),
            Self::UnknownModel => "Unrecognized model profile identifier.".to_string(),
            Self::Exceeded(info) => format!(
                "Daily limit of {} messages reached for {} on your {} plan. Resets at {} (00:00 UTC).",
                info.daily_limit, info.model_id, info.plan_id, info.reset_at
            ),
            Self::PolicyUnconfigured => {
                "Daily usage quotas are not configured. Account usage cannot be enforced or reported; local generation remains available.".to_string()
            }
            Self::IdempotencyConflict => {
                "This request identifier is already bound to a different model or plan.".to_string()
            }
            Self::ReservationConflict(reason) => reason.to_string(),
            Self::ReservationExpired => {
                "The quota reservation expired before it could be committed. Start a new generation to obtain a fresh reservation.".to_string()
            }
            Self::NotFound => "Quota reservation was not found.".to_string(),
            Self::DependencyUnavailable => {
                "Quota accounting is unavailable: the account database is not configured.".to_string()
            }
            Self::Internal => "An unexpected quota accounting error occurred.".to_string(),
        }
    }
}

async fn expire_stale_reservations_sql(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: &str,
) -> Result<(), QuotaError> {
    let (expected_groups, updated_groups): (i64, i64) = sqlx::query_as(
        "WITH expired AS (
            UPDATE quota_reservations
            SET status = 'expired', settled_at = now()
            WHERE user_id = $1::uuid
              AND status = 'reserved'
              AND expires_at <= now()
            RETURNING user_id, period_day, model_id
         ),
         grouped AS (
            SELECT user_id, period_day, model_id, count(*)::int AS cnt
            FROM expired
            GROUP BY user_id, period_day, model_id
         ),
         updated AS (
            UPDATE daily_usage_counters c
            SET reserved_count = c.reserved_count - g.cnt,
                updated_at = now()
            FROM grouped g
            WHERE c.user_id = g.user_id
              AND c.period_day = g.period_day
              AND c.model_id = g.model_id
              AND c.reserved_count >= g.cnt
            RETURNING c.user_id
         )
         SELECT (SELECT count(*) FROM grouped), (SELECT count(*) FROM updated)",
    )
    .bind(user_id)
    .fetch_one(&mut **tx)
    .await
    .map_err(|_| QuotaError::Internal)?;
    if expected_groups != updated_groups {
        return Err(QuotaError::Internal);
    }
    Ok(())
}

async fn utc_period_bounds_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<(String, String, i64), QuotaError> {
    let row: (String, String, i64) = sqlx::query_as(
        "SELECT
            to_char((now() AT TIME ZONE 'UTC')::date, 'YYYY-MM-DD'),
            to_char((date_trunc('day', now() AT TIME ZONE 'UTC') + interval '1 day'), 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"'),
            (EXTRACT(EPOCH FROM ((date_trunc('day', now() AT TIME ZONE 'UTC') + interval '1 day') AT TIME ZONE 'UTC')) * 1000)::bigint",
    )
    .fetch_one(&mut **tx)
    .await
    .map_err(|_| QuotaError::Internal)?;
    Ok(row)
}

pub async fn get_usage_summary(
    pool: Option<&PgPool>,
    user_id: &str,
    policy: QuotaPolicy,
) -> Result<UsageSummaryResponse, QuotaError> {
    let Some(pool) = pool else {
        return Err(QuotaError::DependencyUnavailable);
    };

    if !policy.is_configured() {
        // Do not leak proposed DB seed values or imply a Free/Premium entitlement
        // when no quota policy has been approved and enabled.
        return Ok(UsageSummaryResponse {
            quota_configured: false,
            plan_id: None,
            plan_name: None,
            reset_timezone: None,
            period_day_utc: None,
            reset_at: None,
            resets_at_utc: None,
            reset_at_ms: None,
            models: Vec::new(),
        });
    }

    let (plan_id, plan_name) = billing::resolve_effective_plan(pool, user_id)
        .await
        .map_err(|_| QuotaError::Internal)?;

    let mut tx = pool.begin().await.map_err(|_| QuotaError::Internal)?;
    expire_stale_reservations_sql(&mut tx, user_id).await?;
    let (period_day_utc, reset_at, reset_at_ms) = utc_period_bounds_tx(&mut tx).await?;

    let rows = sqlx::query(
        "SELECT q.model_id,
                q.daily_limit,
                COALESCE(c.committed_count, 0) AS committed_count,
                COALESCE(c.reserved_count, 0) AS reserved_count
         FROM plan_model_quotas q
         LEFT JOIN daily_usage_counters c
           ON c.user_id = $1::uuid
          AND c.period_day = (now() AT TIME ZONE 'UTC')::date
          AND c.model_id = q.model_id
         WHERE q.plan_id = $2
         ORDER BY q.model_id ASC",
    )
    .bind(user_id)
    .bind(&plan_id)
    .fetch_all(&mut *tx)
    .await
    .map_err(|_| QuotaError::Internal)?;

    tx.commit().await.map_err(|_| QuotaError::Internal)?;

    let models = rows
        .into_iter()
        .map(|r| {
            let daily_limit: i32 = r.get("daily_limit");
            let committed: i32 = r.get("committed_count");
            let reserved: i32 = r.get("reserved_count");
            let in_use = committed.saturating_add(reserved);
            let remaining = (daily_limit - in_use).max(0);
            ModelQuotaView {
                model_id: r.get("model_id"),
                daily_limit,
                used: in_use.min(daily_limit),
                reserved,
                remaining,
                exhausted: remaining == 0,
            }
        })
        .collect();

    Ok(UsageSummaryResponse {
        quota_configured: true,
        plan_id: Some(plan_id),
        plan_name: Some(plan_name),
        reset_timezone: Some("UTC"),
        period_day_utc: Some(period_day_utc),
        resets_at_utc: Some(reset_at.clone()),
        reset_at: Some(reset_at),
        reset_at_ms: Some(reset_at_ms),
        models,
    })
}

pub async fn reserve_model_quota(
    pool: Option<&PgPool>,
    user_id: &str,
    request: &ReserveQuotaRequest,
    policy: QuotaPolicy,
) -> Result<QuotaReservationResponse, QuotaError> {
    let raw_model_id = request.model_id.trim();
    let request_id = request.request_id.trim();
    if raw_model_id.is_empty() || raw_model_id.len() > 64 {
        return Err(QuotaError::Validation(
            "Model identifier is required.".to_string(),
        ));
    }
    let Some(model_id) = canonical_model_id(raw_model_id) else {
        return Err(QuotaError::UnknownModel);
    };
    if request_id.len() < 6
        || request_id.len() > 128
        || !request_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | ':'))
    {
        return Err(QuotaError::Validation(
            "Request idempotency identifier must be 6–128 URL-safe characters.".to_string(),
        ));
    }
    let Some(pool) = pool else {
        return Err(QuotaError::DependencyUnavailable);
    };
    if !policy.is_configured() {
        return Err(QuotaError::PolicyUnconfigured);
    }

    let (plan_id, _plan_name) = billing::resolve_effective_plan(pool, user_id)
        .await
        .map_err(|_| QuotaError::Internal)?;

    let mut tx = pool.begin().await.map_err(|_| QuotaError::Internal)?;
    expire_stale_reservations_sql(&mut tx, user_id).await?;
    let (period_day_utc, reset_at, reset_at_ms) = utc_period_bounds_tx(&mut tx).await?;

    // Check daily limit for this `(plan_id, model_id)` pair.
    let limit_row: Option<(i32,)> = sqlx::query_as(
        "SELECT daily_limit FROM plan_model_quotas WHERE plan_id = $1 AND model_id = $2",
    )
    .bind(&plan_id)
    .bind(model_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|_| QuotaError::Internal)?;
    let Some((daily_limit,)) = limit_row else {
        return Err(QuotaError::UnknownModel);
    };

    // Ensure today's counter row exists and lock it exclusively for this user + model.
    sqlx::query(
        "INSERT INTO daily_usage_counters (user_id, period_day, model_id, committed_count, reserved_count)
         VALUES ($1::uuid, (now() AT TIME ZONE 'UTC')::date, $2, 0, 0)
         ON CONFLICT (user_id, period_day, model_id) DO NOTHING",
    )
    .bind(user_id)
    .bind(model_id)
    .execute(&mut *tx)
    .await
    .map_err(|_| QuotaError::Internal)?;

    let counter: (i32, i32) = sqlx::query_as(
        "SELECT committed_count, reserved_count
         FROM daily_usage_counters
         WHERE user_id = $1::uuid
           AND period_day = (now() AT TIME ZONE 'UTC')::date
           AND model_id = $2
         FOR UPDATE",
    )
    .bind(user_id)
    .bind(model_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|_| QuotaError::Internal)?;

    // The idempotency key is scoped to the authenticated account. Reuse is
    // allowed only for the same plan/model, same UTC period, and an active
    // reservation. A terminal reservation must never authorize another run.
    if let Some(existing) = sqlx::query(
        "SELECT id::text AS id, plan_id, model_id, status,
                to_char(period_day, 'YYYY-MM-DD') AS period_day
         FROM quota_reservations
         WHERE user_id = $1::uuid AND request_id = $2",
    )
    .bind(user_id)
    .bind(request_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|_| QuotaError::Internal)?
    {
        let existing_plan: String = existing.get("plan_id");
        let existing_model: String = existing.get("model_id");
        let existing_status: String = existing.get("status");
        let existing_period: String = existing.get("period_day");
        if existing_plan != plan_id
            || existing_model != model_id
            || existing_period != period_day_utc
        {
            tx.rollback().await.ok();
            return Err(QuotaError::IdempotencyConflict);
        }
        if existing_status != "reserved" {
            tx.rollback().await.ok();
            return Err(if existing_status == "expired" {
                QuotaError::ReservationExpired
            } else {
                QuotaError::ReservationConflict(
                    "This quota reservation is already settled. Start a new generation with a new request identifier.",
                )
            });
        }

        let in_use = counter.0.saturating_add(counter.1);
        let remaining = (daily_limit - in_use).max(0);
        tx.commit().await.map_err(|_| QuotaError::Internal)?;
        return Ok(QuotaReservationResponse {
            reservation_id: existing.get("id"),
            plan_id: existing_plan,
            model_id: existing_model,
            daily_limit,
            used: in_use.min(daily_limit),
            reserved: counter.1,
            remaining,
            remaining_after: remaining,
            resets_at_utc: reset_at.clone(),
            reset_at,
            reset_at_ms,
            replayed: true,
        });
    }

    let in_use = counter.0.saturating_add(counter.1);
    if in_use >= daily_limit {
        tx.rollback().await.ok();
        return Err(QuotaError::Exceeded(QuotaExceededDetail {
            plan_id,
            model_id: model_id.to_string(),
            daily_limit,
            used: in_use.min(daily_limit),
            remaining: 0,
            reset_at,
            reset_at_ms,
        }));
    }

    let res_row: (String,) = sqlx::query_as(
        "INSERT INTO quota_reservations
         (user_id, request_id, period_day, model_id, plan_id, status, expires_at)
         VALUES (
            $1::uuid, $2, (now() AT TIME ZONE 'UTC')::date, $3, $4,
            'reserved', now() + make_interval(secs => $5)
         )
         RETURNING id::text",
    )
    .bind(user_id)
    .bind(request_id)
    .bind(model_id)
    .bind(&plan_id)
    .bind(RESERVATION_TTL_SECS as f64)
    .fetch_one(&mut *tx)
    .await
    .map_err(|error| {
        if error
            .as_database_error()
            .is_some_and(|database| database.is_unique_violation())
        {
            QuotaError::IdempotencyConflict
        } else {
            QuotaError::Internal
        }
    })?;

    let Some(next_reserved) = counter.1.checked_add(1) else {
        tx.rollback().await.ok();
        return Err(QuotaError::Internal);
    };
    sqlx::query(
        "UPDATE daily_usage_counters
         SET reserved_count = $1, updated_at = now()
         WHERE user_id = $2::uuid
           AND period_day = (now() AT TIME ZONE 'UTC')::date
           AND model_id = $3",
    )
    .bind(next_reserved)
    .bind(user_id)
    .bind(model_id)
    .execute(&mut *tx)
    .await
    .map_err(|_| QuotaError::Internal)?;

    tx.commit().await.map_err(|_| QuotaError::Internal)?;

    let next_in_use = counter.0 + next_reserved;
    let remaining = (daily_limit - next_in_use).max(0);
    Ok(QuotaReservationResponse {
        reservation_id: res_row.0,
        plan_id,
        model_id: model_id.to_string(),
        daily_limit,
        used: next_in_use.min(daily_limit),
        reserved: next_reserved,
        remaining,
        remaining_after: remaining,
        resets_at_utc: reset_at.clone(),
        reset_at,
        reset_at_ms,
        replayed: false,
    })
}

pub async fn settle_reservation(
    pool: Option<&PgPool>,
    user_id: &str,
    reservation_id: &str,
    commit: bool,
    policy: QuotaPolicy,
) -> Result<UsageSummaryResponse, QuotaError> {
    let trimmed = reservation_id.trim();
    if trimmed.len() != 36 {
        return Err(QuotaError::Validation(
            "Malformed reservation identifier.".to_string(),
        ));
    }
    let Some(pool) = pool else {
        return Err(QuotaError::DependencyUnavailable);
    };

    let mut tx = pool.begin().await.map_err(|_| QuotaError::Internal)?;
    // Expire stale rows before deciding whether this commit/release is
    // idempotent. A reservation that crossed its TTL must never commit merely
    // because no later request has swept it yet.
    expire_stale_reservations_sql(&mut tx, user_id).await?;
    let row = sqlx::query(
        "SELECT status, model_id
         FROM quota_reservations
         WHERE id = $1::uuid AND user_id = $2::uuid
         FOR UPDATE",
    )
    .bind(trimmed)
    .bind(user_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|_| QuotaError::Internal)?
    .ok_or(QuotaError::NotFound)?;

    let status: String = row.get("status");
    let model_id: String = row.get("model_id");

    match status.as_str() {
        "reserved" => {
            let next_status = if commit { "committed" } else { "released" };
            let updated = sqlx::query(
                "UPDATE quota_reservations
                 SET status = $1, settled_at = now()
                 WHERE id = $2::uuid AND user_id = $3::uuid AND status = 'reserved'",
            )
            .bind(next_status)
            .bind(trimmed)
            .bind(user_id)
            .execute(&mut *tx)
            .await
            .map_err(|_| QuotaError::Internal)?;
            if updated.rows_affected() != 1 {
                tx.rollback().await.ok();
                return Err(QuotaError::ReservationConflict(
                    "The quota reservation changed state before settlement.",
                ));
            }

            let counter_update = if commit {
                sqlx::query(
                    "UPDATE daily_usage_counters c
                     SET reserved_count = c.reserved_count - 1,
                         committed_count = c.committed_count + 1,
                         updated_at = now()
                     FROM quota_reservations r
                     WHERE r.id = $1::uuid
                       AND r.user_id = $3::uuid
                       AND c.user_id = r.user_id
                       AND c.period_day = r.period_day
                       AND c.model_id = $2
                       AND c.reserved_count > 0",
                )
                .bind(trimmed)
                .bind(&model_id)
                .bind(user_id)
                .execute(&mut *tx)
                .await
            } else {
                sqlx::query(
                    "UPDATE daily_usage_counters c
                     SET reserved_count = c.reserved_count - 1,
                         updated_at = now()
                     FROM quota_reservations r
                     WHERE r.id = $1::uuid
                       AND r.user_id = $3::uuid
                       AND c.user_id = r.user_id
                       AND c.period_day = r.period_day
                       AND c.model_id = $2
                       AND c.reserved_count > 0",
                )
                .bind(trimmed)
                .bind(&model_id)
                .bind(user_id)
                .execute(&mut *tx)
                .await
            }
            .map_err(|_| QuotaError::Internal)?;
            if counter_update.rows_affected() != 1 {
                tx.rollback().await.ok();
                return Err(QuotaError::Internal);
            }
        }
        "committed" if commit => {}
        "released" | "expired" if !commit => {}
        "expired" if commit => {
            tx.commit().await.map_err(|_| QuotaError::Internal)?;
            return Err(QuotaError::ReservationExpired);
        }
        "committed" | "released" => {
            tx.rollback().await.ok();
            return Err(QuotaError::ReservationConflict(
                "A settled quota reservation cannot be changed to the opposite outcome.",
            ));
        }
        _ => {
            tx.rollback().await.ok();
            return Err(QuotaError::Internal);
        }
    }

    tx.commit().await.map_err(|_| QuotaError::Internal)?;
    get_usage_summary(Some(pool), user_id, policy).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::postgres::PgPoolOptions;

    fn unused_lazy_pool() -> PgPool {
        PgPoolOptions::new()
            .acquire_timeout(std::time::Duration::from_millis(50))
            .connect_lazy("postgres://nexora_test:nexora_test@127.0.0.1:1/nexora_test")
            .expect("syntactically valid isolated lazy PostgreSQL URL")
    }

    #[test]
    fn default_quota_policy_is_not_approved_or_configured() {
        assert!(!QuotaPolicy::default().is_configured());
        assert!(!QuotaPolicy::unconfigured().is_configured());
        assert!(QuotaPolicy::configured().is_configured());
    }

    #[tokio::test]
    async fn unconfigured_summary_returns_no_plan_or_quota_without_database_io() {
        let pool = unused_lazy_pool();
        let summary =
            get_usage_summary(Some(&pool), "verified-user-id", QuotaPolicy::unconfigured())
                .await
                .expect("unconfigured policy is an explicit response");

        assert!(!summary.quota_configured);
        assert!(summary.plan_id.is_none());
        assert!(summary.plan_name.is_none());
        assert!(summary.reset_timezone.is_none());
        assert!(summary.period_day_utc.is_none());
        assert!(summary.resets_at_utc.is_none());
        assert!(summary.models.is_empty());
    }

    #[tokio::test]
    async fn unconfigured_reservation_is_a_typed_policy_error_without_database_io() {
        let pool = unused_lazy_pool();
        let request = ReserveQuotaRequest {
            model_id: "nexora-mini-qwen2.5-coder-7b-instruct-q4-k-m".to_string(),
            request_id: "req-policy-unconfigured-01".to_string(),
        };
        let error = reserve_model_quota(
            Some(&pool),
            "verified-user-id",
            &request,
            QuotaPolicy::unconfigured(),
        )
        .await
        .expect_err("no unapproved quota may be enforced");

        assert_eq!(error.code(), "quota_policy_unconfigured");
        assert_eq!(error.status_code(), 503);
    }
}
