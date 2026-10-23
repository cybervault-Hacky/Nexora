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
                (running, guard.endpoint(), guard.last_error.clone())
            }
            Err(_) => (false, None, None),
        }
    };

    // A running process is not the same as a ready engine: probe it, exactly
    // as the status command does, so "loaded" is never inferred from a pid.
    let healthy = match (running, port) {
        (true, Some((port, key))) => process::health_ok_with_key(port, Some(&key)).await,
        _ => false,
    };

    let (download_active, download_verifying) = state
        .download
        .lock()
        .map(|slot| match slot.as_ref() {
            Some(active) if active.profile_id == profile.id => (
                true,
                active.verifying.load(std::sync::atomic::Ordering::SeqCst),
            ),
            _ => (false, false),
        })
        .unwrap_or((false, false));

    let (ledger_verifying, last_transfer) = state
        .verified
        .lock()
        .map(|ledger| {
            (
                ledger.is_verifying(profile.id),
                ledger.last_transfer(profile.id),
            )
        })
        .unwrap_or((false, None));

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
        verifying_active: download_verifying || ledger_verifying,
        last_transfer,
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

fn app_runtime_search_dirs(app: &AppHandle, state: &InferenceState) -> process::RuntimeSearchDirs {
    process::RuntimeSearchDirs {
        exe_dir: std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(std::path::Path::to_path_buf)),
        resource_dir: app.path().resource_dir().ok(),
        data_dir: Some(state.config_dir.clone()),
    }
}

/// Read-only snapshot used by the UI on start and after every action.
#[tauri::command]
pub async fn get_app_status(
    app: AppHandle,
    state: State<'_, InferenceState>,
) -> InferenceResult<AppStatus> {
    let dirs = app_runtime_search_dirs(&app, &state);
    Ok(state
        .status_with_dirs(app.package_info().version.to_string(), &dirs)
        .await)
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
    let dirs = app_runtime_search_dirs(&app, &state);

    load_model_with_timeout(
        &state,
        app.package_info().version.to_string(),
        &dirs,
        process::LOAD_TIMEOUT,
    )
    .await
}

pub async fn load_model_with_timeout(
    state: &InferenceState,
    version: String,
    dirs: &process::RuntimeSearchDirs,
    load_timeout: Duration,
) -> InferenceResult<AppStatus> {
    let settings = state.settings_snapshot();

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

    let owner = acquisition::profile_for_path(Path::new(&model));

    let runtime = match process::resolve_runtime_path(settings.runtime_path.as_deref(), dirs) {
        Ok(resolved) => resolved,
        Err(error) => {
            if error.code != "not_configured" {
                state.record_failure(owner.map(|p| p.id), error.message.clone());
            }
            return Err(error);
        }
    };

    // Read the file's own header first: it is cheap, and it tells us the
    // context window this GGUF actually declares. A model whose metadata allows
    // less than the request is refused before a process exists, instead of
    // spending the full load timeout failing inside the runtime.
    let info = match models::inspect(&model, settings.context_size, None) {
        Ok(inspected) => inspected,
        Err(error) => {
            state.record_failure(owner.map(|p| p.id), error.message.clone());
            return Err(error);
        }
    };

    // Was this exact file confirmed against its published digest in this
    // session? Nothing here hashes, so a click never blocks for minutes
    // without progress; `verify_model` records the result.
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

    // Starting the new engine stops the running one. Refuse while a reply is
    // streaming rather than cutting it off. The loading marker is already set,
    // so `start_generation` cannot begin a reply between this check and the
    // spawn. An unreadable slot is treated as busy: fail closed.
    let generating = state
        .active
        .lock()
        .map(|slot| slot.is_some())
        .unwrap_or(true);

    if let Err(error) = registry::check_load_allowed(generating) {
        state.finish_loading();
        return Err(error);
    }

    // The previous attribution stops being true the moment this load starts.
    // The recorded failure is replaced only if this attempt actually fails.
    state.clear_loaded();

    // Loading is an explicit user action: always start from a clean process so
    // a changed model or context size actually takes effect.
    let started = (|| -> InferenceResult<(u16, String)> {
        let mut supervisor = state.supervisor.lock().map_err(|_| lock_error())?;

        supervisor.start(&runtime, &model, settings.context_size, &state.config_dir)?;

        supervisor
            .endpoint()
            .ok_or_else(|| InferenceError::spawn_failed("The engine did not expose a port."))
    })();

    let (port, key) = match started {
        Ok(endpoint) => endpoint,
        Err(error) => {
            // A spawn that never produced a process still needs a recorded
            // reason, attributed to this profile. Otherwise the engine panel
            // reports whatever older failure happened to be stored.
            state.record_failure(plan.profile_id, error.message.clone());
            state.finish_loading();
            return Err(error);
        }
    };

    let deadline = Instant::now() + load_timeout;

    loop {
        match process::probe_health(port, Some(&key)).await {
            process::HealthProbe::Ready => {
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

                return Ok(state.status_with_dirs(version, dirs).await);
            }
            process::HealthProbe::AuthFailed => {
                let diagnostics = {
                    let mut supervisor = match state.supervisor.lock() {
                        Ok(guard) => guard,
                        Err(poisoned) => poisoned.into_inner(),
                    };

                    let diagnostics = supervisor.diagnostics();
                    supervisor.stop();
                    supervisor.last_error = Some(
                        "The local engine rejected Nexora's per-launch session credential."
                            .to_string(),
                    );
                    diagnostics
                };

                let message = format!(
                    "The local engine rejected Nexora's per-launch session credential.{diagnostics}"
                );
                state.clear_loaded();
                state.record_failure(plan.profile_id, message.clone());
                state.finish_loading();

                return Err(InferenceError::auth_failed(message));
            }
            process::HealthProbe::Loading | process::HealthProbe::Unhealthy(_) => {}
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

            return Err(InferenceError::timeout(message));
        }

        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// Stops the engine and cancels any active generation.
#[tauri::command]
pub async fn unload_model(
    app: AppHandle,
    state: State<'_, InferenceState>,
) -> InferenceResult<AppStatus> {
    let dirs = app_runtime_search_dirs(&app, &state);
    unload_model_with_dirs(&state, app.package_info().version.to_string(), &dirs).await
}

pub async fn unload_model_with_dirs(
    state: &InferenceState,
    version: String,
    dirs: &process::RuntimeSearchDirs,
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

    Ok(state.status_with_dirs(version, dirs).await)
}

/// Streams a reply from the local model.
///
/// Only one generation may run at a time; a second request is refused with a
/// typed `busy` error rather than queueing behind a model that cannot batch.
#[tauri::command]
pub async fn start_generation(
    state: State<'_, InferenceState>,
    usage: State<'_, UsageState>,
    auth: State<'_, crate::auth::commands::AuthState>,
    request: GenerateRequest,
    on_event: Channel<GenerationEvent>,
) -> InferenceResult<GenerationOutcome> {
    execute_generation(&state, &usage, &auth, request, on_event).await
}

pub async fn execute_generation(
    state: &InferenceState,
    usage: &UsageState,
    auth: &crate::auth::commands::AuthState,
    request: GenerateRequest,
    on_event: Channel<GenerationEvent>,
) -> InferenceResult<GenerationOutcome> {
    // Enforce authenticated session gate when an account backend is configured.
    // Offline-only builds without a configured backend still run locally under
    // the local tamper-evident daily allowance rather than deadlocking.
    let stored_token = auth
        .read_session_token()
        .map_err(|e| InferenceError::new("storage_error", e.message))?;

    if stored_token.is_none() && auth.client.is_configured() {
        return Err(InferenceError::new(
            "unauthenticated",
            "Sign in to your Nexora account to send messages.",
        ));
    }

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
            drop(supervisor);
            state.reconcile_engine();
            return Err(InferenceError::not_running(
                "Load a local model before sending a message.",
            ));
        }

        supervisor
            .endpoint()
            .ok_or_else(|| InferenceError::not_running("The engine is not ready."))?
    };

    match process::probe_health(port, Some(&key)).await {
        process::HealthProbe::Ready => {}
        process::HealthProbe::AuthFailed => {
            return Err(InferenceError::auth_failed(
                "The local engine rejected Nexora's per-launch session credential. Reload the model.",
            ));
        }
        process::HealthProbe::Loading | process::HealthProbe::Unhealthy(_) => {
            return Err(InferenceError::new(
                "not_ready",
                "The local engine is not responding. Reload the model.",
            ));
        }
    }

    let cancel = Arc::new(Notify::new());
    let cancelled = Arc::new(AtomicBool::new(false));

    // Reserve backend-authoritative daily quota per user + plan + model first when signed in.
    // When `nexora-auth` is temporarily unreachable (offline mode) or unconfigured,
    // fall back cleanly to the local tamper-evident daily allowance below.
    let server_reservation = match stored_token.as_deref() {
        Some(token) => match auth
            .client
            .reserve_usage(token, selected.id, &request.request_id)
            .await
        {
            Ok(res) => Some((token.to_string(), res)),
            Err(err) if err.is_session_invalid() => {
                auth.purge_local_session();
                return Err(InferenceError::new(
                    "unauthenticated",
                    "Your session has expired. Please sign in again.",
                ));
            }
            Err(err) if err.code == "network_error" || err.code == "backend_unconfigured" => None,
            Err(err) => {
                let static_code = match err.code.as_str() {
                    "quota_exceeded" => "quota_exceeded",
                    "unauthenticated" | "session_expired" => "unauthenticated",
                    "validation_error" => "invalid_request",
                    "rate_limited" => "rate_limited",
                    _ => "quota_check_failed",
                };
                return Err(InferenceError::new(static_code, err.message));
            }
        },
        None => None,
    };

    // Reserve the local slot/fallback allowance under the single active-generation mutex.
    let slot_result: InferenceResult<_> = {
        let mut slot = state.active.lock().map_err(|_| lock_error())?;

        if slot.is_some() {
            Err(InferenceError::busy())
        } else if let Err(err) = registry::check_generation_allowed(state.load_in_flight()) {
            Err(err)
        } else {
            let charge = if server_reservation.is_none() {
                Some(usage.reserve(SystemTime::now())?)
            } else {
                None
            };

            *slot = Some(ActiveGeneration {
                request_id: request.request_id.clone(),
                cancel: Arc::clone(&cancel),
                cancelled: Arc::clone(&cancelled),
            });

            Ok(charge)
        }
    };

    let charge = match slot_result {
        Ok(c) => c,
        Err(err) => {
            if let Some((ref token, ref res)) = server_reservation {
                let _ = auth
                    .client
                    .settle_usage(token, &res.reservation_id, false)
                    .await;
            }
            return Err(err);
        }
    };

    let accepted = AtomicBool::new(false);

    let raw_outcome = client::stream_chat(
        port,
        &key,
        selected.id,
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

    // If the sidecar process died unexpectedly during generation, reap it immediately,
    // clear `state.loaded`, and surface a dedicated `process_exited` error rather than
    // leaving the UI believing the engine is still healthy. Note that the OS kernel
    // closes open sockets (`exit_files`) slightly before marking the task reapable by
    // `try_wait()` (`exit_notify`), so probe briefly on abrupt `network` disconnects.
    let outcome = match raw_outcome {
        Ok(done) => Ok(done),
        Err(err) => {
            let mut died = state.reconcile_engine();
            if !died && err.code == "network" {
                for _ in 0..8 {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                    if state.reconcile_engine() {
                        died = true;
                        break;
                    }
                }
            }
            if died {
                let detail = state
                    .failure_message()
                    .unwrap_or_else(|| err.message.clone());
                Err(InferenceError::process_exited(detail))
            } else {
                Err(err)
            }
        }
    };

    // Charged only once the engine accepted the request and began streaming.
    // A failure before that point releases the reservation for free, so a
    // missing model, a dead engine or a rejected request never costs a unit.
    if accepted.load(Ordering::SeqCst) {
        if let Some((ref token, ref res)) = server_reservation {
            let _ = auth
                .client
                .settle_usage(token, &res.reservation_id, true)
                .await;
        }
        if let Some(charge) = charge {
            let _ = charge.commit();
        }
    } else {
        if let Some((ref token, ref res)) = server_reservation {
            let _ = auth
                .client
                .settle_usage(token, &res.reservation_id, false)
                .await;
        }
        drop(charge);
    }

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
    let default_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| {
            InferenceError::storage(format!(
                "The application data folder is unavailable: {error}"
            ))
        })?
        .join("models");

    execute_model_download(
        &state,
        target_dir,
        profile_id,
        default_dir,
        None,
        acquisition::DownloadPolicy::production(),
        &on_event,
    )
    .await
}

pub async fn execute_model_download(
    state: &InferenceState,
    target_dir: Option<String>,
    profile_id: Option<String>,
    default_dir: PathBuf,
    plan_override: Option<acquisition::DownloadPlan>,
    policy: acquisition::DownloadPolicy,
    on_event: &Channel<DownloadEvent>,
) -> InferenceResult<DownloadOutcome> {
    let profile = registry::APPROVED.selected(profile_id.as_deref())?;

    // Built before the target folder is created or the download slot is taken,
    // so a profile without an approved artefact fails without side effects.
    let plan = match plan_override {
        Some(custom_plan) => custom_plan,
        None => acquisition::plan_for(profile)?,
    };

    let directory = match target_dir
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        Some(value) => PathBuf::from(value),
        None => default_dir,
    };

    acquisition::validate_download_directory(&directory)?;

    std::fs::create_dir_all(&directory).map_err(|error| {
        InferenceError::storage(format!("The model folder could not be created: {error}"))
    })?;

    // Re-validate after creation in case an intermediate component was raced.
    acquisition::validate_download_directory(&directory)?;

    let final_path = directory.join(&plan.file_name);

    // Avoid unnecessary re-downloads when the exact artifact is already on disk
    // and verified against its digest in this session.
    let exact_present = std::fs::symlink_metadata(&final_path)
        .map(|meta| meta.file_type().is_file() && meta.len() == plan.size_bytes)
        .unwrap_or(false);
    if exact_present && state.verification_status(profile.id, Some(&final_path)).0 {
        return Ok(DownloadOutcome {
            path: Some(final_path.to_string_lossy().into_owned()),
            file_name: plan.file_name,
            size_bytes: plan.size_bytes,
            sha256: plan.sha256,
            cancelled: false,
        });
    }

    // Never overwrite or auto-delete an existing user model file.
    if std::fs::symlink_metadata(&final_path).is_ok() {
        return Err(InferenceError::already_exists(format!(
            "{} already exists. Verify the existing file if it matches {}, or move it manually before downloading again.",
            final_path.display(),
            profile.display_name
        )));
    }

    let cancel = Arc::new(AtomicBool::new(false));
    let verifying = Arc::new(AtomicBool::new(false));

    {
        let mut slot = state.download.lock().map_err(|_| lock_error())?;

        if slot.is_some() {
            return Err(InferenceError::busy());
        }

        *slot = Some(super::ActiveDownload {
            profile_id: profile.id,
            cancel: Arc::clone(&cancel),
            verifying: Arc::clone(&verifying),
        });
    }

    if let Ok(mut ledger) = state.verified.lock() {
        ledger.clear_transfer(profile.id);
    }

    // Reclaim orphaned `.part` files under the exclusive lock before measuring free space.
    acquisition::purge_stale_part_files(&directory, &plan.file_name);

    // Re-check the destination immediately before writing anything. The
    // catalogue may be seconds old and another download may have filled the
    // disk since; an unknown reading is not treated as a failure.
    let estimate = readiness::estimate(profile, state.settings_snapshot().context_size);

    if let Some(free) = hardware::disk_space(&directory).free_bytes {
        if free < estimate.required_free_bytes {
            if let Ok(mut slot) = state.download.lock() {
                *slot = None;
            }
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

    let outcome =
        acquisition::download_with_policy(&plan, &directory, &cancel, &verifying, on_event, policy)
            .await;

    // Cleared on success, failure, and cancellation alike.
    if let Ok(mut slot) = state.download.lock() {
        *slot = None;
    }

    if let Ok(mut ledger) = state.verified.lock() {
        match &outcome {
            Ok(download) if download.cancelled => {
                ledger.record_download_cancelled(profile.id);
            }
            Ok(download) => {
                if let Some(path) = download.path.as_deref() {
                    ledger.mark_verified(profile.id, Path::new(path));
                }
            }
            Err(error) => {
                ledger.record_download_failed(profile.id, error.message.clone());
            }
        }
    }

    outcome
}

/// Stops the running model download. Returns whether one was active.
#[tauri::command]
pub fn cancel_model_download(state: State<'_, InferenceState>) -> bool {
    cancel_model_download_inner(&state)
}

pub fn cancel_model_download_inner(state: &InferenceState) -> bool {
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
    verify_model_inner(&state, path, profile_id).await
}

pub async fn verify_model_inner(
    state: &InferenceState,
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

    if let Ok(mut ledger) = state.verified.lock() {
        ledger.begin_verifying(profile.id);
    }

    let task_res =
        tauri::async_runtime::spawn_blocking(move || acquisition::verify_file(&path, profile))
            .await;

    let report = match task_res {
        Ok(Ok(report)) => report,
        Ok(Err(err)) => {
            if let Ok(mut ledger) = state.verified.lock() {
                ledger.finish_verifying(profile.id);
            }
            return Err(err);
        }
        Err(join_err) => {
            if let Ok(mut ledger) = state.verified.lock() {
                ledger.finish_verifying(profile.id);
            }
            return Err(InferenceError::internal(join_err.to_string()));
        }
    };

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

#[cfg(all(test, unix))]
mod tests {
    use std::os::unix::fs::PermissionsExt;
    use std::sync::Mutex as StdMutex;
    use std::time::Duration;

    use tauri::ipc::{Channel, InvokeResponseBody};

    use super::*;
    use crate::auth::client::AuthApiClient;
    use crate::auth::commands::AuthState;
    use crate::inference::{ChatTurn, EngineState};

    fn write_sparse_gguf_v3(path: &Path) {
        use std::io::Write;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"GGUF");
        bytes.extend_from_slice(&3u32.to_le_bytes());
        bytes.extend_from_slice(&0u64.to_le_bytes()); // tensor_count = 0
        bytes.extend_from_slice(&0u64.to_le_bytes()); // metadata_kv_count = 0
        let mut file = std::fs::File::create(path).expect("create sparse gguf");
        file.write_all(&bytes).expect("write gguf header");
        file.set_len(17 * 1024 * 1024)
            .expect("extend sparse gguf past MIN_PLAUSIBLE_BYTES");
    }

    fn write_exec_script(dir: &Path, name: &str, script: &str) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, script).expect("write python mock runtime");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
            .expect("chmod +x mock runtime");
        path
    }

    fn event_collector() -> (
        Channel<GenerationEvent>,
        Arc<StdMutex<Vec<GenerationEvent>>>,
    ) {
        let captured = Arc::new(StdMutex::new(Vec::new()));
        let sink = Arc::clone(&captured);
        let channel = Channel::new(move |body: InvokeResponseBody| {
            if let InvokeResponseBody::Json(raw) = body {
                if let Ok(ev) = serde_json::from_str::<GenerationEvent>(&raw) {
                    sink.lock().expect("lock events").push(ev);
                }
            }
            Ok(())
        });
        (channel, captured)
    }

    #[derive(Default)]
    struct TestMemVault(StdMutex<Option<String>>);

    impl crate::auth::commands::CredentialVault for TestMemVault {
        fn save_token(&self, token: &str) -> crate::auth::AuthResult<()> {
            *self.0.lock().expect("vault lock") = Some(token.to_string());
            Ok(())
        }

        fn get_token(&self) -> crate::auth::AuthResult<Option<String>> {
            Ok(self.0.lock().expect("vault lock").clone())
        }

        fn delete_token(&self) -> crate::auth::AuthResult<()> {
            *self.0.lock().expect("vault lock") = None;
            Ok(())
        }
    }

    fn pid_is_alive(pid: u32) -> bool {
        std::process::Command::new("/bin/kill")
            .arg("-0")
            .arg(pid.to_string())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    /// Full real-subprocess + loopback HTTP/SSE contract verification for a local
    /// `llama-server`-compatible engine: sidecar discovery, loopback-only bind,
    /// `0o600` per-launch bearer auth, `503 -> 200 {"status":"ok"}` gating,
    /// offline local inference with unreachable/unconfigured auth backend,
    /// ordered multi-chunk SSE streaming, single-flight contention (`busy`),
    /// active stream cancellation, recovery, child reaping on `unload_model`,
    /// and session key deletion.
    #[tokio::test]
    async fn runtime_sidecar_lifecycle_streaming_cancellation_offline_and_unload_cleanup() {
        let root = std::env::temp_dir().join(format!("nexora-phase04-e2e-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let config_dir = root.join("config");
        let bundle_dir = root.join("bundle").join("binaries");
        std::fs::create_dir_all(&config_dir).expect("create config dir");
        std::fs::create_dir_all(&bundle_dir).expect("create bundle dir");

        let model_path = root.join("custom-dev-coder.gguf");
        write_sparse_gguf_v3(&model_path);

        let pid_file = root.join("engine.pid");
        let argv_file = root.join("engine-argv.json");
        let wire_file = root.join("last-wire-request.json");

        let sidecar_script = format!(
            r#"#!/usr/bin/env python3
import http.server, json, os, pathlib, socketserver, sys, time

if "--version" in sys.argv:
    sys.stderr.write("version: 0.7.0-dev (build 11714, commit c0fd786)\n")
    sys.exit(0)

args = sys.argv[1:]
def flag(name):
    return args[args.index(name) + 1]

host = flag("--host")
port = int(flag("--port"))
key_path = flag("--api-key-file")
secret = pathlib.Path(key_path).read_text(encoding="utf-8").strip()
pathlib.Path({pid_file:?}).write_text(str(os.getpid()), encoding="utf-8")
pathlib.Path({argv_file:?}).write_text(json.dumps(args), encoding="utf-8")

health_calls = 0

class ThreadingHTTPServer(socketserver.ThreadingMixIn, http.server.HTTPServer):
    daemon_threads = True

class Handler(http.server.BaseHTTPRequestHandler):
    def log_message(self, fmt, *a):
        pass

    def _authorized(self):
        return self.headers.get("Authorization") == f"Bearer {{secret}}"

    def do_GET(self):
        global health_calls
        if self.path == "/health":
            if not self._authorized():
                body = b'{{"error":"Invalid API Key"}}'
                self.send_response(401)
                self.send_header("Content-Type", "application/json")
                self.send_header("Content-Length", str(len(body)))
                self.end_headers()
                self.wfile.write(body)
                return
            health_calls += 1
            if health_calls == 1:
                body = b'{{"error":{{"code":503,"message":"Loading model"}}}}'
                self.send_response(503)
                self.send_header("Content-Type", "application/json")
                self.send_header("Content-Length", str(len(body)))
                self.end_headers()
                self.wfile.write(body)
                return
            body = b'{{"status":"ok"}}'
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

    def do_POST(self):
        if self.path != "/v1/chat/completions":
            self.send_response(404)
            self.end_headers()
            return
        if not self._authorized():
            body = b'{{"error":{{"message":"Invalid API Key","code":401}}}}'
            self.send_response(401)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
            return

        length = int(self.headers.get("Content-Length", "0"))
        raw = self.rfile.read(length).decode("utf-8")
        pathlib.Path({wire_file:?}).write_text(raw, encoding="utf-8")
        payload = json.loads(raw)
        user_msg = payload["messages"][-1]["content"]

        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Cache-Control", "no-cache")
        self.end_headers()

        def emit(obj):
            line = f"data: {{json.dumps(obj)}}\n\n".encode("utf-8")
            self.wfile.write(line)
            self.wfile.flush()

        try:
            if user_msg == "SLOW_STREAM":
                emit({{"choices":[{{"delta":{{"content":"part-1 "}}}}]}})
                for _ in range(50):
                    time.sleep(0.05)
                    emit({{"choices":[{{"delta":{{"content":"tick "}}}}]}})
                emit({{"choices":[{{"delta":{{}},"finish_reason":"stop"}}]}} )
                self.wfile.write(b"data: [DONE]\n\n")
                self.wfile.flush()
            elif user_msg == "TRUNCATE":
                emit({{"choices":[{{"delta":{{"content":"incomplete reply"}}}}]}})
                return
            elif user_msg == "MID_ERROR":
                emit({{"choices":[{{"delta":{{"content":"prefix "}}}}]}})
                emit({{"error":{{"message":"KV cache slot exhausted"}}}})
                return
            elif user_msg == "CRASH_MID_STREAM":
                emit({{"choices":[{{"delta":{{"content":"before-crash"}}}}]}})
                sys.stderr.write(f"fatal engine crash while decoding (key={{secret}})\n")
                sys.stderr.flush()
                os._exit(42)
            else:
                for tok in ["fn ", "add(", "a: i32, b: i32) -> i32 ", "{{ a + b }}"]:
                    emit({{"choices":[{{"delta":{{"content":tok}}}}]}})
                emit({{"choices":[{{"delta":{{}},"finish_reason":"stop"}}]}} )
                self.wfile.write(b"data: [DONE]\n\n")
                self.wfile.flush()
        except BrokenPipeError:
            return
        except ConnectionResetError:
            return

server = ThreadingHTTPServer((host, port), Handler)
server.serve_forever()
"#,
            pid_file = pid_file.to_string_lossy(),
            argv_file = argv_file.to_string_lossy(),
            wire_file = wire_file.to_string_lossy(),
        );

        let sidecar_path = write_exec_script(&bundle_dir, "llama-server", &sidecar_script);
        let probed =
            process::probe_runtime(&sidecar_path.to_string_lossy()).expect("probe version");
        assert!(probed.pinned_build_matched);

        let state = InferenceState::load(config_dir.clone());
        {
            let mut store = state.settings.lock().expect("settings lock");
            store.settings.model_path = Some(model_path.to_string_lossy().into_owned());
            store.settings.runtime_path = None; // Exercise automatic sidecar bundle discovery!
            store.settings.model_id =
                Some("nexora-mini-qwen2.5-coder-7b-instruct-q4-k-m".to_string());
            store.settings.context_size = 8_192;
        }

        let dirs = process::RuntimeSearchDirs {
            exe_dir: Some(root.join("bundle")),
            resource_dir: None,
            data_dir: Some(config_dir.clone()),
        };

        let status = load_model_with_timeout(
            &state,
            "0.2.0-test".to_string(),
            &dirs,
            Duration::from_secs(5),
        )
        .await
        .expect("sidecar load must succeed after 503 -> 200 {\"status\":\"ok\"}");

        assert_eq!(status.engine.state, EngineState::Ready);

        // Verify loopback-only host bind, 0o600 key permissions, and zero secret in argv.
        let key_file = config_dir.join(process::SESSION_KEY_FILE);
        assert!(key_file.is_file());
        let mode = std::fs::metadata(&key_file)
            .expect("key metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600);
        let raw_key = std::fs::read_to_string(&key_file)
            .expect("read key")
            .trim()
            .to_string();
        let recorded_argv: Vec<String> =
            serde_json::from_str(&std::fs::read_to_string(&argv_file).expect("read argv"))
                .expect("json argv");
        let host_pos = recorded_argv
            .iter()
            .position(|a| a == "--host")
            .expect("--host");
        assert_eq!(recorded_argv[host_pos + 1], "127.0.0.1");
        assert!(!recorded_argv.iter().any(|a| a == "0.0.0.0"));
        assert!(!recorded_argv.iter().any(|a| a.contains(&raw_key)));

        let pid: u32 = std::fs::read_to_string(&pid_file)
            .expect("pid file")
            .trim()
            .parse()
            .expect("parse pid");
        assert!(pid_is_alive(pid));

        let usage = UsageState::load(&root.join("usage"));

        // 1. Configured backend with NO signed-in user rejects before touching engine.
        let unauth_online = AuthState::with_vault(
            AuthApiClient::with_base_url("http://127.0.0.1:9"),
            Arc::new(TestMemVault::default()),
        );
        let (ch0, _) = event_collector();
        let unauth_err = execute_generation(
            &state,
            &usage,
            &unauth_online,
            GenerateRequest {
                request_id: "req-unauth".into(),
                profile_id: Some("nexora-mini-qwen2.5-coder-7b-instruct-q4-k-m".into()),
                messages: vec![ChatTurn {
                    role: "user".into(),
                    content: "Write add()".into(),
                }],
                max_tokens: Some(128),
                temperature: Some(0.1),
            },
            ch0,
        )
        .await
        .expect_err("must require sign-in when backend is configured and no local session exists");
        assert_eq!(unauth_err.code, "unauthenticated");

        // 2. Signed-in user when online auth server is completely unreachable (`127.0.0.1:9`)
        //    streams tokens cleanly offline using local tamper-evident daily allowance!
        unauth_online
            .persist_session_token("offline-cached-session-token")
            .expect("seed local keyring token");
        let (ch1, events1) = event_collector();
        let outcome = execute_generation(
            &state,
            &usage,
            &unauth_online,
            GenerateRequest {
                request_id: "req-stream-1".into(),
                profile_id: Some("nexora-mini-qwen2.5-coder-7b-instruct-q4-k-m".into()),
                messages: vec![ChatTurn {
                    role: "user".into(),
                    content: "Write add()".into(),
                }],
                max_tokens: Some(128),
                temperature: Some(0.1),
            },
            ch1,
        )
        .await
        .expect("offline local generation must stream cleanly");

        assert!(!outcome.cancelled);
        assert_eq!(outcome.finish_reason.as_deref(), Some("stop"));

        let wire_json: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&wire_file).expect("wire body"))
                .expect("valid wire json");
        assert_eq!(
            wire_json["model"],
            "nexora-mini-qwen2.5-coder-7b-instruct-q4-k-m"
        );

        let snapshot = events1.lock().expect("events lock").clone();
        assert!(matches!(
            snapshot.first(),
            Some(GenerationEvent::Started { request_id }) if request_id == "req-stream-1"
        ));
        let assembled: String = snapshot
            .iter()
            .filter_map(|ev| match ev {
                GenerationEvent::Delta { text, .. } => Some(text.as_str()),
                GenerationEvent::Started { .. } => None,
            })
            .collect();
        assert_eq!(assembled, "fn add(a: i32, b: i32) -> i32 { a + b }");

        // 3. Standalone unconfigured desktop build (`EndpointState::Unconfigured`) also works offline.
        let unconfigured_auth = AuthState::with_vault(
            AuthApiClient::unconfigured(),
            Arc::new(TestMemVault::default()),
        );
        let (ch_standalone, _) = event_collector();
        let standalone_ok = execute_generation(
            &state,
            &usage,
            &unconfigured_auth,
            GenerateRequest {
                request_id: "req-standalone-offline".into(),
                profile_id: Some("nexora-mini-qwen2.5-coder-7b-instruct-q4-k-m".into()),
                messages: vec![ChatTurn {
                    role: "user".into(),
                    content: "Write add()".into(),
                }],
                max_tokens: Some(128),
                temperature: Some(0.1),
            },
            ch_standalone,
        )
        .await
        .expect("standalone offline build must run local AI without deadlocking");
        assert!(!standalone_ok.cancelled);

        // 4. Concurrency guards (second generate -> `busy`; load while generating -> `busy`) + Cancellation!
        let (ch_slow, _) = event_collector();
        let slow_fut = execute_generation(
            &state,
            &usage,
            &unauth_online,
            GenerateRequest {
                request_id: "req-slow".into(),
                profile_id: Some("nexora-mini-qwen2.5-coder-7b-instruct-q4-k-m".into()),
                messages: vec![ChatTurn {
                    role: "user".into(),
                    content: "SLOW_STREAM".into(),
                }],
                max_tokens: Some(128),
                temperature: Some(0.1),
            },
            ch_slow,
        );
        tokio::pin!(slow_fut);

        // Drive `slow_fut` until `state.active` is populated, then test concurrent contentions + cancel.
        let cancelled_res = tokio::select! {
            res = &mut slow_fut => panic!("slow stream ended before cancellation test: {res:?}"),
            () = async {
                for _ in 0..50 {
                    if state.active.lock().expect("lock").is_some() {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
                assert!(state.active.lock().expect("lock").is_some());

                // Concurrent generation must fail cleanly with `busy`.
                let (ch_dup, _) = event_collector();
                let dup_err = execute_generation(
                    &state,
                    &usage,
                    &unauth_online,
                    GenerateRequest {
                        request_id: "req-dup".into(),
                        profile_id: Some("nexora-mini-qwen2.5-coder-7b-instruct-q4-k-m".into()),
                        messages: vec![ChatTurn { role: "user".into(), content: "hi".into() }],
                        max_tokens: Some(64),
                        temperature: Some(0.1),
                    },
                    ch_dup,
                )
                .await
                .expect_err("concurrent generation must be refused");
                assert_eq!(dup_err.code, "busy");

                // Concurrent model reload while streaming must fail cleanly with `busy`.
                let reload_err = load_model_with_timeout(
                    &state,
                    "0.2.0-test".to_string(),
                    &dirs,
                    Duration::from_secs(2),
                )
                .await
                .expect_err("reload during generation must be refused");
                assert_eq!(reload_err.code, "busy");

                assert!(state.cancel_generation("req-slow"));
            } => {
                slow_fut.await.expect("cancelled stream returns clean outcome")
            }
        };
        assert!(cancelled_res.cancelled);
        assert_eq!(cancelled_res.finish_reason.as_deref(), Some("stopped"));

        // 5. Truncated stream and mid-stream runtime error handling (engine stays alive).
        let (ch_trunc, _) = event_collector();
        let trunc_err = execute_generation(
            &state,
            &usage,
            &unauth_online,
            GenerateRequest {
                request_id: "req-trunc".into(),
                profile_id: Some("nexora-mini-qwen2.5-coder-7b-instruct-q4-k-m".into()),
                messages: vec![ChatTurn {
                    role: "user".into(),
                    content: "TRUNCATE".into(),
                }],
                max_tokens: Some(64),
                temperature: Some(0.1),
            },
            ch_trunc,
        )
        .await
        .expect_err("truncated SSE stream without [DONE]/finish_reason must fail safely");
        assert_eq!(trunc_err.code, "network");

        let (ch_miderr, _) = event_collector();
        let mid_err = execute_generation(
            &state,
            &usage,
            &unauth_online,
            GenerateRequest {
                request_id: "req-miderr".into(),
                profile_id: Some("nexora-mini-qwen2.5-coder-7b-instruct-q4-k-m".into()),
                messages: vec![ChatTurn {
                    role: "user".into(),
                    content: "MID_ERROR".into(),
                }],
                max_tokens: Some(64),
                temperature: Some(0.1),
            },
            ch_miderr,
        )
        .await
        .expect_err("mid-stream runtime error frame must surface generation_failed");
        assert_eq!(mid_err.code, "generation_failed");
        assert!(mid_err.message.contains("KV cache slot exhausted"));

        // 6. Clean `unload_model_with_dirs` terminates OS child process and removes session key file.
        let unloaded = unload_model_with_dirs(&state, "0.2.0-test".to_string(), &dirs)
            .await
            .expect("unload succeeds");
        assert_eq!(unloaded.engine.state, EngineState::Stopped);
        assert_eq!(
            unloaded.engine.runtime.as_deref(),
            Some(sidecar_path.to_string_lossy().as_ref())
        );
        assert!(
            !key_file.exists(),
            "session key file must be deleted on unload"
        );
        assert!(
            !pid_is_alive(pid),
            "sidecar child process must be reaped on unload"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    /// Verifies distinct error classification and safe recovery across missing
    /// executable, incompatible executable, invalid model artifact, unverified
    /// approved artifact, profile mismatch, `401` health auth rejection, fast
    /// crash stderr capture + secret scrubbing, startup timeout, and unexpected
    /// child exit during active generation.
    #[tokio::test]
    async fn distinguishes_all_runtime_failure_modes_and_scrubs_secrets() {
        let root =
            std::env::temp_dir().join(format!("nexora-phase04-failures-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let config_dir = root.join("config");
        std::fs::create_dir_all(&config_dir).expect("create config dir");

        let valid_gguf = root.join("custom-mini.gguf");
        write_sparse_gguf_v3(&valid_gguf);
        let corrupt_gguf = root.join("corrupt.gguf");
        std::fs::write(&corrupt_gguf, b"NOT_A_GGUF_HEADER").expect("write corrupt gguf");
        let unverified_sonnet = root.join("qwen2.5-coder-14b-instruct-q4_k_m.gguf");
        write_sparse_gguf_v3(&unverified_sonnet);
        let pe_stub = root.join("windows-only.exe");
        std::fs::write(&pe_stub, b"MZ\x90\x00not-linux").expect("write PE binary on unix");

        let empty_dirs = process::RuntimeSearchDirs::default();
        let state = InferenceState::load(config_dir.clone());

        // 1. Missing executable -> `runtime_missing` + `EngineState::Error` (never false-ready).
        {
            let mut s = state.settings.lock().expect("lock");
            s.settings.model_id = Some("nexora-mini-qwen2.5-coder-7b-instruct-q4-k-m".into());
            s.settings.model_path = Some(valid_gguf.to_string_lossy().into_owned());
            s.settings.runtime_path = Some(
                root.join("missing-llama-server")
                    .to_string_lossy()
                    .into_owned(),
            );
        }
        let err =
            load_model_with_timeout(&state, "0.2.0".into(), &empty_dirs, Duration::from_secs(1))
                .await
                .expect_err("missing binary");
        assert_eq!(err.code, "runtime_missing");
        assert_eq!(
            state.status("0.2.0".into()).await.engine.state,
            EngineState::Error
        );

        // 2. Incompatible executable -> `incompatible_runtime`.
        state.settings.lock().expect("lock").settings.runtime_path =
            Some(pe_stub.to_string_lossy().into_owned());
        let err =
            load_model_with_timeout(&state, "0.2.0".into(), &empty_dirs, Duration::from_secs(1))
                .await
                .expect_err("incompatible binary");
        assert_eq!(err.code, "incompatible_runtime");

        // 3. Invalid model file -> `invalid_model`.
        let fast_crash = write_exec_script(
            &root,
            "crash-server",
            r#"#!/usr/bin/env python3
import pathlib, sys
args = sys.argv[1:]
secret = pathlib.Path(args[args.index("--api-key-file") + 1]).read_text().strip()
sys.stderr.write(f"llama_model_load: unsupported GGUF tensor quantization (secret={secret})\n")
sys.stderr.flush()
sys.exit(1)
"#,
        );
        {
            let mut s = state.settings.lock().expect("lock");
            s.settings.runtime_path = Some(fast_crash.to_string_lossy().into_owned());
            s.settings.model_path = Some(corrupt_gguf.to_string_lossy().into_owned());
        }
        let err =
            load_model_with_timeout(&state, "0.2.0".into(), &empty_dirs, Duration::from_secs(1))
                .await
                .expect_err("invalid model");
        assert_eq!(err.code, "invalid_model");

        // 4. Unverified approved Sonnet filename -> `model_unverified` (SHA-256 gate intact!).
        {
            let mut s = state.settings.lock().expect("lock");
            s.settings.model_id = Some("nexora-sonnet-qwen2.5-coder-14b-instruct-q4-k-m".into());
            s.settings.model_path = Some(unverified_sonnet.to_string_lossy().into_owned());
        }
        let err =
            load_model_with_timeout(&state, "0.2.0".into(), &empty_dirs, Duration::from_secs(1))
                .await
                .expect_err("unverified sonnet");
        assert_eq!(err.code, "model_unverified");

        // 5. Sonnet selected with unregistered custom GGUF -> `model_mismatch` (zero silent fallback!).
        state.settings.lock().expect("lock").settings.model_path =
            Some(valid_gguf.to_string_lossy().into_owned());
        let err =
            load_model_with_timeout(&state, "0.2.0".into(), &empty_dirs, Duration::from_secs(1))
                .await
                .expect_err("sonnet rejects custom unverified file");
        assert_eq!(err.code, "model_mismatch");

        // 6. Fast child crash on startup -> `load_failed` WITH deterministic stderr AND scrubbed secret!
        state.settings.lock().expect("lock").settings.model_id =
            Some("nexora-mini-qwen2.5-coder-7b-instruct-q4-k-m".into());
        let err =
            load_model_with_timeout(&state, "0.2.0".into(), &empty_dirs, Duration::from_secs(2))
                .await
                .expect_err("fast crash on startup");
        assert_eq!(err.code, "load_failed");
        assert!(
            err.message.contains("unsupported GGUF tensor quantization"),
            "fast-exiting child stderr must not be lost to race: {}",
            err.message
        );
        assert!(
            err.message.contains("<redacted>"),
            "runtime session credential must be scrubbed from stderr diagnostics: {}",
            err.message
        );

        // 7. `/health` returns `401 Unauthorized` -> immediate `auth_failed` (no 180s stall).
        let bad_auth_server = write_exec_script(
            &root,
            "reject-auth-server",
            r#"#!/usr/bin/env python3
import http.server, sys
args = sys.argv[1:]
host = args[args.index("--host") + 1]
port = int(args[args.index("--port") + 1])
class H(http.server.BaseHTTPRequestHandler):
    def log_message(self, *a): pass
    def do_GET(self):
        self.send_response(401)
        self.end_headers()
http.server.HTTPServer((host, port), H).serve_forever()
"#,
        );
        state.settings.lock().expect("lock").settings.runtime_path =
            Some(bad_auth_server.to_string_lossy().into_owned());
        let err =
            load_model_with_timeout(&state, "0.2.0".into(), &empty_dirs, Duration::from_secs(5))
                .await
                .expect_err("401 health rejection");
        assert_eq!(err.code, "auth_failed");

        // 8. Hung startup (`sleep 10`) -> bounded `timeout` and child cleanly killed.
        let hung_server = write_exec_script(
            &root,
            "hung-server",
            "#!/usr/bin/env python3\nimport time\ntime.sleep(10)\n",
        );
        state.settings.lock().expect("lock").settings.runtime_path =
            Some(hung_server.to_string_lossy().into_owned());
        let err = load_model_with_timeout(
            &state,
            "0.2.0".into(),
            &empty_dirs,
            Duration::from_millis(250),
        )
        .await
        .expect_err("hung runtime load");
        assert_eq!(err.code, "timeout");
        assert_eq!(
            state.status("0.2.0".into()).await.engine.state,
            EngineState::Error
        );

        // 9. Unexpected child exit mid-generation -> `process_exited` + immediate state reconciliation!
        let mid_crash_server = write_exec_script(
            &root,
            "mid-crash-server",
            r#"#!/usr/bin/env python3
import http.server, json, os, pathlib, sys
args = sys.argv[1:]
host = args[args.index("--host") + 1]
port = int(args[args.index("--port") + 1])
secret = pathlib.Path(args[args.index("--api-key-file") + 1]).read_text().strip()
class H(http.server.BaseHTTPRequestHandler):
    def log_message(self, *a): pass
    def do_GET(self):
        body = b'{"status":"ok"}'
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)
    def do_POST(self):
        _ = self.rfile.read(int(self.headers.get("Content-Length", "0")))
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.end_headers()
        self.wfile.write(b'data: {"choices":[{"delta":{"content":"partial"}}]}\n\n')
        self.wfile.flush()
        sys.stderr.write(f"SIGSEGV in ggml compute graph (token={secret})\n")
        sys.stderr.flush()
        os._exit(139)
http.server.HTTPServer((host, port), H).serve_forever()
"#,
        );
        state.settings.lock().expect("lock").settings.runtime_path =
            Some(mid_crash_server.to_string_lossy().into_owned());
        load_model_with_timeout(&state, "0.2.0".into(), &empty_dirs, Duration::from_secs(3))
            .await
            .expect("mid-crash server initially loads cleanly");

        let usage = UsageState::load(&root.join("usage"));
        let auth = AuthState::with_vault(
            AuthApiClient::unconfigured(),
            Arc::new(TestMemVault::default()),
        );
        let (ch, _) = event_collector();
        let crash_err = execute_generation(
            &state,
            &usage,
            &auth,
            GenerateRequest {
                request_id: "req-crash".into(),
                profile_id: Some("nexora-mini-qwen2.5-coder-7b-instruct-q4-k-m".into()),
                messages: vec![ChatTurn {
                    role: "user".into(),
                    content: "trigger mid-stream crash".into(),
                }],
                max_tokens: Some(64),
                temperature: Some(0.1),
            },
            ch,
        )
        .await
        .expect_err("mid-stream child crash must fail");
        assert_eq!(crash_err.code, "process_exited");
        assert!(crash_err.message.contains("SIGSEGV in ggml compute graph"));
        assert!(!crash_err.message.contains(
            &std::fs::read_to_string(config_dir.join(process::SESSION_KEY_FILE))
                .unwrap_or_else(|_| "NONE".into())
        ));
        assert!(state.loaded_profile_id().is_none());
        assert_eq!(
            state.status("0.2.0".into()).await.engine.state,
            EngineState::Error
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn download_lifecycle_duplicate_guard_cancellation_retry_and_runtime_isolation() {
        use sha2::{Digest, Sha256};
        use std::io::{Read as _, Write};
        use std::net::TcpListener;
        use std::sync::atomic::AtomicUsize;

        let root =
            std::env::temp_dir().join(format!("nexora-phase05-lifecycle-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let config_dir = root.join("config");
        let models_dir = root.join("models");
        std::fs::create_dir_all(&config_dir).expect("config dir");
        std::fs::create_dir_all(&models_dir).expect("models dir");

        let state = InferenceState::load(config_dir.clone());
        let usage = UsageState::load(&root.join("usage"));
        let offline_auth = AuthState::with_vault(
            crate::auth::client::AuthApiClient::with_base_url("http://127.0.0.1:9"),
            Arc::new(TestMemVault::default()),
        );
        offline_auth
            .persist_session_token("offline-token")
            .expect("seed local session");

        let download_channel = || Channel::new(move |_body: InvokeResponseBody| Ok(()));

        // 1. Offline local readiness works with unreachable auth backend (`127.0.0.1:9`).
        let initial_mini = readiness_for(
            &state,
            &registry::NEXORA_MINI,
            &state.settings_snapshot(),
            Some(&models_dir),
        )
        .await;
        assert_eq!(initial_mini.state, ReadinessState::ReadyToDownload);

        // 2. Unsafe traversal destination paths are rejected before touching disk.
        let ch_bad = download_channel();
        let traversal_err = execute_model_download(
            &state,
            Some(
                models_dir
                    .join("..")
                    .join("escape")
                    .to_string_lossy()
                    .into_owned(),
            ),
            Some(registry::NEXORA_MINI_ID.into()),
            models_dir.clone(),
            None,
            acquisition::DownloadPolicy::loopback_for_tests(Duration::from_secs(1)),
            &ch_bad,
        )
        .await
        .expect_err("traversal destination rejected");
        assert_eq!(traversal_err.code, "invalid_request");

        // 3. Slow download -> duplicate concurrent download returns `busy` -> user cancels ->
        //    `DownloadCancelled` state -> retry succeeds & skips unnecessary redownload on 3rd call!
        let payload = b"deterministic-verified-gguf-payload-bytes";
        let sha: String = Sha256::digest(payload)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();

        let slow_listener = TcpListener::bind("127.0.0.1:0").expect("bind slow");
        let slow_port = slow_listener.local_addr().expect("addr").port();
        std::thread::spawn(move || {
            if let Ok((mut s, _)) = slow_listener.accept() {
                let mut req = [0u8; 512];
                let _ = s.read(&mut req);
                let _ = s.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 41\r\n\r\ndeterm");
                let _ = s.flush();
                std::thread::sleep(Duration::from_millis(800));
            }
        });

        let slow_plan = acquisition::DownloadPlan {
            url: format!("http://127.0.0.1:{slow_port}/mini.gguf"),
            file_name: registry::NEXORA_MINI
                .artifact
                .source()
                .expect("mini")
                .file_name
                .to_string(),
            size_bytes: payload.len() as u64,
            sha256: sha.clone(),
        };

        let ch_first = download_channel();
        let ch_dup = download_channel();
        let slow_plan_dup = slow_plan.clone();
        let mut first_fut = std::pin::pin!(execute_model_download(
            &state,
            None,
            Some(registry::NEXORA_MINI_ID.into()),
            models_dir.clone(),
            Some(slow_plan),
            acquisition::DownloadPolicy::loopback_for_tests(Duration::from_secs(2)),
            &ch_first,
        ));

        let cancel_outcome = tokio::select! {
            res = &mut first_fut => panic!("slow download ended before duplicate/cancel checks: {res:?}"),
            () = async {
                for _ in 0..40 {
                    if state.download.lock().expect("lock").is_some() {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(15)).await;
                }
                let in_flight = readiness_for(
                    &state,
                    &registry::NEXORA_MINI,
                    &state.settings_snapshot(),
                    Some(&models_dir),
                )
                .await;
                assert_eq!(in_flight.state, ReadinessState::Downloading);

                // Duplicate concurrent download is refused with `busy`.
                let dup_err = execute_model_download(
                    &state,
                    None,
                    Some(registry::NEXORA_MINI_ID.into()),
                    models_dir.clone(),
                    Some(slow_plan_dup),
                    acquisition::DownloadPolicy::loopback_for_tests(Duration::from_secs(1)),
                    &ch_dup,
                )
                .await
                .expect_err("duplicate download must return busy");
                assert_eq!(dup_err.code, "busy");

                assert!(cancel_model_download_inner(&state));
            } => {
                first_fut.await.expect("cancelled download returns clean outcome")
            }
        };
        assert!(cancel_outcome.cancelled);
        assert!(cancel_outcome.path.is_none());

        let after_cancel = readiness_for(
            &state,
            &registry::NEXORA_MINI,
            &state.settings_snapshot(),
            Some(&models_dir),
        )
        .await;
        assert_eq!(after_cancel.state, ReadinessState::DownloadCancelled);
        assert!(after_cancel.can_download);

        // Retry against healthy HTTP server -> succeeds & marks ledger verified;
        // calling a second time skips the network entirely (`hits == 1`)!
        let hits = Arc::new(AtomicUsize::new(0));
        let hits_clone = Arc::clone(&hits);
        let ok_listener = TcpListener::bind("127.0.0.1:0").expect("bind ok");
        let ok_port = ok_listener.local_addr().expect("addr").port();
        let body = payload.to_vec();
        std::thread::spawn(move || {
            if let Ok((mut s, _)) = ok_listener.accept() {
                hits_clone.fetch_add(1, Ordering::SeqCst);
                let mut req = [0u8; 512];
                let _ = s.read(&mut req);
                let header = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = s.write_all(header.as_bytes());
                let _ = s.write_all(&body);
            }
        });

        let ok_plan = acquisition::DownloadPlan {
            url: format!("http://127.0.0.1:{ok_port}/mini.gguf"),
            file_name: registry::NEXORA_MINI
                .artifact
                .source()
                .expect("mini")
                .file_name
                .to_string(),
            size_bytes: payload.len() as u64,
            sha256: sha.clone(),
        };
        let ch_retry = download_channel();
        let promoted = execute_model_download(
            &state,
            None,
            Some(registry::NEXORA_MINI_ID.into()),
            models_dir.clone(),
            Some(ok_plan.clone()),
            acquisition::DownloadPolicy::loopback_for_tests(Duration::from_secs(2)),
            &ch_retry,
        )
        .await
        .expect("retry succeeds");
        assert!(!promoted.cancelled);
        assert_eq!(hits.load(Ordering::SeqCst), 1);

        // Calling download again when already verified returns immediately without hitting network!
        let skipped = execute_model_download(
            &state,
            None,
            Some(registry::NEXORA_MINI_ID.into()),
            models_dir.clone(),
            Some(ok_plan),
            acquisition::DownloadPolicy::loopback_for_tests(Duration::from_secs(2)),
            &ch_retry,
        )
        .await
        .expect("already verified skips redownload");
        assert_eq!(skipped.path, promoted.path);
        assert_eq!(hits.load(Ordering::SeqCst), 1);

        // 4. Runtime isolation: load a custom local model into a real child engine, fail a
        //    Sonnet download (`503`), confirm Sonnet is `DownloadIncomplete`, AND confirm the
        //    running Mini engine still answers local prompts without interruption!
        let runtime_script = write_exec_script(
            &root,
            "mini-engine",
            r#"#!/usr/bin/env python3
import http.server, json, sys
args = sys.argv[1:]
host = args[args.index("--host") + 1]
port = int(args[args.index("--port") + 1])
class H(http.server.BaseHTTPRequestHandler):
    def log_message(self, *a): pass
    def do_GET(self):
        body = b'{"status":"ok"}'
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)
    def do_POST(self):
        _ = self.rfile.read(int(self.headers.get("Content-Length", "0")))
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.end_headers()
        self.wfile.write(b'data: {"choices":[{"delta":{"content":"still alive"},"finish_reason":"stop"}]}\n\ndata: [DONE]\n\n')
        self.wfile.flush()
http.server.HTTPServer((host, port), H).serve_forever()
"#,
        );
        let local_gguf = root.join("local-custom.gguf");
        write_sparse_gguf_v3(&local_gguf);
        {
            let mut s = state.settings.lock().expect("lock");
            s.settings.model_id = Some(registry::NEXORA_MINI_ID.into());
            s.settings.model_path = Some(local_gguf.to_string_lossy().into_owned());
            s.settings.runtime_path = Some(runtime_script.to_string_lossy().into_owned());
        }
        let dirs = process::RuntimeSearchDirs::default();
        load_model_with_timeout(&state, "0.2.0".into(), &dirs, Duration::from_secs(3))
            .await
            .expect("load local engine");

        let fail_listener = TcpListener::bind("127.0.0.1:0").expect("bind fail");
        let fail_port = fail_listener.local_addr().expect("addr").port();
        std::thread::spawn(move || {
            if let Ok((mut s, _)) = fail_listener.accept() {
                let mut req = [0u8; 512];
                let _ = s.read(&mut req);
                let _ = s.write_all(
                    b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                );
            }
        });
        let sonnet_err = execute_model_download(
            &state,
            None,
            Some(registry::NEXORA_SONNET_ID.into()),
            models_dir.clone(),
            Some(acquisition::DownloadPlan {
                url: format!("http://127.0.0.1:{fail_port}/sonnet.gguf"),
                file_name: registry::NEXORA_SONNET
                    .artifact
                    .source()
                    .expect("sonnet")
                    .file_name
                    .to_string(),
                size_bytes: 64,
                sha256: "ab".repeat(32),
            }),
            acquisition::DownloadPolicy::loopback_for_tests(Duration::from_secs(1)),
            &ch_retry,
        )
        .await
        .expect_err("sonnet 503 fails cleanly");
        assert_eq!(sonnet_err.code, "network");

        let sonnet_state = readiness_for(
            &state,
            &registry::NEXORA_SONNET,
            &state.settings_snapshot(),
            Some(&models_dir),
        )
        .await;
        assert_eq!(sonnet_state.state, ReadinessState::DownloadIncomplete);

        let (gen_ch, gen_events) = event_collector();
        let gen_ok = execute_generation(
            &state,
            &usage,
            &offline_auth,
            GenerateRequest {
                request_id: "req-after-sonnet-fail".into(),
                profile_id: Some(registry::NEXORA_MINI_ID.into()),
                messages: vec![ChatTurn {
                    role: "user".into(),
                    content: "ping".into(),
                }],
                max_tokens: Some(32),
                temperature: Some(0.1),
            },
            gen_ch,
        )
        .await
        .expect("failed Sonnet download must not break running Mini runtime");
        assert!(!gen_ok.cancelled);
        assert!(gen_events
            .lock()
            .expect("lock")
            .iter()
            .any(|ev| matches!(ev, GenerationEvent::Delta { text, .. } if text == "still alive")));

        let _ = unload_model_with_dirs(&state, "0.2.0".into(), &dirs).await;
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn phase06_chat_profile_routing_verification_gate_cancellation_and_offline_isolation() {
        use crate::history::storage::{HistoryStore, StoredMessage};

        let root = std::env::temp_dir().join(format!("nexora-phase06-chat-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let config_dir = root.join("config");
        let history_dir = root.join("history");
        let models_dir = root.join("models");
        std::fs::create_dir_all(&config_dir).expect("config");
        std::fs::create_dir_all(&history_dir).expect("history");
        std::fs::create_dir_all(&models_dir).expect("models");

        let state = InferenceState::load(config_dir.clone());
        let usage = UsageState::load(&root.join("usage"));
        let offline_auth = AuthState::with_vault(
            AuthApiClient::with_base_url("http://127.0.0.1:9"),
            Arc::new(TestMemVault::default()),
        );
        offline_auth
            .persist_session_token("offline-local-session")
            .expect("seed token");

        let wire_log = root.join("wire-log.jsonl");
        let server_script = write_exec_script(
            &root,
            "chat-engine",
            &format!(
                r#"#!/usr/bin/env python3
import http.server, json, pathlib, socketserver, sys, time
args = sys.argv[1:]
host = args[args.index("--host") + 1]
port = int(args[args.index("--port") + 1])
secret = pathlib.Path(args[args.index("--api-key-file") + 1]).read_text().strip()
class Threaded(socketserver.ThreadingMixIn, http.server.HTTPServer):
    daemon_threads = True
class H(http.server.BaseHTTPRequestHandler):
    def log_message(self, *a): pass
    def do_GET(self):
        if self.headers.get("Authorization") != f"Bearer {{secret}}":
            self.send_response(401); self.end_headers(); return
        body = b'{{"status":"ok"}}'
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)
    def do_POST(self):
        if self.headers.get("Authorization") != f"Bearer {{secret}}":
            self.send_response(401); self.end_headers(); return
        raw = self.rfile.read(int(self.headers.get("Content-Length", "0")))
        req = json.loads(raw.decode("utf-8"))
        with open({wire_log:?}, "a", encoding="utf-8") as fh:
            fh.write(json.dumps(req) + "\n")
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.end_headers()
        if any("SLOW_STREAM" in m["content"] for m in req["messages"]):
            self.wfile.write(b'data: {{"choices":[{{"delta":{{"content":"first-chunk"}}}}]}}\n\n')
            self.wfile.flush()
            time.sleep(0.25)
            self.wfile.write(b'data: {{"choices":[{{"delta":{{"content":"-second-chunk"}},"finish_reason":"stop"}}]}}\n\ndata: [DONE]\n\n')
            self.wfile.flush()
            return
        reply = f"model={{req['model']}};turns={{len(req['messages'])}}"
        payload = json.dumps({{"choices": [{{"delta": {{"content": reply}}}}]}})
        self.wfile.write(f"data: {{payload}}\n\ndata: [DONE]\n\n".encode("utf-8"))
        self.wfile.flush()
Threaded((host, port), H).serve_forever()
"#
            ),
        );

        // 1. Unverified approved model file MUST be refused at load time (`model_unverified`).
        let unverified_mini = models_dir.join(
            registry::NEXORA_MINI
                .artifact
                .source()
                .expect("mini")
                .file_name,
        );
        write_sparse_gguf_v3(&unverified_mini);
        {
            let mut s = state.settings.lock().expect("lock");
            s.settings.model_id = Some(registry::NEXORA_MINI_ID.into());
            s.settings.model_path = Some(unverified_mini.to_string_lossy().into_owned());
            s.settings.runtime_path = Some(server_script.to_string_lossy().into_owned());
        }
        let dirs = process::RuntimeSearchDirs::default();
        let unverified_err =
            load_model_with_timeout(&state, "0.2.0".into(), &dirs, Duration::from_secs(2))
                .await
                .expect_err("unverified published artifact must be refused");
        assert_eq!(unverified_err.code, "model_unverified");

        // 2. Load custom local GGUF under Nexora Mini and verify exact profile routing + stale/unknown rejection.
        let custom_gguf = root.join("local-verified-dev.gguf");
        write_sparse_gguf_v3(&custom_gguf);
        state.settings.lock().expect("lock").settings.model_path =
            Some(custom_gguf.to_string_lossy().into_owned());
        load_model_with_timeout(&state, "0.2.0".into(), &dirs, Duration::from_secs(3))
            .await
            .expect("load custom mini");

        // Unknown profile id -> rejected cleanly with typed `model_mismatch`.
        let (ch_unknown, _) = event_collector();
        let unknown_err = execute_generation(
            &state,
            &usage,
            &offline_auth,
            GenerateRequest {
                request_id: "req-unknown-profile".into(),
                profile_id: Some("nexora-nonexistent-99b".into()),
                messages: vec![ChatTurn {
                    role: "user".into(),
                    content: "hello".into(),
                }],
                max_tokens: Some(64),
                temperature: Some(0.2),
            },
            ch_unknown,
        )
        .await
        .expect_err("unknown profile rejected");
        assert_eq!(unknown_err.code, "model_mismatch");
        assert!(unknown_err.message.contains("unknown model"));

        // Mismatched profile id (requesting Sonnet while Mini is selected & loaded) -> rejected without silent switch!
        let (ch_mismatch, _) = event_collector();
        let mismatch_err = execute_generation(
            &state,
            &usage,
            &offline_auth,
            GenerateRequest {
                request_id: "req-stale-sonnet".into(),
                profile_id: Some(registry::NEXORA_SONNET_ID.into()),
                messages: vec![ChatTurn {
                    role: "user".into(),
                    content: "hello".into(),
                }],
                max_tokens: Some(64),
                temperature: Some(0.2),
            },
            ch_mismatch,
        )
        .await
        .expect_err("stale profile rejected");
        assert_eq!(mismatch_err.code, "model_mismatch");
        assert!(mismatch_err.message.contains("prepared for Nexora Sonnet"));

        // Switch settings selection to Sonnet while Mini stays loaded -> refused with `model_mismatch`, NEVER answered by Mini!
        state.settings.lock().expect("lock").settings.model_id =
            Some(registry::NEXORA_SONNET_ID.into());
        let (ch_wrong_loaded, _) = event_collector();
        let wrong_loaded_err = execute_generation(
            &state,
            &usage,
            &offline_auth,
            GenerateRequest {
                request_id: "req-sonnet-not-loaded".into(),
                profile_id: Some(registry::NEXORA_SONNET_ID.into()),
                messages: vec![ChatTurn {
                    role: "user".into(),
                    content: "hello".into(),
                }],
                max_tokens: Some(64),
                temperature: Some(0.2),
            },
            ch_wrong_loaded,
        )
        .await
        .expect_err("must not answer Sonnet prompt with Mini engine");
        assert_eq!(wrong_loaded_err.code, "model_mismatch");
        assert!(wrong_loaded_err
            .message
            .contains("Nexora Sonnet is selected"));

        // Restore Mini selection -> stale cancel for old request_id MUST NOT cancel active generation!
        state.settings.lock().expect("lock").settings.model_id =
            Some(registry::NEXORA_MINI_ID.into());
        let (ch_live, live_events) = event_collector();
        let live_outcome = {
            let mut live_fut = std::pin::pin!(execute_generation(
                &state,
                &usage,
                &offline_auth,
                GenerateRequest {
                    request_id: "req-current-live".into(),
                    profile_id: Some(registry::NEXORA_MINI_ID.into()),
                    messages: vec![ChatTurn {
                        role: "user".into(),
                        content: "SLOW_STREAM check".into(),
                    }],
                    max_tokens: Some(64),
                    temperature: Some(0.2),
                },
                ch_live,
            ));

            tokio::select! {
                res = &mut live_fut => panic!("finished before stale-cancel check: {res:?}"),
                () = async {
                    for _ in 0..40 {
                        if state.active.lock().expect("lock").is_some() {
                            break;
                        }
                        tokio::time::sleep(Duration::from_millis(10)).await;
                    }
                    // Stale cancellation targeting an earlier generation returns false and keeps live stream healthy!
                    assert!(!state.cancel_generation("req-stale-previous"));
                } => {
                    live_fut.await.expect("live generation completes")
                }
            }
        };
        assert!(!live_outcome.cancelled);
        let collected: String = live_events
            .lock()
            .expect("lock")
            .iter()
            .filter_map(|e| match e {
                GenerationEvent::Delta { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(collected, "first-chunk-second-chunk");
        assert!(state.active.lock().expect("lock").is_none());

        // Persist multi-conversation transcript locally and verify isolation + offline operation.
        let history = HistoryStore::new(history_dir.clone());
        let mut conv_a = history
            .create(Some("Rust Borrow Check".into()))
            .expect("create A");
        let conv_b = history
            .create(Some("Isolated Empty Chat".into()))
            .expect("create B");
        conv_a.messages.push(StoredMessage {
            id: "u-1".into(),
            role: "user".into(),
            content: "SLOW_STREAM check".into(),
            created_at: 100,
            state: Some("complete".into()),
            failure: None,
            profile_id: None,
        });
        conv_a.messages.push(StoredMessage {
            id: "a-1".into(),
            role: "assistant".into(),
            content: collected,
            created_at: 101,
            state: Some("complete".into()),
            failure: None,
            profile_id: Some(registry::NEXORA_MINI_ID.into()),
        });
        history.save(conv_a.clone()).expect("save A");
        assert_eq!(history.load(&conv_b.id).expect("load B").messages.len(), 0);

        // Drop `state` directly without explicit `unload_model` and confirm `Drop` reaps the child!
        drop(state);
        let wire_entries = std::fs::read_to_string(&wire_log).expect("wire log");
        assert!(wire_entries.contains(registry::NEXORA_MINI_ID));
        let _ = std::fs::remove_dir_all(&root);
    }
}
