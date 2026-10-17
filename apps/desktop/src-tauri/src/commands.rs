use serde::Serialize;
use tauri::Manager;

/// The honest state of local inference in this build.
///
/// `Ready` is intentionally absent: the supervisor does not exist yet, so the
/// core must not be able to report a state it cannot reach. The frontend type
/// accepts `"ready"` so a later phase can add it without a wire break.
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineState {
    NotConnected,
}

/// Read-only engine snapshot returned to the UI.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineStatus {
    pub state: EngineState,
    pub runtime: &'static str,
    pub model: Option<String>,
    pub detail: &'static str,
}

/// Read-only application snapshot returned to the UI.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppStatus {
    pub name: &'static str,
    pub version: String,
    pub engine: EngineStatus,
}

/// Returns the application version and the current engine state.
///
/// This command is intentionally narrow: it reads metadata from the running
/// app handle and returns a static description of the engine. It cannot read
/// files, spawn processes, or mutate state, and therefore needs no capability
/// beyond the Tauri command bridge.
#[tauri::command]
pub fn get_app_status(app: tauri::AppHandle) -> AppStatus {
    AppStatus {
        name: "Nexora",
        version: app.package_info().version.to_string(),
        engine: EngineStatus {
            state: EngineState::NotConnected,
            runtime: "llama.cpp (planned)",
            model: None,
            detail: "No local model is loaded. Replies are scaffold text, not model output.",
        },
    }
}
