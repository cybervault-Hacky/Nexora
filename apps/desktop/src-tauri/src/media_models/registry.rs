//! Static metadata registry for dedicated media models. No coding-model profile
//! is imported here. Definitions are added only with authoritative provenance.

use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModelCategory {
    ImageGeneration,
    ImageEditing,
    VideoGeneration,
    VideoEditingEnhancement,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FileRole {
    Weights,
    TextEncoder,
    Vae,
    Configuration,
    Tokenizer,
    Auxiliary,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BackendRequirement {
    Cpu,
    Cuda,
    DirectMl,
    Vulkan,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelFileDefinition {
    pub id: &'static str,
    pub file_name: &'static str,
    pub role: FileRole,
    pub required: bool,
    /// Missing means the publisher's exact file size has not been validated.
    pub expected_size_bytes: Option<u64>,
    /// SHA-256 is included only when obtained from a trusted, pinned source.
    pub expected_sha256: Option<&'static str>,
    /// A pinned HTTPS URL from the registered source, never caller supplied.
    pub download_url: Option<&'static str>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrustedSource {
    pub official_reference: &'static str,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LicenseInformation {
    pub identifier: Option<&'static str>,
    pub reference: Option<&'static str>,
    pub restrictions: Option<&'static str>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceRequirements {
    pub minimum_ram_bytes: Option<u64>,
    pub recommended_ram_bytes: Option<u64>,
    pub minimum_vram_bytes: Option<u64>,
    pub recommended_vram_bytes: Option<u64>,
    pub required_backend: Option<BackendRequirement>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InferenceRuntimeDefinition {
    pub name: &'static str,
    pub official_reference: &'static str,
    pub supported_backends: &'static [BackendRequirement],
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DefinitionCompatibility {
    AwaitingAssessment,
    ExperimentOnly,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaModelDefinition {
    pub id: &'static str,
    pub display_name: &'static str,
    pub category: ModelCategory,
    pub family: Option<&'static str>,
    pub version: Option<&'static str>,
    pub source: Option<TrustedSource>,
    pub license: LicenseInformation,
    pub files: &'static [ModelFileDefinition],
    pub expected_total_size_bytes: Option<u64>,
    pub requirements: ResourceRequirements,
    pub inference_runtime: Option<InferenceRuntimeDefinition>,
    pub compatibility: DefinitionCompatibility,
    pub downloadable: bool,
    pub experimental: bool,
}

const SD_TURBO_FILES: &[ModelFileDefinition] = &[ModelFileDefinition {
    id: "sd-turbo-checkpoint",
    file_name: "sd_turbo.safetensors",
    role: FileRole::Weights,
    required: true,
    expected_size_bytes: Some(5_214_561_328),
    expected_sha256: Some("3f067a1b943cf162f2b8f8588f6cf5824bd5b4c7d1d88d87164b9ca123616549"),
    download_url: Some("https://huggingface.co/stabilityai/sd-turbo/resolve/9fee4496504b3b0e95e11c80bdf0fb0c7a5d4aa8/sd_turbo.safetensors?download=true"),
}];

/// Stable-diffusion.cpp's documented Wan2.1 T2V recipe consumes these three
/// separate artifacts. Every URL is pinned to the commit that introduced or
/// verified that exact file; the digests and byte counts are Hub Xet pointers.
const WAN21_T2V_13B_FILES: &[ModelFileDefinition] = &[
    ModelFileDefinition {
        id: "wan21-t2v-13b-diffusion",
        file_name: "wan2.1_t2v_1.3B_fp16.safetensors",
        role: FileRole::Weights,
        required: true,
        expected_size_bytes: Some(2_838_303_560),
        expected_sha256: Some("be531024cd9018cb5b48c40cfbb6a6191645b1c792eb8bf4f8c1c6e10f924dc5"),
        download_url: Some("https://huggingface.co/Comfy-Org/Wan_2.1_ComfyUI_repackaged/resolve/5601df2b72d74886b9df858fcd79c809cce4c4f3/split_files/diffusion_models/wan2.1_t2v_1.3B_fp16.safetensors"),
    },
    ModelFileDefinition {
        id: "wan21-t2v-13b-vae",
        file_name: "wan_2.1_vae.safetensors",
        role: FileRole::Vae,
        required: true,
        expected_size_bytes: Some(253_815_318),
        expected_sha256: Some("2fc39d31359a4b0a64f55876d8ff7fa8d780956ae2cb13463b0223e15148976b"),
        download_url: Some("https://huggingface.co/Comfy-Org/Wan_2.1_ComfyUI_repackaged/resolve/dfcea77bcf258496e20c69cd84e8e8e41909bb3b/split_files/vae/wan_2.1_vae.safetensors"),
    },
    ModelFileDefinition {
        id: "wan21-t2v-13b-umt5-q8",
        file_name: "umt5-xxl-encoder-Q8_0.gguf",
        role: FileRole::TextEncoder,
        required: true,
        expected_size_bytes: Some(6_043_068_256),
        expected_sha256: Some("2521d4de0bf9e1cc6549866463ceae85e4ec3239bc6063f7488810be39033bbc"),
        download_url: Some("https://huggingface.co/city96/umt5-xxl-encoder-gguf/resolve/81c2b6ffa438b6101e494a5985a4a9c1b731cfc0/umt5-xxl-encoder-Q8_0.gguf"),
    },
];

const CPU_BACKEND: &[BackendRequirement] = &[BackendRequirement::Cpu];
const WAN21_UPSTREAM_BACKENDS: &[BackendRequirement] = &[
    BackendRequirement::Cpu,
    BackendRequirement::Cuda,
    BackendRequirement::Vulkan,
];

/// SD-Turbo and Wan2.1 T2V are separate media tasks; neither is a coding LLM.
/// Source values (revision, byte count, digest, and license reference) are
/// pinned to publisher or explicitly identified conversion-source metadata.
pub static MEDIA_MODELS: &[MediaModelDefinition] = &[MediaModelDefinition {
    id: "stability-ai-sd-turbo-v1",
    display_name: "SD-Turbo",
    category: ModelCategory::ImageGeneration,
    family: Some("Stable Diffusion 2.1 distilled (Adversarial Diffusion Distillation)"),
    version: Some("1.0; weights commit 9fee4496504b3b0e95e11c80bdf0fb0c7a5d4aa8"),
    source: Some(TrustedSource {
        official_reference: "https://huggingface.co/stabilityai/sd-turbo",
    }),
    license: LicenseInformation {
        identifier: Some("Stability AI Community License (2024-07-05)"),
        reference: Some("https://huggingface.co/stabilityai/sd-turbo/blob/6320d118440daa531996d634045c9f0643b44822/LICENSE.md"),
        restrictions: Some("Use must follow the Stability AI Acceptable Use Policy. Commercial-purpose use requires registration with Stability AI; the license requires an enterprise license above USD 1,000,000 annual revenue. Redistribution and attribution obligations apply when distributing the model or a product using it."),
    },
    files: SD_TURBO_FILES,
    expected_total_size_bytes: Some(5_214_561_328),
    // The model/runtime publishers do not state a trustworthy RAM or VRAM
    // minimum for this exact CPU execution path. Do not fabricate one.
    requirements: ResourceRequirements {
        minimum_ram_bytes: None,
        recommended_ram_bytes: None,
        minimum_vram_bytes: None,
        recommended_vram_bytes: None,
        required_backend: Some(BackendRequirement::Cpu),
    },
    inference_runtime: Some(InferenceRuntimeDefinition {
        name: "stable-diffusion.cpp sd-cli",
        official_reference: "https://github.com/leejet/stable-diffusion.cpp",
        supported_backends: CPU_BACKEND,
    }),
    compatibility: DefinitionCompatibility::AwaitingAssessment,
    downloadable: true,
    experimental: true,
}, MediaModelDefinition {
    id: "wan-ai-wan2_1-t2v-1_3b",
    display_name: "Wan2.1 Text-to-Video 1.3B (FP16)",
    category: ModelCategory::VideoGeneration,
    family: Some("Wan2.1 text-to-video diffusion transformer"),
    version: Some("Wan2.1 T2V 1.3B; FP16 diffusion weights, Wan2.1 VAE, UMT5-XXL encoder Q8_0; each asset pinned to an immutable Hub commit"),
    source: Some(TrustedSource {
        official_reference: "https://huggingface.co/Wan-AI/Wan2.1-T2V-1.3B",
    }),
    license: LicenseInformation {
        identifier: Some("Apache-2.0 (Wan2.1 and Google UMT5; city96 GGUF conversion also declares Apache-2.0)"),
        reference: Some("https://huggingface.co/Wan-AI/Wan2.1-T2V-1.3B"),
        restrictions: Some("Preserve the applicable Apache-2.0 license and notices for Wan2.1, Google UMT5, and the separately sourced city96 GGUF conversion. The selected model card and conversion card declare Apache-2.0; review each upstream license and notice before redistributing artifacts."),
    },
    files: WAN21_T2V_13B_FILES,
    expected_total_size_bytes: Some(9_135_187_134),
    // Published VRAM figures describe selected reference workloads, not a
    // supported minimum for this exact runtime/build. Leave fit thresholds unknown.
    requirements: ResourceRequirements {
        minimum_ram_bytes: None,
        recommended_ram_bytes: None,
        minimum_vram_bytes: None,
        recommended_vram_bytes: None,
        required_backend: None,
    },
    inference_runtime: Some(InferenceRuntimeDefinition {
        name: "stable-diffusion.cpp sd-cli (vid_gen)",
        official_reference: "https://github.com/leejet/stable-diffusion.cpp/blob/40b1788476bbe8fe42fa538ddf7d32408a126b93/docs/wan.md",
        supported_backends: WAN21_UPSTREAM_BACKENDS,
    }),
    compatibility: DefinitionCompatibility::AwaitingAssessment,
    downloadable: true,
    experimental: true,
}];

pub const CATEGORIES: &[ModelCategory] = &[
    ModelCategory::ImageGeneration,
    ModelCategory::ImageEditing,
    ModelCategory::VideoGeneration,
    ModelCategory::VideoEditingEnhancement,
];

/// Exact host allowlist for explicitly registered Hugging Face Hub artifacts.
/// Redirect targets are checked against this same list. Entries are based on
/// Hugging Face's published download-endpoint documentation.
pub const TRUSTED_DOWNLOAD_HOSTS: &[&str] = &[
    "huggingface.co",
    "cdn-lfs.huggingface.co",
    "cas-server.xethub.hf.co",
    "cas-server.xethub-eu.hf.co",
    "transfer.xethub.hf.co",
    "transfer.xethub-eu.hf.co",
    "us.aws.cdn.hf.co",
    "us.gcp.cdn.hf.co",
    "cdn-lfs-us-1.hf.co",
    "cdn-lfs-eu-1.hf.co",
];

pub fn validate_model_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 80
        && id.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

pub fn resolve(id: &str) -> Option<&'static MediaModelDefinition> {
    if !validate_model_id(id) {
        return None;
    }
    MEDIA_MODELS.iter().find(|definition| definition.id == id)
}
