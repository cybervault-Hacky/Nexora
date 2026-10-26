use serde::Serialize;

use crate::media_models::error::MediaModelError;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageGenerationError {
    pub code: String,
    pub message: String,
}

pub type Result<T> = std::result::Result<T, ImageGenerationError>;

impl ImageGenerationError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self { code: code.into(), message: message.into() }
    }

    pub fn invalid(message: impl Into<String>) -> Self { Self::new("invalid_request", message) }
    pub fn busy() -> Self { Self::new("busy", "A local image generation request is already active.") }
    pub fn runtime_missing() -> Self { Self::new("runtime_missing", "No approved stable-diffusion.cpp CPU runtime executable is available to this Nexora installation.") }
    pub fn runtime_incompatible() -> Self { Self::new("runtime_incompatible", "The detected local image runtime did not report the required CPU backend.") }
    pub fn insufficient_memory() -> Self { Self::new("insufficient_memory", "Available system RAM is below the registered checkpoint's raw size screening floor. The local runtime was not started; this is not a complete memory-fit assessment.") }
    pub fn insufficient_storage() -> Self { Self::new("insufficient_storage", "Free space for the application-owned image output is below the reserved staging/output limit. The local runtime was not started.") }
    pub fn model_not_ready(message: impl Into<String>) -> Self { Self::new("model_not_ready", message) }
    pub fn cancelled() -> Self { Self::new("cancelled", "Local image generation was cancelled. Incomplete output was removed.") }
    pub fn cleanup_failed() -> Self { Self::new("cleanup_failed", "The image operation could not confirm cleanup of its private staging files. No success result was returned.") }
    pub fn timeout() -> Self { Self::new("timeout", "Local image generation exceeded its time limit. Incomplete output was removed.") }
    pub fn inference_failed() -> Self { Self::new("inference_failed", "The local image runtime failed to generate an image. No artifact was published.") }
    pub fn output_invalid() -> Self { Self::new("output_invalid", "The local runtime did not produce a valid 512 by 512 PNG image. No artifact was published.") }
    pub fn storage(message: impl Into<String>) -> Self { Self::new("storage", message) }
    pub fn unknown_artifact() -> Self { Self::new("unknown_artifact", "That generated image artifact is not managed by Nexora.") }
}

impl From<MediaModelError> for ImageGenerationError {
    fn from(error: MediaModelError) -> Self {
        Self::new(error.code, error.message)
    }
}
