use super::VideoEditingCapabilityStatus;

/// Reports conventional-processing and AI-enhancement availability separately.
/// This is static and performs no media-file, runtime, or hardware probe.
#[tauri::command]
pub fn get_local_video_editing_status() -> VideoEditingCapabilityStatus {
    super::status()
}
