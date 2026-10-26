//! Phase 5 registered a dedicated Wan2.1 T2V model; Phase 9 extends its
//! read-only status with media-manager lifecycle state. No video inference,
//! runtime resolution, output writing, or playback claim is implemented.
//!
//! This module remains separate from coding-model inference, image generation,
//! and image editing.

pub mod commands;
mod adapter;

pub use adapter::VideoGenerationCapabilityStatus;

pub(crate) fn status(
    lifecycle: Option<crate::media_models::MediaModelLifecycleSnapshot>,
) -> VideoGenerationCapabilityStatus {
    adapter::Wan21T2v13b::status(lifecycle)
}
