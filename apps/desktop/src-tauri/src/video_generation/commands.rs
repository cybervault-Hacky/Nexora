use tauri::State;

use crate::media_models::MediaModelManager;

use super::VideoGenerationCapabilityStatus;

/// Reports Wan2.1 metadata and the media manager's recorded install lifecycle.
/// It performs no adaptive/hardware probe, hash recomputation, runtime probe,
/// model load, or inference.
#[tauri::command]
pub async fn get_local_video_generation_status(
    manager: State<'_, MediaModelManager>,
) -> VideoGenerationCapabilityStatus {
    let lifecycle = manager.lifecycle_snapshot(super::adapter::WAN21_MODEL_ID).await.ok();
    super::status(lifecycle)
}
