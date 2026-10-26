use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaModelError {
    pub code: &'static str,
    pub message: String,
}

pub type Result<T> = std::result::Result<T, MediaModelError>;

impl MediaModelError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self { code, message: message.into() }
    }

    pub fn invalid(message: impl Into<String>) -> Self { Self::new("invalid_request", message) }
    pub fn unknown_model() -> Self { Self::new("unknown_model", "That media model is not registered.") }
    pub fn busy() -> Self { Self::new("busy", "A media-model operation is already active for this model.") }
    pub fn storage(message: impl Into<String>) -> Self { Self::new("storage", message) }
    pub fn network(message: impl Into<String>) -> Self { Self::new("network", message) }
    pub fn integrity(message: impl Into<String>) -> Self { Self::new("integrity_failed", message) }
    pub fn insufficient_storage() -> Self { Self::new("insufficient_storage", "There is not enough free space for the registered model files.") }
    pub fn interrupted() -> Self { Self::new("download_interrupted", "The download was cancelled or interrupted. Partial data was not installed.") }
}

impl From<std::io::Error> for MediaModelError {
    fn from(error: std::io::Error) -> Self {
        Self::storage(format!("A media-model storage operation failed: {error}"))
    }
}
