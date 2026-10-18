use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::error::{InferenceError, InferenceResult};

pub const DEFAULT_CONTEXT_SIZE: u32 = 32_768;
pub const MIN_CONTEXT_SIZE: u32 = 2_048;
pub const MAX_CONTEXT_SIZE: u32 = 131_072;
const SETTINGS_FILE: &str = "inference-settings.json";

/// User-chosen engine configuration.
///
/// Nexora never downloads weights or binaries on its own; both paths point at
/// files the user already has, and both are re-validated on every load.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub model_path: Option<String>,
    pub runtime_path: Option<String>,
    pub context_size: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            model_path: None,
            runtime_path: None,
            context_size: DEFAULT_CONTEXT_SIZE,
        }
    }
}

impl Settings {
    /// Clamps the context into the supported window and trims stray whitespace
    /// from paths. Empty strings become `None` so "unset" has one meaning.
    pub fn normalised(&self) -> Self {
        Self {
            model_path: clean_path(&self.model_path),
            runtime_path: clean_path(&self.runtime_path),
            context_size: self.context_size.clamp(MIN_CONTEXT_SIZE, MAX_CONTEXT_SIZE),
        }
    }
}

fn clean_path(value: &Option<String>) -> Option<String> {
    value
        .as_ref()
        .map(|path| path.trim().trim_matches('"').to_string())
        .filter(|path| !path.is_empty())
}

pub struct SettingsStore {
    path: PathBuf,
    pub settings: Settings,
}

impl SettingsStore {
    /// Loads settings, falling back to defaults on a missing or unreadable
    /// file. A corrupt file is not fatal: the engine simply starts unconfigured.
    pub fn load(config_dir: &Path) -> Self {
        let path = config_dir.join(SETTINGS_FILE);
        let settings = fs::read_to_string(&path)
            .ok()
            .and_then(|text| serde_json::from_str::<Settings>(&text).ok())
            .unwrap_or_default()
            .normalised();

        Self { path, settings }
    }

    pub fn save(&self, settings: Settings) -> InferenceResult<Settings> {
        let settings = settings.normalised();

        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                InferenceError::storage(format!("Could not create the settings folder: {error}"))
            })?;
        }

        let text = serde_json::to_string_pretty(&settings)?;
        fs::write(&self.path, text).map_err(|error| {
            InferenceError::storage(format!("Could not save inference settings: {error}"))
        })?;

        Ok(settings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalises_paths_and_clamps_context() {
        let settings = Settings {
            model_path: Some("  \"C:/models/a.gguf\"  ".into()),
            runtime_path: Some("   ".into()),
            context_size: 1,
        }
        .normalised();

        assert_eq!(settings.model_path.as_deref(), Some("C:/models/a.gguf"));
        assert_eq!(settings.runtime_path, None);
        assert_eq!(settings.context_size, MIN_CONTEXT_SIZE);
    }

    #[test]
    fn caps_context_above_the_maximum() {
        let settings = Settings {
            context_size: 10_000_000,
            ..Settings::default()
        }
        .normalised();

        assert_eq!(settings.context_size, MAX_CONTEXT_SIZE);
    }

    #[test]
    fn round_trips_through_json() {
        let settings = Settings {
            model_path: Some("D:/models/qwen.gguf".into()),
            runtime_path: Some("D:/tools/llama-server.exe".into()),
            context_size: 16_384,
        };

        let text = serde_json::to_string(&settings).expect("serialise");
        let parsed: Settings = serde_json::from_str(&text).expect("deserialise");

        assert_eq!(parsed.model_path, settings.model_path);
        assert_eq!(parsed.context_size, 16_384);
    }

    #[test]
    fn defaults_are_sane() {
        let settings = Settings::default();

        assert_eq!(settings.context_size, DEFAULT_CONTEXT_SIZE);
        assert!(settings.model_path.is_none());
    }
}
