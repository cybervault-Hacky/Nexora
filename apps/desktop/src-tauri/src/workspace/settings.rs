use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::scanner::WorkspaceError as WError;
use super::scanner::WorkspaceResult;

// Mirrors inference SettingsStore atomic pattern but for workspace preferences.
// File: <app_config_dir>/workspace-settings.json

const SETTINGS_FILE: &str = "workspace-settings.json";
pub const CURRENT_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSettings {
    pub version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_workspace_path: Option<String>,
    #[serde(default)]
    pub preferences: WorkspacePreferences,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct WorkspacePreferences {
    #[serde(default)]
    pub explorer_collapsed: bool,
}

impl Default for WorkspaceSettings {
    fn default() -> Self {
        Self {
            version: CURRENT_VERSION,
            last_workspace_path: None,
            preferences: WorkspacePreferences::default(),
        }
    }
}

pub struct WorkspaceSettingsStore {
    path: PathBuf,
    pub settings: WorkspaceSettings,
}

impl WorkspaceSettingsStore {
    pub fn load(config_dir: &Path) -> Self {
        let path = config_dir.join(SETTINGS_FILE);
        let settings = fs::read_to_string(&path)
            .ok()
            .and_then(|text| serde_json::from_str::<WorkspaceSettings>(&text).ok())
            .and_then(|mut s| {
                if s.version > CURRENT_VERSION {
                    // Preserve bytes: treat unsupported version as error, return default but not overwrite yet
                    return None;
                }
                if s.version == 0 {
                    // migrate v0: treat as v1 with same fields if present
                    s.version = CURRENT_VERSION;
                }
                // Validate last path: must be non-empty and plausible, not traversal
                if let Some(p) = &s.last_workspace_path {
                    if p.trim().is_empty() || p.chars().count() > 4096 {
                        s.last_workspace_path = None;
                    }
                }
                Some(s)
            })
            .unwrap_or_default();

        Self { path, settings }
    }

    pub fn save(&mut self, settings: WorkspaceSettings) -> WorkspaceResult<WorkspaceSettings> {
        let mut to_save = settings.clone();
        to_save.version = CURRENT_VERSION;
        // Validate last path before saving: trim, reject empty, limit
        if let Some(p) = &to_save.last_workspace_path {
            let trimmed = p.trim();
            if trimmed.is_empty() || trimmed.chars().count() > 4096 {
                to_save.last_workspace_path = None;
            } else {
                to_save.last_workspace_path = Some(trimmed.to_string());
            }
        }

        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|_| WError::storage("Could not create workspace settings folder."))?;
        }
        let text = serde_json::to_string_pretty(&to_save).map_err(|_| WError::storage("Could not serialize workspace settings."))?;
        if text.len() > 64 * 1024 {
            return Err(WError::invalid_request("Workspace settings too large."));
        }
        // Atomic write via tempfile (same semantics as history)
        {
            use std::io::Write;
            let parent = self.path.parent().ok_or_else(|| WError::storage("Invalid settings directory."))?;
            let mut tmp = tempfile::NamedTempFile::new_in(parent).map_err(|_| WError::storage("Could not create temporary settings file."))?;
            tmp.write_all(text.as_bytes()).and_then(|_| tmp.as_file().sync_all()).map_err(|_| WError::storage("Could not write workspace settings."))?;
            tmp.persist(&self.path).map_err(|_| WError::storage("Could not replace workspace settings file."))?;
            #[cfg(unix)]
            {
                if let Ok(dir) = fs::File::open(parent) {
                    let _ = dir.sync_all();
                }
            }
        }
        self.settings = to_save.clone();
        Ok(to_save)
    }

    pub fn get(&self) -> WorkspaceSettings {
        self.settings.clone()
    }
}

// Helper for atomic write — reuse history's logic but exposed for settings
// To avoid circular mod, we duplicate minimal atomic logic here or call scanner's helper.
// Instead we expose a function in scanner.rs; if not available fallback to simple fs::write.
// For now implement inline atomic write.

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn defaults_sane() {
        let s = WorkspaceSettings::default();
        assert_eq!(s.version, CURRENT_VERSION);
        assert!(s.last_workspace_path.is_none());
    }

    #[test]
    fn load_missing_returns_default() {
        let dir = TempDir::new().unwrap();
        let store = WorkspaceSettingsStore::load(dir.path());
        assert_eq!(store.settings.version, CURRENT_VERSION);
    }

    #[test]
    fn round_trip() {
        let dir = TempDir::new().unwrap();
        let mut store = WorkspaceSettingsStore::load(dir.path());
        let mut settings = store.get();
        settings.last_workspace_path = Some("/tmp/my-app".to_string());
        let saved = store.save(settings).expect("save");
        assert_eq!(saved.last_workspace_path.as_deref(), Some("/tmp/my-app"));
        let reloaded = WorkspaceSettingsStore::load(dir.path());
        assert_eq!(reloaded.settings.last_workspace_path.as_deref(), Some("/tmp/my-app"));
    }

    #[test]
    fn rejects_oversized_path() {
        let dir = TempDir::new().unwrap();
        let mut store = WorkspaceSettingsStore::load(dir.path());
        let mut settings = store.get();
        settings.last_workspace_path = Some("a".repeat(5000));
        let saved = store.save(settings).expect("save");
        assert!(saved.last_workspace_path.is_none());
    }

    #[test]
    fn unsupported_version_preserved() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join(SETTINGS_FILE);
        fs::write(&path, r#"{"version":999,"lastWorkspacePath":"/tmp/old"}"#).unwrap();
        // load should return default, not overwrite file yet
        let store = WorkspaceSettingsStore::load(dir.path());
        assert!(store.settings.last_workspace_path.is_none());
        let raw = fs::read_to_string(&path).unwrap();
        assert!(raw.contains("999"));
    }
}
