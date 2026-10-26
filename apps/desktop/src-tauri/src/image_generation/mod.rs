//! Local text-to-image integration for the Mini experience.
//!
//! The only implemented execution path is the registered SD-Turbo checkpoint
//! through a stable-diffusion.cpp CLI resolved from fixed application-resource
//! locations and forced to CPU. This checkout does not bundle or pin that CLI,
//! and shared admission currently leaves runtime evidence unknown. The
//! coding-model engine is never called. Each CLI process owns model
//! load/use/unload for one request.

pub mod commands;
mod error;

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::adaptive;
use crate::inference::hardware;
use crate::media_models::{LifecycleState, MediaModelManager, ModelUseGuard};
use crate::resource_management::{
    AdmissionContext, AdmissionReasonCode, AdmissionStatus, AssetReadiness, HostResourceSnapshot,
    MediaResourceCoordinator, MediaWorkloadProfile, ReservationOutcome,
    ResourceOperationId, RuntimeAvailabilityEvidence, RuntimeCompatibilityEvidence, WorkloadShape,
};

use self::error::{ImageGenerationError as GenError, Result};

pub const SD_TURBO_MODEL_ID: &str = "stability-ai-sd-turbo-v1";
const SD_TURBO_FILE_ID: &str = "sd-turbo-checkpoint";
const SD_TURBO_RAW_BYTES: u64 = 5_214_561_328;
const WIDTH: u32 = 512;
const HEIGHT: u32 = 512;
const MAX_PROMPT_CHARS: usize = 2_000;
const MAX_PROMPT_BYTES: usize = 8_192;
const MAX_OUTPUT_BYTES: u64 = 64 * 1024 * 1024;
const MIN_OUTPUT_DISK_RESERVE: u64 = MAX_OUTPUT_BYTES * 2;
const MAX_RUN_TIME: Duration = Duration::from_secs(60 * 60);
const PROBE_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_PROBE_OUTPUT: usize = 32 * 1024;
const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ImageQualityProfile {
    Light,
    Balanced,
    Quality,
}

impl Default for ImageQualityProfile {
    fn default() -> Self { Self::Balanced }
}

impl ImageQualityProfile {
    /// SD-Turbo is documented for one to four steps. These presets affect only
    /// that supported quality/latency control; they do not promise performance.
    fn steps(self) -> u32 {
        match self {
            Self::Light => 1,
            Self::Balanced => 2,
            Self::Quality => 4,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateImageRequest {
    pub prompt: String,
    #[serde(default)]
    pub profile: ImageQualityProfile,
    #[serde(default)]
    pub seed: Option<u32>,
}

impl GenerateImageRequest {
    fn validate(&self) -> Result<()> {
        let prompt = self.prompt.trim();
        if prompt.is_empty() || prompt.chars().count() > MAX_PROMPT_CHARS || prompt.len() > MAX_PROMPT_BYTES {
            return Err(GenError::invalid("Enter a prompt of 1 to 2,000 characters (maximum 8 KiB)."));
        }
        if prompt.chars().any(char::is_control) {
            return Err(GenError::invalid("The prompt must not contain control characters."));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GenerationStage {
    Preparing,
    Running,
    Cancelling,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveGenerationStatus {
    pub request_id: String,
    pub stage: GenerationStage,
    /// stable-diffusion.cpp does not expose structured step progress through
    /// the supported CLI contract, so no fabricated percentage is reported.
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageGenerationStatus {
    pub model_id: String,
    pub model_state: LifecycleState,
    pub model_validated: bool,
    pub runtime_available: bool,
    pub runtime_cpu_backend_confirmed: bool,
    pub runtime_version_verified: bool,
    pub model_runtime_compatibility_confirmed: bool,
    /// This phase does not ship or pin a tested runtime binary.
    pub usable: bool,
    pub selected_backend: Option<String>,
    pub available_ram_bytes: Option<u64>,
    pub model_storage_free_bytes: Option<u64>,
    pub output_storage_free_bytes: Option<u64>,
    pub can_attempt_generation: bool,
    pub active_generation: Option<ActiveGenerationStatus>,
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedImageArtifact {
    pub schema_version: u16,
    pub artifact_id: String,
    pub model_id: String,
    pub model_version: String,
    pub mime_type: String,
    pub width: u32,
    pub height: u32,
    pub bytes: u64,
    pub sha256: String,
    pub seed: u32,
    pub profile: ImageQualityProfile,
    pub steps: u32,
    pub backend: String,
    pub created_at_unix_ms: u64,
}

struct GenerationControl {
    request_id: String,
    cancel: AtomicBool,
    stage: Mutex<GenerationStage>,
}

struct GenerationSlotGuard<'a> {
    manager: &'a ImageGenerationManager,
    control: Arc<GenerationControl>,
}

struct StagingDirectory(PathBuf);

impl Drop for StagingDirectory {
    fn drop(&mut self) { let _ = cleanup_staging(&self.0); }
}

impl Drop for GenerationSlotGuard<'_> {
    fn drop(&mut self) {
        // If the invoking webview future is dropped, the detached blocking
        // worker still sees cancellation and stops its child process.
        self.control.cancel.store(true, Ordering::SeqCst);
        if let Ok(mut active) = self.manager.active.lock() {
            if active.as_ref().is_some_and(|current| Arc::ptr_eq(current, &self.control)) {
                *active = None;
            }
        }
    }
}

pub struct ImageGenerationManager {
    output_root: PathBuf,
    resource_dir: Option<PathBuf>,
    executable_dir: Option<PathBuf>,
    active: Mutex<Option<Arc<GenerationControl>>>,
    resources: MediaResourceCoordinator,
}

impl ImageGenerationManager {
    pub fn new(
        output_root: PathBuf,
        resource_dir: Option<PathBuf>,
        executable_dir: Option<PathBuf>,
        resources: MediaResourceCoordinator,
    ) -> Self {
        Self { output_root, resource_dir, executable_dir, active: Mutex::new(None), resources }
    }

    pub fn cancel(&self) -> Result<bool> {
        let active = self.active.lock().map_err(|_| GenError::storage("Image-generation state is unavailable."))?;
        if let Some(control) = active.as_ref() {
            control.cancel.store(true, Ordering::SeqCst);
            if let Ok(mut stage) = control.stage.lock() { *stage = GenerationStage::Cancelling; }
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn shutdown(&self) {
        if let Ok(active) = self.active.lock() {
            if let Some(control) = active.as_ref() {
                control.cancel.store(true, Ordering::SeqCst);
                if let Ok(mut stage) = control.stage.lock() { *stage = GenerationStage::Cancelling; }
            }
        }
    }

    pub fn status_snapshot(&self) -> Option<ActiveGenerationStatus> {
        let active = self.active.lock().ok()?;
        let control = active.as_ref()?;
        let stage = *control.stage.lock().ok()?;
        Some(ActiveGenerationStatus {
            request_id: control.request_id.clone(),
            stage,
            detail: match stage {
                GenerationStage::Preparing => "Checking validated model files, runtime, and local resources.".to_string(),
                GenerationStage::Running => "The local CPU runtime is active. Step-level progress is not available from this runtime interface.".to_string(),
                GenerationStage::Cancelling => "Stopping the local process and discarding its incomplete output.".to_string(),
            },
        })
    }

    pub fn availability(
        &self,
        model_state: LifecycleState,
        assets: AssetReadiness,
        caps: &adaptive::AdaptiveCapabilities,
        runtime_available: bool,
        cpu_backend_confirmed: bool,
    ) -> Result<ImageGenerationStatus> {
        let ram = caps.resources.available_memory_bytes.value;
        let model_storage_free = caps.resources.model_storage_free_bytes.value;
        let output_storage_free = hardware::disk_space(&self.output_root).free_bytes;
        let validated = model_state == LifecycleState::InstalledValidated;
        let resource_profile = MediaWorkloadProfile::registered_image_generation(
            SD_TURBO_MODEL_ID,
            MIN_OUTPUT_DISK_RESERVE,
            SD_TURBO_RAW_BYTES,
            (WIDTH, HEIGHT),
            vec![1, 2, 4],
        );
        let runtime_compatibility = if !cpu_backend_confirmed {
            RuntimeCompatibilityEvidence::Unknown {
                reason: "The status query did not confirm the requested CPU backend; runtime/model compatibility remains unknown.".to_string(),
            }
        } else {
            RuntimeCompatibilityEvidence::Unknown {
                reason: "The SD-Turbo runtime package is not version-pinned and model/runtime compatibility is not confirmed.".to_string(),
            }
        };
        let admission = self.resources.assess(
            &resource_profile,
            &AdmissionContext {
                assets,
                runtime_availability: if runtime_available {
                    RuntimeAvailabilityEvidence::Available {
                        runtime: "stable-diffusion.cpp sd-cli at an approved application resource location".to_string(),
                        version_pinned: false,
                    }
                } else {
                    RuntimeAvailabilityEvidence::Unavailable {
                        reason: "No SD-Turbo CPU runtime is available at the approved application resource locations.".to_string(),
                    }
                },
                runtime_compatibility,
                host: HostResourceSnapshot::from_adaptive(caps, output_storage_free),
                workload: WorkloadShape::Image { width: WIDTH, height: HEIGHT, steps: 2 },
            },
        );
        let can_attempt = admission.status == AdmissionStatus::AllowedByKnownConstraints;
        let mut limitations = vec![
            "The publisher/runtime do not document an exact RAM or VRAM minimum for this CPU path. The 5,214,561,328-byte checkpoint size is only a lower-bound screening value, not a sufficiency guarantee.".to_string(),
            "Generation uses CPU only. GPU acceleration is not selected or inferred from GPU presence.".to_string(),
            "The CLI reports no structured diffusion-step progress; status is limited to preparation and process-running states.".to_string(),
            "The runtime binary is not included in this checkout. A packaged CPU build must be supplied before generation can run.".to_string(),
            "No stable-diffusion.cpp release is pinned and tested by Nexora, so model/runtime compatibility remains unconfirmed even if a runtime executable is bundled.".to_string(),
        ];
        if runtime_available { limitations.retain(|item| !item.starts_with("The runtime binary is not included")); }
        if ram.is_none() { limitations.push("Available RAM could not be measured; generation is blocked rather than guessing.".to_string()); }
        if output_storage_free.is_none() { limitations.push("Free output storage could not be measured; generation is blocked rather than guessing.".to_string()); }
        if !cpu_backend_confirmed && runtime_available { limitations.push("The detected runtime executable did not confirm its CPU device/backend.".to_string()); }
        if ram.map_or(false, |available| available < SD_TURBO_RAW_BYTES) {
            limitations.push("Available RAM is below the checkpoint's raw size; generation is blocked.".to_string());
        }
        if output_storage_free.map_or(false, |free| free < MIN_OUTPUT_DISK_RESERVE) {
            limitations.push("Output volume has less than the reserved 128 MiB required for safe staging.".to_string());
        }
        // The active process uses fixed-size SD-Turbo output settings. Profiles
        // control steps only and map to the model card's documented 1-4 range.
        limitations.push("Output is fixed at the model-card-preferred 512×512 PNG size; profiles map to 1, 2, or 4 denoising steps.".to_string());
        Ok(ImageGenerationStatus {
            model_id: SD_TURBO_MODEL_ID.to_string(),
            model_state,
            model_validated: validated,
            runtime_available,
            runtime_cpu_backend_confirmed: cpu_backend_confirmed,
            runtime_version_verified: false,
            model_runtime_compatibility_confirmed: false,
            usable: false,
            selected_backend: if cpu_backend_confirmed { Some("cpu".to_string()) } else { None },
            available_ram_bytes: ram,
            model_storage_free_bytes: model_storage_free,
            output_storage_free_bytes: output_storage_free,
            can_attempt_generation: can_attempt,
            active_generation: self.status_snapshot(),
            limitations,
        })
    }

    pub async fn generate(
        &self,
        models: &MediaModelManager,
        request: GenerateImageRequest,
    ) -> Result<GeneratedImageArtifact> {
        request.validate()?;
        let control = Arc::new(GenerationControl {
            request_id: random_id()?,
            cancel: AtomicBool::new(false),
            stage: Mutex::new(GenerationStage::Preparing),
        });
        {
            let mut active = self.active.lock().map_err(|_| GenError::storage("Image-generation state is unavailable."))?;
            if active.is_some() { return Err(GenError::busy()); }
            *active = Some(control.clone());
        }
        let _slot = GenerationSlotGuard { manager: self, control: control.clone() };
        self.generate_inner(models, request, control).await
    }

    async fn generate_inner(
        &self,
        models: &MediaModelManager,
        request: GenerateImageRequest,
        control: Arc<GenerationControl>,
    ) -> Result<GeneratedImageArtifact> {
        if control.cancel.load(Ordering::SeqCst) { return Err(GenError::cancelled()); }

        // Use the existing Phase 1 snapshot and Phase 2 lifecycle record. The
        // shared gate runs before re-hashing weights or probing/starting a
        // runtime; the registered runtime/model compatibility remains unknown.
        let capabilities = adaptive::collect(models.storage_root());
        let output_free = hardware::disk_space(&self.output_root).free_bytes;
        let lifecycle = models.lifecycle_snapshot(SD_TURBO_MODEL_ID).await.map_err(GenError::from)?;
        let resource_profile = MediaWorkloadProfile::registered_image_generation(
            SD_TURBO_MODEL_ID,
            MIN_OUTPUT_DISK_RESERVE,
            SD_TURBO_RAW_BYTES,
            (WIDTH, HEIGHT),
            vec![1, 2, 4],
        );
        let admission_context = AdmissionContext {
            assets: AssetReadiness::from_lifecycle(&lifecycle),
            runtime_availability: RuntimeAvailabilityEvidence::Unknown {
                reason: "The SD-Turbo runtime is not established as available by a reviewed, version-pinned package.".to_string(),
            },
            runtime_compatibility: RuntimeCompatibilityEvidence::Unknown {
                reason: "SD-Turbo/runtime compatibility is not confirmed; manager validation alone does not establish it.".to_string(),
            },
            host: HostResourceSnapshot::from_adaptive(&capabilities, output_free),
            workload: WorkloadShape::Image {
                width: WIDTH,
                height: HEIGHT,
                steps: request.profile.steps(),
            },
        };
        let operation_id = ResourceOperationId::from_native_id(&control.request_id)
            .ok_or_else(|| GenError::storage("A secure media operation identifier was malformed."))?;
        let reservation = match self.resources.reserve(
            operation_id,
            &resource_profile,
            &admission_context,
        ) {
            ReservationOutcome::Reserved { decision: _, reservation } => reservation,
            ReservationOutcome::Denied(decision) => {
                let reason_codes: Vec<_> = decision.reasons.iter().map(|reason| reason.code).collect();
                if decision.status == AdmissionStatus::Rejected
                    && reason_codes.contains(&AdmissionReasonCode::CheckpointSizeScreeningFloor)
                {
                    return Err(GenError::insufficient_memory());
                }
                if decision.status == AdmissionStatus::Rejected
                    && reason_codes.contains(&AdmissionReasonCode::InsufficientTemporaryDisk)
                {
                    return Err(GenError::insufficient_storage());
                }
                let code = match decision.status {
                    AdmissionStatus::Rejected => "resource_admission_rejected",
                    AdmissionStatus::Deferred => "resource_admission_deferred",
                    AdmissionStatus::Unknown => "resource_admission_unknown",
                    AdmissionStatus::Unavailable => "resource_unavailable",
                    AdmissionStatus::AllowedByKnownConstraints => "resource_admission_error",
                };
                return Err(GenError::new(code, decision.user_message()));
            }
        };
        if control.cancel.load(Ordering::SeqCst) { return Err(GenError::cancelled()); }

        let runtime = self.resolve_runtime().ok_or_else(GenError::runtime_missing)?;
        let runtime_for_probe = runtime.clone();
        let probe = tauri::async_runtime::spawn_blocking(move || probe_cpu_backend(&runtime_for_probe))
            .await
            .map_err(|_| GenError::runtime_incompatible())??;
        if !probe.cpu_backend_confirmed { return Err(GenError::runtime_incompatible()); }

        let validation = models.validate(SD_TURBO_MODEL_ID).await.map_err(GenError::from)?;
        if !validation.integrity_complete || validation.state != LifecycleState::InstalledValidated {
            return Err(GenError::model_not_ready("Download SD-Turbo and confirm that its registered SHA-256 checksum validates before generating."));
        }
        if control.cancel.load(Ordering::SeqCst) { return Err(GenError::cancelled()); }
        let (model_guard, locked_model) = models
            .lock_validated_artifact(SD_TURBO_MODEL_ID, SD_TURBO_FILE_ID)
            .await
            .map_err(GenError::from)?;
        let model_path = locked_model.path().to_path_buf();

        let runtime_root = runtime.parent().map(Path::to_path_buf)
            .ok_or_else(GenError::runtime_missing)?;
        let output_root = self.output_root.clone();
        let control_for_worker = control.clone();
        let profile = request.profile;
        let steps = profile.steps();
        let seed = match request.seed { Some(seed) => seed, None => random_seed()? };
        let prompt = request.prompt.trim().to_string();
        let worker = tauri::async_runtime::spawn_blocking(move || {
            // Keep capacity reserved for the complete child-process lifetime,
            // including load, inference, output validation, and cleanup.
            let _resource_reservation = reservation;
            let _model_guard: ModelUseGuard = model_guard;
            let _locked_model = locked_model;
            run_generation(
                &runtime,
                &runtime_root,
                &output_root,
                &model_path,
                prompt,
                profile,
                steps,
                seed,
                &control_for_worker,
            )
        });
        // Keep the registry's per-model use lock alive until the child exits so
        // manager removal cannot unlink the checkpoint while it is being read.
        worker.await.map_err(|_| GenError::inference_failed())?
    }

    fn resolve_runtime(&self) -> Option<PathBuf> {
        #[cfg(windows)]
        let binary_name = "sd-cli.exe";
        #[cfg(not(windows))]
        let binary_name = "sd-cli";
        for base in [self.resource_dir.as_deref(), self.executable_dir.as_deref()].into_iter().flatten() {
            let Ok(base_meta) = fs::symlink_metadata(base) else { continue; };
            if base_meta.file_type().is_symlink() || !base_meta.is_dir() { continue; }
            let Ok(canonical_base) = fs::canonicalize(base) else { continue; };
            for directory_name in ["image-runtime", "binaries"] {
                let runtime_dir = base.join(directory_name);
                let Ok(directory_meta) = fs::symlink_metadata(&runtime_dir) else { continue; };
                if directory_meta.file_type().is_symlink() || !directory_meta.is_dir() { continue; }
                let candidate = runtime_dir.join(binary_name);
                let Ok(metadata) = fs::symlink_metadata(&candidate) else { continue; };
                if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() == 0 { continue; }
                let Ok(canonical) = fs::canonicalize(&candidate) else { continue; };
                let Ok(canonical_dir) = fs::canonicalize(&runtime_dir) else { continue; };
                if canonical.parent() == Some(canonical_dir.as_path()) && canonical.starts_with(&canonical_base) {
                    return Some(canonical);
                }
            }
        }
        None
    }

    pub(crate) fn artifact_root(&self) -> PathBuf { self.output_root.clone() }

    pub(crate) fn get_artifact_from_root(root: &Path, artifact_id: &str) -> Result<GeneratedImageArtifact> {
        if !valid_id(artifact_id) { return Err(GenError::unknown_artifact()); }
        let root_meta = fs::symlink_metadata(root).map_err(|_| GenError::unknown_artifact())?;
        if root_meta.file_type().is_symlink() || !root_meta.is_dir() { return Err(GenError::unknown_artifact()); }
        let record_path = root.join(format!("{artifact_id}.json"));
        let image_path = root.join(format!("{artifact_id}.png"));
        let record_meta = fs::symlink_metadata(&record_path).map_err(|_| GenError::unknown_artifact())?;
        let image_meta = fs::symlink_metadata(&image_path).map_err(|_| GenError::unknown_artifact())?;
        if record_meta.file_type().is_symlink() || !record_meta.is_file() || record_meta.len() > 64 * 1024
            || image_meta.file_type().is_symlink() || !image_meta.is_file() || image_meta.len() > MAX_OUTPUT_BYTES
        { return Err(GenError::unknown_artifact()); }
        let encoded = fs::read(record_path).map_err(|_| GenError::unknown_artifact())?;
        let artifact: GeneratedImageArtifact = serde_json::from_slice(&encoded).map_err(|_| GenError::unknown_artifact())?;
        if artifact.schema_version != 1
            || artifact.artifact_id != artifact_id
            || artifact.model_id != SD_TURBO_MODEL_ID
            || artifact.model_version != "1.0; weights commit 9fee4496504b3b0e95e11c80bdf0fb0c7a5d4aa8"
            || artifact.mime_type != "image/png"
            || artifact.width != WIDTH
            || artifact.height != HEIGHT
            || artifact.steps != artifact.profile.steps()
            || artifact.bytes != image_meta.len()
            || artifact.backend != "cpu"
        {
            return Err(GenError::unknown_artifact());
        }
        let (bytes, digest) = hash_and_validate_png(&image_path, artifact.width, artifact.height)?;
        if bytes != artifact.bytes || !digest.eq_ignore_ascii_case(&artifact.sha256) {
            return Err(GenError::new("integrity_failed", "The generated image changed after Nexora recorded it."));
        }
        Ok(artifact)
    }
}

#[derive(Debug)]
struct RuntimeProbe {
    cpu_backend_confirmed: bool,
}

fn probe_cpu_backend(runtime: &Path) -> Result<RuntimeProbe> {
    let (code, stdout, stderr) = run_capture(runtime, &["--list-devices"], PROBE_TIMEOUT)?;
    if !code.success() { return Err(GenError::runtime_incompatible()); }
    let output = format!("{}\n{}", String::from_utf8_lossy(&stdout), String::from_utf8_lossy(&stderr)).to_ascii_lowercase();
    Ok(RuntimeProbe { cpu_backend_confirmed: output.lines().any(|line| line.contains("cpu")) })
}

fn run_capture(executable: &Path, args: &[&str], timeout: Duration) -> Result<(std::process::ExitStatus, Vec<u8>, Vec<u8>)> {
    let mut command = Command::new(executable);
    command.args(args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    configure_hidden_window(&mut command);
    let mut child = command.spawn().map_err(|_| GenError::runtime_missing())?;
    let stdout = child.stdout.take().ok_or_else(GenError::runtime_incompatible)?;
    let stderr = child.stderr.take().ok_or_else(GenError::runtime_incompatible)?;
    let stdout_reader = thread::spawn(move || read_bounded(stdout));
    let stderr_reader = thread::spawn(move || read_bounded(stderr));
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(50)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(GenError::runtime_incompatible());
            }
        }
    };
    let stdout = stdout_reader.join().map_err(|_| GenError::runtime_incompatible())?
        .map_err(|_| GenError::runtime_incompatible())?;
    let stderr = stderr_reader.join().map_err(|_| GenError::runtime_incompatible())?
        .map_err(|_| GenError::runtime_incompatible())?;
    Ok((status, stdout, stderr))
}

fn read_bounded(mut reader: impl Read) -> std::io::Result<Vec<u8>> {
    let mut output = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        let count = reader.read(&mut chunk)?;
        if count == 0 { break; }
        output.extend_from_slice(&chunk[..count]);
        if output.len() > MAX_PROBE_OUTPUT {
            let remove = output.len() - MAX_PROBE_OUTPUT;
            output.drain(..remove);
        }
    }
    Ok(output)
}

fn run_generation(
    runtime: &Path,
    runtime_root: &Path,
    output_root: &Path,
    model_path: &Path,
    prompt: String,
    profile: ImageQualityProfile,
    steps: u32,
    seed: u32,
    control: &GenerationControl,
) -> Result<GeneratedImageArtifact> {
    ensure_output_layout(output_root)?;
    let artifact_id = control.request_id.clone();
    if !valid_id(&artifact_id) { return Err(GenError::storage("A generation identifier was malformed.")); }
    let staging = output_root.join(".staging").join(&artifact_id);
    fs::create_dir(&staging).map_err(|_| GenError::storage("A private image staging folder could not be created."))?;
    let _staging_guard = StagingDirectory(staging.clone());
    let staged_image = staging.join("output.png");
    let final_image = output_root.join(format!("{artifact_id}.png"));
    let final_record = output_root.join(format!("{artifact_id}.json"));

    let result = run_cli(runtime, runtime_root, model_path, &staged_image, &prompt, steps, seed, control);
    if let Err(error) = result {
        return Err(cleanup_after_error(&staging, error));
    }
    if control.cancel.load(Ordering::SeqCst) {
        return Err(cleanup_after_error(&staging, GenError::cancelled()));
    }
    let (bytes, digest) = match hash_and_validate_png(&staged_image, WIDTH, HEIGHT) {
        Ok(value) => value,
        Err(_) => return Err(cleanup_after_error(&staging, GenError::output_invalid())),
    };
    if bytes > MAX_OUTPUT_BYTES {
        return Err(cleanup_after_error(&staging, GenError::output_invalid()));
    }
    if fs::hard_link(&staged_image, &final_image).is_err() {
        return Err(cleanup_after_error(
            &staging,
            GenError::storage("A unique generated-image destination could not be published without overwriting an existing artifact."),
        ));
    }
    let artifact = GeneratedImageArtifact {
        schema_version: 1,
        artifact_id: artifact_id.clone(),
        model_id: SD_TURBO_MODEL_ID.to_string(),
        model_version: "1.0; weights commit 9fee4496504b3b0e95e11c80bdf0fb0c7a5d4aa8".to_string(),
        mime_type: "image/png".to_string(),
        width: WIDTH,
        height: HEIGHT,
        bytes,
        sha256: digest,
        seed,
        profile,
        steps,
        backend: "cpu".to_string(),
        created_at_unix_ms: now_ms(),
    };
    let encoded = match serde_json::to_vec(&artifact) {
        Ok(encoded) => encoded,
        Err(_) => {
            if remove_owned_output(&final_image).is_err() || cleanup_staging(&staging).is_err() {
                return Err(GenError::cleanup_failed());
            }
            return Err(GenError::storage("Generated-image metadata could not be encoded."));
        }
    };
    let staged_record = staging.join("artifact.json");
    let metadata_result = OpenOptions::new().write(true).create_new(true).open(&staged_record)
        .and_then(|mut file| { file.write_all(&encoded)?; file.sync_all() });
    if metadata_result.is_err() || fs::hard_link(&staged_record, &final_record).is_err() {
        if remove_owned_output(&final_image).is_err() || cleanup_staging(&staging).is_err() {
            return Err(GenError::cleanup_failed());
        }
        return Err(GenError::storage("Generated-image metadata could not be published without overwriting an existing artifact."));
    }
    cleanup_staging(&staging).map_err(|_| GenError::cleanup_failed())?;
    Ok(artifact)
}

fn run_cli(
    runtime: &Path,
    runtime_root: &Path,
    model_path: &Path,
    output: &Path,
    prompt: &str,
    steps: u32,
    seed: u32,
    control: &GenerationControl,
) -> Result<()> {
    if control.cancel.load(Ordering::SeqCst) { return Err(GenError::cancelled()); }
    let mut command = Command::new(runtime);
    command
        .current_dir(runtime_root)
        .arg("-m").arg(model_path)
        .arg("-p").arg(prompt)
        .arg("-W").arg(WIDTH.to_string())
        .arg("-H").arg(HEIGHT.to_string())
        .arg("--steps").arg(steps.to_string())
        .arg("--cfg-scale").arg("0.0")
        .arg("--backend").arg("cpu")
        .arg("-s").arg(seed.to_string())
        .arg("-o").arg(output)
        .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    configure_hidden_window(&mut command);
    let mut child = command.spawn().map_err(|_| GenError::runtime_missing())?;
    if let Ok(mut stage) = control.stage.lock() { *stage = GenerationStage::Running; }
    let deadline = Instant::now() + MAX_RUN_TIME;
    loop {
        if fs::symlink_metadata(output).map_or(false, |metadata| metadata.len() > MAX_OUTPUT_BYTES) {
            stop_child(&mut child);
            return Err(GenError::output_invalid());
        }
        if control.cancel.load(Ordering::SeqCst) {
            stop_child(&mut child);
            return Err(GenError::cancelled());
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() { return Err(GenError::inference_failed()); }
                return Ok(());
            }
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(100)),
            _ => {
                stop_child(&mut child);
                return Err(GenError::timeout());
            }
        }
    }
}

fn stop_child(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(windows)]
fn configure_hidden_window(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    command.creation_flags(0x0800_0000);
}

#[cfg(not(windows))]
fn configure_hidden_window(_command: &mut Command) {}

fn ensure_output_layout(root: &Path) -> Result<()> {
    fs::create_dir_all(root).map_err(|_| GenError::storage("The application-owned image output folder could not be prepared."))?;
    let root_meta = fs::symlink_metadata(root).map_err(|_| GenError::storage("The image output folder could not be inspected."))?;
    if root_meta.file_type().is_symlink() || !root_meta.is_dir() {
        return Err(GenError::storage("The application-owned image output root is not a normal directory."));
    }
    let staging = root.join(".staging");
    match fs::symlink_metadata(&staging) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => return Err(GenError::storage("The image staging folder is unsafe.")),
        Ok(_) => {},
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => fs::create_dir(&staging).map_err(|_| GenError::storage("The image staging folder could not be created."))?,
        Err(_) => return Err(GenError::storage("The image staging folder could not be inspected.")),
    }
    Ok(())
}

fn cleanup_staging(staging: &Path) -> Result<()> {
    match fs::symlink_metadata(staging) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(GenError::cleanup_failed()),
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => Err(GenError::cleanup_failed()),
        Ok(_) => fs::remove_dir_all(staging).map_err(|_| GenError::cleanup_failed()),
    }
}

fn cleanup_after_error(staging: &Path, primary_error: GenError) -> GenError {
    if cleanup_staging(staging).is_err() {
        GenError::cleanup_failed()
    } else {
        primary_error
    }
}

/// Removes only the regular output file this request created. Symlinks and
/// unrelated filesystem objects are never followed or removed.
fn remove_owned_output(path: &Path) -> std::io::Result<()> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => Err(
            std::io::Error::new(std::io::ErrorKind::Other, "output is not a regular owned file"),
        ),
        Ok(_) => fs::remove_file(path),
    }
}

fn hash_and_validate_png(path: &Path, expected_width: u32, expected_height: u32) -> Result<(u64, String)> {
    let metadata = fs::symlink_metadata(path).map_err(|_| GenError::output_invalid())?;
    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() < 45 || metadata.len() > MAX_OUTPUT_BYTES {
        return Err(GenError::output_invalid());
    }
    let mut file = File::open(path).map_err(|_| GenError::output_invalid())?;
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.read_to_end(&mut bytes).map_err(|_| GenError::output_invalid())?;
    if bytes.len() as u64 != metadata.len() || bytes.len() < 45 || &bytes[..8] != PNG_SIGNATURE {
        return Err(GenError::output_invalid());
    }
    let mut offset = 8usize;
    let mut saw_ihdr = false;
    let mut saw_idat = false;
    let mut saw_iend = false;
    while offset.checked_add(12).map_or(false, |end| end <= bytes.len()) {
        let length = u32::from_be_bytes(bytes[offset..offset + 4].try_into().map_err(|_| GenError::output_invalid())?) as usize;
        let end = offset.checked_add(12).and_then(|start| start.checked_add(length)).ok_or_else(GenError::output_invalid)?;
        if end > bytes.len() { return Err(GenError::output_invalid()); }
        let kind = &bytes[offset + 4..offset + 8];
        let data_start = offset + 8;
        let data_end = data_start + length;
        let expected_crc = u32::from_be_bytes(bytes[data_end..data_end + 4].try_into().map_err(|_| GenError::output_invalid())?);
        if crc32fast::hash(&bytes[offset + 4..data_end]) != expected_crc { return Err(GenError::output_invalid()); }
        if !saw_ihdr {
            if kind != b"IHDR" || length != 13 { return Err(GenError::output_invalid()); }
            let width = u32::from_be_bytes(bytes[data_start..data_start + 4].try_into().map_err(|_| GenError::output_invalid())?);
            let height = u32::from_be_bytes(bytes[data_start + 4..data_start + 8].try_into().map_err(|_| GenError::output_invalid())?);
            let bit_depth = bytes[data_start + 8];
            let color_type = bytes[data_start + 9];
            let compression = bytes[data_start + 10];
            let filter = bytes[data_start + 11];
            let interlace = bytes[data_start + 12];
            if width != expected_width || height != expected_height
                || !matches!((color_type, bit_depth), (2, 8) | (2, 16) | (6, 8) | (6, 16))
                || compression != 0 || filter != 0 || interlace > 1
            { return Err(GenError::output_invalid()); }
            saw_ihdr = true;
        } else if kind == b"IHDR" {
            return Err(GenError::output_invalid());
        }
        if kind == b"IDAT" { saw_idat = true; }
        if kind == b"IEND" {
            if length != 0 || !saw_idat || end != bytes.len() { return Err(GenError::output_invalid()); }
            saw_iend = true;
            break;
        }
        offset = end;
    }
    if !saw_ihdr || !saw_iend { return Err(GenError::output_invalid()); }

    // Decode the actual pixel stream as well as checking PNG chunk framing and
    // CRCs; a syntactically plausible header with corrupt compressed pixels is
    // not a successful generation artifact.
    let image_file = File::open(path).map_err(|_| GenError::output_invalid())?;
    let mut decoder = png::Decoder::new(std::io::BufReader::new(image_file));
    decoder.set_transformations(png::Transformations::IDENTITY);
    let mut reader = decoder.read_info().map_err(|_| GenError::output_invalid())?;
    if reader.info().width != expected_width || reader.info().height != expected_height {
        return Err(GenError::output_invalid());
    }
    let output_buffer_size = reader.output_buffer_size().ok_or_else(GenError::output_invalid)?;
    if output_buffer_size > (WIDTH as usize * HEIGHT as usize * 8) {
        return Err(GenError::output_invalid());
    }
    let mut decoded_pixels = vec![0u8; output_buffer_size];
    let frame = reader.next_frame(&mut decoded_pixels).map_err(|_| GenError::output_invalid())?;
    if frame.width != expected_width || frame.height != expected_height {
        return Err(GenError::output_invalid());
    }
    let digest = Sha256::digest(&bytes).iter().map(|byte| format!("{byte:02x}")).collect::<String>();
    Ok((bytes.len() as u64, digest))
}

fn random_id() -> Result<String> {
    let mut bytes = [0u8; 16];
    getrandom::getrandom(&mut bytes).map_err(|_| GenError::storage("A secure generation identifier could not be created."))?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn random_seed() -> Result<u32> {
    let mut bytes = [0u8; 4];
    getrandom::getrandom(&mut bytes).map_err(|_| GenError::storage("A secure generation seed could not be created."))?;
    Ok(u32::from_le_bytes(bytes))
}

fn valid_id(value: &str) -> bool {
    value.len() == 32 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).ok()
        .and_then(|time| u64::try_from(time.as_millis()).ok()).unwrap_or(0)
}
