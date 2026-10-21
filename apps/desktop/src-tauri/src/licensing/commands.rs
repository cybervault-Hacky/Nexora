use super::validator::{to_licence_status, validate_licence_key};
use super::{ActivateLicencePayload, LicenceError, LicenceStatus};
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::State;

pub struct LicenceState {
    pub active_key: Mutex<Option<String>>,
    config_path: Option<PathBuf>,
}

impl LicenceState {
    pub fn new(config_dir: Option<PathBuf>) -> Self {
        let config_path = config_dir.map(|p| p.join("licence.json"));
        let initial_key = config_path.as_ref().and_then(|p| {
            std::fs::read_to_string(p).ok().and_then(|content| {
                serde_json::from_str::<serde_json::Value>(&content)
                    .ok()
                    .and_then(|v| {
                        v.get("licenceKey")
                            .and_then(|k| k.as_str().map(|s| s.to_string()))
                    })
            })
        });

        Self {
            active_key: Mutex::new(initial_key),
            config_path,
        }
    }

    fn persist_key(&self, key: Option<&str>) {
        if let Some(ref path) = self.config_path {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let data = serde_json::json!({
                "licenceKey": key,
                "updatedAt": std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0),
            });
            if let Ok(serialized) = serde_json::to_string_pretty(&data) {
                let _ = std::fs::write(path, serialized);
            }
        }
    }
}

#[tauri::command]
pub async fn licence_get_status(
    state: State<'_, LicenceState>,
) -> Result<LicenceStatus, LicenceError> {
    let active_key = {
        let guard = state.active_key.lock().map_err(|_| LicenceError {
            code: "lock_error".to_string(),
            message: "Failed to acquire licence state lock.".to_string(),
        })?;
        guard.clone()
    };

    match active_key {
        Some(key) => match validate_licence_key(&key) {
            Ok(validated) => Ok(to_licence_status(Some(&validated), Some(key))),
            Err(_) => {
                // Key expired or became invalid; return community tier with inactive status
                Ok(LicenceStatus::default())
            }
        },
        None => Ok(LicenceStatus::default()),
    }
}

#[tauri::command]
pub async fn licence_activate(
    state: State<'_, LicenceState>,
    request: ActivateLicencePayload,
) -> Result<LicenceStatus, LicenceError> {
    let validated = validate_licence_key(&request.licence_key)?;
    let key_to_save = request.licence_key.trim().to_uppercase();

    {
        let mut guard = state.active_key.lock().map_err(|_| LicenceError {
            code: "lock_error".to_string(),
            message: "Failed to acquire licence state lock.".to_string(),
        })?;
        *guard = Some(key_to_save.clone());
    }

    state.persist_key(Some(&key_to_save));
    Ok(to_licence_status(Some(&validated), Some(key_to_save)))
}

#[tauri::command]
pub async fn licence_deactivate(
    state: State<'_, LicenceState>,
) -> Result<LicenceStatus, LicenceError> {
    {
        let mut guard = state.active_key.lock().map_err(|_| LicenceError {
            code: "lock_error".to_string(),
            message: "Failed to acquire licence state lock.".to_string(),
        })?;
        *guard = None;
    }

    state.persist_key(None);
    Ok(LicenceStatus::default())
}
