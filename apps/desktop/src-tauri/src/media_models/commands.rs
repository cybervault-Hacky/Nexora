use tauri::ipc::Channel;
use tauri::State;

use super::download::DownloadProgress;
use super::error::Result;
use super::{
    DownloadResult, MediaModelCatalog, MediaModelManager, MediaModelView, ValidationResult,
};

#[tauri::command]
pub fn list_media_models(manager: State<'_, MediaModelManager>) -> Result<MediaModelCatalog> {
    manager.list()
}

#[tauri::command]
pub fn get_media_model(
    manager: State<'_, MediaModelManager>,
    model_id: String,
) -> Result<MediaModelView> {
    manager.get(&model_id)
}

/// Downloads registered files only. Destinations and URLs come from the Rust
/// registry; the renderer supplies only a stable model ID and progress channel.
#[tauri::command]
pub async fn download_media_model(
    manager: State<'_, MediaModelManager>,
    model_id: String,
    on_event: Channel<DownloadProgress>,
) -> Result<DownloadResult> {
    manager.download(&model_id, &on_event).await
}

#[tauri::command]
pub fn cancel_media_model_download(
    manager: State<'_, MediaModelManager>,
    model_id: String,
) -> Result<bool> {
    manager.cancel(&model_id)
}

#[tauri::command]
pub async fn validate_media_model(
    manager: State<'_, MediaModelManager>,
    model_id: String,
) -> Result<ValidationResult> {
    manager.validate(&model_id).await
}

#[tauri::command]
pub fn remove_media_model(
    manager: State<'_, MediaModelManager>,
    model_id: String,
) -> Result<bool> {
    manager.remove(&model_id)
}
