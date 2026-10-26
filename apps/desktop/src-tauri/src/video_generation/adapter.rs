use serde::Serialize;

use crate::media_models::{LifecycleState, MediaModelLifecycleSnapshot};

pub(crate) const WAN21_MODEL_ID: &str = "wan-ai-wan2_1-t2v-1_3b";
const SD_CPP_REVISION: &str = "40b1788476bbe8fe42fa538ddf7d32408a126b93";
const WAN21_RUNTIME_DOCS: &str = "https://github.com/leejet/stable-diffusion.cpp/blob/40b1788476bbe8fe42fa538ddf7d32408a126b93/docs/wan.md";
const WAN21_MODEL_CARD: &str = "https://huggingface.co/Wan-AI/Wan2.1-T2V-1.3B";
const WAN21_LICENSE: &str = "https://github.com/Wan-Video/Wan2.1/blob/main/LICENSE.txt";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoGenerationCapabilityStatus {
    pub candidate_model_id: String,
    pub candidate_model_name: String,
    pub task: String,
    pub model_registered_with_media_manager: bool,
    pub model_lifecycle_state: Option<String>,
    /// Manager-recorded status, not a fresh checksum operation.
    pub manager_reported_downloaded: Option<bool>,
    pub manager_reported_integrity_validated: Option<bool>,
    pub manager_recorded_required_file_count: Option<usize>,
    pub manager_state_note: String,
    pub model_loaded: bool,
    pub usable_for_text_to_video: bool,
    pub required_model_file_count: u8,
    pub registered_model_size_bytes: u64,
    pub model_license: String,
    pub model_license_reference: String,
    pub runtime_name: String,
    pub runtime_source_revision: String,
    pub runtime_reference: String,
    pub upstream_windows_support_documented: bool,
    pub upstream_backends_documented: Vec<String>,
    pub runtime_bundle_available: bool,
    pub runtime_package_version_pinned: bool,
    pub runtime_binary_revision_verified: bool,
    pub runtime_probe_performed: bool,
    pub model_runtime_compatibility_confirmed: bool,
    pub runtime_compatibility_status: &'static str,
    pub model_specific_resource_fit_confirmed: bool,
    pub hardware_compatibility: &'static str,
    pub generation_command_available: bool,
    pub nexora_output_pipeline_available: bool,
    pub upstream_video_containers_documented: Vec<String>,
    pub selected_diffusion_weights: String,
    pub selected_text_encoder: String,
    pub resource_evidence: String,
    pub limitations: Vec<String>,
}

/// Reviewed model and upstream runtime adapter boundary only. This reports
/// the Phase 2 manager lifecycle without adaptive/hardware probing, but does
/// not resolve or launch a runtime, load weights, or infer video.
pub(crate) struct Wan21T2v13b;

impl Wan21T2v13b {
    pub(crate) fn status(lifecycle: Option<MediaModelLifecycleSnapshot>) -> VideoGenerationCapabilityStatus {
        let (lifecycle_state, manager_reported_downloaded, manager_reported_integrity_validated,
            manager_recorded_required_file_count, manager_state_note) = match lifecycle {
            Some(snapshot) => (
                Some(lifecycle_state_name(snapshot.state).to_string()),
                snapshot.manager_reported_downloaded,
                snapshot.manager_reported_integrity_validated,
                snapshot.recorded_required_file_count,
                snapshot.note,
            ),
            None => (
                None,
                None,
                None,
                None,
                "The manager lifecycle could not be read; installation state is unknown.".to_string(),
            ),
        };
        VideoGenerationCapabilityStatus {
            candidate_model_id: WAN21_MODEL_ID.to_string(),
            candidate_model_name: "Wan2.1 Text-to-Video 1.3B".to_string(),
            task: "text_to_video_only".to_string(),
            model_registered_with_media_manager: true,
            model_lifecycle_state,
            manager_reported_downloaded,
            manager_reported_integrity_validated,
            manager_recorded_required_file_count,
            manager_state_note,
            // There is no model-loader implementation in Nexora for this task.
            model_loaded: false,
            usable_for_text_to_video: false,
            required_model_file_count: 3,
            registered_model_size_bytes: 9_135_187_134,
            model_license: "Apache-2.0 (Wan2.1 and Google UMT5; the selected city96 GGUF conversion also declares Apache-2.0)".to_string(),
            model_license_reference: WAN21_LICENSE.to_string(),
            runtime_name: "stable-diffusion.cpp sd-cli, vid_gen mode".to_string(),
            runtime_source_revision: SD_CPP_REVISION.to_string(),
            runtime_reference: WAN21_RUNTIME_DOCS.to_string(),
            upstream_windows_support_documented: true,
            upstream_backends_documented: vec![
                "cpu".to_string(),
                "cuda".to_string(),
                "vulkan".to_string(),
                "metal".to_string(),
                "opencl".to_string(),
                "sycl".to_string(),
            ],
            // Source documentation is a possible adapter target only. No
            // matching, pinned Nexora executable or build features are known.
            runtime_bundle_available: false,
            runtime_package_version_pinned: false,
            runtime_binary_revision_verified: false,
            runtime_probe_performed: false,
            model_runtime_compatibility_confirmed: false,
            runtime_compatibility_status: "unknown",
            model_specific_resource_fit_confirmed: false,
            hardware_compatibility: "unknown",
            generation_command_available: false,
            nexora_output_pipeline_available: false,
            upstream_video_containers_documented: vec![
                "avi (MJPEG)".to_string(),
                "webm (requires the documented WebM build features)".to_string(),
                "animated webp (requires the documented WebP build feature)".to_string(),
            ],
            selected_diffusion_weights: "FP16 safetensors".to_string(),
            selected_text_encoder: "UMT5-XXL encoder GGUF Q8_0".to_string(),
            resource_evidence: "The Wan authors publish 8.19 GB VRAM and about four minutes for a five-second 480p RTX 4090 reference run without quantization. This is not a minimum, a local measurement, or a guarantee for the selected mixed-precision assets and stable-diffusion.cpp build. No model/runtime-specific CPU RAM minimum or peak working-set figure is established.".to_string(),
            limitations: vec![
                format!("The model candidate is registered for Phase 2 download, checksum validation, and lifecycle management only. Its three files total 9,135,187,134 bytes (about 8.51 GiB); presence or successful checksum validation is not proof of runtime compatibility or usability. See {WAN21_MODEL_CARD}."),
                "The registered candidate is text-to-video only. No image input, image editing, video editing, enhancement, or image-generation fallback is part of this adapter.".to_string(),
                "The upstream stable-diffusion.cpp source documents Wan2.1 T2V invocation with -M vid_gen and the diffusion, VAE, and T5 assets, and its README lists Windows plus CPU/CUDA/Vulkan backends. That source is not a pinned or bundled Nexora executable; platform and backend support in a particular binary remain unverified.".to_string(),
                "Upstream video output is AVI/MJPEG, WebM, or animated WebP; WebM/WebP depend on optional build features. MP4 output is not established by the reviewed CLI contract. Nexora has no encoder integration, unique managed video artifact store, or structural container validation yet.".to_string(),
                "No model-specific RAM/VRAM minimum is entered into adaptive readiness. The publisher's 8.19 GB VRAM reference is not treated as a guaranteed minimum; CPU practicality and complete inference working-set requirements remain unknown.".to_string(),
                "No runtime probing, hardware check, model loading, inference, output write, progress reporting, cancellation, duration/FPS/resolution/seed control, or generation-success IPC is implemented.".to_string(),
                "Before a genuine generation command is exposed, Nexora needs a pinned/verified compatible runtime build, supported invocation and cancellation contract, safe private output destination, and validation of the actual encoded video container before reporting success.".to_string(),
                format!("The reviewed stable-diffusion.cpp documentation revision is {SD_CPP_REVISION}; it is a source/documentation reference, not a Nexora runtime binary pin."),
            ],
        }
    }
}

fn lifecycle_state_name(state: LifecycleState) -> &'static str {
    match state {
        LifecycleState::NotInstalled => "not_installed",
        LifecycleState::AwaitingCompatibilityAssessment => "awaiting_compatibility_assessment",
        LifecycleState::ReadyToDownload => "ready_to_download",
        LifecycleState::Downloading => "downloading",
        LifecycleState::DownloadInterrupted => "download_interrupted",
        LifecycleState::DownloadedNotValidated => "downloaded_not_validated",
        LifecycleState::InstalledValidated => "installed_validated",
        LifecycleState::Incompatible => "incompatible",
        LifecycleState::Failed => "failed",
        LifecycleState::Removing => "removing",
    }
}
