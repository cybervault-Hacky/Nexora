//! Phase 10's separate, status-only video editing/enhancement boundary.
//!
//! Conventional media processing is not available without an established
//! decoder/encoder implementation and safe source/output contracts. AI-based
//! temporal enhancement is a separate unavailable capability. No video edit,
//! conversion, enhancement, progress, or cancellation command is implemented.

pub mod commands;
mod adapter;

pub use adapter::VideoEditingCapabilityStatus;

pub(crate) fn status() -> VideoEditingCapabilityStatus {
    adapter::VideoEditingBoundary::status()
}
