//! Status-only boundary for the Sonnet advanced image-generation candidate.
//!
//! FLUX.1 [schnell] is separate from both coding profiles and Mini's SD-Turbo
//! path. Upstream sources document a plausible runtime pairing, but a trusted
//! weight manifest and Nexora-compatible runtime are not established. There is
//! deliberately no generation, loading, cancellation, or artifact command.

pub mod adapter;
pub mod commands;

pub use adapter::{FluxSchnellCandidate, SonnetImageGenerationCapabilityStatus};
