use serde::Serialize;
use tauri::Manager;

/// The honest state of local inference in this build.
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
    pub inference_available: bool,
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
/// files, spawn processes, or mutate state, and therefore needs no extra
/// capability beyond the Tauri command bridge.
#[tauri::command]
pub fn get_app_status(app: tauri::AppHandle) -> AppStatus {
    AppStatus {
        name: "Nexora",
        version: app.package_info().version.to_string(),
        engine: EngineStatus {
            state: EngineState::NotConnected,
            inference_available: false,
            runtime: "llama.cpp (planned)",
            model: None,
            detail: "The local inference engine is not part of this foundation build. Replies are labelled scaffold text, not model output.",
        },
    }
}
