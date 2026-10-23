//! Local inference: process supervision, model validation, and streaming chat.
//!
//! The engine is a child process talked to over loopback HTTP. Nothing in this
//! module accepts commands, sizes, or paths from the model, and no model output
//! is ever executed.

pub mod acquisition;
pub mod budget;
pub mod client;
pub mod commands;
pub mod error;
pub mod hardware;
pub mod loading;
pub mod models;
pub mod process;
pub mod readiness;
pub mod registry;
pub mod settings;
pub mod sse;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use tokio::sync::Notify;

use self::error::{InferenceError, InferenceResult};
use self::settings::{Settings, SettingsStore};

pub const MAX_TURNS: usize = 64;
pub const MAX_TURN_CHARS: usize = 100_000;
pub const MAX_CONVERSATION_CHARS: usize = 400_000;
pub const MAX_OUTPUT_TOKENS: u32 = 8_192;

/// One chat turn as sent by the UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatTurn {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateRequest {
    pub request_id: String,
    pub messages: Vec<ChatTurn>,
    #[serde(default)]
    pub max_tokens: Option<u32>,
    #[serde(default)]
    pub temperature: Option<f32>,
    /// The profile the UI prepared this message under. When present it must be
    /// the selected profile; the core never trusts it instead of its settings.
    #[serde(default)]
    pub profile_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ValidatedRequest {
    pub max_tokens: u32,
    pub temperature: f32,
}

impl GenerateRequest {
    /// Rejects malformed or oversized input before it reaches the engine.
    pub fn validate(&self) -> InferenceResult<ValidatedRequest> {
        if self.request_id.trim().is_empty() {
            return Err(InferenceError::invalid_request(
                "The request is missing an identifier.",
            ));
        }

        if self.request_id.len() > 120 {
            return Err(InferenceError::invalid_request(
                "The request identifier is too long.",
            ));
        }

        if self.request_id.trim() != self.request_id
            || self.request_id.chars().any(char::is_control)
        {
            return Err(InferenceError::invalid_request(
                "The request identifier contains invalid characters.",
            ));
        }

        if self.messages.is_empty() {
            return Err(InferenceError::invalid_request("There is nothing to send."));
        }

        if self.messages.len() > MAX_TURNS {
            return Err(InferenceError::invalid_request(format!(
                "A conversation can contain at most {MAX_TURNS} turns."
            )));
        }

        let mut total = 0usize;

        for turn in &self.messages {
            if !matches!(turn.role.as_str(), "system" | "user" | "assistant") {
                return Err(InferenceError::invalid_request(
                    "A message has an unsupported role.",
                ));
            }

            if turn.content.trim().is_empty() {
                return Err(InferenceError::invalid_request("A message is empty."));
            }

            let chars = turn.content.chars().count();

            if chars > MAX_TURN_CHARS {
                return Err(InferenceError::invalid_request(
                    "A single message is too long.",
                ));
            }

            total += chars;
        }

        if total > MAX_CONVERSATION_CHARS {
            return Err(InferenceError::invalid_request(
                "This conversation is too large for the local context. Start a new chat.",
            ));
        }

        let max_tokens = self.max_tokens.unwrap_or(1_024).clamp(1, MAX_OUTPUT_TOKENS);

        let temperature = self.temperature.unwrap_or(0.2).clamp(0.0, 2.0);

        Ok(ValidatedRequest {
            max_tokens,
            temperature,
        })
    }
}

/// Events streamed to the UI while a response is generated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(test, derive(Deserialize))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GenerationEvent {
    Started {
        #[serde(rename = "requestId")]
        request_id: String,
    },
    Delta {
        #[serde(rename = "requestId")]
        request_id: String,
        text: String,
    },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerationOutcome {
    pub request_id: String,
    pub finish_reason: Option<String>,
    pub cancelled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineState {
    NotConfigured,
    Stopped,
    Loading,
    Ready,
    Generating,
    Error,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineStatus {
    pub state: EngineState,
    pub runtime: Option<String>,
    pub model: Option<String>,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppStatus {
    pub name: &'static str,
    pub version: String,
    pub engine: EngineStatus,
    pub model_path: Option<String>,
    pub runtime_path: Option<String>,
    pub context_size: u32,
}

pub struct ActiveGeneration {
    pub request_id: String,
    pub cancel: Arc<Notify>,
    pub cancelled: Arc<AtomicBool>,
}

/// The model the running engine actually loaded.
///
/// Recorded at `load_model` time from the chosen path, so a request can be
/// refused when it would otherwise be answered by the wrong model. This is a
/// name-level attribution, not a proof about the bytes on disk.
#[derive(Debug, Clone)]
pub struct LoadedModel {
    /// The registered profile the file name belongs to, when it matches one.
    pub profile_id: Option<&'static str>,
    pub path: String,
    /// The context window the engine was actually started with. The settings
    /// can change while an engine is running; prompts must fit what is running,
    /// not what is configured.
    pub context_size: u32,
}

/// A load or runtime failure, attributed to a profile when it is known.
///
/// Kept separately from `loaded`: a failed load leaves nothing loaded, but the
/// row still needs to explain itself rather than silently offering the same
/// file again.
#[derive(Debug, Clone)]
pub struct LoadFailure {
    pub profile_id: Option<&'static str>,
    pub message: String,
}

/// A download that is writing to disk right now.
///
/// Carries the profile id so a second profile's row cannot be shown as
/// "downloading" just because some other file is in flight.
pub struct ActiveDownload {
    pub profile_id: &'static str,
    pub cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    pub verifying: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

pub struct InferenceState {
    pub supervisor: Mutex<process::Supervisor>,
    pub settings: Mutex<SettingsStore>,
    pub active: Mutex<Option<ActiveGeneration>>,
    /// What the engine last loaded. Cleared on unload and on a failed load.
    pub loaded: Mutex<Option<LoadedModel>>,
    /// Digest results for this session, per profile.
    pub verified: Mutex<readiness::VerificationLedger>,
    /// The last load or runtime failure, for rows and the engine panel.
    pub last_failure: Mutex<Option<LoadFailure>>,
    /// The profile a load is currently starting, before the engine confirms.
    ///
    /// The outer `Option` is "a load is in flight"; the inner one is the
    /// profile, which is unknown for a GGUF no profile publishes.
    pub loading: Mutex<Option<Option<&'static str>>>,
    /// Set while a model download runs, so a second one is refused and the
    /// first can be cancelled. `false` is never stored; the flag is created
    /// once per download.
    pub download: Mutex<Option<ActiveDownload>>,
    pub config_dir: PathBuf,
}

/// Pure state derivation, so engine transitions can be tested without spawning
/// a process. The order encodes the precedence: an in-flight generation wins,
/// then a healthy engine, then one that is up but not ready, then a recorded
/// failure, then configuration, and finally a configured-but-stopped engine.
fn classify_engine(
    running: bool,
    healthy: bool,
    generating: bool,
    configured: bool,
    last_error: Option<&str>,
) -> (EngineState, String) {
    if generating {
        return (
            EngineState::Generating,
            "Generating a reply on this machine.".to_string(),
        );
    }

    if running && healthy {
        return (
            EngineState::Ready,
            "The local model is loaded and ready.".to_string(),
        );
    }

    if running {
        return (
            EngineState::Loading,
            "The local engine is starting and loading the model.".to_string(),
        );
    }

    if let Some(error) = last_error {
        return (EngineState::Error, error.to_string());
    }

    if !configured {
        return (
            EngineState::NotConfigured,
            "Choose the llama-server runtime and a GGUF model to enable local inference."
                .to_string(),
        );
    }

    (
        EngineState::Stopped,
        "The engine is configured but not running. Load the model to start it.".to_string(),
    )
}

impl InferenceState {
    pub fn load(config_dir: PathBuf) -> Self {
        // A session key from a crashed run is useless — the process that read
        // it is gone — so it is removed rather than left on disk for days.
        let _ = std::fs::remove_file(config_dir.join(process::SESSION_KEY_FILE));

        Self {
            supervisor: Mutex::new(process::Supervisor::new()),
            settings: Mutex::new(SettingsStore::load(&config_dir)),
            active: Mutex::new(None),
            loaded: Mutex::new(None),
            verified: Mutex::new(readiness::VerificationLedger::default()),
            last_failure: Mutex::new(None),
            loading: Mutex::new(None),
            download: Mutex::new(None),
            config_dir,
        }
    }

    /// The context window of the running engine, when one is recorded.
    pub fn loaded_context(&self) -> Option<u32> {
        self.loaded
            .lock()
            .ok()
            .and_then(|slot| slot.as_ref().map(|model| model.context_size))
    }

    /// Records the model the engine is now running.
    pub fn set_loaded(&self, model: LoadedModel) {
        if let Ok(mut slot) = self.loaded.lock() {
            *slot = Some(model);
        }
    }

    /// Claims the single load slot, returning false when a load is already in
    /// flight.
    ///
    /// Check-and-set happen under one lock, so two clicks cannot start two
    /// engine processes. Readiness uses the marker to show "Loading" instead of
    /// "Ready to load". It is not evidence of success: only `set_loaded`
    /// follows a health check.
    pub fn begin_loading(&self, profile_id: Option<&'static str>) -> bool {
        match self.loading.lock() {
            Ok(mut slot) if slot.is_none() => {
                *slot = Some(profile_id);
                true
            }
            _ => false,
        }
    }

    /// Clears the in-flight load marker, on success and failure alike.
    pub fn finish_loading(&self) {
        if let Ok(mut slot) = self.loading.lock() {
            *slot = None;
        }
    }

    /// Whether any load is in flight, including one for a custom GGUF whose
    /// profile is unknown.
    pub fn load_in_flight(&self) -> bool {
        self.loading
            .lock()
            .map(|slot| slot.is_some())
            .unwrap_or(false)
    }

    /// The profile whose load is in flight, when one is.
    pub fn loading_profile_id(&self) -> Option<&'static str> {
        self.loading.lock().ok().and_then(|slot| *slot).flatten()
    }

    /// Records why a load or runtime attempt failed, attributed when known.
    pub fn record_failure(&self, profile_id: Option<&'static str>, message: impl Into<String>) {
        if let Ok(mut slot) = self.last_failure.lock() {
            *slot = Some(LoadFailure {
                profile_id,
                message: message.into(),
            });
        }
    }

    /// Clears the recorded failure after a load that actually succeeded.
    pub fn clear_failure(&self) {
        if let Ok(mut slot) = self.last_failure.lock() {
            *slot = None;
        }
    }

    /// The profile the last failure was attributed to, when it was.
    pub fn failed_profile_id(&self) -> Option<&'static str> {
        self.last_failure
            .lock()
            .ok()
            .and_then(|slot| slot.as_ref().and_then(|failure| failure.profile_id))
    }

    /// The recorded reason the last load or runtime attempt failed.
    ///
    /// A failure can happen before a child process exists (a missing file, an
    /// unusable runtime path), so the supervisor has no stderr tail to report.
    /// Without this the panel would fall back to "configured but stopped" and
    /// hide the reason entirely.
    pub fn failure_message(&self) -> Option<String> {
        self.last_failure
            .lock()
            .ok()
            .and_then(|slot| slot.as_ref().map(|failure| failure.message.clone()))
    }

    /// Reaps a crashed engine and drops the "loaded" mark that is no longer
    /// true.
    ///
    /// Returns true when something was actually cleared. The supervisor keeps
    /// the reason the child died, which is recorded against the profile that
    /// was loaded so readiness reports a runtime failure instead of pretending
    /// the file is merely ready to load again.
    pub fn reconcile_engine(&self) -> bool {
        let reason = {
            let mut supervisor = match self.supervisor.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };

            if supervisor.is_running() {
                return false;
            }

            supervisor.last_error.clone()
        };

        let previous = self.loaded.lock().ok().and_then(|mut slot| slot.take());

        let Some(model) = previous else {
            return false;
        };

        self.record_failure(
            model.profile_id,
            reason.unwrap_or_else(|| "The local engine stopped unexpectedly.".to_string()),
        );

        true
    }

    /// Forgets the loaded model after an unload or a failed load.
    pub fn clear_loaded(&self) {
        if let Ok(mut slot) = self.loaded.lock() {
            *slot = None;
        }
    }

    /// The registered profile id the engine is running, when it is known.
    pub fn loaded_profile_id(&self) -> Option<&'static str> {
        self.loaded
            .lock()
            .ok()
            .and_then(|slot| slot.as_ref().and_then(|model| model.profile_id))
    }

    /// Digest status for a profile **at one path**, as `(verified, failed)`.
    ///
    /// Path-aware on purpose: confirming a copy in one folder must not bless a
    /// different file of the same name somewhere else.
    pub fn verification_status(
        &self,
        profile_id: &str,
        path: Option<&std::path::Path>,
    ) -> (bool, bool) {
        let Some(path) = path else {
            return (false, false);
        };

        self.verified
            .lock()
            .map(|ledger| {
                (
                    ledger.is_verified(profile_id, path),
                    ledger.has_failed(profile_id),
                )
            })
            .unwrap_or((false, false))
    }

    pub fn settings_snapshot(&self) -> Settings {
        self.settings
            .lock()
            .map(|store| store.settings.clone())
            .unwrap_or_default()
    }

    /// Builds the status shown in the UI from the real process state.
    ///
    /// A failed health check is reported as `loading` or `error`, never as
    /// "ready", and a configured-but-stopped engine is distinct from an
    /// unconfigured one.
    #[cfg(test)]
    pub async fn status(&self, version: String) -> AppStatus {
        let dirs = process::RuntimeSearchDirs {
            exe_dir: std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(std::path::Path::to_path_buf)),
            resource_dir: None,
            data_dir: Some(self.config_dir.clone()),
        };
        self.status_with_dirs(version, &dirs).await
    }

    /// Same as [`Self::status`], additionally checking caller-supplied bundle
    /// resource/sidecar search directories when `settings.runtime_path` is unset.
    pub async fn status_with_dirs(
        &self,
        version: String,
        dirs: &process::RuntimeSearchDirs,
    ) -> AppStatus {
        // A child that died since the last read makes the "loaded" mark stale.
        // Reap it here, before anything reports what is running.
        self.reconcile_engine();

        let settings = self.settings_snapshot();

        let (running, endpoint, last_error) = {
            let mut supervisor = match self.supervisor.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };

            let running = supervisor.is_running();

            (
                running,
                supervisor.endpoint(),
                supervisor.last_error.clone(),
            )
        };

        let generating = self
            .active
            .lock()
            .map(|slot| slot.is_some())
            .unwrap_or(false);

        // The supervisor only knows about a child that started. A load that
        // failed before spawning still has a recorded reason; report it rather
        // than pretending the engine is merely stopped.
        let last_error = last_error.or_else(|| self.failure_message());

        let healthy = match (running, endpoint) {
            (true, Some((port, key))) => process::health_ok_with_key(port, Some(&key)).await,
            _ => false,
        };

        let discovered_runtime = process::discover_sidecar(dirs)
            .ok()
            .flatten()
            .map(|path| path.to_string_lossy().into_owned());
        let resolved_runtime = settings.runtime_path.clone().or(discovered_runtime);
        let configured = settings.model_path.is_some() && resolved_runtime.is_some();
        let (state, detail) = classify_engine(
            running,
            healthy,
            generating,
            configured,
            last_error.as_deref(),
        );

        // While an engine is recorded as loaded, the panel must name the file
        // that engine is actually running. `settings.model_path` can be edited
        // without a reload, and reporting it here would claim the running
        // process had changed model. Once the engine stops, the configured path
        // is the honest answer again.
        let running_path = self
            .loaded
            .lock()
            .ok()
            .and_then(|slot| slot.as_ref().map(|model| model.path.clone()));

        AppStatus {
            name: "Nexora",
            version,
            engine: EngineStatus {
                state,
                runtime: resolved_runtime,
                // Owned up front: `settings.model_path` is moved into this
                // payload a few lines below, and a borrow here would fight it.
                model: running_path
                    .or_else(|| settings.model_path.clone())
                    .as_deref()
                    .and_then(|path| {
                        std::path::Path::new(path)
                            .file_name()
                            .and_then(|name| name.to_str())
                            .map(str::to_string)
                    }),
                detail,
            },
            // Configuration, not the running engine: the setup form prefills
            // from these and must not be rewritten under the user.
            model_path: settings.model_path,
            runtime_path: settings.runtime_path,
            context_size: settings.context_size,
        }
    }

    /// Clears the active generation only when it is still the same request.
    pub fn finish_generation(&self, request_id: &str) {
        if let Ok(mut slot) = self.active.lock() {
            let matches = slot
                .as_ref()
                .map(|active| active.request_id == request_id)
                .unwrap_or(false);

            if matches {
                *slot = None;
            }
        }
    }

    pub fn cancel_generation(&self, request_id: &str) -> bool {
        let Ok(slot) = self.active.lock() else {
            return false;
        };

        let Some(active) = slot.as_ref() else {
            return false;
        };

        if active.request_id != request_id {
            return false;
        }

        active.cancelled.store(true, Ordering::SeqCst);
        active.cancel.notify_one();

        true
    }

    /// Cancels all active background tasks and reaps any supervised child runtime.
    pub fn shutdown(&self) {
        if let Ok(slot) = self.active.lock() {
            if let Some(active) = slot.as_ref() {
                active.cancelled.store(true, Ordering::SeqCst);
                active.cancel.notify_one();
            }
        }

        if let Ok(slot) = self.download.lock() {
            if let Some(download) = slot.as_ref() {
                download.cancel.store(true, Ordering::SeqCst);
            }
        }

        let mut supervisor = match self.supervisor.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        supervisor.stop();
        drop(supervisor);
        self.clear_loaded();
    }
}

impl Drop for InferenceState {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(messages: Vec<ChatTurn>) -> GenerateRequest {
        GenerateRequest {
            request_id: "req-1".into(),
            messages,
            max_tokens: None,
            temperature: None,
            profile_id: None,
        }
    }

    fn turn(role: &str, content: &str) -> ChatTurn {
        ChatTurn {
            role: role.into(),
            content: content.into(),
        }
    }

    #[test]
    fn accepts_a_normal_request_with_defaults() {
        let validated = request(vec![turn("user", "hello")])
            .validate()
            .expect("valid");

        assert_eq!(validated.max_tokens, 1_024);
        assert!((validated.temperature - 0.2).abs() < f32::EPSILON);
    }

    #[test]
    fn clamps_tokens_and_temperature() {
        let mut req = request(vec![turn("user", "hello")]);
        req.max_tokens = Some(999_999);
        req.temperature = Some(-4.0);

        let validated = req.validate().expect("valid");

        assert_eq!(validated.max_tokens, MAX_OUTPUT_TOKENS);
        assert!(validated.temperature >= 0.0);
    }

    #[test]
    fn rejects_an_empty_conversation() {
        let error = request(Vec::new()).validate().unwrap_err();
        assert_eq!(error.code, "invalid_request");
    }

    #[test]
    fn rejects_an_unknown_role() {
        let error = request(vec![turn("tool", "hi")]).validate().unwrap_err();
        assert_eq!(error.code, "invalid_request");
    }

    #[test]
    fn rejects_an_empty_message() {
        let error = request(vec![turn("user", "   ")]).validate().unwrap_err();
        assert_eq!(error.code, "invalid_request");
    }

    #[test]
    fn rejects_oversized_input() {
        let huge = "a".repeat(MAX_TURN_CHARS + 1);
        let error = request(vec![turn("user", &huge)]).validate().unwrap_err();

        assert_eq!(error.code, "invalid_request");
    }

    #[test]
    fn reports_each_engine_transition() {
        let ready = classify_engine(true, true, false, true, None);
        assert!(matches!(ready.0, EngineState::Ready));

        let loading = classify_engine(true, false, false, true, None);
        assert!(matches!(loading.0, EngineState::Loading));

        let generating = classify_engine(true, true, true, true, None);
        assert!(matches!(generating.0, EngineState::Generating));

        let stopped = classify_engine(false, false, false, true, None);
        assert!(matches!(stopped.0, EngineState::Stopped));

        let unconfigured = classify_engine(false, false, false, false, None);
        assert!(matches!(unconfigured.0, EngineState::NotConfigured));

        let failed = classify_engine(false, false, false, true, Some("it crashed"));
        assert!(matches!(failed.0, EngineState::Error));
        assert_eq!(failed.1, "it crashed");
    }

    #[test]
    fn a_failure_outranks_a_missing_configuration() {
        let (state, _) = classify_engine(false, false, false, false, Some("no device"));

        assert!(matches!(state, EngineState::Error));
    }

    #[test]
    fn generating_wins_over_a_transient_health_failure() {
        let (state, _) = classify_engine(true, false, true, true, None);

        assert!(matches!(state, EngineState::Generating));
    }

    #[test]
    fn rejects_too_many_turns() {
        let messages = (0..=MAX_TURNS)
            .map(|index| turn("user", &format!("message {index}")))
            .collect();

        let error = request(messages).validate().unwrap_err();

        assert_eq!(error.code, "invalid_request");
    }
}
