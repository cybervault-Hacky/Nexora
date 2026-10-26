//! Dedicated local image/video model acquisition and installation foundation.
//! Kept separate from `inference::registry`, which owns chat coding models.

pub mod commands;
mod download;
pub(crate) mod error;
mod registry;
mod storage;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::sync::atomic::Ordering;

use serde::Serialize;
use tauri::ipc::Channel;

use crate::adaptive::{self, AdaptiveCapabilities, DetectionConfidence};
use self::download::{DownloadControl, DownloadProgress};
use self::error::{MediaModelError, Result};
use self::registry::{
    DefinitionCompatibility, ModelCategory, ModelFileDefinition, MEDIA_MODELS,
};
pub(crate) use self::registry::{BackendRequirement, MediaModelDefinition};
use self::storage::{InstallationRecord, InstalledFileRecord};
pub(crate) use self::storage::LifecycleState;

const MAX_REGISTERED_MODEL_BYTES: u64 = 512 * 1024 * 1024 * 1024;

/// Application-data storage, deliberately separate from `app_data/models` used
/// by the existing coding-model manager.
pub struct MediaModelManager {
    root: PathBuf,
    active: Mutex<HashMap<String, Arc<DownloadControl>>>,
    operations: Arc<Mutex<HashSet<String>>>,
}

struct OperationGuard {
    operations: Arc<Mutex<HashSet<String>>>,
    model_id: &'static str,
}

/// Holds the manager's per-model operation lock while a runtime reads the
/// registered artifact, preventing removal or replacement during inference.
pub(crate) struct ModelUseGuard {
    _guard: OperationGuard,
}

impl Drop for OperationGuard {
    fn drop(&mut self) {
        if let Ok(mut operations) = self.operations.lock() {
            operations.remove(self.model_id);
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompatibilityAssessment {
    pub status: CompatibilityStatus,
    pub reasons: Vec<String>,
    /// This manager reports only resource evidence; runtime/model compatibility
    /// remains false until a later phase evaluates it.
    pub runtime_compatibility_confirmed: bool,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CompatibilityStatus {
    AwaitingAssessment,
    ResourceConstrained,
    ResourceHeadroomObserved,
    Unknown,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaModelView {
    pub definition: MediaModelDefinition,
    pub state: LifecycleState,
    pub progress: Option<DownloadProgress>,
    /// This registry layer does not assert runtime availability or infer usability.
    pub usable: bool,
    pub compatibility: CompatibilityAssessment,
    pub installation: Option<InstallationRecord>,
    pub integrity_note: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaModelCatalog {
    pub categories: Vec<ModelCategory>,
    pub models: Vec<MediaModelView>,
    pub storage_directory_available: bool,
    pub catalog_note: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadResult {
    pub model_id: String,
    pub state: LifecycleState,
    pub downloaded_file_count: usize,
    pub integrity_complete: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationResult {
    pub model_id: String,
    pub state: LifecycleState,
    pub verified_file_count: usize,
    pub integrity_complete: bool,
    pub message: String,
}

/// Read-only installation lifecycle evidence without collecting adaptive or
/// hardware data. Integrity here is the manager's recorded validation state;
/// this snapshot does not recompute large-file hashes.
pub(crate) struct MediaModelLifecycleSnapshot {
    pub state: LifecycleState,
    pub manager_reported_downloaded: Option<bool>,
    pub manager_reported_integrity_validated: Option<bool>,
    pub recorded_required_file_count: Option<usize>,
    pub note: String,
}

impl MediaModelManager {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            active: Mutex::new(HashMap::new()),
            operations: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    pub(crate) fn storage_root(&self) -> &Path { &self.root }

    /// Read the registered model's managed lifecycle without invoking the
    /// adaptive collector. Filesystem metadata work runs off the UI thread. The
    /// result does not re-hash weights or prove runtime compatibility.
    pub(crate) async fn lifecycle_snapshot(&self, model_id: &str) -> Result<MediaModelLifecycleSnapshot> {
        let definition = resolve(model_id)?;
        let is_downloading = self.active.lock()
            .map_err(|_| MediaModelError::storage("The media-model operation state is unavailable."))?
            .contains_key(definition.id);
        let root = self.root.clone();
        tokio::task::spawn_blocking(move || lifecycle_snapshot_from(&root, definition, is_downloading))
            .await
            .map_err(|_| MediaModelError::storage("The media-model lifecycle reader stopped unexpectedly."))?
    }

    /// Returns the path to a checksum-validated registry file while holding a
    /// lock that excludes concurrent download, validation, and removal.
    pub(crate) async fn lock_validated_artifact(
        &self,
        model_id: &str,
        file_id: &str,
    ) -> Result<(ModelUseGuard, crate::inference::models::LockedFile)> {
        let definition = resolve(model_id)?;
        let file = definition.files.iter().find(|file| file.id == file_id && file.required)
            .ok_or_else(|| MediaModelError::invalid("The requested model artifact is not registered."))?;
        if file.expected_sha256.is_none() {
            return Err(MediaModelError::integrity("The registered model file has no trusted checksum."));
        }
        let guard = self.begin_operation(definition.id)?;
        let record = storage::load_record(&self.root, definition.id)?
            .ok_or_else(|| MediaModelError::new("not_installed", "No managed installation is recorded for this model."))?;
        if record.state != LifecycleState::InstalledValidated {
            return Err(MediaModelError::new("model_not_validated", "The registered model must be downloaded and checksum-validated before generation."));
        }
        let saved = record.files.iter().find(|saved| saved.file_id == file.id && saved.file_name == file.file_name)
            .ok_or_else(|| MediaModelError::new("not_installed", "A required model artifact is not recorded."))?;
        if !saved.integrity_verified || saved.sha256 != file.expected_sha256.unwrap_or_default() {
            return Err(MediaModelError::integrity("The manager record does not confirm the registered model checksum."));
        }
        storage::check_artifact_directory(&self.root, definition.id)?;
        let path = storage::artifact_path(&self.root, definition.id, file)?;
        let metadata = std::fs::symlink_metadata(&path)
            .map_err(|_| MediaModelError::new("not_installed", "The validated model artifact is no longer present."))?;
        if !metadata.file_type().is_file()
            || file.expected_size_bytes.map_or(false, |expected| expected != metadata.len())
            || metadata.len() != saved.bytes
        {
            return Err(MediaModelError::integrity("The validated model artifact changed after validation."));
        }
        let expected_sha256 = file.expected_sha256.unwrap_or_default().to_string();
        let expected_bytes = saved.bytes;
        let guard = ModelUseGuard { _guard: guard };
        let (guard, locked_file) = tokio::task::spawn_blocking(move || {
            use std::io::Read;
            use sha2::Digest;
            let mut locked = crate::inference::models::LockedFile::open(&path)
                .map_err(|_| MediaModelError::storage("The registered model artifact could not be opened safely."))?;
            let mut hasher = sha2::Sha256::new();
            let mut buffer = vec![0u8; 1024 * 1024];
            let mut bytes = 0u64;
            loop {
                let read = locked.file_mut().read(&mut buffer)?;
                if read == 0 { break; }
                hasher.update(&buffer[..read]);
                bytes = bytes.saturating_add(read as u64);
            }
            let digest = hasher.finalize().iter().map(|byte| format!("{byte:02x}")).collect::<String>();
            if bytes != expected_bytes || !digest.eq_ignore_ascii_case(&expected_sha256) {
                return Err(MediaModelError::integrity("The registered model artifact changed after validation."));
            }
            Ok((guard, locked))
        }).await.map_err(|_| MediaModelError::storage("The registered model artifact verifier stopped unexpectedly."))??;
        Ok((guard, locked_file))
    }

    fn begin_operation(&self, model_id: &'static str) -> Result<OperationGuard> {
        let mut operations = self.operations.lock()
            .map_err(|_| MediaModelError::storage("The media-model operation state is unavailable."))?;
        if !operations.insert(model_id.to_string()) { return Err(MediaModelError::busy()); }
        drop(operations);
        Ok(OperationGuard { operations: self.operations.clone(), model_id })
    }

    pub fn list(&self) -> Result<MediaModelCatalog> {
        let capabilities = adaptive::collect(&self.root);
        let mut models = Vec::with_capacity(MEDIA_MODELS.len());
        for definition in MEDIA_MODELS {
            models.push(self.view(definition, &capabilities));
        }
        let catalog_note = if models.is_empty() {
            "No image/video model definition is registered.".to_string()
        } else {
            "Registered media models have source, artifact, and license metadata appropriate to their task. Catalog registration and validated files track acquisition and integrity only; they do not prove runtime availability, model/runtime compatibility, hardware fit, or generation success.".to_string()
        };
        Ok(MediaModelCatalog {
            categories: registry::CATEGORIES.to_vec(),
            models,
            storage_directory_available: self.root.exists(),
            catalog_note,
        })
    }

    pub fn get(&self, model_id: &str) -> Result<MediaModelView> {
        let definition = resolve(model_id)?;
        let capabilities = adaptive::collect(&self.root);
        Ok(self.view(definition, &capabilities))
    }

    fn view(&self, definition: &'static MediaModelDefinition, capabilities: &AdaptiveCapabilities) -> MediaModelView {
        let active = self.active.lock().ok().and_then(|downloads| downloads.get(definition.id).cloned());
        let progress = active.as_ref().map(|control| DownloadProgress {
            model_id: definition.id.to_string(),
            file_id: control.current_file_id.lock().ok().and_then(|file_id| file_id.clone()),
            received_bytes: control.received_bytes.load(Ordering::SeqCst),
            total_bytes: match control.total_bytes.load(Ordering::SeqCst) { 0 => None, bytes => Some(bytes) },
        });
        let (installation, mut state, integrity_note) = match storage::load_record(&self.root, definition.id) {
            Ok(Some(mut record)) => {
                let mut state = record.state;
                let mut note = record.last_error.clone().unwrap_or_else(|| {
                    if record.files.iter().any(|file| !file.integrity_verified) {
                        "One or more files have no trusted checksum; installation is not validated.".to_string()
                    } else { "Local record loaded.".to_string() }
                });
                if matches!(state, LifecycleState::DownloadedNotValidated | LifecycleState::InstalledValidated)
                    && !storage::record_files_present(&self.root, definition, &record)
                {
                    state = LifecycleState::Failed;
                    note = "A recorded model artifact is missing, changed in size, or stored through an unsafe directory.".to_string();
                }
                record.state = state;
                if state == LifecycleState::Failed { record.last_error = Some(note.clone()); }
                (Some(record), state, note)
            }
            Ok(None) => (None, default_state(definition), "No managed installation is recorded.".to_string()),
            Err(error) => (None, LifecycleState::Failed, error.message),
        };
        if active.is_some() { state = LifecycleState::Downloading; }
        MediaModelView {
            definition: *definition,
            state,
            progress,
            usable: false,
            compatibility: assess_compatibility(definition, capabilities),
            installation,
            integrity_note,
        }
    }

    pub async fn download(
        &self,
        model_id: &str,
        on_event: &Channel<DownloadProgress>,
    ) -> Result<DownloadResult> {
        let definition = resolve(model_id)?;
        validate_definition(definition)?;
        if !definition.downloadable || definition.files.iter().all(|file| !file.required) {
            return Err(MediaModelError::new("model_not_downloadable", "This registered model is not approved for download."));
        }
        if !definition.files.iter().filter(|file| file.required).all(|file| file.download_url.is_some()) {
            return Err(MediaModelError::new("source_unavailable", "One or more required model files have no trusted download source."));
        }
        let _operation = self.begin_operation(definition.id)?;

        let control = Arc::new(DownloadControl::new());
        {
            let mut active = self.active.lock().map_err(|_| MediaModelError::storage("The media-model operation state is unavailable."))?;
            if active.contains_key(definition.id) { return Err(MediaModelError::busy()); }
            active.insert(definition.id.to_string(), control.clone());
        }
        let operation = self.download_inner(definition, &control, on_event).await;
        if let Ok(mut active) = self.active.lock() { active.remove(definition.id); }
        operation
    }

    async fn download_inner(
        &self,
        definition: &'static MediaModelDefinition,
        control: &Arc<DownloadControl>,
        on_event: &Channel<DownloadProgress>,
    ) -> Result<DownloadResult> {
        let capabilities = adaptive::collect(&self.root);
        let known_size = required_download_bytes(definition)?;
        if let (Some(required), Some(free)) = (known_size, capabilities.resources.model_storage_free_bytes.value) {
            if free < required { return Err(MediaModelError::insufficient_storage()); }
        }

        storage::ensure_layout(&self.root, definition.id)?;
        let mut record = storage::load_record(&self.root, definition.id)?
            .unwrap_or_else(|| InstallationRecord::new(definition.id, LifecycleState::ReadyToDownload));
        if record.state == LifecycleState::InstalledValidated {
            return Err(MediaModelError::new("already_installed", "This media model is already recorded as validated."));
        }
        record.state = LifecycleState::Downloading;
        record.last_error = None;
        record.touch();
        storage::save_record(&self.root, definition, &record)?;

        let download_result = async {
            for file in definition.files.iter().filter(|file| file.required) {
                if control.cancel.load(Ordering::SeqCst) { return Err(MediaModelError::interrupted()); }
                if let Ok(mut current_file) = control.current_file_id.lock() {
                    *current_file = Some(file.id.to_string());
                }
                let path = storage::artifact_path(&self.root, definition.id, file)?;
                if let Some(existing) = record.files.iter().find(|entry| entry.file_id == file.id) {
                    if file_is_unchanged(&path, existing, file).await? {
                        continue;
                    }
                    return Err(MediaModelError::integrity("A previously downloaded managed file changed on disk; remove it explicitly before retrying."));
                }
                match std::fs::symlink_metadata(&path) {
                    Ok(_) => return Err(MediaModelError::new("already_exists", "An unrecorded file already exists at a managed model destination; it was not overwritten.")),
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(_) => return Err(MediaModelError::storage("A managed model destination could not be inspected.")),
                }
                let already_downloaded = record.files.iter().try_fold(0u64, |sum, saved| sum.checked_add(saved.bytes))
                    .ok_or_else(|| MediaModelError::new("size_limit", "Recorded model file sizes exceed the supported total."))?;
                let remaining_limit = MAX_REGISTERED_MODEL_BYTES.saturating_sub(already_downloaded);
                let downloaded = download::download_file(
                    definition.id,
                    file,
                    &self.root,
                    remaining_limit,
                    control,
                    on_event,
                ).await?;
                record.files.push(InstalledFileRecord {
                    file_id: file.id.to_string(),
                    file_name: file.file_name.to_string(),
                    bytes: downloaded.bytes,
                    sha256: downloaded.sha256,
                    integrity_verified: downloaded.integrity_verified,
                });
                record.touch();
                storage::save_record(&self.root, definition, &record)?;
            }
            let required_files: Vec<_> = definition.files.iter().filter(|file| file.required).collect();
            let complete = required_files.iter().all(|required| {
                record.files.iter().any(|saved| saved.file_id == required.id && saved.integrity_verified)
            });
            record.state = LifecycleState::DownloadedNotValidated;
            record.last_error = if complete { None } else { Some("Download completed, but trusted checksum metadata is missing for one or more files.".to_string()) };
            record.touch();
            storage::save_record(&self.root, definition, &record)?;
            Ok(DownloadResult {
                model_id: definition.id.to_string(),
                state: record.state,
                downloaded_file_count: record.files.len(),
                integrity_complete: complete,
                message: if complete { "Files downloaded and checked against trusted registry checksums; run model validation before use.".to_string() }
                    else { "Files downloaded, but missing trusted checksums leave integrity validation incomplete.".to_string() },
            })
        }.await;

        if let Err(error) = &download_result {
            record.state = if error.code == "download_interrupted" || error.code == "network" {
                LifecycleState::DownloadInterrupted
            } else { LifecycleState::Failed };
            record.last_error = Some(error.message.clone());
            record.touch();
            let _ = storage::save_record(&self.root, definition, &record);
        }
        download_result
    }

    pub fn cancel(&self, model_id: &str) -> Result<bool> {
        let definition = resolve(model_id)?;
        let active = self.active.lock().map_err(|_| MediaModelError::storage("The media-model operation state is unavailable."))?;
        if let Some(control) = active.get(definition.id) {
            control.cancel.store(true, Ordering::SeqCst);
            Ok(true)
        } else { Ok(false) }
    }

    pub async fn validate(&self, model_id: &str) -> Result<ValidationResult> {
        let definition = resolve(model_id)?;
        let _operation = self.begin_operation(definition.id)?;
        let mut record = storage::load_record(&self.root, definition.id)?
            .ok_or_else(|| MediaModelError::new("not_installed", "No downloaded files are recorded for this model."))?;
        storage::check_artifact_directory(&self.root, definition.id)?;
        let mut verified = 0usize;
        let mut all_checksums = true;
        for file in definition.files.iter().filter(|file| file.required) {
            let saved = record.files.iter().find(|saved| saved.file_id == file.id && saved.file_name == file.file_name)
                .ok_or_else(|| MediaModelError::new("not_installed", "A required model file has not been downloaded."))?;
            let saved_bytes = saved.bytes;
            let saved_sha256 = saved.sha256.clone();
            let path = storage::artifact_path(&self.root, definition.id, file)?;
            let (bytes, digest) = hash_file(path).await?;
            if bytes != saved_bytes || !digest.eq_ignore_ascii_case(&saved_sha256) {
                record.state = LifecycleState::Failed;
                record.last_error = Some("A downloaded model file changed since the manager recorded it.".to_string());
                let _ = storage::save_record(&self.root, definition, &record);
                return Err(MediaModelError::integrity("A downloaded model file changed since the manager recorded it."));
            }
            if file.expected_size_bytes.is_some_and(|expected| expected != bytes) {
                record.state = LifecycleState::Failed;
                record.last_error = Some("A downloaded model file does not match its registered size.".to_string());
                let _ = storage::save_record(&self.root, definition, &record);
                return Err(MediaModelError::integrity("A downloaded model file does not match its registered size."));
            }
            if let Some(expected) = file.expected_sha256 {
                if !digest.eq_ignore_ascii_case(expected) {
                    record.state = LifecycleState::Failed;
                    record.last_error = Some("A downloaded model file failed its trusted SHA-256 check.".to_string());
                    let _ = storage::save_record(&self.root, definition, &record);
                    return Err(MediaModelError::integrity("A downloaded model file failed its trusted SHA-256 check."));
                }
                verified += 1;
            } else {
                all_checksums = false;
            }
        }
        record.state = if all_checksums { LifecycleState::InstalledValidated } else { LifecycleState::DownloadedNotValidated };
        record.last_error = if all_checksums { None } else { Some("No trusted checksum is available for one or more required files; installation remains unvalidated.".to_string()) };
        record.touch();
        storage::save_record(&self.root, definition, &record)?;
        Ok(ValidationResult {
            model_id: definition.id.to_string(),
            state: record.state,
            verified_file_count: verified,
            integrity_complete: all_checksums,
            message: if all_checksums { "All required files match trusted registry integrity metadata.".to_string() }
                else { "File hashes were calculated, but no trusted checksum exists for at least one required file; the model is not validated.".to_string() },
        })
    }

    pub fn remove(&self, model_id: &str) -> Result<bool> {
        let definition = resolve(model_id)?;
        let _operation = self.begin_operation(definition.id)?;
        let Some(mut record) = storage::load_record(&self.root, definition.id)? else {
            return Ok(false);
        };
        record.state = LifecycleState::Removing;
        record.touch();
        storage::save_record(&self.root, definition, &record)?;
        storage::remove_owned_files(&self.root, definition, &record)?;
        Ok(true)
    }
}

fn lifecycle_snapshot_from(
    root: &Path,
    definition: &'static MediaModelDefinition,
    is_downloading: bool,
) -> Result<MediaModelLifecycleSnapshot> {
    let Some(record) = storage::load_record(root, definition.id)? else {
        return Ok(MediaModelLifecycleSnapshot {
            state: if is_downloading { LifecycleState::Downloading } else { default_state(definition) },
            manager_reported_downloaded: None,
            manager_reported_integrity_validated: None,
            recorded_required_file_count: None,
            note: "No managed installation record exists; unrecorded files are not treated as a Nexora model installation.".to_string(),
        });
    };

    let mut state = if is_downloading { LifecycleState::Downloading } else { record.state };
    if !is_downloading
        && matches!(state, LifecycleState::DownloadedNotValidated | LifecycleState::InstalledValidated)
        && !storage::record_files_present(root, definition, &record)
    {
        state = LifecycleState::Failed;
    }

    let required: Vec<_> = definition.files.iter().filter(|file| file.required).collect();
    let recorded_required_file_count = required.iter().filter(|file| {
        record.files.iter().any(|saved| saved.file_id == file.id && saved.file_name == file.file_name)
    }).count();
    let complete_files_present = !required.is_empty()
        && recorded_required_file_count == required.len()
        && storage::record_files_present(root, definition, &record);
    let integrity_recorded = !required.is_empty() && required.iter().all(|file| {
        let Some(expected_sha256) = file.expected_sha256 else { return false; };
        record.files.iter().any(|saved| {
            saved.file_id == file.id
                && saved.file_name == file.file_name
                && saved.integrity_verified
                && saved.sha256.eq_ignore_ascii_case(expected_sha256)
        })
    });
    let manager_reported_integrity_validated = Some(
        state == LifecycleState::InstalledValidated && complete_files_present && integrity_recorded,
    );

    Ok(MediaModelLifecycleSnapshot {
        state,
        manager_reported_downloaded: Some(complete_files_present),
        manager_reported_integrity_validated,
        recorded_required_file_count: Some(recorded_required_file_count),
        note: "Lifecycle values come from the media-model manager record and current required-file presence/size checks. This status does not recompute SHA-256 or establish runtime compatibility.".to_string(),
    })
}

pub(crate) fn registered_definition(model_id: &str) -> Option<&'static MediaModelDefinition> {
    registry::resolve(model_id)
}

fn resolve(model_id: &str) -> Result<&'static MediaModelDefinition> {
    if !registry::validate_model_id(model_id) { return Err(MediaModelError::invalid("The media-model identifier is invalid.")); }
    registry::resolve(model_id).ok_or_else(MediaModelError::unknown_model)
}

fn default_state(definition: &MediaModelDefinition) -> LifecycleState {
    if definition.downloadable && definition.files.iter().any(|file| file.required) {
        // Download readiness is about a registered source and safe artifact
        // metadata only; it is not a runtime-compatibility guarantee.
        LifecycleState::ReadyToDownload
    } else if definition.compatibility == DefinitionCompatibility::AwaitingAssessment {
        LifecycleState::AwaitingCompatibilityAssessment
    } else {
        LifecycleState::NotInstalled
    }
}

fn assess_compatibility(definition: &MediaModelDefinition, capabilities: &AdaptiveCapabilities) -> CompatibilityAssessment {
    let mut reasons = Vec::new();
    let requirements = definition.requirements;
    let ram = capabilities.resources.available_memory_bytes.value;
    let total_ram = capabilities.resources.total_memory_bytes.value;
    let vram = capabilities.resources.dedicated_vram_bytes.value;

    let ram_low = requirements.minimum_ram_bytes.is_some_and(|minimum| {
        total_ram.is_some_and(|total| total < minimum) || ram.is_some_and(|available| available < minimum)
    });
    let vram_low = requirements.minimum_vram_bytes.is_some_and(|minimum| vram.is_some_and(|available| available < minimum));
    if ram_low || vram_low {
        reasons.push("A known minimum resource requirement exceeds a confirmed local reading; this is a preliminary resource warning, not an inference compatibility verdict.".to_string());
        return CompatibilityAssessment { status: CompatibilityStatus::ResourceConstrained, reasons, runtime_compatibility_confirmed: false };
    }

    let mut unknown = false;
    if requirements.minimum_ram_bytes.is_none() || (ram.is_none() || total_ram.is_none()) { unknown = true; }
    if requirements.minimum_vram_bytes.is_some() && vram.is_none() { unknown = true; }
    if let Some(backend) = requirements.required_backend {
        let detected = capabilities.hardware.acceleration_backends.iter().find(|item| backend_matches(&item.backend, backend));
        if detected.map_or(true, |item| item.availability != DetectionConfidence::Confirmed) { unknown = true; }
    }
    if requirements.minimum_ram_bytes.is_none() && requirements.minimum_vram_bytes.is_none() && requirements.required_backend.is_none() { unknown = true; }
    if definition.compatibility == DefinitionCompatibility::ExperimentOnly { unknown = true; }
    if unknown {
        reasons.push("Model-specific requirements, GPU memory, backend availability, or compatibility evidence are incomplete.".to_string());
        CompatibilityAssessment { status: CompatibilityStatus::Unknown, reasons, runtime_compatibility_confirmed: false }
    } else {
        reasons.push("Reported minimum resources appear available, but model/runtime compatibility is not validated.".to_string());
        CompatibilityAssessment { status: CompatibilityStatus::ResourceHeadroomObserved, reasons, runtime_compatibility_confirmed: false }
    }
}

fn backend_matches(actual: &adaptive::AccelerationBackend, required: BackendRequirement) -> bool {
    matches!((actual, required),
        (&adaptive::AccelerationBackend::Cpu, BackendRequirement::Cpu)
        | (&adaptive::AccelerationBackend::Cuda, BackendRequirement::Cuda)
        | (&adaptive::AccelerationBackend::DirectMl, BackendRequirement::DirectMl)
        | (&adaptive::AccelerationBackend::Vulkan, BackendRequirement::Vulkan))
}

fn validate_definition(definition: &MediaModelDefinition) -> Result<()> {
    if !registry::validate_model_id(definition.id) { return Err(MediaModelError::invalid("A registered media-model ID is invalid.")); }
    let source = definition.source.ok_or_else(|| MediaModelError::new("source_unavailable", "No official source reference is recorded for this model."))?;
    let reference = reqwest::Url::parse(source.official_reference)
        .map_err(|_| MediaModelError::new("source_unavailable", "The official model reference is malformed."))?;
    if reference.scheme() != "https" || reference.username() != "" || reference.password().is_some() {
        return Err(MediaModelError::new("untrusted_source", "Official model references must use HTTPS without embedded credentials."));
    }
    if definition.files.is_empty() { return Err(MediaModelError::new("model_not_downloadable", "The registered model has no artifact files.")); }
    if definition.expected_total_size_bytes.is_some_and(|size| size == 0 || size > MAX_REGISTERED_MODEL_BYTES) {
        return Err(MediaModelError::new("size_limit", "The registered total model size is outside the supported range."));
    }
    let mut ids = std::collections::HashSet::new();
    let mut names = std::collections::HashSet::new();
    for file in definition.files {
        if !registry::validate_model_id(file.id) || !ids.insert(file.id) || !names.insert(file.file_name) {
            return Err(MediaModelError::invalid("Registered artifact identifiers and filenames must be unique and valid."));
        }
        storage::validate_leaf_name(file.file_name)?;
        if let Some(url) = file.download_url {
            let parsed = reqwest::Url::parse(url).map_err(|_| MediaModelError::new("source_unavailable", "A registered download URL is malformed."))?;
            download::validate_registered_url(parsed)?;
        }
        if let Some(size) = file.expected_size_bytes {
            if size == 0 || size > 256 * 1024 * 1024 * 1024 { return Err(MediaModelError::new("size_limit", "A registered artifact size is outside the supported range.")); }
        }
        if let Some(hash) = file.expected_sha256 {
            if hash.len() != 64 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) { return Err(MediaModelError::invalid("A registered SHA-256 value is malformed.")); }
        }
    }
    Ok(())
}

fn required_download_bytes(definition: &MediaModelDefinition) -> Result<Option<u64>> {
    let mut sum = 0u64;
    let mut all_sizes_known = true;
    for file in definition.files.iter().filter(|file| file.required) {
        if let Some(size) = file.expected_size_bytes {
            sum = sum.checked_add(size).ok_or_else(|| MediaModelError::new("size_limit", "The registered model size exceeds the supported range."))?;
        } else {
            all_sizes_known = false;
        }
    }
    if sum > MAX_REGISTERED_MODEL_BYTES {
        return Err(MediaModelError::new("size_limit", "The registered model size exceeds the supported total limit."));
    }
    if let Some(total) = definition.expected_total_size_bytes {
        if all_sizes_known && total != sum {
            return Err(MediaModelError::invalid("The registered total size does not match the required artifact sizes."));
        }
        return Ok(Some(total));
    }
    Ok(all_sizes_known.then_some(sum))
}

async fn file_is_unchanged(path: &Path, record: &InstalledFileRecord, definition: &ModelFileDefinition) -> Result<bool> {
    let (bytes, sha256) = hash_file(path.to_path_buf()).await?;
    Ok(bytes == record.bytes
        && sha256.eq_ignore_ascii_case(&record.sha256)
        && definition.expected_size_bytes.map_or(true, |expected| expected == bytes)
        && definition.expected_sha256.map_or(true, |expected| expected.eq_ignore_ascii_case(&sha256)))
}

async fn hash_file(path: PathBuf) -> Result<(u64, String)> {
    tokio::task::spawn_blocking(move || {
        use std::io::Read;
        let mut file = crate::inference::models::LockedFile::open(&path)
            .map_err(|_| MediaModelError::storage("A managed model artifact is not a regular, readable file."))?;
        let mut buffer = vec![0u8; 1024 * 1024];
        let mut hasher = sha2::Sha256::new();
        let mut bytes = 0u64;
        loop {
            let read = file.file_mut().read(&mut buffer)?;
            if read == 0 { break; }
            hasher.update(&buffer[..read]);
            bytes = bytes.saturating_add(read as u64);
        }
        use sha2::Digest;
        let digest = hasher.finalize().iter().map(|byte| format!("{byte:02x}")).collect();
        Ok((bytes, digest))
    }).await.map_err(|_| MediaModelError::storage("The model file validator stopped unexpectedly."))?
}
