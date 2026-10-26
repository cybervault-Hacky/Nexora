//! Model-specific image-editing capability metadata.
//!
//! InstructPix2Pix is a documented instruction-conditioned editor, but Nexora
//! has no pinned compatible runtime bundle. This module is metadata only: no
//! executor, request dispatch, output artifact, or success-shaped path exists.

#[derive(Debug, Clone)]
pub(crate) struct ImageEditingAdapterDescriptor {
    pub model_id: &'static str,
    pub model_name: &'static str,
    pub model_card: &'static str,
    pub weights_revision: &'static str,
    pub weights_file: &'static str,
    pub weights_format: &'static str,
    pub weights_bytes: u64,
    pub weights_sha256: &'static str,
    pub asset_manifest_complete: bool,
    pub asset_manifest_note: &'static str,
    pub runtime_name: &'static str,
    pub runtime_reference: &'static str,
    /// Pins reviewed upstream source code only, not a supported runtime package.
    pub runtime_source_revision: &'static str,
    pub license_reference: &'static str,
    pub license_note: &'static str,
    pub resource_evidence: &'static str,
    pub accepts_source_image: bool,
    pub accepts_natural_language_instruction: bool,
    pub supports_mask_input: bool,
    pub supports_region_selection: bool,
}

/// Model-level metadata that a future real runtime adapter can implement
/// independently of text-to-image generation. It does not provide inference.
pub(crate) trait ImageEditingAdapter {
    fn descriptor(&self) -> ImageEditingAdapterDescriptor;
}

pub(crate) struct InstructPix2PixBoundary;

impl ImageEditingAdapter for InstructPix2PixBoundary {
    fn descriptor(&self) -> ImageEditingAdapterDescriptor {
        ImageEditingAdapterDescriptor {
            model_id: "timbrooks/instruct-pix2pix",
            model_name: "InstructPix2Pix",
            model_card: "https://huggingface.co/timbrooks/instruct-pix2pix",
            // Immutable commit containing the publisher's SafeTensors LFS
            // pointer; not a claim that the file is installed or executable.
            weights_revision: "c0d64774d2012658405318248fb7fff79db71158",
            weights_file: "instruct-pix2pix-00-22000.safetensors",
            weights_format: "safetensors",
            weights_bytes: 7_703_416_451,
            weights_sha256: "fbc31a67aab29ad38eef622a252d3fa4b0a81d4a4dc65da416cf45b13aa67688",
            asset_manifest_complete: false,
            asset_manifest_note: "The pinned checkpoint is known, but a complete immutable, checksummed manifest for every Diffusers pipeline component/configuration and any safety assets has not been established for Nexora.",
            runtime_name: "Hugging Face Diffusers StableDiffusionInstructPix2PixPipeline (upstream source reference only)",
            runtime_reference: "https://github.com/huggingface/diffusers/blob/c60830ee365d520ab52b110dda562dd26f7b4d7f/src/diffusers/pipelines/stable_diffusion/pipeline_stable_diffusion_instruct_pix2pix.py",
            runtime_source_revision: "c60830ee365d520ab52b110dda562dd26f7b4d7f",
            license_reference: "https://github.com/timothybrooks/instruct-pix2pix/blob/658ad5e4d91546ee1f800a79ba63a3bd76fd93c7/stable_diffusion/LICENSE",
            license_note: "The model card labels the model MIT; the authors warn that the Stable Diffusion-derived checkpoint carries CreativeML Open RAIL-M use-based restrictions. Review both the model card and inherited/component terms before distribution.",
            resource_evidence: "The authors report a quickstart tested on a GPU with more than 18 GB VRAM. This is one tested configuration, not a minimum or a guarantee. No dependable minimum RAM/VRAM, CPU performance, or Nexora Windows workload requirement is established.",
            accepts_source_image: true,
            accepts_natural_language_instruction: true,
            // The reviewed InstructPix2Pix public call takes an instruction and
            // source image; it does not expose a mask/region input contract.
            supports_mask_input: false,
            supports_region_selection: false,
        }
    }
}
