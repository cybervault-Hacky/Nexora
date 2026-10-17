use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tauri::ipc::Channel;
use tauri::{AppHandle, State};
use tokio::sync::Notify;

use super::error::{InferenceError, InferenceResult};
use super::hardware::{self, SystemInfo};
use super::models::{self, ModelInfo};
use super::process::{self, RuntimeInfo};
use super::settings::{Settings, DEFAULT_CONTEXT_SIZE};
use super::{
    client, ActiveGeneration, AppStatus, GenerateRequest, GenerationEvent, GenerationOutcome,
    InferenceState,
};

fn lock_error() -> InferenceError {
    InferenceError::internal("The engine state is unavailable. Restart Nexora.")
}

/// Read-only snapshot used by the UI on start and after every action.
#[tauri::command]
pub async fn get_app_status(
    app: AppHandle,
    state: State<'_, InferenceState>,
) -> InferenceResult<AppStatus> {
    Ok(state.status(app.package_info().version.to_string()).await)
}

#[tauri::command]
pub fn get_system_info() -> SystemInfo {
    hardware::system_info()
}

#[tauri::command]
pub fn get_settings(state: State<'_, InferenceState>) -> Settings {
    state.settings_snapshot()
}

/// Persists the user's model/runtime choice after validating both paths.
#[tauri::command]
pub fn save_settings(
    state: State<'_, InferenceState>,
    settings: Settings,
) -> InferenceResult<Settings> {
    let settings = settings.normalised();
    let memory = hardware::system_info().available_ram_gib;

    if let Some(runtime) = settings.runtime_path.as_deref() {
        models::looks_like_executable(runtime)?;
    }

    if let Some(model) = settings.model_path.as_deref() {
        models::inspect(model, settings.context_size, memory)?;
    }

    let mut store = state.settings.lock().map_err(|_| lock_error())?;
    let saved = store.save(settings)?;
    store.settings = saved.clone();

    Ok(saved)
}

/// Header-level validation of a candidate model, surfaced before loading.
#[tauri::command]
pub fn validate_model(
    path: String,
    context_size: Option<u32>,
) -> InferenceResult<ModelInfo> {
    let memory = hardware::system_info().available_ram_gib;

    models::inspect(
        &path,
        context_size.unwrap_or(DEFAULT_CONTEXT_SIZE),
        memory,
    )
}

/// Runs `llama-server --version` so a wrong binary is caught early.
#[tauri::command]
pub async fn probe_runtime(path: String) -> InferenceResult<RuntimeInfo> {
    tauri::async_runtime::spawn_blocking(move || process::probe_runtime(&path))
        .await
        .map_err(|error| InferenceError::internal(error.to_string()))?
}

/// Starts the engine and waits until the model reports loaded.
#[tauri::command]
pub async fn load_model(
    app: AppHandle,
    state: State<'_, InferenceState>,
) -> InferenceResult<AppStatus> {
    let settings = state.settings_snapshot();
    let version = app.package_info().version.to_string();

    let model = settings.model_path.clone().ok_or_else(|| {
        InferenceError::not_configured("Choose a GGUF model file before loading the engine.")
    })?;

    let runtime = settings.runtime_path.clone().ok_or_else(|| {
        InferenceError::not_configured("Choose the llama-server executable before loading.")
    })?;

    // Loading is an explicit user action: always start from a clean process so
    // a changed model or context size actually takes effect.
    let port = {
        let mut supervisor = state.supervisor.lock().map_err(|_| lock_error())?;

        supervisor.start(&runtime, &model, settings.context_size, &state.config_dir)?;

        supervisor
            .endpoint()
            .map(|(port, _)| port)
            .ok_or_else(|| InferenceError::spawn_failed("The engine did not expose a port."))?
    };

    let deadline = Instant::now() + process::LOAD_TIMEOUT;

    loop {
        if process::health_ok(port).await {
            return Ok(state.status(version.clone()).await);
        }

        // A runtime that exits during startup must fail at once with its own
        // message. Without this check a missing or unsupported model looked
        // like a three-minute load and then a generic timeout.
        let failure = {
            let mut supervisor = match state.supervisor.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };

            supervisor.poll_failure()
        };

        if let Some(reason) = failure {
            let message = format!("The local engine could not load the model. {reason}");

            let mut supervisor = match state.supervisor.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };

            supervisor.stop();
            // Remember the human-readable reason so the sidebar reports a real
            // failure until the next successful load.
            supervisor.last_error = Some(message.clone());

            return Err(InferenceError::load_failed(message));
        }

        if Instant::now() >= deadline {
            let diagnostics = {
                let mut supervisor = match state.supervisor.lock() {
                    Ok(guard) => guard,
                    Err(poisoned) => poisoned.into_inner(),
                };

                let diagnostics = supervisor.diagnostics();
                supervisor.stop();

                supervisor.last_error = Some(
                    "The engine did not finish loading the model in time. The model may be too large for this machine."
                        .to_string(),
                );

                diagnostics
            };

            let message = format!(
                "The engine did not finish loading the model in time. The model may be too large for this machine. {diagnostics}"
            )
            .trim()
            .to_string();

            return Err(InferenceError::load_failed(message));
        }

        tokio::time::sleep(Duration::from_millis(300)).await;
    }
}

/// Stops the engine and cancels any active generation.
#[tauri::command]
pub async fn unload_model(
    app: AppHandle,
    state: State<'_, InferenceState>,
) -> InferenceResult<AppStatus> {
    if let Ok(slot) = state.active.lock() {
        if let Some(active) = slot.as_ref() {
            active.cancelled.store(true, std::sync::atomic::Ordering::SeqCst);
            active.cancel.notify_one();
        }
    }

    {
        let mut supervisor = state.supervisor.lock().map_err(|_| lock_error())?;
        supervisor.stop();
        supervisor.last_error = None;
    }

    Ok(state.status(app.package_info().version.to_string()).await)
}

/// Streams a reply from the local model.
///
/// Only one generation may run at a time; a second request is refused with a
/// typed `busy` error rather than queueing behind a model that cannot batch.
#[tauri::command]
pub async fn start_generation(
    state: State<'_, InferenceState>,
    request: GenerateRequest,
    on_event: Channel<GenerationEvent>,
) -> InferenceResult<GenerationOutcome> {
    let validated = request.validate()?;

    let (port, key) = {
        let mut supervisor = state.supervisor.lock().map_err(|_| lock_error())?;

        if !supervisor.is_running() {
            return Err(InferenceError::not_running(
                "Load a local model before sending a message.",
            ));
        }

        supervisor
            .endpoint()
            .ok_or_else(|| InferenceError::not_running("The engine is not ready."))?
    };

    if !process::health_ok(port).await {
        return Err(InferenceError::new(
            "not_ready",
            "The local engine is not responding. Reload the model.",
        ));
    }

    let cancel = Arc::new(Notify::new());
    let cancelled = Arc::new(AtomicBool::new(false));

    {
        let mut slot = state.active.lock().map_err(|_| lock_error())?;

        if slot.is_some() {
            return Err(InferenceError::busy());
        }

        *slot = Some(ActiveGeneration {
            request_id: request.request_id.clone(),
            cancel: Arc::clone(&cancel),
            cancelled: Arc::clone(&cancelled),
        });
    }

    let outcome = client::stream_chat(
        port,
        &key,
        &request.request_id,
        &request.messages,
        validated.max_tokens,
        validated.temperature,
        &on_event,
        cancel,
        cancelled,
    )
    .await;

    state.finish_generation(&request.request_id);

    // Cancellation does not need a request here: dropping the response closes
    // the connection, and b11714 stops generating for that slot on its own.
    // `POST /slots/{id}?action=erase` would return 501 because Nexora does not
    // start the runtime with `--slot-save-path`.
    outcome
}

/// Stops the generation with this id. Returns whether a match was found.
#[tauri::command]
pub fn cancel_generation(state: State<'_, InferenceState>, request_id: String) -> bool {
    state.cancel_generation(&request_id)
}
