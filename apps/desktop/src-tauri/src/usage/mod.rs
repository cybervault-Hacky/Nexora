//! Local generation usage bookkeeping.
//!
//! This counter is device-local and informational; it is not an account,
//! billing, Premium-entitlement, or tamper-resistant authority. No local daily
//! limit is currently configured. A future approved quota must be explicit;
//! an absent value is represented as unconfigured, not as a numerical limit.
//!
//! When the local engine accepts a request and begins streaming, the generation
//! is recorded against the current UTC calendar day. Requests rejected before
//! streaming are released. If the existing record is corrupt or unreadable it
//! is preserved, its count is reported as unknown, and local inference remains
//! available while no local policy is configured.

pub mod commands;
pub mod store;

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

pub use store::Record;

/// Current local usage-record format/meaning. Legacy provisional records are
/// accepted below and their counts are retained rather than reset.
pub const POLICY_ID: &str = "nexora-local-generation-accounting-v2";
const LEGACY_POLICY_ID: &str = "nexora-mini-free-generations-v1";
/// Bumped only when the persisted shape changes; unknown versions are preserved
/// as unusable instead of being silently rewritten.
pub const RECORD_VERSION: u32 = 1;

pub const FILE_NAME: &str = "daily-usage.txt";

const SECONDS_PER_DAY: i64 = 86_400;
const MILLIS_PER_DAY: i64 = 86_400_000;
/// A record may legitimately name tomorrow (a clock near midnight, or a
/// time-zone change). Anything further ahead is corruption, not a real period.
const FUTURE_TOLERANCE_DAYS: i64 = 1;
/// Sanity ceiling for a stored counter; anything above is treated as damage.
const MAX_STORED_USED: u32 = 1_000_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageError {
    pub code: &'static str,
    pub message: String,
}

impl UsageError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn exhausted(used: u32, allowance: u32, reset_at_ms: i64) -> Self {
        Self::new(
            "quota_exhausted",
            format!(
                "The configured local generation allowance is used up ({} of {} generations in this UTC day). It resets at {}.",
                used,
                allowance,
                format_instant(reset_at_ms)
            ),
        )
    }

    pub fn unavailable(detail: impl Into<String>) -> Self {
        Self::new(
            "usage_unavailable",
            format!(
                "Local usage information could not be read or recorded. A configured local limit cannot be checked safely. {}",
                detail.into()
            ),
        )
    }
}

/// The snapshot the UI renders. A missing allowance is explicitly
/// unconfigured; `used: None` means the persisted count could not be trusted.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageStatus {
    pub policy_configured: bool,
    pub allowance: Option<u32>,
    pub used: Option<u32>,
    pub remaining: Option<u32>,
    /// UTC calendar day this snapshot belongs to, `YYYY-MM-DD`.
    pub day: String,
    /// Present only when an actual local quota policy is configured.
    pub reset_at_ms: Option<i64>,
    pub policy_id: Option<String>,
    pub exhausted: bool,
    /// Set when an existing record was preserved but could not be trusted.
    pub notice: Option<String>,
    /// Set when the record cannot be read or written.
    pub error: Option<String>,
}

/// Days since the Unix epoch in UTC. Negative before 1970.
pub fn day_index(now: SystemTime) -> i64 {
    unix_seconds(now).div_euclid(SECONDS_PER_DAY)
}

pub fn unix_seconds(now: SystemTime) -> i64 {
    match now.duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_secs().min(i64::MAX as u64) as i64,
        Err(error) => -(error.duration().as_secs().min(i64::MAX as u64) as i64),
    }
}

pub fn unix_millis(now: SystemTime) -> i64 {
    match now.duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_millis().min(i64::MAX as u128) as i64,
        Err(error) => -(error.duration().as_millis().min(i64::MAX as u128) as i64),
    }
}

fn reset_instant(day: i64) -> i64 {
    day.saturating_add(1).saturating_mul(MILLIS_PER_DAY)
}

/// `YYYY-MM-DD` from days since the epoch (Howard Hinnant's civil algorithm).
pub fn day_label(day: i64) -> String {
    let (year, month, day_of_month) = civil_from_days(day);

    format!("{year:04}-{month:02}-{day_of_month:02}")
}

fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days.saturating_add(719_468);
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let day_of_era = (z - era * 146_097) as u64;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era as i64 + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day_of_month = (day_of_year - (153 * month_prime + 2) / 5 + 1) as u32;
    let month = if month_prime < 10 {
        month_prime + 3
    } else {
        month_prime - 9
    } as u32;

    (
        if month <= 2 { year + 1 } else { year },
        month,
        day_of_month,
    )
}

/// Human-readable instant for error copy. The UI formats its own local time;
/// this is only used inside a message string, so UTC is stated explicitly.
pub fn format_utc_instant(ms: i64) -> String {
    format_instant(ms)
}

fn format_instant(ms: i64) -> String {
    let day = ms.div_euclid(MILLIS_PER_DAY);
    let seconds = ms.rem_euclid(MILLIS_PER_DAY) / 1_000;
    let hour = seconds / 3_600;
    let minute = seconds % 3_600 / 60;

    format!("{} {:02}:{:02} UTC", day_label(day), hour, minute)
}

/// The current UTC-day count from a record. A prior day belongs to a completed
/// period; a future record is unknown without an approved limit, and fails
/// closed when one is explicitly supplied.
fn used_today(
    record_day: i64,
    record_used: u32,
    today: i64,
    allowance: Option<u32>,
) -> Option<u32> {
    if record_day == today {
        Some(record_used)
    } else if record_day > today {
        allowance
    } else {
        Some(0)
    }
}

struct Inner {
    record: Record,
    /// Local generation charges that have been reserved but not settled, keyed
    /// by the UTC day on which each generation was admitted.
    reserved: BTreeMap<i64, u32>,
    notice: Option<String>,
    write_error: Option<String>,
    /// False for unreadable/invalid existing data. Never overwrite such a file
    /// with a fresh zero counter.
    persistence_usable: bool,
}

/// Process-wide local usage bookkeeping. One instance is managed by Tauri and
/// shared by generation and status commands. Production has no local limit.
pub struct UsageState {
    store: store::Store,
    inner: Mutex<Inner>,
    daily_allowance: Option<u32>,
}

impl UsageState {
    /// Reads local usage bookkeeping without enabling a generation limit.
    pub fn load(config_dir: &Path) -> Self {
        Self::load_at(config_dir, SystemTime::now())
    }

    /// Loads the local record at `now`. Production currently has no approved
    /// local daily allowance, so this always records with policy unconfigured.
    pub fn load_at(config_dir: &Path, now: SystemTime) -> Self {
        Self::load_inner(config_dir, now, None)
    }

    #[cfg(test)]
    fn load_with_policy_at(config_dir: &Path, now: SystemTime, allowance: u32) -> Self {
        assert!(allowance > 0, "test policy must use a positive allowance");
        Self::load_inner(config_dir, now, Some(allowance))
    }

    fn load_inner(config_dir: &Path, now: SystemTime, daily_allowance: Option<u32>) -> Self {
        let store = store::Store::new(config_dir.join(FILE_NAME));
        let today = day_index(now);

        let (record, notice, persistence_usable, write_error) = match store.load() {
            store::Loaded::Missing => (fresh(today, 0), None, true, None),
            store::Loaded::Record(record) => match sanitise(record, today) {
                Ok((record, notice)) => (record, notice, true, None),
                Err(reason) => {
                    let detail = format!(
                        "The existing usage record was not trusted ({reason}); it was preserved and the prior count is unknown."
                    );
                    (fresh(today, 0), Some(detail.clone()), false, Some(detail))
                }
            },
            store::Loaded::Unreadable(detail) => {
                let detail = format!(
                    "The existing usage record could not be read ({detail}); it was preserved and the prior count is unknown."
                );
                (fresh(today, 0), Some(detail.clone()), false, Some(detail))
            }
        };

        Self {
            store,
            inner: Mutex::new(Inner {
                record,
                reserved: BTreeMap::new(),
                notice,
                write_error,
                persistence_usable,
            }),
            daily_allowance,
        }
    }

    fn lock(&self) -> Result<MutexGuard<'_, Inner>, UsageError> {
        self.inner
            .lock()
            .map_err(|_| UsageError::unavailable("The usage state is locked by a failed request."))
    }

    /// Local usage snapshot for `now`, without deriving a limit or entitlement.
    pub fn status(&self, now: SystemTime) -> UsageStatus {
        let today = day_index(now);
        let allowance = self.daily_allowance;

        let Ok(mut inner) = self.lock() else {
            return UsageStatus {
                policy_configured: allowance.is_some(),
                allowance,
                used: None,
                remaining: allowance.map(|_| 0),
                day: day_label(today),
                reset_at_ms: allowance.map(|_| reset_instant(today)),
                policy_id: allowance.map(|_| POLICY_ID.to_string()),
                exhausted: allowance.is_some(),
                notice: None,
                error: Some(
                    "The local usage state is unavailable. Local generation remains available while no local limit is configured.".to_string(),
                ),
            };
        };

        // Retry only a previously valid record. An unreadable/corrupt record is
        // deliberately left untouched instead of being overwritten with zero.
        if inner.write_error.is_some() && inner.persistence_usable {
            match self.store.save(&inner.record) {
                Ok(()) => inner.write_error = None,
                Err(error) => inner.write_error = Some(error),
            }
        }

        let used = if inner.persistence_usable {
            used_today(inner.record.day, inner.record.used, today, allowance)
        } else {
            None
        };
        let reserved = inner.reserved.get(&today).copied().unwrap_or(0);
        let remaining = allowance
            .map(|limit| limit.saturating_sub(used.unwrap_or(limit).saturating_add(reserved)));

        UsageStatus {
            policy_configured: allowance.is_some(),
            allowance,
            used,
            remaining,
            day: day_label(today),
            reset_at_ms: allowance.map(|_| reset_instant(today)),
            policy_id: allowance.map(|_| POLICY_ID.to_string()),
            exhausted: allowance.is_some() && remaining == Some(0),
            notice: inner.notice.clone(),
            error: inner.write_error.clone(),
        }
    }

    /// Reserves local usage bookkeeping. With no local policy configured this
    /// never gates inference; a damaged counter remains visibly unknown.
    pub fn reserve(&self, now: SystemTime) -> Result<Charge<'_>, UsageError> {
        let today = day_index(now);
        let mut inner = match self.inner.lock() {
            Ok(inner) => inner,
            Err(_) if self.daily_allowance.is_none() => {
                return Ok(Charge {
                    state: self,
                    day: today,
                    tracking: false,
                    settled: false,
                });
            }
            Err(_) => {
                return Err(UsageError::unavailable(
                    "The configured usage state is locked by a failed request.",
                ));
            }
        };

        if self.daily_allowance.is_some() && !inner.persistence_usable {
            return Err(UsageError::unavailable(
                inner
                    .write_error
                    .clone()
                    .unwrap_or_else(|| "The previous local usage count is unknown.".to_string()),
            ));
        }

        // Retry transient writes, but do not make storage errors a gate when no
        // quota policy is configured. Invalid existing files are never retried
        // or overwritten.
        if inner.write_error.is_some() && inner.persistence_usable {
            match self.store.save(&inner.record) {
                Ok(()) => inner.write_error = None,
                Err(error) => {
                    inner.write_error = Some(error.clone());
                    if self.daily_allowance.is_some() {
                        return Err(UsageError::unavailable(error));
                    }
                }
            }
        }

        let used = if inner.persistence_usable {
            used_today(
                inner.record.day,
                inner.record.used,
                today,
                self.daily_allowance,
            )
        } else {
            None
        };
        let reserved = inner.reserved.get(&today).copied().unwrap_or(0);
        if let Some(allowance) = self.daily_allowance {
            if used.unwrap_or(allowance).saturating_add(reserved) >= allowance {
                return Err(UsageError::exhausted(
                    used.unwrap_or(allowance),
                    allowance,
                    reset_instant(today),
                ));
            }
        }

        let reserved = inner.reserved.entry(today).or_insert(0);
        *reserved = reserved.saturating_add(1);

        Ok(Charge {
            state: self,
            day: today,
            tracking: true,
            settled: false,
        })
    }

    fn release_one(&self, day: i64) {
        if let Ok(mut inner) = self.lock() {
            let remove_entry = if let Some(reserved) = inner.reserved.get_mut(&day) {
                *reserved = reserved.saturating_sub(1);
                *reserved == 0
            } else {
                false
            };
            if remove_entry {
                inner.reserved.remove(&day);
            }
        }
    }

    /// Records one accepted local generation. A damaged/unreadable source file
    /// is left unchanged; the error is surfaced to the usage panel, not used to
    /// block local inference while no local allowance is configured.
    fn record_commit(&self, day: i64) -> Result<(), UsageError> {
        let mut inner = self.lock()?;
        let remove_entry = if let Some(reserved) = inner.reserved.get_mut(&day) {
            *reserved = reserved.saturating_sub(1);
            *reserved == 0
        } else {
            false
        };
        if remove_entry {
            inner.reserved.remove(&day);
        }

        if !inner.persistence_usable {
            return Err(UsageError::unavailable(
                inner
                    .write_error
                    .clone()
                    .unwrap_or_else(|| "The previous local usage count is unknown.".to_string()),
            ));
        }
        if inner.record.day > day {
            let detail = "The stored usage day is ahead of the system date; the existing record was preserved.";
            inner.notice = Some(detail.to_string());
            inner.write_error = Some(detail.to_string());
            return Err(UsageError::unavailable(detail));
        }

        let base = if inner.record.day == day {
            inner.record.used
        } else {
            0
        };
        let record = Record {
            version: RECORD_VERSION,
            policy: POLICY_ID.to_string(),
            day,
            used: base.saturating_add(1).min(MAX_STORED_USED),
            updated_ms: unix_millis(SystemTime::now()),
        };

        match self.store.save(&record) {
            Ok(()) => {
                inner.record = record;
                inner.notice = None;
                inner.write_error = None;
                Ok(())
            }
            Err(error) => {
                // Keep the accepted generation in memory even if persistence fails.
                inner.record = record;
                inner.write_error = Some(error.clone());
                Err(UsageError::unavailable(error))
            }
        }
    }
}

/// A held local usage record. Dropping it without committing releases its
/// in-memory reservation; it never grants or represents an account entitlement.
pub struct Charge<'a> {
    state: &'a UsageState,
    day: i64,
    tracking: bool,
    settled: bool,
}

impl Charge<'_> {
    pub fn commit(mut self) -> Result<(), UsageError> {
        self.settled = true;
        if self.tracking {
            self.state.record_commit(self.day)
        } else {
            Err(UsageError::unavailable(
                "The local usage lock was unavailable; no local policy is configured, so generation remains available.",
            ))
        }
    }

    #[allow(dead_code)]
    pub fn release(mut self) {
        self.settled = true;
        if self.tracking {
            self.state.release_one(self.day);
        }
    }
}

impl std::fmt::Debug for Charge<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Charge")
            .field("day", &self.day)
            .field("tracking", &self.tracking)
            .field("settled", &self.settled)
            .finish()
    }
}

impl Drop for Charge<'_> {
    fn drop(&mut self) {
        if !self.settled && self.tracking {
            self.state.release_one(self.day);
        }
    }
}

fn fresh(day: i64, used: u32) -> Record {
    Record {
        version: RECORD_VERSION,
        policy: POLICY_ID.to_string(),
        day,
        used,
        updated_ms: 0,
    }
}

/// Validates a persisted record without replacing bad data or clamping usage
/// down to a policy number. The prior provisional policy's count is retained as
/// local history because it is still a valid record of generations that ran.
fn sanitise(record: Record, today: i64) -> Result<(Record, Option<String>), String> {
    if record.version != RECORD_VERSION {
        return Err(format!("unsupported version {}", record.version));
    }

    let legacy_policy = record.policy == LEGACY_POLICY_ID;
    if record.policy != POLICY_ID && !legacy_policy {
        return Err("a different usage policy wrote it".to_string());
    }

    if record.day < 0 {
        return Err("the recorded day is not plausible".to_string());
    }

    if record.used > MAX_STORED_USED {
        return Err("the recorded counter is not plausible".to_string());
    }

    if record.day > today.saturating_add(FUTURE_TOLERANCE_DAYS) {
        return Err("the recorded day is implausibly far in the future".to_string());
    }

    let notice = legacy_policy.then(|| {
        "A prior local usage record was retained; no daily generation limit is configured."
            .to_string()
    });
    Ok((
        Record {
            policy: POLICY_ID.to_string(),
            ..record
        },
        notice,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    static NEXT: AtomicU64 = AtomicU64::new(0);

    fn scratch(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "nexora-usage-{label}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn at(day: i64, seconds: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs((day * SECONDS_PER_DAY) as u64 + seconds)
    }

    const TODAY: i64 = 20_746; // 2026-10-20
    const TEST_DAILY_LIMIT: u32 = 3; // Test-only fixture; no production allowance.

    fn state(label: &str, now: SystemTime) -> (UsageState, PathBuf) {
        let dir = scratch(label);
        let state = UsageState::load_with_policy_at(&dir, now, TEST_DAILY_LIMIT);
        assert_eq!(state.status(now).used, Some(0));
        (state, dir)
    }

    #[test]
    fn a_new_install_reports_usage_but_has_no_configured_limit() {
        let now = at(TODAY, 9 * 3_600);
        let dir = scratch("fresh-unconfigured");
        let status = UsageState::load_at(&dir, now).status(now);

        assert!(!status.policy_configured);
        assert_eq!(status.allowance, None);
        assert_eq!(status.used, Some(0));
        assert_eq!(status.remaining, None);
        assert_eq!(status.day, "2026-10-20");
        assert_eq!(status.reset_at_ms, None);
        assert_eq!(status.policy_id, None);
        assert!(!status.exhausted);
        assert!(status.notice.is_none());
        assert!(status.error.is_none());

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn unconfigured_local_usage_does_not_enforce_the_legacy_cutoff() {
        let now = at(TODAY, 9 * 3_600);
        let dir = scratch("unconfigured-many");
        let state = UsageState::load_at(&dir, now);

        // More than the former provisional local cutoff; this is a regression
        // probe only, not a product quota value.
        for _ in 0..60 {
            state
                .reserve(now)
                .expect("unconfigured local generation remains available")
                .commit()
                .expect("usage record remains writable");
        }

        let status = state.status(now);
        assert!(!status.policy_configured);
        assert_eq!(status.allowance, None);
        assert_eq!(status.used, Some(60));
        assert_eq!(status.remaining, None);
        assert!(!status.exhausted);

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_committed_generation_is_counted_once() {
        let now = at(TODAY, 9 * 3_600);
        let (state, dir) = state("count", now);

        let charge = state.reserve(now).expect("reserve");
        charge.commit().expect("commit");

        let status = state.status(now);
        assert_eq!(status.used, Some(1));
        assert_eq!(status.remaining, Some(TEST_DAILY_LIMIT - 1));

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_released_reservation_costs_nothing() {
        let now = at(TODAY, 9 * 3_600);
        let (state, dir) = state("release", now);

        {
            let _charge = state.reserve(now).expect("reserve");
            // Simulates a request that failed before streaming started.
        }

        let status = state.status(now);
        assert_eq!(status.used, Some(0));
        assert_eq!(status.remaining, Some(TEST_DAILY_LIMIT));

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn the_last_unit_is_granted_and_the_next_is_refused() {
        let now = at(TODAY, 9 * 3_600);
        let (state, dir) = state("boundary", now);

        for _ in 0..TEST_DAILY_LIMIT {
            state
                .reserve(now)
                .expect("reserve")
                .commit()
                .expect("commit");
        }

        let status = state.status(now);
        assert_eq!(status.used, Some(TEST_DAILY_LIMIT));
        assert_eq!(status.remaining, Some(0));
        assert!(status.exhausted);

        let error = state.reserve(now).expect_err("exhausted");
        assert_eq!(error.code, "quota_exhausted");
        assert!(error.message.contains("resets at"));

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn concurrent_reservations_cannot_exceed_the_allowance() {
        let now = at(TODAY, 9 * 3_600);
        let (state, dir) = state("race", now);
        let state = Arc::new(state);
        let mut handles = Vec::new();

        for _ in 0..(TEST_DAILY_LIMIT * 4) {
            let state = Arc::clone(&state);
            handles.push(std::thread::spawn(move || match state.reserve(now) {
                Ok(charge) => {
                    charge.commit().expect("commit");
                    1
                }
                Err(error) => {
                    assert_eq!(error.code, "quota_exhausted");
                    0
                }
            }));
        }

        let granted: u32 = handles.into_iter().map(|h| h.join().unwrap()).sum();

        assert_eq!(granted, TEST_DAILY_LIMIT);
        assert_eq!(state.status(now).used, Some(TEST_DAILY_LIMIT));

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn an_uncommitted_reservation_holds_a_unit_until_it_settles() {
        let now = at(TODAY, 9 * 3_600);
        let (state, dir) = state("held", now);

        let charge = state.reserve(now).expect("reserve");
        assert_eq!(state.status(now).remaining, Some(TEST_DAILY_LIMIT - 1));

        charge.commit().expect("commit");
        assert_eq!(state.status(now).remaining, Some(TEST_DAILY_LIMIT - 1));

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn usage_survives_a_same_day_restart() {
        let now = at(TODAY, 9 * 3_600);
        let dir = scratch("restart");
        let first = UsageState::load_with_policy_at(&dir, now, TEST_DAILY_LIMIT);

        for _ in 0..3 {
            first
                .reserve(now)
                .expect("reserve")
                .commit()
                .expect("commit");
        }

        let reopened = UsageState::load_with_policy_at(&dir, now, TEST_DAILY_LIMIT);
        let status = reopened.status(now);

        assert_eq!(status.used, Some(3));
        assert_eq!(status.day, "2026-10-20");

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_new_utc_day_starts_a_fresh_allowance() {
        let yesterday = at(TODAY, 23 * 3_600);
        let dir = scratch("rollover");
        let state = UsageState::load_with_policy_at(&dir, yesterday, TEST_DAILY_LIMIT);

        for _ in 0..TEST_DAILY_LIMIT {
            state
                .reserve(yesterday)
                .expect("reserve")
                .commit()
                .expect("commit");
        }

        let today = at(TODAY + 1, 0);
        let status = state.status(today);

        assert_eq!(status.used, Some(0));
        assert_eq!(status.remaining, Some(TEST_DAILY_LIMIT));
        assert_eq!(status.day, "2026-10-21");
        assert!(!status.exhausted);

        // The first generation of the new day is charged to the new period.
        state
            .reserve(today)
            .expect("reserve")
            .commit()
            .expect("commit");
        assert_eq!(state.status(today).used, Some(1));

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_charge_stays_in_the_period_it_was_reserved_in() {
        let before_midnight = at(TODAY, 23 * 3_600 + 3_500);
        let dir = scratch("crossing");
        let state = UsageState::load_with_policy_at(&dir, before_midnight, TEST_DAILY_LIMIT);

        let charge = state.reserve(before_midnight).expect("reserve");
        let day = charge.day;
        charge.commit().expect("commit");

        assert_eq!(day, TODAY);
        assert_eq!(state.status(at(TODAY, 23 * 3_600 + 3_590)).used, Some(1));
        assert_eq!(state.status(at(TODAY + 1, 5)).used, Some(0));

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn moving_the_clock_back_does_not_hand_out_usage() {
        let now = at(TODAY, 9 * 3_600);
        let dir = scratch("backwards");
        let state = UsageState::load_with_policy_at(&dir, now, TEST_DAILY_LIMIT);

        for _ in 0..TEST_DAILY_LIMIT {
            state
                .reserve(now)
                .expect("reserve")
                .commit()
                .expect("commit");
        }

        let yesterday = at(TODAY - 2, 9 * 3_600);
        let status = state.status(yesterday);

        // Moving the clock back cannot buy usage: today still counts as spent.
        assert_eq!(status.used, Some(TEST_DAILY_LIMIT));
        assert_eq!(status.day, "2026-10-18");
        assert!(status.exhausted);
        assert_eq!(
            state.reserve(yesterday).expect_err("exhausted").code,
            "quota_exhausted"
        );

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_record_that_claims_tomorrow_is_kept_rather_than_reset() {
        let now = at(TODAY, 9 * 3_600);
        let dir = scratch("tomorrow");
        std::fs::write(
            dir.join(FILE_NAME),
            format!(
                "version=1\npolicy={POLICY_ID}\nday={}\nused={TEST_DAILY_LIMIT}\nupdated=0\n",
                TODAY + 1
            ),
        )
        .unwrap();

        let state = UsageState::load_with_policy_at(&dir, now, TEST_DAILY_LIMIT);
        let status = state.status(now);

        assert_eq!(status.used, Some(TEST_DAILY_LIMIT));
        assert!(status.exhausted);
        assert!(status.notice.is_none());

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn an_implausible_future_record_is_preserved_and_fails_closed_with_a_policy() {
        let now = at(TODAY, 9 * 3_600);
        let dir = scratch("future");
        let original = format!(
            "version=1\npolicy={POLICY_ID}\nday={}\nused={TEST_DAILY_LIMIT}\nupdated=0\n",
            TODAY + 4_000
        );
        std::fs::write(dir.join(FILE_NAME), &original).unwrap();

        let state = UsageState::load_with_policy_at(&dir, now, TEST_DAILY_LIMIT);
        let status = state.status(now);

        assert_eq!(status.used, None);
        assert_eq!(status.remaining, Some(0));
        assert!(status.exhausted);
        assert!(status.notice.as_deref().unwrap_or("").contains("future"));
        assert_eq!(
            state.reserve(now).expect_err("unknown count cannot authorize a configured policy").code,
            "usage_unavailable"
        );
        assert_eq!(std::fs::read_to_string(dir.join(FILE_NAME)).unwrap(), original);

        let _ = std::fs::remove_dir_all(dir);
    }

    /// Regression: an invalid counter must not be silently replaced with a
    /// fresh count. Without a configured policy it remains unknown but does not
    /// gate local inference or authorize a reset/write-over of the source file.
    #[test]
    fn a_far_future_record_remains_unknown_without_a_local_limit() {
        let now = at(TODAY, 9 * 3_600);
        let dir = scratch("far-future");
        let original = format!(
            "version=1\npolicy={POLICY_ID}\nday={}\nused=0\nupdated=0\n",
            TODAY + 1_000_000
        );
        std::fs::write(dir.join(FILE_NAME), &original).unwrap();

        let state = UsageState::load_at(&dir, now);
        let status = state.status(now);

        assert!(!status.policy_configured);
        assert_eq!(status.used, None);
        assert_eq!(status.remaining, None);
        assert!(!status.exhausted);
        assert!(status.notice.as_deref().unwrap_or("").contains("future"));

        let charge = state.reserve(now).expect("local generation is not gated");
        assert!(charge.commit().is_err(), "unknown usage must be reported");
        assert_eq!(std::fs::read_to_string(dir.join(FILE_NAME)).unwrap(), original);
        assert_eq!(state.status(now).used, None);

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_counter_above_the_allowance_is_not_clamped_down() {
        let now = at(TODAY, 9 * 3_600);
        let dir = scratch("clamp");
        std::fs::write(
            dir.join(FILE_NAME),
            format!("version=1\npolicy={POLICY_ID}\nday={TODAY}\nused=999\nupdated=0\n"),
        )
        .unwrap();

        let state = UsageState::load_with_policy_at(&dir, now, TEST_DAILY_LIMIT);
        let status = state.status(now);

        assert_eq!(status.used, Some(999));
        assert_eq!(status.remaining, Some(0));
        assert!(status.exhausted);

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_corrupt_file_is_preserved_and_reported_as_unknown() {
        let now = at(TODAY, 9 * 3_600);
        let dir = scratch("corrupt");
        let original = "not a usage record at all";
        std::fs::write(dir.join(FILE_NAME), original).unwrap();

        let state = UsageState::load_with_policy_at(&dir, now, TEST_DAILY_LIMIT);
        let status = state.status(now);

        assert_eq!(status.used, None);
        assert_eq!(status.remaining, Some(0));
        assert!(status.exhausted);
        assert!(status.notice.is_some());
        assert_eq!(
            state.reserve(now).expect_err("cannot safely enforce unknown usage").code,
            "usage_unavailable"
        );
        assert_eq!(std::fs::read_to_string(dir.join(FILE_NAME)).unwrap(), original);

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn unconfigured_corrupt_storage_does_not_block_local_generation_or_get_overwritten() {
        let now = at(TODAY, 9 * 3_600);
        let dir = scratch("corrupt-unconfigured");
        let original = "not a usage record at all";
        std::fs::write(dir.join(FILE_NAME), original).unwrap();

        let state = UsageState::load_at(&dir, now);
        assert_eq!(state.status(now).used, None);
        let charge = state.reserve(now).expect("local inference must remain available");
        assert!(charge.commit().is_err(), "usage recording failure is surfaced");
        assert_eq!(std::fs::read_to_string(dir.join(FILE_NAME)).unwrap(), original);
        assert_eq!(state.status(now).used, None);
        assert!(state.status(now).error.is_some());

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_record_from_another_policy_is_not_trusted_or_replaced() {
        let now = at(TODAY, 9 * 3_600);
        let dir = scratch("policy");
        let original = format!("version=1\npolicy=some-other-policy\nday={TODAY}\nused=7\nupdated=0\n");
        std::fs::write(dir.join(FILE_NAME), &original).unwrap();

        let state = UsageState::load_with_policy_at(&dir, now, TEST_DAILY_LIMIT);
        let status = state.status(now);

        assert_eq!(status.used, None);
        assert!(status.notice.is_some());
        assert_eq!(
            state.reserve(now).expect_err("unknown policy cannot be enforced").code,
            "usage_unavailable"
        );
        assert_eq!(std::fs::read_to_string(dir.join(FILE_NAME)).unwrap(), original);

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_legacy_local_counter_is_retained_without_carrying_forward_its_limit() {
        let now = at(TODAY, 9 * 3_600);
        let dir = scratch("legacy-policy");
        std::fs::write(
            dir.join(FILE_NAME),
            format!("version=1\npolicy={LEGACY_POLICY_ID}\nday={TODAY}\nused=50\nupdated=0\n"),
        )
        .unwrap();

        let state = UsageState::load_at(&dir, now);
        let status = state.status(now);
        assert!(!status.policy_configured);
        assert_eq!(status.allowance, None);
        assert_eq!(status.used, Some(50));
        assert_eq!(status.remaining, None);
        assert!(status.notice.is_some());

        state
            .reserve(now)
            .expect("unconfigured local generation remains available")
            .commit()
            .expect("valid usage history can be updated");
        let stored = std::fs::read_to_string(dir.join(FILE_NAME)).unwrap();
        assert!(stored.contains(&format!("policy={POLICY_ID}")));
        assert!(stored.contains("used=51"));

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn an_unreadable_storage_path_is_unknown_and_fails_closed_when_limited() {
        let now = at(TODAY, 9 * 3_600);
        let parent = scratch("blocked");
        // A file where the directory should be: the record cannot be read/written.
        let blocked = parent.join("blocked");
        std::fs::write(&blocked, b"x").unwrap();

        let state = UsageState::load_with_policy_at(&blocked, now, TEST_DAILY_LIMIT);
        let status = state.status(now);
        assert_eq!(status.used, None);
        assert_eq!(status.remaining, Some(0));
        assert!(status.error.is_some());

        let error = state.reserve(now).expect_err("unknown count must fail closed");
        assert_eq!(error.code, "usage_unavailable");

        let _ = std::fs::remove_dir_all(parent);
    }

    #[test]
    fn utc_day_labels_are_correct_across_month_and_leap_boundaries() {
        assert_eq!(day_label(0), "1970-01-01");
        assert_eq!(day_label(19_782), "2024-02-29");
        assert_eq!(day_label(20_746), "2026-10-20");
        assert_eq!(day_label(20_777), "2026-11-20");
        assert_eq!(day_label(-1), "1969-12-31");
    }

    #[test]
    fn the_reset_instant_is_the_next_utc_midnight() {
        let dir = scratch("instant");
        let status = UsageState::load_with_policy_at(&dir, at(TODAY, 0), TEST_DAILY_LIMIT).status(at(TODAY, 0));

        assert_eq!(status.reset_at_ms, Some((TODAY + 1) * MILLIS_PER_DAY));
        assert_eq!(status.reset_at_ms.unwrap() - TODAY * MILLIS_PER_DAY, MILLIS_PER_DAY);

        let _ = std::fs::remove_dir_all(dir);
    }
}
