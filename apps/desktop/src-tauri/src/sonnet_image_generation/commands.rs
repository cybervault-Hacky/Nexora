use super::{FluxSchnellCandidate, SonnetImageGenerationCapabilityStatus};

/// Reports reviewed Sonnet-image candidate and integration blockers only.
/// Performs no runtime/hardware probe, download, model load, or inference.
#[tauri::command]
pub fn get_sonnet_image_generation_status() -> SonnetImageGenerationCapabilityStatus {
    FluxSchnellCandidate::status()
}
