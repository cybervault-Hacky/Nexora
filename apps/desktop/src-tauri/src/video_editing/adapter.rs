use serde::Serialize;

/// Read-only capability boundary for conventional video processing and
/// model-based video enhancement. Neither capability has an integrated local
/// implementation in Nexora; no request or output path is exposed.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoEditingCapabilityStatus {
    pub conventional_processing_available: bool,
    pub supported_operation_count: u8,
    pub source_video_import_available: bool,
    pub input_format_contract_established: bool,
    pub external_media_runtime_configured: bool,
    pub runtime_version_pinned: bool,
    pub runtime_probe_performed: bool,
    pub runtime_compatibility_status: &'static str,
    pub windows_compatibility: &'static str,
    pub supported_input_formats: Vec<String>,
    pub supported_output_formats: Vec<String>,
    pub output_artifact_store_available: bool,
    pub output_validation_available: bool,
    pub processing_progress_available: bool,
    pub cancellation_available: bool,
    pub ai_enhancement_candidate_model_id: Option<String>,
    pub ai_enhancement_model_registered: bool,
    pub ai_enhancement_runtime_available: bool,
    pub ai_enhancement_usable: bool,
    pub hardware_compatibility: &'static str,
    pub resource_evidence: String,
    pub limitations: Vec<String>,
}

pub(crate) struct VideoEditingBoundary;

impl VideoEditingBoundary {
    pub fn status() -> VideoEditingCapabilityStatus {
        VideoEditingCapabilityStatus {
            conventional_processing_available: false,
            supported_operation_count: 0,
            source_video_import_available: false,
            input_format_contract_established: false,
            external_media_runtime_configured: false,
            runtime_version_pinned: false,
            runtime_probe_performed: false,
            runtime_compatibility_status: "unknown",
            windows_compatibility: "unknown",
            supported_input_formats: Vec::new(),
            supported_output_formats: Vec::new(),
            output_artifact_store_available: false,
            output_validation_available: false,
            processing_progress_available: false,
            cancellation_available: false,
            // No video-enhancement candidate is established. Wan2.1 remains a
            // text-to-video model; image models are not treated as video models.
            ai_enhancement_candidate_model_id: None,
            ai_enhancement_model_registered: false,
            ai_enhancement_runtime_available: false,
            ai_enhancement_usable: false,
            hardware_compatibility: "unknown",
            resource_evidence: "No media-processing runtime, video input contract, or video-enhancement model/runtime is integrated. No workload or hardware requirements can be established from this application boundary.",
            limitations: vec![
                "No version-pinned video-processing library or external media executable is declared or configured in the desktop dependencies/resources; no system executable is searched for or assumed to exist.".to_string(),
                "The application has no secure source-video import/store or video attachment flow. The Phase 4 PNG-only image-source store is not a video input boundary.".to_string(),
                "No supported source container/codec, input-size/duration/dimension limit, trim/frame/resize/transcode operation, or conversion setting is established.".to_string(),
                "No managed video output-artifact store, staging/publication contract, media-structure validation, or playable-output metadata exists.".to_string(),
                "Wan2.1 T2V is registered only for text-to-video. FLUX.1 [schnell] is a text-to-image candidate, and InstructPix2Pix is an image editor; none is represented as video editing or enhancement.".to_string(),
                "No dedicated temporal video-enhancement model, complete asset manifest, supported inference runtime, Windows/backend evidence, or trustworthy resource requirements have been established.".to_string(),
                "This status performs no file, runtime, or hardware probe. It exposes no processing, progress, cancellation, source-reference, or output-artifact command and uses no cloud fallback.".to_string(),
            ],
        }
    }
}
