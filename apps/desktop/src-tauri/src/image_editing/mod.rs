//! Phase 4 image-editing boundary, extended in Phase 8 with richer capability
//! evidence. It remains separate from text-to-image generation and coding.
//!
//! InstructPix2Pix has a documented Diffusers image-plus-instruction pipeline,
//! but Nexora has no complete managed model bundle or pinned supported runtime.
//! Only capability reporting and Phase 4 secure PNG source import/retrieval are
//! implemented; no edit request, output artifact, progress, or cancellation path
//! is exposed.

pub mod commands;
mod adapter;
mod error;
mod input;

use std::path::PathBuf;

use serde::Serialize;

use self::adapter::{ImageEditingAdapter, InstructPix2PixBoundary};
use self::input::SourceImageStore;

pub use self::error::ImageEditingError;
pub use self::input::{ImageEditSourceArtifact as SourceArtifact, ImportImageEditSourceRequest as ImportSourceRequest};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageEditingCapabilityStatus {
    pub candidate_model_id: String,
    pub candidate_model_name: String,
    pub candidate_model_card: String,
    pub model_weights_revision: String,
    pub model_weights_file: String,
    pub model_weights_format: String,
    pub model_weights_bytes: u64,
    pub model_weights_sha256: String,
    pub required_asset_manifest_complete: bool,
    pub required_asset_manifest_note: String,
    pub model_registered_with_media_manager: bool,
    /// None means the unregistered candidate has no manager-reported local state.
    pub model_downloaded: Option<bool>,
    pub model_integrity_validated: Option<bool>,
    pub model_loaded: bool,
    pub runtime_name: String,
    pub runtime_reference: String,
    /// Pins an upstream source-code reference, not a supported runtime package.
    pub runtime_source_revision: String,
    pub runtime_package_version_pinned: bool,
    pub license_reference: String,
    pub license_note: String,
    pub runtime_bundle_available: bool,
    pub runtime_probe_performed: bool,
    pub upstream_instruction_editing_supported: bool,
    pub model_runtime_compatibility_confirmed: bool,
    pub runtime_compatibility_status: &'static str,
    pub windows_compatibility: &'static str,
    pub hardware_compatibility: &'static str,
    pub minimum_ram_bytes: Option<u64>,
    pub minimum_vram_bytes: Option<u64>,
    pub resource_evidence: String,
    pub supports_instruction_guided_editing: bool,
    pub supports_mask_input: bool,
    pub supports_region_selection: bool,
    pub source_image_import_available: bool,
    pub edit_command_available: bool,
    pub output_validation_pipeline_available: bool,
    pub usable_for_instruction_editing: bool,
    pub limitations: Vec<String>,
}

pub struct ImageEditingManager {
    sources: SourceImageStore,
}

impl ImageEditingManager {
    pub fn new(root: PathBuf) -> Self {
        Self { sources: SourceImageStore::new(root) }
    }

    pub fn status(&self) -> ImageEditingCapabilityStatus {
        let candidate = InstructPix2PixBoundary;
        let adapter = candidate.descriptor();
        debug_assert!(adapter.accepts_source_image && adapter.accepts_natural_language_instruction);
        ImageEditingCapabilityStatus {
            candidate_model_id: adapter.model_id.to_string(),
            candidate_model_name: adapter.model_name.to_string(),
            candidate_model_card: adapter.model_card.to_string(),
            model_weights_revision: adapter.weights_revision.to_string(),
            model_weights_file: adapter.weights_file.to_string(),
            model_weights_format: adapter.weights_format.to_string(),
            model_weights_bytes: adapter.weights_bytes,
            model_weights_sha256: adapter.weights_sha256.to_string(),
            required_asset_manifest_complete: adapter.asset_manifest_complete,
            required_asset_manifest_note: adapter.asset_manifest_note.to_string(),
            // The candidate is deliberately not registered until its complete
            // installable asset set and runtime contract are supportable.
            model_registered_with_media_manager: false,
            // No manager registration means download/integrity lifecycle data
            // is unavailable; the status command does not inspect the filesystem.
            model_downloaded: None,
            model_integrity_validated: None,
            model_loaded: false,
            runtime_name: adapter.runtime_name.to_string(),
            runtime_reference: adapter.runtime_reference.to_string(),
            runtime_source_revision: adapter.runtime_source_revision.to_string(),
            runtime_package_version_pinned: false,
            license_reference: adapter.license_reference.to_string(),
            license_note: adapter.license_note.to_string(),
            runtime_bundle_available: false,
            runtime_probe_performed: false,
            upstream_instruction_editing_supported: true,
            model_runtime_compatibility_confirmed: false,
            runtime_compatibility_status: "unknown",
            windows_compatibility: "unknown",
            hardware_compatibility: "unknown",
            minimum_ram_bytes: None,
            minimum_vram_bytes: None,
            resource_evidence: adapter.resource_evidence.to_string(),
            supports_instruction_guided_editing: adapter.accepts_source_image
                && adapter.accepts_natural_language_instruction,
            supports_mask_input: adapter.supports_mask_input,
            supports_region_selection: adapter.supports_region_selection,
            source_image_import_available: true,
            edit_command_available: false,
            output_validation_pipeline_available: false,
            usable_for_instruction_editing: false,
            limitations: vec![
                "InstructPix2Pix is a genuine instruction-guided image-editing model; it is distinct from SD-Turbo generation and the FLUX.1-schnell text-to-image candidate.".to_string(),
                "The pinned 7.7 GB SafeTensors checkpoint has a trusted SHA-256 pointer, but the full Diffusers pipeline asset/configuration manifest is not pinned and checksummed for Nexora. The checkpoint alone is not registered or offered for download.".to_string(),
                "Hugging Face Diffusers documents an InstructPix2Pix pipeline accepting a source image and written instruction. The reviewed call does not define a mask or region-selection input; Nexora exposes neither.".to_string(),
                "The upstream Diffusers source reference is pinned, but no Diffusers/PyTorch package version, supported runtime bundle, or Nexora build configuration is selected or integrated.".to_string(),
                "Windows execution, exact backend support, model/runtime compatibility in Nexora, and minimum RAM/VRAM remain unknown. The authors' greater-than-18-GB-VRAM quickstart is a tested setup, not a minimum or guarantee.".to_string(),
                "No model registration, download, integrity validation of a local installation, model load, edit command, progress/cancellation path, validated output artifact, or inference usability is available.".to_string(),
                "The Phase 4 source import accepts only non-animated RGB/RGBA 8-bit PNG bytes up to 8 MiB and 4 megapixels. It accepts no filesystem path and does not provide a chat attachment picker.".to_string(),
            ],
        }
    }

    pub(crate) fn sources_root_for_worker(&self) -> PathBuf {
        self.sources.root().to_path_buf()
    }
}
