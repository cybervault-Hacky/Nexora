//! Local inference: process supervision, model validation, and streaming chat.
//!
//! The engine is a child process talked to over loopback HTTP. Nothing in this
//! module accepts commands, sizes, or paths from the model, and no model output
//! is ever executed.

pub mod client;
pub mod commands;
pub mod error;
pub mod hardware;
pub mod models;
pub mod process;
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
                return Err(InferenceError::invalid_request(
                    "A message is empty.",
                ));
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

        let max_tokens = self
            .max_tokens
            .unwrap_or(1_024)
            .clamp(1, MAX_OUTPUT_TOKENS);

        let temperature = self.temperature.unwrap_or(0.2).clamp(0.0, 2.0);

        Ok(ValidatedRequest {
            max_tokens,
            temperature,
        })
    }
}

/// Events streamed to the UI while a response is generated.
#[derive(Debug, Clone, Serialize)]
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

#[derive(Debug, Clone, Serialize)]
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

pub struct InferenceState {
    pub supervisor: Mutex<process::Supervisor>,
    pub settings: Mutex<SettingsStore>,
    pub active: Mutex<Option<ActiveGeneration>>,
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
            config_dir,
        }
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
    pub async fn status(&self, version: String) -> AppStatus {
        let settings = self.settings_snapshot();

        let (running, port, last_error) = {
            let mut supervisor = match self.supervisor.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };

            let running = supervisor.is_running();

            (
                running,
                supervisor.endpoint().map(|(port, _)| port),
                supervisor.last_error.clone(),
            )
        };

        let generating = self
            .active
            .lock()
            .map(|slot| slot.is_some())
            .unwrap_or(false);

        let healthy = match (running, port) {
            (true, Some(port)) => process::health_ok(port).await,
            _ => false,
        };

        let configured = settings.model_path.is_some() && settings.runtime_path.is_some();
        let (state, detail) = classify_engine(
            running,
            healthy,
            generating,
            configured,
            last_error.as_deref(),
        );

        AppStatus {
            name: "Nexora",
            version,
            engine: EngineStatus {
                state,
                runtime: settings.runtime_path.clone(),
                model: settings
                    .model_path
                    .as_ref()
                    .and_then(|path| {
                        std::path::Path::new(path)
                            .file_name()
                            .and_then(|name| name.to_str())
                            .map(str::to_string)
                    }),
                detail,
            },
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
