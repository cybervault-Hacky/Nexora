//! Daily free usage allowance for local generation.
//!
//! Nexora Mini is local-first: there is no account, no server and no way to
//! verify anything remotely. The allowance is therefore a *local product
//! policy*, recorded on this machine and enforced in the Rust command layer.
//!
//! ## What is counted
//!
//! One **generation** — a chat reply, a patch proposal or a patch repair, all
//! of which go through `start_generation`. The unit is deliberately not a
//! token count: the streamed OpenAI-compatible response Nexora reads does not
//! carry per-request token usage, and estimating tokens from characters and
//! calling them tokens would be dishonest. A generation is either counted or
//! it is not, and both the counter and the tests can prove which happened.
//!
//! ## When it is counted
//!
//! A unit is charged only when the local engine accepted the request and began
//! streaming. A click that never reaches the model, a missing model, an
//! unreachable engine, a busy engine or a request the engine rejects costs
//! nothing. Once streaming has started the generation is charged even if the
//! user stops it or it times out, because the model really did run. The charge
//! is held as a reservation first, so two concurrent requests can never both
//! spend the last unit.
//!
//! ## When it resets
//!
//! At 00:00 UTC. The boundary is computed from the system clock every time it
//! is needed — never from a stored countdown — so restarts, sleep and clock
//! changes cannot make the app claim a reset that has not happened. A local
//! midnights reset would need the OS time zone; the Rust side deliberately
//! does not link a time-zone database in this phase, so the policy is UTC and
//! the UI shows the resulting instant in the user's own clock.

pub mod commands;
pub mod store;

use std::path::Path;
use std::sync::{Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

pub use store::Record;

/// Identifies the accounting rules, so a future change cannot silently reuse
/// old counters under a new meaning.
pub const POLICY_ID: &str = "nexora-mini-free-generations-v1";
/// Bumped only when the persisted shape changes; unknown versions are reset.
pub const RECORD_VERSION: u32 = 1;
/// **Provisional.** The user has not approved a final number. 50 is chosen as
/// a generous single-developer day that is still a real limit, and it sits in
/// one constant so a later product decision changes nothing else.
pub const DAILY_ALLOWANCE: u32 = 50;
pub const ALLOWANCE_IS_PROVISIONAL: bool = true;

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
                "The daily free allowance is used up ({} of {} generations today). It resets at {}.",
                used.min(allowance),
                allowance,
                format_instant(reset_at_ms)
            ),
        )
    }

    pub fn unavailable(detail: impl Into<String>) -> Self {
        Self::new(
            "usage_unavailable",
            format!(
                "Nexora could not read or record the daily usage allowance, so it will not start a generation it cannot account for. {}",
                detail.into()
            ),
        )
    }
}

/// The snapshot the UI renders. Every field is authoritative: the frontend
/// never computes remaining usage itself.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageStatus {
    pub allowance: u32,
    pub used: u32,
    pub remaining: u32,
    /// UTC calendar day this snapshot belongs to, `YYYY-MM-DD`.
    pub day: String,
    pub reset_at_ms: i64,
    pub policy_id: &'static str,
    pub provisional: bool,
    pub exhausted: bool,
    /// Set when a damaged record was repaired, so the UI can say so.
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
fn format_instant(ms: i64) -> String {
    let day = ms.div_euclid(MILLIS_PER_DAY);
    let seconds = ms.rem_euclid(MILLIS_PER_DAY) / 1_000;
    let hour = seconds / 3_600;
    let minute = seconds % 3_600 / 60;

    format!("{} {:02}:{:02} UTC", day_label(day), hour, minute)
}

/// How much of today's allowance a record says is already spent.
///
/// A record dated before today belongs to a finished period, so today starts
/// empty. A record dated *after* today can only exist because the clock moved
/// backwards, so today counts as used up: moving the clock is never a way to
/// get more generations. Nothing here writes to disk; the rule is applied every
/// time status or a reservation is computed, including if the clock changes
/// while the app is running.
fn used_today(record_day: i64, record_used: u32, today: i64) -> u32 {
    if record_day == today {
        record_used.min(DAILY_ALLOWANCE)
    } else if record_day > today {
        DAILY_ALLOWANCE
    } else {
        0
    }
}

struct Inner {
    record: Record,
    /// Charges that have been approved but have not finished starting yet.
    reserved: u32,
    notice: Option<String>,
    write_error: Option<String>,
}

/// Process-wide allowance state. One instance is managed by Tauri and shared by
/// the generation command and the status command, so there is exactly one
/// counter — no second tracking system and no frontend copy.
pub struct UsageState {
    store: store::Store,
    inner: Mutex<Inner>,
}

impl UsageState {
    /// Reads the record once at startup. An unreadable or invalid file never
    /// stops the app from starting; it is repaired to a fresh period and the
    /// repair is reported to the UI instead of being hidden.
    pub fn load(config_dir: &Path) -> Self {
        Self::load_at(config_dir, SystemTime::now())
    }

    /// Loads the record as it would be seen at `now`.
    ///
    /// `load` is this with the system clock. Splitting them keeps the record's
    /// "today" pinned to the same instant the caller is reasoning about, so a
    /// test that crosses a day boundary cannot accidentally observe a record
    /// dated ahead of its own clock.
    pub fn load_at(config_dir: &Path, now: SystemTime) -> Self {
        let store = store::Store::new(config_dir.join(FILE_NAME));
        let today = day_index(now);

        let (record, notice) = match store.load() {
            store::Loaded::Missing => (fresh(today, 0), None),
            store::Loaded::Record(record) => match sanitise(record, today) {
                Ok((record, notice)) => (record, notice),
                Err(reason) => (
                    fresh(today, 0),
                    Some(format!(
                        "The stored usage record was unusable ({reason}), so today's allowance was restarted."
                    )),
                ),
            },
            store::Loaded::Unreadable(detail) => (
                fresh(today, 0),
                Some(format!(
                    "The stored usage record could not be read ({detail}), so today's allowance was restarted."
                )),
            ),
        };

        Self {
            store,
            inner: Mutex::new(Inner {
                record,
                reserved: 0,
                notice,
                write_error: None,
            }),
        }
    }

    fn lock(&self) -> Result<MutexGuard<'_, Inner>, UsageError> {
        self.inner
            .lock()
            .map_err(|_| UsageError::unavailable("The usage state is locked by a failed request."))
    }

    /// The allowance snapshot for `now`, rolled to the current period without
    /// writing anything.
    pub fn status(&self, now: SystemTime) -> UsageStatus {
        let today = day_index(now);

        let Ok(mut inner) = self.lock() else {
            return UsageStatus {
                allowance: DAILY_ALLOWANCE,
                used: DAILY_ALLOWANCE,
                remaining: 0,
                day: day_label(today),
                reset_at_ms: reset_instant(today),
                policy_id: POLICY_ID,
                provisional: ALLOWANCE_IS_PROVISIONAL,
                exhausted: true,
                notice: None,
                error: Some(
                    "The daily usage state is unavailable. Restart Nexora to restore it."
                        .to_string(),
                ),
            };
        };

        // Retry a failed write when the UI asks for status: that is the natural
        // moment to recover, and it keeps a transient disk error from blocking
        // the rest of the day.
        if inner.write_error.is_some() {
            match self.store.save(&inner.record) {
                Ok(()) => inner.write_error = None,
                Err(error) => inner.write_error = Some(error),
            }
        }

        let used = used_today(inner.record.day, inner.record.used, today);
        let reserved = inner.reserved.min(DAILY_ALLOWANCE.saturating_sub(used));
        let remaining = DAILY_ALLOWANCE.saturating_sub(used.saturating_add(reserved));

        UsageStatus {
            allowance: DAILY_ALLOWANCE,
            used,
            remaining,
            day: day_label(today),
            reset_at_ms: reset_instant(today),
            policy_id: POLICY_ID,
            provisional: ALLOWANCE_IS_PROVISIONAL,
            exhausted: remaining == 0,
            notice: inner.notice.clone(),
            error: inner.write_error.clone(),
        }
    }

    /// Approves one generation, or refuses it. The returned [`Charge`] must be
    /// committed once streaming has started, or dropped/released if it never
    /// did.
    pub fn reserve(&self, now: SystemTime) -> Result<Charge<'_>, UsageError> {
        let today = day_index(now);
        let mut inner = self.lock()?;

        // Fail closed while a previous charge could not be persisted, but retry
        // first so a transient error heals without a restart.
        if inner.write_error.is_some() {
            match self.store.save(&inner.record) {
                Ok(()) => inner.write_error = None,
                Err(error) => {
                    inner.write_error = Some(error.clone());
                    return Err(UsageError::unavailable(error));
                }
            }
        }

        let used = used_today(inner.record.day, inner.record.used, today);
        let day = today;

        if used.saturating_add(inner.reserved) >= DAILY_ALLOWANCE {
            return Err(UsageError::exhausted(
                used,
                DAILY_ALLOWANCE,
                reset_instant(day),
            ));
        }

        inner.reserved = inner.reserved.saturating_add(1);

        Ok(Charge {
            state: self,
            day,
            settled: false,
        })
    }

    fn release_one(&self) {
        if let Ok(mut inner) = self.lock() {
            inner.reserved = inner.reserved.saturating_sub(1);
        }
    }

    /// Charges exactly one unit for `day` and persists it. The in-memory
    /// counter is trusted even if the write fails, so the running session stays
    /// limited; the failure is reported and blocks further charges until it is
    /// resolved.
    fn record_commit(&self, day: i64) -> Result<(), UsageError> {
        let mut inner = self.lock()?;
        inner.reserved = inner.reserved.saturating_sub(1);

        let base = if inner.record.day == day {
            inner.record.used.min(DAILY_ALLOWANCE)
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
                // Keep the charge in memory: the generation really happened.
                inner.record = record;
                inner.write_error = Some(error.clone());
                Err(UsageError::unavailable(error))
            }
        }
    }
}

/// A held unit. Dropping it without committing releases it, so an early return
/// can never leak a reservation.
pub struct Charge<'a> {
    state: &'a UsageState,
    day: i64,
    settled: bool,
}

impl Charge<'_> {
    pub fn day(&self) -> i64 {
        self.day
    }

    pub fn commit(mut self) -> Result<(), UsageError> {
        self.settled = true;
        self.state.record_commit(self.day)
    }

    #[allow(dead_code)]
    pub fn release(mut self) {
        self.settled = true;
        self.state.release_one();
    }
}

impl std::fmt::Debug for Charge<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Charge")
            .field("day", &self.day)
            .field("settled", &self.settled)
            .finish()
    }
}

impl Drop for Charge<'_> {
    fn drop(&mut self) {
        if !self.settled {
            self.state.release_one();
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

/// Rejects anything a different policy, a future version or a damaged file
/// could mean. `used` above the allowance is clamped rather than reset: a
/// malformed counter must never hand out a fresh allowance.
///
/// A date further ahead than the tolerance is damage — left alone it would lock
/// the allowance for years — so it is repaired to *today, fully used*. That
/// keeps the repair from becoming a free reset. The rule applies however far
/// ahead the stored day is: only a negative day is rejected outright, because
/// there is no plausible clock that produces one.
fn sanitise(record: Record, today: i64) -> Result<(Record, Option<String>), String> {
    if record.version != RECORD_VERSION {
        return Err(format!("unsupported version {}", record.version));
    }

    if record.policy != POLICY_ID {
        return Err("a different usage policy wrote it".to_string());
    }

    if record.day < 0 {
        return Err("the recorded day is not plausible".to_string());
    }

    if record.used > MAX_STORED_USED {
        return Err("the recorded counter is not plausible".to_string());
    }

    if record.day > today + FUTURE_TOLERANCE_DAYS {
        return Ok((
            Record {
                day: today,
                used: DAILY_ALLOWANCE,
                ..record
            },
            Some(
                "The stored usage record was dated in the future, so today's allowance is treated as used."
                    .to_string(),
            ),
        ));
    }

    Ok((
        Record {
            used: record.used.min(DAILY_ALLOWANCE),
            ..record
        },
        None,
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

    fn state(label: &str, now: SystemTime) -> (UsageState, PathBuf) {
        let dir = scratch(label);
        let state = UsageState::load_at(&dir, now);
        assert_eq!(state.status(now).used, 0);
        (state, dir)
    }

    #[test]
    fn a_new_install_has_the_full_allowance() {
        let now = at(TODAY, 9 * 3_600);
        let (state, dir) = state("fresh", now);
        let status = state.status(now);

        assert_eq!(status.allowance, DAILY_ALLOWANCE);
        assert_eq!(status.used, 0);
        assert_eq!(status.remaining, DAILY_ALLOWANCE);
        assert!(!status.exhausted);
        assert_eq!(status.day, "2026-10-20");
        assert_eq!(status.reset_at_ms, (TODAY + 1) * MILLIS_PER_DAY);
        assert_eq!(status.policy_id, POLICY_ID);
        assert!(status.provisional);
        assert!(status.notice.is_none());
        assert!(status.error.is_none());

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_committed_generation_is_counted_once() {
        let now = at(TODAY, 9 * 3_600);
        let (state, dir) = state("count", now);

        let charge = state.reserve(now).expect("reserve");
        charge.commit().expect("commit");

        let status = state.status(now);
        assert_eq!(status.used, 1);
        assert_eq!(status.remaining, DAILY_ALLOWANCE - 1);

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
        assert_eq!(status.used, 0);
        assert_eq!(status.remaining, DAILY_ALLOWANCE);

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn the_last_unit_is_granted_and_the_next_is_refused() {
        let now = at(TODAY, 9 * 3_600);
        let (state, dir) = state("boundary", now);

        for _ in 0..DAILY_ALLOWANCE {
            state
                .reserve(now)
                .expect("reserve")
                .commit()
                .expect("commit");
        }

        let status = state.status(now);
        assert_eq!(status.used, DAILY_ALLOWANCE);
        assert_eq!(status.remaining, 0);
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

        for _ in 0..(DAILY_ALLOWANCE * 4) {
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

        assert_eq!(granted, DAILY_ALLOWANCE);
        assert_eq!(state.status(now).used, DAILY_ALLOWANCE);

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn an_uncommitted_reservation_holds_a_unit_until_it_settles() {
        let now = at(TODAY, 9 * 3_600);
        let (state, dir) = state("held", now);

        let charge = state.reserve(now).expect("reserve");
        assert_eq!(state.status(now).remaining, DAILY_ALLOWANCE - 1);

        charge.commit().expect("commit");
        assert_eq!(state.status(now).remaining, DAILY_ALLOWANCE - 1);

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn usage_survives_a_same_day_restart() {
        let now = at(TODAY, 9 * 3_600);
        let dir = scratch("restart");
        let first = UsageState::load_at(&dir, now);

        for _ in 0..3 {
            first
                .reserve(now)
                .expect("reserve")
                .commit()
                .expect("commit");
        }

        let reopened = UsageState::load_at(&dir, now);
        let status = reopened.status(now);

        assert_eq!(status.used, 3);
        assert_eq!(status.day, "2026-10-20");

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_new_utc_day_starts_a_fresh_allowance() {
        let yesterday = at(TODAY, 23 * 3_600);
        let dir = scratch("rollover");
        let state = UsageState::load_at(&dir, yesterday);

        for _ in 0..DAILY_ALLOWANCE {
            state
                .reserve(yesterday)
                .expect("reserve")
                .commit()
                .expect("commit");
        }

        let today = at(TODAY + 1, 0);
        let status = state.status(today);

        assert_eq!(status.used, 0);
        assert_eq!(status.remaining, DAILY_ALLOWANCE);
        assert_eq!(status.day, "2026-10-21");
        assert!(!status.exhausted);

        // The first generation of the new day is charged to the new period.
        state
            .reserve(today)
            .expect("reserve")
            .commit()
            .expect("commit");
        assert_eq!(state.status(today).used, 1);

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_charge_stays_in_the_period_it_was_reserved_in() {
        let before_midnight = at(TODAY, 23 * 3_600 + 3_500);
        let dir = scratch("crossing");
        let state = UsageState::load_at(&dir, before_midnight);

        let charge = state.reserve(before_midnight).expect("reserve");
        let day = charge.day();
        charge.commit().expect("commit");

        assert_eq!(day, TODAY);
        assert_eq!(state.status(at(TODAY, 23 * 3_600 + 3_590)).used, 1);
        assert_eq!(state.status(at(TODAY + 1, 5)).used, 0);

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn moving_the_clock_back_does_not_hand_out_usage() {
        let now = at(TODAY, 9 * 3_600);
        let dir = scratch("backwards");
        let state = UsageState::load_at(&dir, now);

        for _ in 0..DAILY_ALLOWANCE {
            state
                .reserve(now)
                .expect("reserve")
                .commit()
                .expect("commit");
        }

        let yesterday = at(TODAY - 2, 9 * 3_600);
        let status = state.status(yesterday);

        // Moving the clock back cannot buy usage: today still counts as spent.
        assert_eq!(status.used, DAILY_ALLOWANCE);
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
                "version=1\npolicy={POLICY_ID}\nday={}\nused={DAILY_ALLOWANCE}\nupdated=0\n",
                TODAY + 1
            ),
        )
        .unwrap();

        let state = UsageState::load_at(&dir, now);
        let status = state.status(now);

        assert_eq!(status.used, DAILY_ALLOWANCE);
        assert!(status.exhausted);
        assert!(status.notice.is_none());

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn an_implausible_future_record_is_repaired_and_reported() {
        let now = at(TODAY, 9 * 3_600);
        let dir = scratch("future");
        std::fs::write(
            dir.join(FILE_NAME),
            format!(
                "version=1\npolicy={POLICY_ID}\nday={}\nused={DAILY_ALLOWANCE}\nupdated=0\n",
                TODAY + 4_000
            ),
        )
        .unwrap();

        let state = UsageState::load_at(&dir, now);
        let status = state.status(now);

        // A repair must not become a free reset.
        assert_eq!(status.used, DAILY_ALLOWANCE);
        assert!(status.exhausted);
        assert!(status.notice.is_some());
        assert!(status.notice.unwrap().contains("future"));

        let _ = std::fs::remove_dir_all(dir);
    }

    /// Regression: a record dated far beyond the old plausibility gate used to
    /// fall through to the "unreadable" path, which restarts the period with a
    /// full allowance. A tampered date must never be worth more than an
    /// exhausted day. (Phase 8.)
    #[test]
    fn a_far_future_record_is_not_a_free_reset() {
        let now = at(TODAY, 9 * 3_600);
        let dir = scratch("far-future");
        std::fs::write(
            dir.join(FILE_NAME),
            format!(
                "version=1\npolicy={POLICY_ID}\nday={}\nused=0\nupdated=0\n",
                TODAY + 1_000_000
            ),
        )
        .unwrap();

        let state = UsageState::load_at(&dir, now);
        let status = state.status(now);

        assert_eq!(status.used, DAILY_ALLOWANCE);
        assert_eq!(status.remaining, 0);
        assert!(status.exhausted);
        assert!(status.notice.as_deref().unwrap_or("").contains("future"));

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_counter_above_the_allowance_is_clamped_not_reset() {
        let now = at(TODAY, 9 * 3_600);
        let dir = scratch("clamp");
        std::fs::write(
            dir.join(FILE_NAME),
            format!("version=1\npolicy={POLICY_ID}\nday={TODAY}\nused=999\nupdated=0\n"),
        )
        .unwrap();

        let state = UsageState::load_at(&dir, now);
        let status = state.status(now);

        assert_eq!(status.used, DAILY_ALLOWANCE);
        assert_eq!(status.remaining, 0);
        assert!(status.exhausted);

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_corrupt_file_is_repaired_and_reported() {
        let now = at(TODAY, 9 * 3_600);
        let dir = scratch("corrupt");
        std::fs::write(dir.join(FILE_NAME), "not a usage record at all").unwrap();

        let state = UsageState::load_at(&dir, now);
        let status = state.status(now);

        assert_eq!(status.used, 0);
        assert_eq!(status.remaining, DAILY_ALLOWANCE);
        assert!(status.notice.is_some());

        // The repair is written on the next charge, so it does not recur.
        state
            .reserve(now)
            .expect("reserve")
            .commit()
            .expect("commit");
        let reopened = UsageState::load_at(&dir, now);
        assert_eq!(reopened.status(now).used, 1);
        assert!(reopened.status(now).notice.is_none());

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_record_from_another_policy_is_not_trusted() {
        let now = at(TODAY, 9 * 3_600);
        let dir = scratch("policy");
        std::fs::write(
            dir.join(FILE_NAME),
            format!("version=1\npolicy=some-other-policy\nday={TODAY}\nused=7\nupdated=0\n"),
        )
        .unwrap();

        let state = UsageState::load_at(&dir, now);
        let status = state.status(now);

        assert_eq!(status.used, 0);
        assert!(status.notice.is_some());

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn an_unwritable_record_fails_closed_without_losing_the_charge() {
        let now = at(TODAY, 9 * 3_600);
        let parent = scratch("blocked");
        // A file where the directory should be: every create/write fails.
        let blocked = parent.join("blocked");
        std::fs::write(&blocked, b"x").unwrap();

        let state = UsageState::load_at(&blocked, now);
        assert_eq!(state.status(now).used, 0);

        let charge = state.reserve(now).expect("reserve");
        assert!(charge.commit().is_err());

        let status = state.status(now);
        assert_eq!(
            status.used, 1,
            "the running session still counts the charge"
        );
        assert!(status.error.is_some(), "the write failure is reported");

        let error = state.reserve(now).expect_err("fail closed");
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
        let status = UsageState::load_at(&dir, at(TODAY, 0)).status(at(TODAY, 0));

        assert_eq!(status.reset_at_ms, (TODAY + 1) * MILLIS_PER_DAY);
        assert_eq!(status.reset_at_ms - TODAY * MILLIS_PER_DAY, MILLIS_PER_DAY);

        let _ = std::fs::remove_dir_all(dir);
    }
}
