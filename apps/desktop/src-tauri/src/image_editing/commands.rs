use tauri::State;

use super::{ImageEditingCapabilityStatus, ImageEditingManager};
use super::{ImageEditingError, ImportSourceRequest, SourceArtifact};

/// Reports the reviewed editing-model candidate and the app's explicit runtime
/// blocker. This command performs no runtime probe, hardware read, or inference.
#[tauri::command]
pub fn get_local_image_editing_status(
    manager: State<'_, ImageEditingManager>,
) -> ImageEditingCapabilityStatus {
    manager.status()
}

/// Imports already-selected PNG bytes into app-owned storage. The renderer sends
/// image bytes, never a filesystem path; the backend validates before writing.
#[tauri::command]
pub async fn import_image_edit_source(
    manager: State<'_, ImageEditingManager>,
    request: ImportSourceRequest,
) -> Result<SourceArtifact, ImageEditingError> {
    let root = manager.sources_root_for_worker();
    tauri::async_runtime::spawn_blocking(move || {
        super::input::SourceImageStore::new(root).import(request)
    })
    .await
    .map_err(|_| ImageEditingError::storage("Source-image import stopped unexpectedly."))?
}

/// Removes one app-managed source by opaque ID. Renderer-supplied paths are never accepted.
#[tauri::command]
pub async fn remove_image_edit_source(
    manager: State<'_, ImageEditingManager>,
    source_id: String,
) -> Result<(), ImageEditingError> {
    let root = manager.sources_root_for_worker();
    tauri::async_runtime::spawn_blocking(move || {
        super::input::SourceImageStore::new(root).remove(&source_id)
    })
    .await
    .map_err(|_| ImageEditingError::storage("Source-image removal stopped unexpectedly."))?
}

/// Revalidates stored source metadata and image integrity by opaque ID only.
#[tauri::command]
pub async fn get_image_edit_source(
    manager: State<'_, ImageEditingManager>,
    source_id: String,
) -> Result<SourceArtifact, ImageEditingError> {
    let root = manager.sources_root_for_worker();
    tauri::async_runtime::spawn_blocking(move || {
        super::input::SourceImageStore::new(root).get(&source_id)
    })
    .await
    .map_err(|_| ImageEditingError::storage("Source-image metadata retrieval stopped unexpectedly."))?
}
