use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use serde::Serialize;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};
use tokio::sync::Notify;

use super::acquisition::{self, DownloadEvent, DownloadOutcome, IntegrityReport};
use super::budget;
use super::error::{InferenceError, InferenceResult};
use super::hardware::{self, SystemInfo};
use super::loading;
use super::models::{self, ModelInfo};
use super::process::{self, RuntimeInfo};
use super::readiness::{
    self, DiskStatus, EngineFacts, FileFacts, MemoryStatus, Readiness, ReadinessInput,
    ReadinessState,
};
use super::registry::{self, ModelProfile};
use super::settings::{Settings, DEFAULT_CONTEXT_SIZE};
use super::{
    client, ActiveGeneration, AppStatus, GenerateRequest, GenerationEvent, GenerationOutcome,
    InferenceState, LoadedModel,
};
use crate::usage::{UsageError, UsageState};

fn lock_error() -> InferenceError {
    InferenceError::internal("The engine state is unavailable. Restart Nexora.")
}

/// Refuses a profile change while the engine is answering or loading.
///
/// Both the chat selector and the settings form come through here, so there is
/// one rule rather than two that can drift apart.
fn ensure_switch_allowed(state: &InferenceState, target: &ModelProfile) -> InferenceResult<()> {
    let generating = state
        .active
        .lock()
        .map(|slot| slot.is_some())
        .unwrap_or(false);

    registry::check_switch(target, generating, state.load_in_flight())
}

impl From<UsageError> for InferenceError {
    fn from(error: UsageError) -> Self {
        InferenceError::new(error.code, error.message)
    }
}

/// The folder a download lands in when the user does not pick one.
fn default_model_dir(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_data_dir().ok().map(|dir| dir.join("models"))
}

/// Bytes already written to an unfinished `.part` file for this profile.
///
/// Partial files are named `.{file_name}.{random}.part` by the downloader. They
/// are reported so the UI can say a download was interrupted, and they are
/// never treated as an installed model.
fn partial_bytes(directory: &Path, file_name: &str) -> Option<u64> {
    let entries = std::fs::read_dir(directory).ok()?;
    let prefix = format!(".{file_name}.");
    let mut total = 0u64;
    let mut found = false;

    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };

        if name.starts_with(&prefix) && name.ends_with(".part") {
            if let Ok(metadata) = entry.metadata() {
                total = total.saturating_add(metadata.len());
                found = true;
            }
        }
    }

    found.then_some(total)
}

/// The installed copy of a profile, if one exists at the published name and
/// exact length. Shared by readiness and the catalogue so the two rows can
/// never disagree about whether a file is installed.
fn installed_for(
    profile: &ModelProfile,
    settings: &Settings,
    directory: Option<&Path>,
) -> Option<PathBuf> {
    let mut candidates = Vec::new();

    if let Some(path) = settings.model_path.as_deref() {
        candidates.push(PathBuf::from(path));
    }

    if let (Some(directory), Some(source)) = (directory, profile.artifact.source()) {
        candidates.push(directory.join(source.file_name));
    }

    acquisition::installed_path(profile, &candidates)
}

/// Builds the authoritative readiness for one profile.
///
/// Reads real system values and real file state; it never downloads, loads, or
/// launches anything. Unknown measurements stay unknown. The only change it can
/// make is reaping an engine process that has already exited.
async fn readiness_for(
    state: &InferenceState,
    profile: &'static ModelProfile,
    settings: &Settings,
    directory: Option<&Path>,
) -> Readiness {
    // A child that died since the last snapshot makes the "loaded" mark stale.
    // Reap it once here so both the rows and the engine panel agree.
    state.reconcile_engine();

    let memory = hardware::system_info();

    let memory_status = match (memory.total_ram_gib, memory.available_ram_gib) {
        (Some(total_gib), Some(available_gib)) => MemoryStatus::Known {
            total_gib,
            available_gib,
        },
        // Either half missing means the picture is incomplete; treat it as
        // unknown rather than pairing a real total with a guessed available.
        _ => MemoryStatus::Unknown,
    };

    let disk_status = match directory.and_then(|path| hardware::disk_space(path).free_bytes) {
        Some(free) => DiskStatus::Known(free),
        None => DiskStatus::Unknown,
    };

    let installed = installed_for(profile, settings, directory);

    // A same-named file of a different length at the destination is not the
    // approved artefact. Nexora refuses to overwrite it, so readiness must say
    // so instead of offering a download that would fail immediately.
    let wrong_size_bytes = match (directory, profile.artifact.source(), installed.as_ref()) {
        (Some(directory), Some(source), None) => {
            std::fs::metadata(directory.join(source.file_name))
                .ok()
                .map(|metadata| metadata.len())
                .filter(|bytes| *bytes != source.size_bytes)
        }
        _ => None,
    };

    // A failure recorded earlier only applies while the offending file exists.
    // Removing it must clear the block, otherwise there is no way back.
    let (verified, failed) = state.verification_status(profile.id, installed.as_deref());
    let verification_failed =
        failed && (installed.is_some() || wrong_size_bytes.is_some()) && !verified;

    let partial = match (directory, profile.artifact.source()) {
        (Some(directory), Some(source)) if installed.is_none() => {
            partial_bytes(directory, source.file_name)
        }
        _ => None,
    };

    // One short lock: read its handle, port, and last error. The guard is
    // released before the health probe awaits. Reaping here is read-only for
    // settings and never starts a process.
    let (running, port, last_error) = {
        let supervisor = state.supervisor.lock();

        match supervisor {
            Ok(mut guard) => {
                let running = guard.is_running();
                (
                    running,
                    guard.endpoint().map(|(port, _)| port),
                    guard.last_error.clone(),
                )
            }
            Err(_) => (false, None, None),
        }
    };

    // A running process is not the same as a ready engine: probe it, exactly
    // as the status command does, so "loaded" is never inferred from a pid.
    let healthy = match (running, port) {
        (true, Some(port)) => process::health_ok(port).await,
        _ => false,
    };

    let download_active = state
        .download
        .lock()
        .map(|slot| {
            slot.as_ref()
                .is_some_and(|active| active.profile_id == profile.id)
        })
        .unwrap_or(false);

    readiness::assess(&ReadinessInput {
        profile,
        context_size: settings.context_size,
        disk: disk_status,
        memory: memory_status,
        files: FileFacts {
            installed_exact: installed.is_some(),
            partial_bytes: partial,
            verified,
            verification_failed,
            wrong_size_bytes,
        },
        download_active,
        engine: EngineFacts {
            running,
            healthy,
            loaded_profile_id: state.loaded_profile_id().map(str::to_string),
            failed_profile_id: state.failed_profile_id().map(str::to_string),
            loading_profile_id: state.loading_profile_id().map(str::to_string),
            last_error,
        },
    })
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

/// Free space at the folder models are stored in, or at a folder the caller
/// names. `None` fields mean the platform could not measure it; the UI shows
/// "unknown" rather than a number it made up.
#[tauri::command]
pub fn get_disk_space(
    app: AppHandle,
    state: State<'_, InferenceState>,
    target_dir: Option<String>,
) -> hardware::DiskSpace {
    let settings = state.settings_snapshot();

    let requested = target_dir
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from);

    let path = requested
        .or_else(|| settings.model_path.clone().map(PathBuf::from))
        .or_else(|| default_model_dir(&app));

    match path {
        Some(path) => hardware::disk_space(&path),
        None => hardware::DiskSpace {
            free_bytes: None,
            total_bytes: None,
            unavailable_reason: Some("The model folder could not be located."),
        },
    }
}

#[tauri::command]
pub fn get_settings(state: State<'_, InferenceState>) -> Settings {
    state.settings_snapshot()
}

/// Returns metadata for the profile the user has selected.
///
/// Read-only: it does not inspect the filesystem, does not claim that the
/// model is installed, and never starts the engine. With no choice recorded
/// this is Nexora Mini, so an existing installation sees the same profile it
/// saw before selection existed.
#[tauri::command]
pub fn get_model_profile(state: State<'_, InferenceState>) -> InferenceResult<ModelProfile> {
    let settings = state.settings_snapshot();

    registry::APPROVED.selected(settings.selected_id()).copied()
}

/// One row of the model selector: the profile plus its real availability.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelCatalogEntry {
    pub profile: ModelProfile,
    pub selected: bool,
    /// True when the engine is running this profile.
    pub active: bool,
    /// Where an installed copy was found, if one was.
    pub installed_path: Option<String>,
    /// False when the artefact is not approved, so downloading is refused.
    pub downloadable: bool,
    /// The readiness state name; mirrors `readiness.state`.
    pub status: &'static str,
    pub detail: String,
    /// The single authoritative answer for this row.
    pub readiness: Readiness,
}

/// The selector's data source.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelCatalog {
    pub entries: Vec<ModelCatalogEntry>,
    /// True while an engine process is running.
    pub running: bool,
    /// The profile the engine loaded, when the file name matched one.
    pub loaded_profile_id: Option<&'static str>,
    /// Non-null when `start_generation` would refuse because the selection and
    /// the loaded model disagree. The composer shows this verbatim instead of
    /// re-deriving the rule in the UI, so the two can never drift apart.
    pub selection_error: Option<String>,
}

/// Reads every approved profile and its readiness.
///
/// Truthfulness rules: `readiness` is the single authority for what can be
/// done with a model. A file being present is never enough to call it
/// installed — the published name and exact length must match, and "verified"
/// requires a digest check in this session. Nothing here downloads, launches a
/// process, or changes settings.
#[tauri::command]
pub async fn get_model_catalog(
    app: AppHandle,
    state: State<'_, InferenceState>,
) -> InferenceResult<ModelCatalog> {
    let settings = state.settings_snapshot();
    let chosen = registry::APPROVED.selected(settings.selected_id())?;

    let directory = default_model_dir(&app);

    let mut entries = Vec::new();

    for profile in registry::APPROVED.all() {
        let readiness = readiness_for(&state, profile, &settings, directory.as_deref()).await;
        let installed = installed_for(profile, &settings, directory.as_deref());

        entries.push(ModelCatalogEntry {
            profile: *profile,
            selected: profile.id == chosen.id,
            active: readiness.state == ReadinessState::Loaded,
            installed_path: installed
                .as_ref()
                .map(|path| path.to_string_lossy().into_owned()),
            downloadable: readiness.can_download,
            status: readiness.state.as_str(),
            detail: readiness.detail.clone(),
            readiness,
        });
    }

    // Reap a crashed child while reading the row, exactly as the status
    // command does, so `running` is not a stale handle.
    let running = state
        .supervisor
        .lock()
        .map(|mut supervisor| supervisor.is_running())
        .unwrap_or(false);

    let selection_error = registry::check_selection(chosen, state.loaded_profile_id())
        .err()
        .map(|error| error.message);

    Ok(ModelCatalog {
        entries,
        running,
        loaded_profile_id: state.loaded_profile_id(),
        selection_error,
    })
}

/// The authoritative readiness for the selected model.
///
/// A separate command so the model manager can refresh one row without
/// re-reading the whole catalogue. Same computation, same source of truth.
#[tauri::command]
pub async fn check_model_readiness(
    app: AppHandle,
    state: State<'_, InferenceState>,
) -> InferenceResult<Readiness> {
    let settings = state.settings_snapshot();
    let chosen = registry::APPROVED.selected(settings.selected_id())?;
    let directory = default_model_dir(&app);

    Ok(readiness_for(&state, chosen, &settings, directory.as_deref()).await)
}

/// Persists the chosen profile without touching paths or engine state.
///
/// Kept separate from `save_settings` so switching models never re-validates
/// (or silently rewrites) the user's configured files, and never starts a
/// process. An unknown id is refused rather than rewritten to the default.
#[tauri::command]
pub fn select_model(state: State<'_, InferenceState>, id: String) -> InferenceResult<Settings> {
    let profile = registry::APPROVED.resolve(&id)?;

    ensure_switch_allowed(&state, profile)?;

    let mut store = state.settings.lock().map_err(|_| lock_error())?;
    let mut settings = store.settings.clone();
    settings.model_id = Some(profile.id.to_string());

    let saved = store.save(settings)?;
    store.settings = saved.clone();

    Ok(saved)
}

/// Persists the user's model/runtime choice after validating both paths.
#[tauri::command]
pub fn save_settings(
    state: State<'_, InferenceState>,
    settings: Settings,
) -> InferenceResult<Settings> {
    let settings = settings.normalised();
    let memory = hardware::system_info().available_ram_gib;

    // An unknown id must fail here rather than at generation time, and it must
    // never be silently rewritten to the default profile.
    let profile = registry::APPROVED.selected(settings.selected_id())?;

    // The settings form can also carry `model_id`. A change made there is the
    // same switch as the chat selector's, so it obeys the same rule; editing
    // paths or the context window for the profile already selected is allowed.
    let stored = registry::APPROVED.selected(state.settings_snapshot().selected_id())?;

    if stored.id != profile.id {
        ensure_switch_allowed(&state, profile)?;
    }

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
pub fn validate_model(path: String, context_size: Option<u32>) -> InferenceResult<ModelInfo> {
    let memory = hardware::system_info().available_ram_gib;

    models::inspect(&path, context_size.unwrap_or(DEFAULT_CONTEXT_SIZE), memory)
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

    // The stored choice must resolve before anything is inspected. An unknown
    // id is refused here, never rewritten to the default profile.
    let selected = registry::APPROVED.selected(settings.selected_id())?;

    // Trim once, here. Everything below — the ledger key, the profile lookup,
    // the argv — must agree on the same string.
    let model = settings
        .model_path
        .as_deref()
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .map(str::to_string)
        .ok_or_else(|| {
            InferenceError::not_configured("Choose a GGUF model file before loading the engine.")
        })?;

    let runtime = settings
        .runtime_path
        .as_deref()
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .map(str::to_string)
        .ok_or_else(|| {
            InferenceError::not_configured("Choose the llama-server executable before loading.")
        })?;

    // Read the file's own header first: it is cheap, and it tells us the
    // context window this GGUF actually declares. A model whose metadata allows
    // less than the request is refused before a process exists, instead of
    // spending the full load timeout failing inside the runtime.
    let info = models::inspect(&model, settings.context_size, None)?;

    // Was this exact file confirmed against its published digest in this
    // session? Nothing here hashes, so a click never blocks for minutes
    // without progress; `verify_model` records the result.
    let owner = acquisition::profile_for_path(Path::new(&model));
    let verified = owner
        .map(|profile| {
            state
                .verification_status(profile.id, Some(Path::new(&model)))
                .0
        })
        .unwrap_or(false);

    // One decision for everything that must be true before a spawn: the
    // selection may only load its own verified artefact (Nexora Mini may also
    // load an unregistered GGUF), and the context window must be supported by
    // both the app and the file.
    let plan = loading::plan(
        selected,
        &model,
        settings.context_size,
        info.context_length,
        verified,
    )?;

    // Refuse to start a load the machine cannot hold, before a process exists.
    // Only applies when the file is a published artefact, because that is the
    // only case with verified size and cache metadata.
    if let Some(profile) = plan.gates_for() {
        let estimate = readiness::estimate(profile, settings.context_size);

        if let hardware::SystemInfo {
            available_ram_gib: Some(available),
            ..
        } = hardware::system_info()
        {
            let usable = (available - estimate.memory_headroom_gib).max(0.0);

            if estimate.estimated_memory_gib > usable {
                return Err(InferenceError::new(
                    "insufficient_memory",
                    format!(
                        "{} needs about {:.1} GiB of memory at {} tokens (weights {:.1} + cache {:.2} + {:.1} runtime), but only {:.1} GiB is available. Close other applications or lower the context window. Nothing was started.",
                        profile.display_name,
                        estimate.estimated_memory_gib,
                        estimate.context_size,
                        estimate.weights_gib,
                        estimate.kv_cache_gib,
                        estimate.runtime_overhead_gib,
                        available
                    ),
                ));
            }
        }
    }

    // Claim the single load slot before anything is spawned. A second click
    // while the first load is still waiting for the engine is refused instead
    // of racing it — and must not disturb the engine that is already loaded,
    // which is why the mark is cleared only after the claim succeeds.
    if !state.begin_loading(plan.profile_id) {
        return Err(InferenceError::busy());
    }

    // The previous attribution stops being true the moment this load starts.
    // The recorded failure is replaced only if this attempt actually fails.
    state.clear_loaded();

    // Loading is an explicit user action: always start from a clean process so
    // a changed model or context size actually takes effect.
    let started = (|| -> InferenceResult<u16> {
        let mut supervisor = state.supervisor.lock().map_err(|_| lock_error())?;

        supervisor.start(&runtime, &model, settings.context_size, &state.config_dir)?;

        supervisor
            .endpoint()
            .map(|(port, _)| port)
            .ok_or_else(|| InferenceError::spawn_failed("The engine did not expose a port."))
    })();

    let port = match started {
        Ok(port) => port,
        Err(error) => {
            state.finish_loading();
            return Err(error);
        }
    };

    let deadline = Instant::now() + process::LOAD_TIMEOUT;

    loop {
        if process::health_ok(port).await {
            // Record which registered profile the engine is running. This is
            // what lets `start_generation` refuse a Sonnet request while Mini
            // is loaded, instead of answering with the wrong model. A file no
            // profile publishes is deliberately not attributed.
            state.set_loaded(LoadedModel {
                profile_id: plan.profile_id,
                path: model.clone(),
                context_size: settings.context_size,
            });
            state.clear_failure();
            state.finish_loading();

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

            drop(supervisor);
            state.clear_loaded();
            state.record_failure(plan.profile_id, message.clone());
            state.finish_loading();

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

            state.clear_loaded();
            state.record_failure(plan.profile_id, message.clone());
            state.finish_loading();

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
    // Stopping the child while a load is still polling would leave that load
    // to report a failure the user did not cause. Refuse instead.
    if state.load_in_flight() {
        return Err(InferenceError::busy());
    }

    if let Ok(slot) = state.active.lock() {
        if let Some(active) = slot.as_ref() {
            active
                .cancelled
                .store(true, std::sync::atomic::Ordering::SeqCst);
            active.cancel.notify_one();
        }
    }

    {
        let mut supervisor = state.supervisor.lock().map_err(|_| lock_error())?;
        supervisor.stop();
        // A deliberate unload is a reset: the old error no longer describes
        // what is running, and leaving it would make the rows report a failure
        // that the user has already cleared.
        supervisor.last_error = None;
    }

    state.clear_loaded();
    state.clear_failure();

    Ok(state.status(app.package_info().version.to_string()).await)
}

/// Streams a reply from the local model.
///
/// Only one generation may run at a time; a second request is refused with a
/// typed `busy` error rather than queueing behind a model that cannot batch.
#[tauri::command]
pub async fn start_generation(
    state: State<'_, InferenceState>,
    usage: State<'_, UsageState>,
    request: GenerateRequest,
    on_event: Channel<GenerationEvent>,
) -> InferenceResult<GenerationOutcome> {
    let validated = request.validate()?;

    let settings = state.settings_snapshot();
    let selected = registry::APPROVED.selected(settings.selected_id())?;

    // The UI names the profile it composed the message under. It must still be
    // the selected one: a chat tab rendered before a switch would otherwise be
    // answered by a different model than its composer shows.
    registry::check_request_profile(request.profile_id.as_deref(), selected)?;

    // A prompt that cannot fit the configured window would be silently cut by
    // the runtime, so the model would answer with part of the conversation
    // missing. Refuse it here: no engine work, no usage reserved.
    let prompt_tokens =
        budget::conversation_tokens(request.messages.iter().map(|turn| turn.content.as_str()));

    // The engine that will answer may have been started with an older context
    // than the settings currently hold. The running engine is what matters.
    let context_size = state.loaded_context().unwrap_or(settings.context_size);

    if let Some(overflow) = budget::overflow(prompt_tokens, validated.max_tokens, context_size) {
        return Err(InferenceError::invalid_request(overflow.message()));
    }

    // Refuse before spending the daily unit or touching the engine when the
    // selection and the running model disagree. This is the guard that stops a
    // Sonnet request from being silently answered by Mini.
    registry::check_selection(selected, state.loaded_profile_id())?;

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

    // Reserve the daily allowance before the engine is asked to work. The
    // reservation is taken under the same lock as the single active slot, so
    // two requests cannot both spend the last unit. A refused reservation
    // leaves the slot untouched; an early return drops the charge and releases
    // the unit without counting anything.
    let charge = {
        let mut slot = state.active.lock().map_err(|_| lock_error())?;

        if slot.is_some() {
            return Err(InferenceError::busy());
        }

        let charge = usage.reserve(SystemTime::now())?;

        *slot = Some(ActiveGeneration {
            request_id: request.request_id.clone(),
            cancel: Arc::clone(&cancel),
            cancelled: Arc::clone(&cancelled),
        });

        charge
    };

    let accepted = AtomicBool::new(false);

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
        &accepted,
    )
    .await;

    state.finish_generation(&request.request_id);

    // Charged only once the engine accepted the request and began streaming.
    // A failure before that point releases the reservation for free, so a
    // missing model, a dead engine or a rejected request never costs a unit.
    if accepted.load(Ordering::SeqCst) {
        // A failed write is recorded in `UsageState` and reported by
        // `get_usage_status`; it must not turn a finished reply into an error.
        let _ = charge.commit();
    } else {
        drop(charge);
    }

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

/// Downloads the approved Nexora Mini artefact into a folder and verifies it.
///
/// Never called on startup: the user presses a button, sees progress, and can
/// cancel. The file is written under a temporary name and renamed only after
/// its length and SHA-256 match the registry.
#[tauri::command]
pub async fn start_model_download(
    app: AppHandle,
    state: State<'_, InferenceState>,
    target_dir: Option<String>,
    profile_id: Option<String>,
    on_event: Channel<DownloadEvent>,
) -> InferenceResult<DownloadOutcome> {
    let profile = registry::APPROVED.selected(profile_id.as_deref())?;

    // Built before the target folder is created or the download slot is taken,
    // so a profile without an approved artefact fails without side effects.
    let plan = acquisition::plan_for(profile)?;

    let directory = match target_dir
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        Some(value) => {
            let path = PathBuf::from(value);

            if !path.is_absolute() {
                return Err(InferenceError::invalid_request(
                    "Choose an absolute folder for the model download.",
                ));
            }

            path
        }
        None => app
            .path()
            .app_data_dir()
            .map_err(|error| {
                InferenceError::storage(format!(
                    "The application data folder is unavailable: {error}"
                ))
            })?
            .join("models"),
    };

    std::fs::create_dir_all(&directory).map_err(|error| {
        InferenceError::storage(format!("The model folder could not be created: {error}"))
    })?;

    // Re-check the destination immediately before writing anything. The
    // catalogue may be seconds old and another download may have filled the
    // disk since; an unknown reading is not treated as a failure.
    let estimate = readiness::estimate(profile, state.settings_snapshot().context_size);

    if let Some(free) = hardware::disk_space(&directory).free_bytes {
        if free < estimate.required_free_bytes {
            return Err(InferenceError::new(
                "insufficient_storage",
                format!(
                    "The download folder needs {} free for {} plus a reserve, but only {} is available. Nothing was written.",
                    readiness::format_bytes(estimate.required_free_bytes),
                    readiness::format_bytes(estimate.model_bytes),
                    readiness::format_bytes(free)
                ),
            ));
        }
    }

    let cancel = Arc::new(AtomicBool::new(false));

    {
        let mut slot = state.download.lock().map_err(|_| lock_error())?;

        if slot.is_some() {
            return Err(InferenceError::busy());
        }

        *slot = Some(super::ActiveDownload {
            profile_id: profile.id,
            cancel: Arc::clone(&cancel),
        });
    }

    let outcome = acquisition::download(&plan, &directory, &cancel, &on_event).await;

    // Cleared on success, failure, and cancellation alike.
    if let Ok(mut slot) = state.download.lock() {
        *slot = None;
    }

    // A published file passed the length and digest checks inside `download`,
    // so it is verified by construction. A cancelled download installs nothing
    // and must not leave a "verified" mark behind.
    if let Ok(download) = &outcome {
        if let (false, Some(path)) = (download.cancelled, download.path.as_deref()) {
            if let Ok(mut ledger) = state.verified.lock() {
                ledger.mark_verified(profile.id, Path::new(path));
            }
        }
    }

    outcome
}

/// Stops the running model download. Returns whether one was active.
#[tauri::command]
pub fn cancel_model_download(state: State<'_, InferenceState>) -> bool {
    match state.download.lock() {
        Ok(slot) => match slot.as_ref() {
            Some(active) => {
                active.cancel.store(true, Ordering::SeqCst);
                true
            }
            None => false,
        },
        Err(_) => false,
    }
}

/// Hashes a file the user chose and compares it with the approved profile.
///
/// Runs off the async runtime: hashing a 4.7 GiB file takes seconds on CPU and
/// must not stall other IPC commands.
#[tauri::command]
pub async fn verify_model(
    state: State<'_, InferenceState>,
    path: String,
    profile_id: Option<String>,
) -> InferenceResult<IntegrityReport> {
    let chosen = match profile_id {
        Some(id) => id,
        None => state
            .settings_snapshot()
            .selected_id()
            .map(str::to_string)
            .unwrap_or_else(|| registry::DEFAULT_MODEL_ID.to_string()),
    };

    let profile = registry::APPROVED.resolve(&chosen)?;

    let report =
        tauri::async_runtime::spawn_blocking(move || acquisition::verify_file(&path, profile))
            .await
            .map_err(|error| InferenceError::internal(error.to_string()))??;

    // Record the outcome so readiness reflects what was actually checked.
    // A mismatch is remembered as a failure until a later check succeeds.
    if let Ok(mut ledger) = state.verified.lock() {
        if report.digest_matches && report.size_matches {
            ledger.mark_verified(profile.id, Path::new(&report.path));
        } else {
            ledger.mark_failed(profile.id);
        }
    }

    Ok(report)
}
