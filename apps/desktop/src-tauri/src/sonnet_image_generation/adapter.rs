use serde::Serialize;

/// Dedicated media-model identity. This is intentionally not a coding-model ID.
pub const FLUX_SCHNELL_MEDIA_MODEL_ID: &str = "black-forest-labs-flux-1-schnell";
const FLUX_MODEL_CARD_REVISION: &str = "206c4d344174a265f8b4fc85d15bdcdf757df6f9";
const SD_CPP_SOURCE_REVISION: &str = "0321ce17fbf5eeca6f31b28b20577b789642d769";
const OFFICIAL_MODEL_CARD: &str = "https://github.com/black-forest-labs/flux/blob/206c4d344174a265f8b4fc85d15bdcdf757df6f9/model_cards/FLUX.1-schnell.md";
const OFFICIAL_MODEL_REPO: &str = "https://github.com/black-forest-labs/flux/tree/206c4d344174a265f8b4fc85d15bdcdf757df6f9";
const OFFICIAL_WEIGHT_LICENSE: &str = "https://github.com/black-forest-labs/flux/blob/206c4d344174a265f8b4fc85d15bdcdf757df6f9/model_licenses/LICENSE-FLUX1-schnell";
const RUNTIME_GUIDE: &str = "https://github.com/leejet/stable-diffusion.cpp/blob/0321ce17fbf5eeca6f31b28b20577b789642d769/docs/flux.md";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SonnetImageGenerationCapabilityStatus {
    pub candidate_model_id: String,
    pub candidate_model_name: String,
    pub parameter_count: u64,
    pub official_model_card: String,
    /// Pins the reviewed model metadata, not the model weights themselves.
    pub model_card_source_revision: String,
    pub official_model_repository: String,
    pub official_repository_reported_md5: String,
    pub model_weights_revision: Option<String>,
    pub model_weights_size_bytes: Option<u64>,
    pub model_weights_sha256: Option<String>,
    pub registered_with_media_model_manager: bool,
    pub required_component_count: u8,
    pub required_asset_manifest_complete: bool,
    pub downloadable: bool,
    pub license_identifier: String,
    pub license_reference: String,
    pub publisher_use_limitations: Vec<String>,
    pub runtime_name: String,
    pub runtime_source_revision: String,
    pub runtime_source_guide: String,
    /// Upstream source documents FLUX.1-schnell support; this is not a Nexora binary claim.
    pub upstream_runtime_support_documented: bool,
    pub upstream_windows_support_documented: bool,
    pub upstream_backends_documented: Vec<String>,
    /// No candidate-compatible executable is declared/bundled for Nexora.
    pub runtime_bundle_configured: bool,
    /// This status boundary deliberately performs no filesystem/runtime probe.
    pub runtime_probe_performed: bool,
    pub runtime_binary_revision_verified: bool,
    pub model_runtime_compatibility_confirmed: bool,
    pub hardware_compatibility: String,
    pub minimum_ram_bytes: Option<u64>,
    pub minimum_vram_bytes: Option<u64>,
    pub resource_evidence: String,
    pub generation_command_available: bool,
    pub output_validation_pipeline_available: bool,
    pub inference_usable: bool,
    pub limitations: Vec<String>,
}

/// A source-evidence boundary only. No executor, loader, downloader, runtime
/// resolver, artifact writer, or generation-success path is implemented here.
pub struct FluxSchnellCandidate;

impl FluxSchnellCandidate {
    pub fn status() -> SonnetImageGenerationCapabilityStatus {
        SonnetImageGenerationCapabilityStatus {
            candidate_model_id: FLUX_SCHNELL_MEDIA_MODEL_ID.to_string(),
            candidate_model_name: "FLUX.1 [schnell]".to_string(),
            parameter_count: 12_000_000_000,
            official_model_card: OFFICIAL_MODEL_CARD.to_string(),
            model_card_source_revision: FLUX_MODEL_CARD_REVISION.to_string(),
            official_model_repository: OFFICIAL_MODEL_REPO.to_string(),
            // BFL publishes this MD5 in its pinned README. It is not a SHA-256
            // and cannot satisfy Nexora's trusted media-model installation contract.
            official_repository_reported_md5: "a9e1e277b9b16add186f38e3f5a34044".to_string(),
            model_weights_revision: None,
            model_weights_size_bytes: None,
            model_weights_sha256: None,
            registered_with_media_model_manager: false,
            // The pinned upstream CLI recipe lists the diffusion weights, VAE,
            // CLIP-L, and T5-XXL components; no pinned, hashed manifest is registered.
            required_component_count: 4,
            required_asset_manifest_complete: false,
            downloadable: false,
            license_identifier: "Apache-2.0".to_string(),
            license_reference: OFFICIAL_WEIGHT_LICENSE.to_string(),
            publisher_use_limitations: vec![
                "The publisher's model card lists out-of-scope uses; review it separately from Apache-2.0 license obligations.".to_string(),
                "Preserve the applicable Apache-2.0 license and copyright notices if model artifacts are ever redistributed.".to_string(),
            ],
            runtime_name: "stable-diffusion.cpp sd-cli (FLUX.1-schnell source-documented path)".to_string(),
            runtime_source_revision: SD_CPP_SOURCE_REVISION.to_string(),
            runtime_source_guide: RUNTIME_GUIDE.to_string(),
            upstream_runtime_support_documented: true,
            upstream_windows_support_documented: true,
            upstream_backends_documented: vec![
                "cpu".to_string(),
                "cuda".to_string(),
                "vulkan".to_string(),
                "metal".to_string(),
                "opencl".to_string(),
                "sycl".to_string(),
            ],
            runtime_bundle_configured: false,
            runtime_probe_performed: false,
            runtime_binary_revision_verified: false,
            model_runtime_compatibility_confirmed: false,
            hardware_compatibility: "unknown".to_string(),
            minimum_ram_bytes: None,
            minimum_vram_bytes: None,
            resource_evidence: "The pinned stable-diffusion.cpp FLUX guide says some Flux configurations can run with 4–6 GB VRAM, but it does not establish a Nexora minimum for this exact model/assets/build. No dependable CPU RAM minimum or exact asset-storage total is established here; all model-specific fit values remain unknown.".to_string(),
            generation_command_available: false,
            output_validation_pipeline_available: false,
            inference_usable: false,
            limitations: vec![
                "The BFL model-card commit pins reviewed metadata, not the Hugging Face weight revision. The official repository reports an MD5, but Nexora's manager requires a trusted SHA-256 and exact asset metadata.".to_string(),
                "The runtime guide lists four required components (FLUX weights, VAE, CLIP-L, and T5-XXL), but a complete immutable, checksummed manifest with sizes and licenses for every selected artifact has not been established.".to_string(),
                "Upstream stable-diffusion.cpp documents FLUX.1-schnell and Windows usage, but Nexora has no pinned, checksummed, model-compatible runtime binary or validated build configuration.".to_string(),
                "No Sonnet-specific model registration, download, validation, loading, generation command, progress/cancellation path, or advanced-image artifact pipeline exists.".to_string(),
                "The existing SD-Turbo runtime is an independent Mini path. Its presence or adapter does not establish FLUX support or make Mini generation usable.".to_string(),
            ],
        }
    }
}
