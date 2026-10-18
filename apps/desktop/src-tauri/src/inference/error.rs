use std::fmt;

/// A typed, serialisable failure returned to the UI.
///
/// `code` is a stable machine-readable token; `message` is written for a person
/// and never contains prompt text, file contents, or the API key.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InferenceError {
    pub code: &'static str,
    pub message: String,
}

pub type InferenceResult<T> = Result<T, InferenceError>;

impl InferenceError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn invalid_request(message: impl Into<String>) -> Self {
        Self::new("invalid_request", message)
    }

    pub fn not_configured(message: impl Into<String>) -> Self {
        Self::new("not_configured", message)
    }

    pub fn invalid_model(message: impl Into<String>) -> Self {
        Self::new("invalid_model", message)
    }

    pub fn spawn_failed(message: impl Into<String>) -> Self {
        Self::new("spawn_failed", message)
    }

    pub fn load_failed(message: impl Into<String>) -> Self {
        Self::new("load_failed", message)
    }

    pub fn not_running(message: impl Into<String>) -> Self {
        Self::new("not_running", message)
    }

    pub fn busy() -> Self {
        Self::new(
            "busy",
            "A response is already being generated. Stop it before sending again.",
        )
    }

    pub fn timeout(message: impl Into<String>) -> Self {
        Self::new("timeout", message)
    }

    pub fn network(message: impl Into<String>) -> Self {
        Self::new("network", message)
    }

    pub fn protocol(message: impl Into<String>) -> Self {
        Self::new("protocol", message)
    }

    pub fn storage(message: impl Into<String>) -> Self {
        Self::new("storage", message)
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new("internal", message)
    }
}

impl fmt::Display for InferenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for InferenceError {}

impl From<std::io::Error> for InferenceError {
    fn from(error: std::io::Error) -> Self {
        Self::new("io", error.to_string())
    }
}

impl From<reqwest::Error> for InferenceError {
    fn from(error: reqwest::Error) -> Self {
        Self::new("network", error.to_string())
    }
}

impl From<serde_json::Error> for InferenceError {
    fn from(error: serde_json::Error) -> Self {
        Self::new("protocol", error.to_string())
    }
}
