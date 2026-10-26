//! On-disk records for the media model manager. Media weights are kept under
//! app-data/media-models, separate from coding models and application config.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use super::error::{MediaModelError, Result};
use super::registry::{MediaModelDefinition, ModelFileDefinition};

const MANIFEST: &str = "installation.json";
const MAX_MANIFEST_BYTES: u64 = 1024 * 1024;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleState {
    NotInstalled,
    AwaitingCompatibilityAssessment,
    ReadyToDownload,
    Downloading,
    DownloadInterrupted,
    DownloadedNotValidated,
    InstalledValidated,
    Incompatible,
    Failed,
    Removing,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledFileRecord {
    pub file_id: String,
    pub file_name: String,
    pub bytes: u64,
    pub sha256: String,
    pub integrity_verified: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallationRecord {
    pub schema_version: u16,
    pub model_id: String,
    pub state: LifecycleState,
    pub files: Vec<InstalledFileRecord>,
    pub last_error: Option<String>,
    pub updated_at_unix_ms: u64,
}

impl InstallationRecord {
    pub fn new(model_id: &str, state: LifecycleState) -> Self {
        Self {
            schema_version: 1,
            model_id: model_id.to_string(),
            state,
            files: Vec::new(),
            last_error: None,
            updated_at_unix_ms: now_ms(),
        }
    }

    pub fn touch(&mut self) {
        self.updated_at_unix_ms = now_ms();
    }
}

pub fn model_dir(root: &Path, model_id: &str) -> Result<PathBuf> {
    if !super::registry::validate_model_id(model_id) {
        return Err(MediaModelError::invalid("The media-model identifier is invalid."));
    }
    Ok(root.join(model_id))
}

pub fn artifact_path(root: &Path, model_id: &str, file: &ModelFileDefinition) -> Result<PathBuf> {
    validate_leaf_name(file.file_name)?;
    Ok(model_dir(root, model_id)?.join("files").join(file.file_name))
}

pub fn staging_dir(root: &Path, model_id: &str) -> Result<PathBuf> {
    Ok(model_dir(root, model_id)?.join("staging"))
}

pub fn manifest_path(root: &Path, model_id: &str) -> Result<PathBuf> {
    Ok(model_dir(root, model_id)?.join(MANIFEST))
}

pub fn check_artifact_directory(root: &Path, model_id: &str) -> Result<()> {
    let directory = model_dir(root, model_id)?;
    match fs::symlink_metadata(&directory) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err(MediaModelError::storage("The registered model folder is not a normal directory."));
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    }
    ensure_existing_child_dir_or_missing(&directory.join("files"))
}

pub fn ensure_layout(root: &Path, model_id: &str) -> Result<(PathBuf, PathBuf)> {
    fs::create_dir_all(root).map_err(|error| MediaModelError::storage(format!("The media-model storage folder could not be prepared: {error}")))?;
    let root_meta = fs::symlink_metadata(root)?;
    if root_meta.file_type().is_symlink() || !root_meta.is_dir() {
        return Err(MediaModelError::storage("The media-model storage root is not a normal directory."));
    }

    let directory = model_dir(root, model_id)?;
    match fs::symlink_metadata(&directory) {
        Ok(meta) if meta.file_type().is_symlink() || !meta.is_dir() => {
            return Err(MediaModelError::storage("The registered model folder is not a normal directory."));
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => fs::create_dir(&directory)?,
        Err(error) => return Err(error.into()),
    }

    let files = directory.join("files");
    let staging = directory.join("staging");
    ensure_child_dir(&files)?;
    ensure_child_dir(&staging)?;
    Ok((files, staging))
}

fn ensure_child_dir(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() || !meta.is_dir() => {
            Err(MediaModelError::storage("A media-model subfolder is not a normal directory."))
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir(path)?;
            Ok(())
        }
        Err(error) => Err(error.into()),
    }
}

pub fn load_record(root: &Path, model_id: &str) -> Result<Option<InstallationRecord>> {
    let directory = model_dir(root, model_id)?;
    match fs::symlink_metadata(&directory) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err(MediaModelError::storage("The registered model folder is not a normal directory."));
        }
        Ok(_) => {}
    }
    let path = manifest_path(root, model_id)?;
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() > MAX_MANIFEST_BYTES {
        return Err(MediaModelError::storage("The local media-model record is unsafe or exceeds the size limit."));
    }
    let bytes = fs::read(&path)?;
    let mut record: InstallationRecord = serde_json::from_slice(&bytes)
        .map_err(|_| MediaModelError::storage("The local media-model record is malformed."))?;
    if record.schema_version != 1 || record.model_id != model_id {
        return Err(MediaModelError::storage("The local media-model record does not match its registered model."));
    }
    // A process crash cannot leave a model marked as actively downloading.
    if record.state == LifecycleState::Downloading {
        record.state = LifecycleState::DownloadInterrupted;
        record.last_error = Some("The previous download did not finish.".to_string());
        record.touch();
    } else if record.state == LifecycleState::Removing {
        record.state = LifecycleState::Failed;
        record.last_error = Some("A previous removal was interrupted; owned files may remain.".to_string());
        record.touch();
    }
    Ok(Some(record))
}

pub fn record_files_present(
    root: &Path,
    definition: &MediaModelDefinition,
    record: &InstallationRecord,
) -> bool {
    if check_artifact_directory(root, definition.id).is_err() {
        return false;
    }
    definition.files.iter().filter(|file| file.required).all(|file| {
        let Some(saved) = record.files.iter().find(|saved| {
            saved.file_id == file.id && saved.file_name == file.file_name
        }) else {
            return false;
        };
        let Ok(path) = artifact_path(root, definition.id, file) else {
            return false;
        };
        std::fs::symlink_metadata(path).is_ok_and(|metadata| {
            metadata.file_type().is_file()
                && metadata.len() == saved.bytes
                && file.expected_size_bytes.map_or(true, |expected| expected == metadata.len())
        })
    })
}

pub fn save_record(root: &Path, definition: &MediaModelDefinition, record: &InstallationRecord) -> Result<()> {
    if record.model_id != definition.id || !super::registry::validate_model_id(&record.model_id) {
        return Err(MediaModelError::invalid("The installation record is not owned by this registered model."));
    }
    ensure_layout(root, definition.id)?;
    let target = manifest_path(root, definition.id)?;
    let mut temp = tempfile::NamedTempFile::new_in(model_dir(root, definition.id)?)
        .map_err(|error| MediaModelError::storage(format!("A temporary installation record could not be created: {error}")))?;
    let encoded = serde_json::to_vec(record)
        .map_err(|_| MediaModelError::storage("The local media-model record could not be encoded."))?;
    if encoded.len() as u64 > MAX_MANIFEST_BYTES {
        return Err(MediaModelError::storage("The local media-model record exceeds the size limit."));
    }
    temp.write_all(&encoded)?;
    temp.as_file().sync_all()?;
    temp.persist(&target)
        .map_err(|_| MediaModelError::storage("The local media-model record could not be replaced safely."))?;
    Ok(())
}

/// Deletes only registered filenames, manager-owned staging fragments and the
/// manager's manifest. Unknown files/directories are left untouched.
pub fn remove_owned_files(root: &Path, definition: &MediaModelDefinition, record: &InstallationRecord) -> Result<()> {
    let directory = model_dir(root, definition.id)?;
    let metadata = match fs::symlink_metadata(&directory) {
        Ok(meta) => meta,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(MediaModelError::storage("The registered model folder is not a normal directory."));
    }

    let files = directory.join("files");
    let staging = directory.join("staging");
    ensure_existing_child_dir_or_missing(&files)?;
    ensure_existing_child_dir_or_missing(&staging)?;
    for installed in &record.files {
        if let Some(file) = definition.files.iter().find(|file| file.id == installed.file_id && file.file_name == installed.file_name) {
            let path = artifact_path(root, definition.id, file)?;
            remove_regular_file(&path)?;
        }
    }
    remove_matching_staging(&staging, definition)?;
    remove_regular_file(&directory.join(MANIFEST))?;
    remove_empty_dir(&files)?;
    remove_empty_dir(&staging)?;
    remove_empty_dir(&directory)?;
    Ok(())
}

fn ensure_existing_child_dir_or_missing(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            Err(MediaModelError::storage("A media-model subfolder is not a normal directory."))
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn remove_matching_staging(directory: &Path, definition: &MediaModelDefinition) -> Result<()> {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    for entry in entries {
        let entry = entry?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        let owned_temp = definition.files.iter().any(|file| {
            name.strip_prefix(&format!("{}.", file.id))
                .and_then(|suffix| suffix.strip_suffix(".part"))
                .is_some_and(|nonce| nonce.len() == 32 && nonce.bytes().all(|byte| byte.is_ascii_hexdigit()))
        });
        if owned_temp {
            remove_regular_file(&entry.path())?;
        }
    }
    Ok(())
}

fn remove_regular_file(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            Err(MediaModelError::storage("A managed artifact path is not a regular file; it was not removed."))
        }
        Ok(_) => { fs::remove_file(path)?; Ok(()) }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn remove_empty_dir(path: &Path) -> Result<()> {
    match fs::read_dir(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
        Ok(mut entries) if entries.next().is_some() => return Ok(()),
        Ok(_) => {}
    }
    match fs::remove_dir(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

pub fn validate_leaf_name(name: &str) -> Result<()> {
    let stem = name.split('.').next().unwrap_or("");
    let reserved = [
        "CON", "PRN", "AUX", "NUL", "CONIN$", "CONOUT$", "COM1", "COM2", "COM3",
        "COM4", "COM5", "COM6", "COM7", "COM8", "COM9", "LPT1", "LPT2", "LPT3",
        "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    let invalid_character = name.chars().any(|character| {
        character.is_control()
            || matches!(character, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*')
    });
    if name.is_empty() || name.len() > 240 || name == "." || name == ".."
        || name.starts_with('.') || name.ends_with('.') || name.ends_with(' ')
        || invalid_character || reserved.iter().any(|candidate| stem.eq_ignore_ascii_case(candidate))
    {
        return Err(MediaModelError::invalid("A registered model filename is not a safe Windows-compatible path component."));
    }
    Ok(())
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).ok()
        .and_then(|duration| u64::try_from(duration.as_millis()).ok()).unwrap_or(0)
}
