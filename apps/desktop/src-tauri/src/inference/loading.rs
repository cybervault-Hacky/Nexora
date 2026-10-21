//! Profile-aware decisions about *what* may be loaded.
//!
//! The supervisor used to trust `settings.model_path` and only work out
//! afterwards which profile, if any, the file belonged to. That allowed a
//! selection and a file to disagree until the first generation was refused.
//! This module decides before a process exists whether the chosen file is
//! allowed for the selected profile, and returns the attribution the rest of
//! the app should record.
//!
//! Rules:
//!
//! * A published artefact may only be loaded for the profile it belongs to.
//!   Choosing Nexora Sonnet while pointing at Nexora Mini's file is refused,
//!   not silently loaded and refused later.
//! * A published artefact is only loaded after its digest was confirmed in this
//!   session. The ledger is path-keyed, so a copy elsewhere does not count.
//! * Nexora Mini — the default — additionally accepts a file that is not a
//!   published artefact, because connecting a GGUF the user already has is a
//!   supported workflow. Any other profile does not.
//! * The context window is validated against the app's supported range and
//!   against the maximum the GGUF itself declares. It is never reduced to make
//!   a load succeed.
//!
//! No filesystem access happens here: the caller has already inspected the
//! file, and everything below is a decision over plain values.

use std::path::PathBuf;

use super::error::{InferenceError, InferenceResult};
use super::registry::{self, ArtifactState, ModelProfile, DEFAULT_MODEL_ID};
use super::settings::{MAX_CONTEXT_SIZE, MIN_CONTEXT_SIZE};

/// What the caller should load, and who it should be attributed to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadPlan {
    pub path: PathBuf,
    /// The registered profile this file belongs to, by published file name.
    pub profile_id: Option<&'static str>,
    /// True when the file is not a published artefact (Nexora Mini only).
    pub custom: bool,
}

impl LoadPlan {
    /// The profile whose readiness gates should be applied, when known.
    pub fn gates_for(&self) -> Option<&'static ModelProfile> {
        self.profile_id.and_then(|id| registry::APPROVED.by_id(id))
    }
}

/// The file name component of a user-supplied path.
fn file_name_of(path: &str) -> String {
    PathBuf::from(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_string()
}

/// Decides whether `model_path` may be loaded for `selected`.
///
/// `declared_max_context` is the maximum the GGUF itself reports, when the
/// caller was able to read it. `verified` is the session digest result for the
/// registered owner of this path, if any.
pub fn plan(
    selected: &ModelProfile,
    model_path: &str,
    context_size: u32,
    declared_max_context: Option<u32>,
    verified: bool,
) -> InferenceResult<LoadPlan> {
    let path = model_path.trim();

    if path.is_empty() {
        return Err(InferenceError::not_configured(
            "Choose a GGUF model file before loading the engine.",
        ));
    }

    // Range first: a bad number is not the file's fault and should not be
    // reported as one.
    if !(MIN_CONTEXT_SIZE..=MAX_CONTEXT_SIZE).contains(&context_size) {
        return Err(InferenceError::invalid_request(format!(
            "A context window of {context_size} tokens is outside the supported range of {MIN_CONTEXT_SIZE} to {MAX_CONTEXT_SIZE}. Nothing was started."
        )));
    }

    if let Some(maximum) = declared_max_context {
        if context_size > maximum {
            return Err(InferenceError::invalid_model(format!(
                "This model file reports a maximum context of {maximum} tokens, but Nexora is set to {context_size}. Lower the context window before loading."
            )));
        }
    }

    if !matches!(selected.artifact, ArtifactState::Approved(_)) {
        return Err(InferenceError::new(
            "artifact_pending",
            format!(
                "{} has no approved artefact, so Nexora will not load it.",
                selected.display_name
            ),
        ));
    }

    let file_name = file_name_of(path);

    match registry::APPROVED.by_file_name(&file_name) {
        Some(owner) if owner.id != selected.id => Err(InferenceError::new(
            "model_mismatch",
            format!(
                "The file is {file_name}, which is {}. {} is selected, so Nexora will not load a different model's file. Select {} or choose its own file.",
                owner.display_name, selected.display_name, owner.display_name
            ),
        )),
        Some(owner) => {
            if !verified {
                return Err(InferenceError::new(
                    "model_unverified",
                    format!(
                        "{} has not been verified in this session. Verify the file once, then load it.",
                        owner.display_name
                    ),
                ));
            }

            Ok(LoadPlan {
                path: PathBuf::from(path),
                profile_id: Some(owner.id),
                custom: false,
            })
        }
        // A file that no profile publishes. This is the documented way to use
        // your own GGUF, and it is deliberately limited to the default profile
        // so a second profile cannot quietly run something unverified.
        None if selected.id == DEFAULT_MODEL_ID => Ok(LoadPlan {
            path: PathBuf::from(path),
            profile_id: None,
            custom: true,
        }),
        None => Err(InferenceError::new(
            "model_mismatch",
            format!(
                "{} can only load its published artefact ({}). This file is {file_name}. Nothing was started.",
                selected.display_name,
                selected
                    .artifact
                    .source()
                    .map(|source| source.file_name)
                    .unwrap_or("the approved GGUF")
            ),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inference::registry::{NEXORA_MINI, NEXORA_SONNET};

    const MINI_FILE: &str = "qwen2.5-coder-7b-instruct-q4_k_m.gguf";
    const SONNET_FILE: &str = "qwen2.5-coder-14b-instruct-q4_k_m.gguf";

    fn mini_path() -> String {
        format!("/models/{MINI_FILE}")
    }

    fn sonnet_path() -> String {
        format!("/models/{SONNET_FILE}")
    }

    #[test]
    fn the_selected_profile_loads_its_own_verified_file() {
        let plan = plan(&NEXORA_SONNET, &sonnet_path(), 8_192, Some(32_768), true).unwrap();

        assert_eq!(plan.profile_id, Some(NEXORA_SONNET.id));
        assert!(!plan.custom);
        assert_eq!(plan.path, PathBuf::from(sonnet_path()));
    }

    #[test]
    fn mini_keeps_working_with_its_own_file() {
        let plan = plan(&NEXORA_MINI, &mini_path(), 32_768, Some(32_768), true).unwrap();

        assert_eq!(plan.profile_id, Some(NEXORA_MINI.id));
        assert!(!plan.custom);
    }

    #[test]
    fn mini_still_accepts_a_file_no_profile_publishes() {
        // The documented "connect a GGUF you already have" workflow.
        let plan = plan(&NEXORA_MINI, "/models/my-model.gguf", 8_192, None, false).unwrap();

        assert_eq!(plan.profile_id, None);
        assert!(plan.custom);
    }

    #[test]
    fn sonnet_refuses_a_custom_file() {
        let error = plan(&NEXORA_SONNET, "/models/my-model.gguf", 8_192, None, false).unwrap_err();

        assert_eq!(error.code, "model_mismatch");
        assert!(error.message.contains("Nexora Sonnet"));
        assert!(error.message.contains(SONNET_FILE));
    }

    #[test]
    fn a_selection_cannot_load_the_other_profiles_file() {
        let error = plan(&NEXORA_SONNET, &mini_path(), 8_192, None, true).unwrap_err();

        assert_eq!(error.code, "model_mismatch");
        assert!(error.message.contains("Nexora Mini"));

        let reverse = plan(&NEXORA_MINI, &sonnet_path(), 8_192, None, true).unwrap_err();

        assert_eq!(reverse.code, "model_mismatch");
        assert!(reverse.message.contains("Nexora Sonnet"));
    }

    #[test]
    fn an_unverified_published_artefact_is_refused() {
        let error = plan(&NEXORA_SONNET, &sonnet_path(), 8_192, Some(32_768), false).unwrap_err();

        assert_eq!(error.code, "model_unverified");
        assert!(error.message.contains("Verify"));
    }

    #[test]
    fn a_custom_file_is_never_reported_as_verified() {
        // A custom file cannot be verified against a profile, so the ledger
        // must not be consulted for it; `verified: true` is irrelevant.
        let error = plan(&NEXORA_SONNET, "/models/custom.gguf", 8_192, None, true).unwrap_err();

        assert_eq!(error.code, "model_mismatch");
    }

    #[test]
    fn a_context_below_the_supported_minimum_is_refused() {
        let error = plan(&NEXORA_MINI, &mini_path(), 512, Some(32_768), true).unwrap_err();

        assert_eq!(error.code, "invalid_request");
        assert!(error
            .message
            .contains(MIN_CONTEXT_SIZE.to_string().as_str()));
    }

    #[test]
    fn a_context_above_the_supported_maximum_is_refused() {
        let error = plan(&NEXORA_MINI, &mini_path(), MAX_CONTEXT_SIZE + 1, None, true).unwrap_err();

        assert_eq!(error.code, "invalid_request");
    }

    #[test]
    fn a_context_above_the_file_declared_maximum_is_refused_without_changing_it() {
        let error = plan(&NEXORA_MINI, &mini_path(), 16_384, Some(8_192), true).unwrap_err();

        assert_eq!(error.code, "invalid_model");
        assert!(error.message.contains("8192"));
        assert!(error.message.contains("16384"));
    }

    #[test]
    fn an_empty_path_is_not_configured_rather_than_a_model_error() {
        assert_eq!(
            plan(&NEXORA_MINI, "   ", 8_192, None, false)
                .unwrap_err()
                .code,
            "not_configured"
        );
    }

    #[test]
    fn a_profile_without_an_approved_artefact_cannot_be_loaded() {
        let mut pending = NEXORA_SONNET;
        pending.artifact = ArtifactState::Pending {
            note: "No artefact recorded.",
        };

        let error = plan(&pending, &sonnet_path(), 8_192, None, true).unwrap_err();

        assert_eq!(error.code, "artifact_pending");
    }

    #[test]
    fn the_plan_points_at_the_profile_a_path_belongs_to() {
        let plan = plan(&NEXORA_SONNET, &sonnet_path(), 8_192, None, true).unwrap();

        assert_eq!(
            plan.gates_for().map(|profile| profile.id),
            Some(NEXORA_SONNET.id)
        );
        assert_eq!(
            plan.gates_for()
                .map(|profile| profile.artifact.source().unwrap().size_bytes),
            Some(8_988_110_272)
        );
    }
}
