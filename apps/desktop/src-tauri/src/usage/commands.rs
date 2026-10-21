use std::time::SystemTime;

use tauri::State;

use super::{UsageState, UsageStatus};

/// The authoritative allowance snapshot.
///
/// The UI renders exactly these numbers; it never derives remaining usage or
/// the reset instant on its own.
#[tauri::command]
pub fn get_usage_status(state: State<'_, UsageState>) -> UsageStatus {
    state.status(SystemTime::now())
}
