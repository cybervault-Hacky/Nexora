use tauri::State;

use crate::adaptive;
use crate::media_models::MediaModelManager;
use crate::resource_management::AssetReadiness;

use super::error::{ImageGenerationError, Result};
use super::{
    probe_cpu_backend, GenerateImageRequest, GeneratedImageArtifact,
    ImageGenerationManager, ImageGenerationStatus,
};

#[tauri::command]
pub async fn get_local_image_generation_status(
    models: State<'_, MediaModelManager>,
    manager: State<'_, ImageGenerationManager>,
) -> Result<ImageGenerationStatus> {
    let capabilities = adaptive::collect(models.storage_root());
    let lifecycle = models.lifecycle_snapshot(super::SD_TURBO_MODEL_ID).await
        .map_err(ImageGenerationError::from)?;
    let runtime_path = manager.resolve_runtime();
    let runtime_available = runtime_path.is_some();
    let cpu_backend_confirmed = match runtime_path {
        Some(path) => tauri::async_runtime::spawn_blocking(move || probe_cpu_backend(&path))
            .await
            .map_err(|_| ImageGenerationError::runtime_incompatible())?
            .map(|probe| probe.cpu_backend_confirmed)
            .unwrap_or(false),
        None => false,
    };
    let asset_readiness = AssetReadiness::from_lifecycle(&lifecycle);
    manager.availability(lifecycle.state, asset_readiness, &capabilities, runtime_available, cpu_backend_confirmed)
}

/// Performs local text-to-image inference only. The renderer supplies prompt,
/// preset, and optional seed; model paths, runtime path, output paths, and
/// backend choice are controlled by the native manager.
#[tauri::command]
pub async fn generate_local_image(
    models: State<'_, MediaModelManager>,
    manager: State<'_, ImageGenerationManager>,
    request: GenerateImageRequest,
) -> Result<GeneratedImageArtifact> {
    manager.generate(&models, request).await
}

#[tauri::command]
pub fn cancel_local_image_generation(
    manager: State<'_, ImageGenerationManager>,
) -> Result<bool> {
    manager.cancel()
}

#[tauri::command]
pub async fn get_generated_image_artifact(
    manager: State<'_, ImageGenerationManager>,
    artifact_id: String,
) -> Result<GeneratedImageArtifact> {
    let root = manager.artifact_root();
    tauri::async_runtime::spawn_blocking(move || {
        ImageGenerationManager::get_artifact_from_root(&root, &artifact_id)
    })
    .await
    .map_err(|_| ImageGenerationError::storage("Generated-image metadata could not be read."))?
}
