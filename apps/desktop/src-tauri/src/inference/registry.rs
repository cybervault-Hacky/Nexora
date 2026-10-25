//! The approved model catalogue for Nexora.
//!
//! Two profiles are registered: Nexora Mini (the default, 7B Q4_K_M) and
//! Nexora Sonnet (14B Q4_K_M), each pinned to an immutable repository revision,
//! byte size, and SHA-256 digest with `locally_verified: false` until verified
//! on real target hardware.
//! A profile describes an artefact, not a file that is present on this machine:
//! nothing in this module reads the filesystem, starts a process, downloads
//! weights, or changes the user's configured model path. A successful
//! resolution means "this metadata is known and internally consistent", never
//! "the model is installed" and never "the model will run".

use serde::Serialize;

use super::error::{InferenceError, InferenceResult};
use super::settings::MAX_CONTEXT_SIZE;

/// Stable identifier for the approved Nexora Mini model profile.
pub const NEXORA_MINI_ID: &str = "nexora-mini-qwen2.5-coder-7b-instruct-q4-k-m";

/// Stable identifier for the Nexora Sonnet model profile.
pub const NEXORA_SONNET_ID: &str = "nexora-sonnet-qwen2.5-coder-14b-instruct-q4-k-m";

/// The profile used when the user has not chosen one.
///
/// Mini stays the default so an existing installation behaves exactly as it did
/// before selection existed.
pub const DEFAULT_MODEL_ID: &str = NEXORA_MINI_ID;

/// Where the approved weights come from.
///
/// The repository revision, byte size, and digest are all published by the
/// host, so the artefact can be identified without downloading it. `sha256`
/// is the digest the host records for the LFS object; it was read from that
/// metadata, not computed here, because no weights were fetched.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelSource {
    pub repository: &'static str,
    pub revision: &'static str,
    pub file_name: &'static str,
    pub size_bytes: u64,
    pub sha256: &'static str,
}

/// Whether a profile's artefact has been approved for download.
///
/// Nexora refuses to fetch or verify a file whose length and digest it cannot
/// check, so a profile without recorded artefact metadata is deliberately not
/// downloadable. It stays selectable and visible so the limitation is shown
/// rather than hidden, but no command will claim it is installed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum ArtifactState {
    Approved(ModelSource),
    /// No approved artefact is recorded.
    ///
    /// Both shipped profiles are approved, so nothing constructs this today;
    /// it stays part of the state machine (and of the serialised contract the
    /// UI switches on) so a profile can never be treated as installable
    /// without a length and digest to check it against.
    #[allow(dead_code)]
    Pending {
        note: &'static str,
    },
}

impl ArtifactState {
    pub fn source(self) -> Option<ModelSource> {
        match self {
            ArtifactState::Approved(source) => Some(source),
            ArtifactState::Pending { .. } => None,
        }
    }

    pub fn is_approved(self) -> bool {
        matches!(self, ArtifactState::Approved(_))
    }
}

/// The licence the publisher attaches to the weights.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LicenceRef {
    pub id: &'static str,
    pub reference: &'static str,
}

/// Context guidance for one artefact.
///
/// `recommended` is the value Nexora should offer for this profile.
/// `documented_max` is the ceiling the publisher claims for this exact file;
/// anything above it is provisional and unsupported. Neither number is a local
/// measurement, and Windows validation is still outstanding, so the profile is
/// not presented as verified compatibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextGuidance {
    pub recommended: u32,
    pub documented_max: u32,
}

/// Metadata for one model artefact.
///
/// This is deliberately a flat description rather than a trait or a plugin
/// point. The application serves one model at a time, and a second profile is
/// another value in the registry — no caller needs to change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelProfile {
    pub id: &'static str,
    pub display_name: &'static str,
    pub publisher: &'static str,
    pub upstream_model: &'static str,
    /// Revision of the published weights this artefact was converted from.
    /// `None` while the upstream revision has not been recorded.
    pub upstream_revision: Option<&'static str>,
    pub format: &'static str,
    pub quantization: &'static str,
    pub licence: LicenceRef,
    pub artifact: ArtifactState,
    pub context: ContextGuidance,
    /// KV-cache bytes per token, from the publisher's `config.json`
    /// (2 x layers x kv_heads x head_dim x 2 bytes for an f16 cache).
    ///
    /// This is published architecture, not a measurement. `None` means the
    /// geometry is unknown and readiness falls back to a blunt allowance.
    pub kv_bytes_per_token: Option<u64>,
    pub compatibility_notes: &'static [&'static str],
    /// True only after a recorded local inference run against this artefact.
    pub locally_verified: bool,
}

/// The approved Nexora Mini profile.
///
/// Values are pinned to the Phase 3 evaluation
/// (`docs/nexora-mini/PHASE_3_MODEL_EVALUATION.md`, section 6.1).
pub const NEXORA_MINI: ModelProfile = ModelProfile {
    id: NEXORA_MINI_ID,
    display_name: "Nexora Mini",
    publisher: "Qwen team, Alibaba Group",
    upstream_model: "Qwen/Qwen2.5-Coder-7B-Instruct",
    upstream_revision: Some("c03e6d358207e414f1eca0bb1891e29f1db0e242"),
    format: "GGUF",
    quantization: "Q4_K_M",
    licence: LicenceRef {
        id: "Apache-2.0",
        reference: "https://huggingface.co/Qwen/Qwen2.5-Coder-7B-Instruct-GGUF/blob/13fb94bfda8c8cf22497dc57b78f391a9acb426a/LICENSE",
    },
    artifact: ArtifactState::Approved(ModelSource {
        repository: "Qwen/Qwen2.5-Coder-7B-Instruct-GGUF",
        revision: "13fb94bfda8c8cf22497dc57b78f391a9acb426a",
        file_name: "qwen2.5-coder-7b-instruct-q4_k_m.gguf",
        size_bytes: 4_683_073_536,
        sha256: "509287f78cb4d4cf6b3843734733b914b2c158e43e22a7f4bf5e963800894d3c",
    }),
    context: ContextGuidance {
        recommended: 32_768,
        documented_max: 32_768,
    },
    // 2 x 28 layers x 4 KV heads x 128 dims x 2 bytes = 57,344 bytes/token.
    kv_bytes_per_token: Some(57_344),
    compatibility_notes: &[
        "The publisher documents 32,768 tokens for this file; the 131,072 in its metadata needs YaRN (vLLM only).",
        "Use the single-file artefact; split GGUF shards are not supported.",
        "Nexora passes no GPU-offload or KV-cache flags, so this profile runs on CPU.",
        "Written against llama.cpp build 11714; another build is reported, not blocked.",
    ],
    locally_verified: false,
};

/// The Nexora Sonnet profile.
///
/// The artefact metadata below was read from the publisher's own model API at
/// the pinned commit, not computed here and not copied from another file. A
/// 40-character commit is immutable, so the byte length and LFS object id
/// describe one specific file permanently.
///
/// `locally_verified` stays `false`: no generation has been recorded against
/// this file, and the verification environment has neither the memory nor a
/// route to the host.
pub const NEXORA_SONNET: ModelProfile = ModelProfile {
    id: NEXORA_SONNET_ID,
    display_name: "Nexora Sonnet",
    publisher: "Qwen team, Alibaba Group",
    upstream_model: "Qwen/Qwen2.5-Coder-14B-Instruct",
    // The base model's published commit. The quantiser does not state which
    // base revision it converted, so this records provenance, not proof.
    upstream_revision: Some("aedcc2d42b622764e023cf882b6652e646b95671"),
    format: "GGUF",
    quantization: "Q4_K_M",
    licence: LicenceRef {
        id: "Apache-2.0",
        reference: "https://huggingface.co/Qwen/Qwen2.5-Coder-14B-Instruct-GGUF/blob/d0a692ef765eefbf2fabb130b3cb2e8917e3d225/LICENSE",
    },
    artifact: ArtifactState::Approved(ModelSource {
        repository: "Qwen/Qwen2.5-Coder-14B-Instruct-GGUF",
        revision: "d0a692ef765eefbf2fabb130b3cb2e8917e3d225",
        file_name: "qwen2.5-coder-14b-instruct-q4_k_m.gguf",
        size_bytes: 8_988_110_272,
        sha256: "c1e659736d89ac1065fb495330fb824d94001974a4bfa78e7270e43476a8d940",
    }),
    context: ContextGuidance {
        recommended: 8_192,
        documented_max: 32_768,
    },
    // 2 x 48 layers x 8 KV heads x 128 dims x 2 bytes = 196,608 bytes/token.
    kv_bytes_per_token: Some(196_608),
    compatibility_notes: &[
        "The publisher documents 32,768 tokens for this GGUF; the 131,072 in its metadata needs YaRN, which only vLLM supports.",
        "Use the single-file artefact; the repository also ships split shards, which Nexora does not support.",
        "Qwen2 architecture, which llama.cpp supports; the same build requirement as Nexora Mini applies.",
        "Weights are 8.99 GB (8.37 GiB); the 8,192-token default keeps the KV cache near 1.5 GiB.",
        "Nexora passes no GPU-offload or KV-cache flags, so this profile runs on CPU.",
    ],
    locally_verified: false,
};

/// A fixed set of registered profiles, in display order.
///
/// The registry is metadata only. It does not launch executables, resolve
/// filesystem paths, or imply that any profile is present on disk.
pub struct ModelRegistry {
    profiles: &'static [ModelProfile],
}

impl ModelRegistry {
    /// Builds a registry over a static profile table.
    ///
    /// `static` storage is required so resolution can return a borrow that
    /// outlives the call without cloning the profile.
    pub const fn new(profiles: &'static [ModelProfile]) -> Self {
        Self { profiles }
    }

    /// Resolves a stable identifier to its profile.
    ///
    /// Blank, unknown, duplicated, or malformed entries fail with a typed
    /// error instead of returning partial metadata.
    pub fn resolve(&self, id: &str) -> InferenceResult<&'static ModelProfile> {
        find_unique(self.profiles, id)
    }

    /// Every registered profile, in display order.
    pub fn all(&self) -> &'static [ModelProfile] {
        self.profiles
    }

    /// The profile that publishes `file_name`, when one does.
    ///
    /// Identity only: the name identifies which artefact this is supposed to
    /// be. Length and digest are checked separately, and never inferred here.
    pub fn by_file_name(&self, file_name: &str) -> Option<&'static ModelProfile> {
        self.profiles.iter().find(|profile| {
            profile
                .artifact
                .source()
                .is_some_and(|source| source.file_name == file_name)
        })
    }

    /// The profile with this id, when it is registered. Quiet lookup for
    /// callers that already validated the id.
    pub fn by_id(&self, id: &str) -> Option<&'static ModelProfile> {
        self.profiles.iter().find(|profile| profile.id == id)
    }

    /// Resolves the user's choice, falling back to the default profile when
    /// nothing has been chosen. An explicitly unknown id is an error: silently
    /// substituting another model would be worse than refusing.
    pub fn selected(&self, chosen: Option<&str>) -> InferenceResult<&'static ModelProfile> {
        match chosen.map(str::trim).filter(|id| !id.is_empty()) {
            Some(id) => self.resolve(id),
            None => self.resolve(DEFAULT_MODEL_ID),
        }
    }
}

static PROFILES: [ModelProfile; 2] = [NEXORA_MINI, NEXORA_SONNET];

/// The registry the application exposes.
pub static APPROVED: ModelRegistry = ModelRegistry::new(&PROFILES);

fn find_unique<'a>(profiles: &'a [ModelProfile], id: &str) -> InferenceResult<&'a ModelProfile> {
    let wanted = id.trim();

    if wanted.is_empty() {
        return Err(InferenceError::invalid_request(
            "A model identifier is required.",
        ));
    }

    let mut matches = profiles.iter().filter(|profile| profile.id == wanted);

    let Some(profile) = matches.next() else {
        let shown: String = wanted.chars().take(80).collect();

        return Err(InferenceError::unknown_model(format!(
            "No model profile is registered as \"{shown}\"."
        )));
    };

    if matches.next().is_some() {
        return Err(InferenceError::internal(
            "The model registry contains duplicate identifiers.",
        ));
    }

    validate(profile)?;

    Ok(profile)
}

/// Checks the profile a UI request says it was prepared for.
///
/// The desktop core reads the selection from its own settings, so this is not
/// how the model is chosen — it is how a *stale* caller is caught. A chat tab
/// that was rendered before the selection changed would otherwise be answered
/// by a different model than the one its composer names, which is exactly the
/// silent substitution this project refuses to do.
///
/// Every request must identify the profile its composer captured. A missing or
/// blank id is refused rather than silently substituting the current selection.
pub fn check_request_profile(
    requested: Option<&str>,
    selected: &ModelProfile,
) -> InferenceResult<()> {
    let Some(requested) = requested.map(str::trim).filter(|id| !id.is_empty()) else {
        return Err(InferenceError::new(
            "model_mismatch",
            format!(
                "This message did not identify the model it was prepared for. {} is selected. Refresh the model list and send it again. Nothing was sent.",
                selected.display_name
            ),
        ));
    };

    let requested = match APPROVED.by_id(requested) {
        Some(profile) => profile,
        None => {
            return Err(InferenceError::new(
                "model_mismatch",
                format!(
                    "This message was prepared for an unknown model. {} is selected. Refresh the model list and send it again.",
                    selected.display_name
                ),
            ))
        }
    };

    if requested.id == selected.id {
        return Ok(());
    }

    Err(InferenceError::new(
        "model_mismatch",
        format!(
            "This message was prepared for {}, but {} is selected now. Nothing was sent; send it again so it goes to the right model.",
            requested.display_name, selected.display_name
        ),
    ))
}

/// Whether the selection may change right now.
///
/// Switching is refused while the engine is answering or loading, because
/// either would leave a request running under a profile the user no longer has
/// selected. Nothing is queued and nothing is cancelled on the user's behalf:
/// they stop the reply (or wait for the load) and switch explicitly.
pub fn check_switch(
    target: &ModelProfile,
    generation_active: bool,
    load_in_flight: bool,
) -> InferenceResult<()> {
    if generation_active {
        return Err(InferenceError::new(
            "busy",
            format!(
                "A reply is still streaming. Stop it before switching to {}.",
                target.display_name
            ),
        ));
    }

    if load_in_flight {
        return Err(InferenceError::new(
            "busy",
            format!(
                "A model is still loading. Wait for it to finish before switching to {}.",
                target.display_name
            ),
        ));
    }

    Ok(())
}

/// Whether a model may be loaded right now.
///
/// Loading starts a new engine process, and that replaces the running one. A
/// reply that is still streaming would be cut off mid-answer, so the load waits
/// until the reply finishes or the user stops it. Nothing is cancelled for them.
pub fn check_load_allowed(generation_active: bool) -> InferenceResult<()> {
    if generation_active {
        return Err(InferenceError::new(
            "busy",
            "A reply is still streaming. Stop it before loading a model.",
        ));
    }

    Ok(())
}

/// Whether a new message may be sent right now.
///
/// While a load is in flight the engine that will answer is not yet known, and
/// the one that was running is being replaced. A message sent in that window
/// could be answered by whichever process happens to be healthy, so it waits.
pub fn check_generation_allowed(load_in_flight: bool) -> InferenceResult<()> {
    if load_in_flight {
        return Err(InferenceError::new(
            "busy",
            "A model is still loading. Wait for it to finish, then send your message.",
        ));
    }

    Ok(())
}

/// Confirms that the model the engine is running is the one the user selected.
///
/// This is the guard against silently answering a Sonnet request with Mini.
/// A file that matches no registered artefact is only accepted while the
/// default profile is selected, which keeps pre-selection installations
/// working without letting an unidentified file stand in for a named model.
///
/// Taking the profile rather than an id keeps the pending branch testable with
/// a profile that is not in the registry.
pub fn check_selection(
    chosen: &ModelProfile,
    loaded: Option<&str>,
    has_loaded_model: bool,
) -> InferenceResult<()> {
    // A profile that cannot be installed cannot be the model being answered
    // with, so say that plainly instead of reporting a mismatch.
    if !chosen.artifact.is_approved() {
        return Err(InferenceError::new(
            "artifact_pending",
            format!(
                "{} cannot be used yet: no approved artefact is recorded. Choose another model, or record the artefact metadata first.",
                chosen.display_name
            ),
        ));
    }

    if !has_loaded_model {
        return Err(InferenceError::new(
            "model_mismatch",
            format!(
                "{} is selected, but no model is confirmed loaded. Load the selected model before sending.",
                chosen.display_name
            ),
        ));
    }

    match loaded {
        Some(id) if id == chosen.id => Ok(()),
        Some(id) => {
            let running = APPROVED
                .resolve(id)
                .map(|profile| profile.display_name)
                .unwrap_or("another model");

            Err(InferenceError::new(
                "model_mismatch",
                format!(
                    "{} is selected, but the engine has {} loaded. Load the selected model before sending.",
                    chosen.display_name, running
                ),
            ))
        }
        None if chosen.id == DEFAULT_MODEL_ID => Ok(()),
        None => Err(InferenceError::new(
            "model_mismatch",
            format!(
                "{} is selected, but Nexora cannot confirm which model the engine has loaded. Load the selected model before sending.",
                chosen.display_name
            ),
        )),
    }
}

/// Rejects metadata that would be unsafe to present as an approved profile.
///
/// The checks are cheap and run at resolution time, so a malformed static entry
/// can never reach the UI just because it compiles.
fn validate(profile: &ModelProfile) -> InferenceResult<()> {
    if !is_stable_id(profile.id) {
        return Err(inconsistent("id"));
    }

    if profile.display_name.trim().is_empty()
        || profile.publisher.trim().is_empty()
        || profile.upstream_model.trim().is_empty()
        || profile.format.trim().is_empty()
        || profile.quantization.trim().is_empty()
    {
        return Err(inconsistent("descriptive metadata"));
    }

    if profile.licence.id.trim().is_empty() || profile.licence.reference.trim().is_empty() {
        return Err(inconsistent("licence"));
    }

    // An unrecorded revision is allowed; a recorded but malformed one is not.
    if let Some(revision) = profile.upstream_revision {
        if !is_git_revision(revision) {
            return Err(inconsistent("upstream revision"));
        }
    }

    match profile.artifact {
        ArtifactState::Approved(source) => {
            if source.repository.trim().is_empty()
                || !is_git_revision(source.revision)
                || !source.file_name.ends_with(".gguf")
                || source.size_bytes == 0
                || !is_sha256(source.sha256)
            {
                return Err(inconsistent("source artefact"));
            }
        }
        ArtifactState::Pending { note } => {
            if note.trim().is_empty() {
                return Err(inconsistent("pending artefact note"));
            }
        }
    }

    if profile.context.recommended == 0
        || profile.context.recommended > profile.context.documented_max
        || profile.context.documented_max > MAX_CONTEXT_SIZE
    {
        return Err(inconsistent("context guidance"));
    }

    if profile
        .compatibility_notes
        .iter()
        .any(|note| note.trim().is_empty())
    {
        return Err(inconsistent("compatibility notes"));
    }

    Ok(())
}

fn inconsistent(field: &str) -> InferenceError {
    InferenceError::internal(format!("The model profile is invalid: {field}."))
}

/// Lowercase letters, digits, dots, and hyphens only.
fn is_stable_id(value: &str) -> bool {
    !value.is_empty()
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-' || byte == b'.'
        })
}

/// A lowercase, 64-character SHA-256 digest.
fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

/// A full, lowercase, 40-character Git object id.
fn is_git_revision(value: &str) -> bool {
    value.len() == 40
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mutated(apply: impl FnOnce(&mut ModelProfile)) -> ModelProfile {
        let mut profile = NEXORA_MINI;
        apply(&mut profile);
        profile
    }

    /// Applies a change to Mini's approved artefact metadata.
    fn mutated_source(apply: impl FnOnce(&mut ModelSource)) -> ModelProfile {
        let mut profile = NEXORA_MINI;

        match &mut profile.artifact {
            ArtifactState::Approved(source) => apply(source),
            ArtifactState::Pending { .. } => panic!("Mini must stay approved"),
        }

        profile
    }

    fn mini_source(profile: &ModelProfile) -> ModelSource {
        profile.artifact.source().expect("Mini artefact")
    }

    #[test]
    fn mini_profile_matches_the_phase_3_recommendation() {
        assert_eq!(NEXORA_MINI.id, NEXORA_MINI_ID);
        assert_eq!(NEXORA_MINI.display_name, "Nexora Mini");
        assert_eq!(NEXORA_MINI.publisher, "Qwen team, Alibaba Group");
        assert_eq!(NEXORA_MINI.upstream_model, "Qwen/Qwen2.5-Coder-7B-Instruct");
        assert_eq!(
            NEXORA_MINI.upstream_revision,
            Some("c03e6d358207e414f1eca0bb1891e29f1db0e242")
        );
        assert_eq!(NEXORA_MINI.format, "GGUF");
        assert_eq!(NEXORA_MINI.quantization, "Q4_K_M");
        assert_eq!(NEXORA_MINI.licence.id, "Apache-2.0");

        let source = mini_source(&NEXORA_MINI);

        assert_eq!(source.repository, "Qwen/Qwen2.5-Coder-7B-Instruct-GGUF");
        assert_eq!(source.revision, "13fb94bfda8c8cf22497dc57b78f391a9acb426a");
        assert_eq!(source.file_name, "qwen2.5-coder-7b-instruct-q4_k_m.gguf");
        assert_eq!(source.size_bytes, 4_683_073_536);
        assert_eq!(
            source.sha256,
            "509287f78cb4d4cf6b3843734733b914b2c158e43e22a7f4bf5e963800894d3c"
        );
        // Keep the licence pointer at the same immutable artifact revision;
        // this consistency check is not independent licence verification.
        assert!(NEXORA_MINI.licence.reference.contains(source.revision));
        assert_eq!(NEXORA_MINI.context.recommended, 32_768);
        assert_eq!(NEXORA_MINI.context.documented_max, 32_768);
    }

    #[test]
    fn no_profile_claims_local_verification() {
        for profile in APPROVED.all() {
            assert!(!profile.locally_verified, "{}", profile.id);
        }
    }

    #[test]
    fn looks_profiles_up_by_published_file_name() {
        assert_eq!(
            APPROVED
                .by_file_name("qwen2.5-coder-14b-instruct-q4_k_m.gguf")
                .map(|profile| profile.id),
            Some(NEXORA_SONNET_ID)
        );
        assert_eq!(
            APPROVED
                .by_file_name("qwen2.5-coder-7b-instruct-q4_k_m.gguf")
                .map(|profile| profile.id),
            Some(NEXORA_MINI_ID)
        );
        assert!(APPROVED.by_file_name("my-model.gguf").is_none());
        assert!(APPROVED.by_file_name("").is_none());
    }

    #[test]
    fn looks_profiles_up_by_id_without_an_error() {
        assert_eq!(
            APPROVED.by_id(NEXORA_MINI_ID).map(|p| p.id),
            Some(NEXORA_MINI_ID)
        );
        assert_eq!(
            APPROVED.by_id(NEXORA_SONNET_ID).map(|p| p.id),
            Some(NEXORA_SONNET_ID)
        );
        assert!(APPROVED.by_id("not-a-model").is_none());
        assert!(APPROVED.by_id("").is_none());
    }

    #[test]
    fn the_registry_registers_mini_and_sonnet_in_order() {
        let ids: Vec<&str> = APPROVED.all().iter().map(|profile| profile.id).collect();

        assert_eq!(ids, vec![NEXORA_MINI_ID, NEXORA_SONNET_ID]);
    }

    #[test]
    fn sonnet_matches_the_published_artefact() {
        assert_eq!(NEXORA_SONNET.display_name, "Nexora Sonnet");
        assert_eq!(NEXORA_SONNET.publisher, "Qwen team, Alibaba Group");
        assert_eq!(
            NEXORA_SONNET.upstream_model,
            "Qwen/Qwen2.5-Coder-14B-Instruct"
        );
        assert_eq!(NEXORA_SONNET.format, "GGUF");
        assert_eq!(NEXORA_SONNET.quantization, "Q4_K_M");
        assert_eq!(NEXORA_SONNET.licence.id, "Apache-2.0");

        let source = NEXORA_SONNET
            .artifact
            .source()
            .expect("the Sonnet artefact is approved");

        assert_eq!(source.repository, "Qwen/Qwen2.5-Coder-14B-Instruct-GGUF");
        assert_eq!(source.revision, "d0a692ef765eefbf2fabb130b3cb2e8917e3d225");
        assert_eq!(source.file_name, "qwen2.5-coder-14b-instruct-q4_k_m.gguf");
        assert_eq!(source.size_bytes, 8_988_110_272);
        assert_eq!(
            source.sha256,
            "c1e659736d89ac1065fb495330fb824d94001974a4bfa78e7270e43476a8d940"
        );
        assert_eq!(
            NEXORA_SONNET.upstream_revision,
            Some("aedcc2d42b622764e023cf882b6652e646b95671")
        );

        // The licence reference points at the same immutable commit as the
        // weights, so the two cannot drift apart.
        assert!(NEXORA_SONNET.licence.reference.contains(source.revision));
    }

    #[test]
    fn kv_geometry_matches_the_published_configs() {
        // Qwen2.5-Coder-7B-Instruct: 28 layers, 4 KV heads, 128 head_dim.
        assert_eq!(NEXORA_MINI.kv_bytes_per_token, Some(28 * 4 * 128 * 2 * 2));
        // Qwen2.5-Coder-14B-Instruct: 48 layers, 8 KV heads, 128 head_dim.
        assert_eq!(NEXORA_SONNET.kv_bytes_per_token, Some(48 * 8 * 128 * 2 * 2));
    }

    #[test]
    fn sonnet_context_matches_the_publisher_note() {
        // The card states "Full 32,768 tokens" for the GGUF and puts 131,072
        // behind YaRN, which llama.cpp does not enable here.
        assert_eq!(NEXORA_SONNET.context.documented_max, 32_768);
        assert_eq!(NEXORA_SONNET.context.recommended, 8_192);
    }

    /// A profile with no approved artefact, used to keep the pending path
    /// covered even though every registered profile is currently approved.
    fn pending_profile() -> ModelProfile {
        mutated(|profile| {
            profile.artifact = ArtifactState::Pending {
                note: "No artefact recorded.",
            }
        })
    }
    #[test]
    fn mini_stays_the_default_selection() {
        assert_eq!(DEFAULT_MODEL_ID, NEXORA_MINI_ID);

        let resolved = APPROVED.selected(None).expect("default profile");

        assert_eq!(resolved.id, NEXORA_MINI_ID);
        assert_eq!(APPROVED.selected(Some("   ")).unwrap().id, NEXORA_MINI_ID);
    }

    #[test]
    fn resolves_the_stable_id() {
        let profile = APPROVED.resolve(NEXORA_MINI_ID).expect("approved profile");

        assert_eq!(profile.id, NEXORA_MINI_ID);
        assert_eq!(profile.display_name, "Nexora Mini");
    }

    #[test]
    fn resolves_sonnet_by_id() {
        let profile = APPROVED.resolve(NEXORA_SONNET_ID).expect("sonnet profile");

        assert_eq!(profile.display_name, "Nexora Sonnet");
    }

    #[test]
    fn resolution_trims_surrounding_whitespace() {
        let profile = APPROVED
            .resolve("  nexora-mini-qwen2.5-coder-7b-instruct-q4-k-m  ")
            .expect("approved profile");

        assert_eq!(profile.id, NEXORA_MINI_ID);
    }

    #[test]
    fn rejects_an_unknown_id() {
        let error = APPROVED.resolve("nexora-mini-not-a-model").unwrap_err();

        assert_eq!(error.code, "unknown_model");
        assert!(error.message.contains("nexora-mini-not-a-model"));
    }

    #[test]
    fn an_unknown_selection_is_an_error_not_a_fallback() {
        let error = APPROVED
            .selected(Some("nexora-sonnet-missing"))
            .unwrap_err();

        assert_eq!(error.code, "unknown_model");
    }

    #[test]
    fn rejects_a_blank_id() {
        let error = APPROVED.resolve("   ").unwrap_err();

        assert_eq!(error.code, "invalid_request");
    }

    #[test]
    fn rejects_duplicate_registrations() {
        let duplicates = [NEXORA_MINI, NEXORA_MINI];
        let error = find_unique(&duplicates, NEXORA_MINI_ID).unwrap_err();

        assert_eq!(error.code, "internal");
    }

    #[test]
    fn rejects_a_profile_without_a_display_name() {
        let profile = mutated(|profile| profile.display_name = "  ");
        let error = validate(&profile).unwrap_err();

        assert_eq!(error.code, "internal");
    }

    #[test]
    fn rejects_a_profile_with_a_non_gguf_artefact() {
        let profile = mutated_source(|source| source.file_name = "model.bin");

        assert!(validate(&profile).is_err());
    }

    #[test]
    fn rejects_a_profile_with_a_malformed_revision() {
        let profile = mutated_source(|source| source.revision = "deadbeef");

        assert!(validate(&profile).is_err());
    }

    #[test]
    fn rejects_a_profile_with_a_malformed_upstream_revision() {
        let profile = mutated(|profile| profile.upstream_revision = Some(""));
        assert!(validate(&profile).is_err());

        let profile = mutated(|profile| profile.upstream_revision = Some("c03e6d35"));
        assert!(validate(&profile).is_err());
    }

    #[test]
    fn accepts_a_profile_with_no_recorded_upstream_revision() {
        let profile = mutated(|profile| profile.upstream_revision = None);

        assert!(validate(&profile).is_ok());
    }

    #[test]
    fn rejects_a_pending_profile_without_a_note() {
        let profile = mutated(|profile| profile.artifact = ArtifactState::Pending { note: "  " });

        assert!(validate(&profile).is_err());
    }

    #[test]
    fn rejects_a_profile_with_a_malformed_digest() {
        for digest in [
            String::new(),
            "509287f7".to_string(),
            mini_source(&NEXORA_MINI).sha256.to_uppercase(),
            "z".repeat(64),
        ] {
            let leaked: &'static str = Box::leak(digest.into_boxed_str());
            let profile = mutated_source(|source| source.sha256 = leaked);

            assert!(validate(&profile).is_err());
        }
    }

    #[test]
    fn rejects_a_profile_with_an_inconsistent_context() {
        let profile = mutated(|profile| profile.context.recommended = 40_960);

        assert!(validate(&profile).is_err());
    }

    #[test]
    fn rejects_context_beyond_the_settings_maximum() {
        let profile = mutated(|profile| {
            profile.context.documented_max = MAX_CONTEXT_SIZE + 1;
            profile.context.recommended = MAX_CONTEXT_SIZE + 1;
        });

        assert!(validate(&profile).is_err());
    }

    #[test]
    fn rejects_an_empty_compatibility_note() {
        let profile = mutated(|profile| profile.compatibility_notes = &[""]);

        assert!(validate(&profile).is_err());
    }

    #[test]
    fn generation_matches_only_the_selected_profile() {
        assert!(check_selection(&NEXORA_MINI, Some(NEXORA_MINI_ID), true).is_ok());
        assert!(check_selection(&NEXORA_SONNET, Some(NEXORA_SONNET_ID), true).is_ok());
    }

    #[test]
    fn selecting_sonnet_never_runs_on_a_loaded_mini() {
        let error = check_selection(&NEXORA_SONNET, Some(NEXORA_MINI_ID), true).unwrap_err();

        assert_eq!(error.code, "model_mismatch");
        assert!(error.message.contains("Nexora Sonnet is selected"));
        assert!(error.message.contains("Nexora Mini"));
    }

    #[test]
    fn selecting_mini_never_runs_on_a_loaded_sonnet() {
        let error = check_selection(&NEXORA_MINI, Some(NEXORA_SONNET_ID), true).unwrap_err();

        assert_eq!(error.code, "model_mismatch");
        assert!(error.message.contains("Nexora Mini is selected"));
        assert!(error.message.contains("Nexora Sonnet"));
    }

    #[test]
    fn an_unapproved_model_is_refused_and_told_why() {
        let profile = pending_profile();

        for loaded in [None, Some(NEXORA_MINI_ID)] {
            let error = check_selection(&profile, loaded, true).unwrap_err();

            assert_eq!(error.code, "artifact_pending");
            assert!(error.message.contains("cannot be used yet"));
            assert!(error.message.contains("no approved artefact"));
        }
    }

    #[test]
    fn an_unidentified_file_cannot_stand_in_for_sonnet() {
        let error = check_selection(&NEXORA_SONNET, None, true).unwrap_err();

        assert_eq!(error.code, "model_mismatch");
    }

    #[test]
    fn the_default_profile_accepts_an_unknown_file_only_when_loaded_is_confirmed() {
        // Mini may run a user-provided custom GGUF, but an absent loaded record
        // must never stand in for that successful runtime attribution.
        assert!(check_selection(&NEXORA_MINI, None, true).is_ok());

        let error = check_selection(&NEXORA_MINI, None, false).unwrap_err();
        assert_eq!(error.code, "model_mismatch");
        assert!(error.message.contains("no model is confirmed loaded"));
    }
    #[test]
    fn serialises_with_stable_camel_case_keys() {
        let value = serde_json::to_value(NEXORA_MINI).expect("serialise profile");

        assert_eq!(value["id"], NEXORA_MINI_ID);
        assert_eq!(value["displayName"], "Nexora Mini");
        assert_eq!(value["upstreamModel"], "Qwen/Qwen2.5-Coder-7B-Instruct");
        assert_eq!(
            value["upstreamRevision"],
            "c03e6d358207e414f1eca0bb1891e29f1db0e242"
        );
        assert_eq!(value["artifact"]["state"], "approved");
        assert_eq!(
            value["artifact"]["sha256"],
            "509287f78cb4d4cf6b3843734733b914b2c158e43e22a7f4bf5e963800894d3c"
        );
        assert_eq!(
            value["artifact"]["fileName"],
            "qwen2.5-coder-7b-instruct-q4_k_m.gguf"
        );
        assert_eq!(value["artifact"]["sizeBytes"], 4_683_073_536u64);
        assert_eq!(value["context"]["documentedMax"], 32_768);
        assert_eq!(value["licence"]["id"], "Apache-2.0");
        assert_eq!(value["locallyVerified"], false);
    }

    #[test]
    fn a_request_prepared_for_the_selected_profile_is_accepted() {
        assert!(check_request_profile(Some(NEXORA_MINI_ID), &NEXORA_MINI).is_ok());
        assert!(check_request_profile(Some(NEXORA_SONNET_ID), &NEXORA_SONNET).is_ok());
        // Surrounding whitespace is trimmed without changing the captured id.
        assert!(check_request_profile(Some("  nexora-mini  "), &NEXORA_MINI).is_ok());
    }

    #[test]
    fn a_missing_or_blank_profile_id_is_refused_without_dispatch() {
        for requested in [None, Some(""), Some("  ")] {
            let error = check_request_profile(requested, &NEXORA_MINI).unwrap_err();
            assert_eq!(error.code, "model_mismatch");
            assert!(error.message.contains("did not identify the model"));
            assert!(error.message.contains("Nothing was sent"));
        }
    }

    #[test]
    fn a_stale_request_cannot_be_answered_by_another_profile() {
        let error = check_request_profile(Some(NEXORA_MINI_ID), &NEXORA_SONNET).unwrap_err();

        assert_eq!(error.code, "model_mismatch");
        assert!(error.message.contains("Nexora Mini"));
        assert!(error.message.contains("Nexora Sonnet"));
        assert!(error.message.contains("Nothing was sent"));

        let reverse = check_request_profile(Some(NEXORA_SONNET_ID), &NEXORA_MINI).unwrap_err();

        assert_eq!(reverse.code, "model_mismatch");
        assert!(reverse.message.contains("Nexora Sonnet"));
        assert!(reverse.message.contains("Nexora Mini"));
    }

    #[test]
    fn an_unknown_requested_profile_is_refused_rather_than_ignored() {
        let error = check_request_profile(Some("not-a-model"), &NEXORA_MINI).unwrap_err();

        assert_eq!(error.code, "model_mismatch");
        assert!(error.message.contains("unknown model"));
    }

    #[test]
    fn the_selection_can_change_while_the_engine_is_idle() {
        assert!(check_switch(&NEXORA_SONNET, false, false).is_ok());
        assert!(check_switch(&NEXORA_MINI, false, false).is_ok());
    }

    #[test]
    fn the_selection_cannot_change_during_a_generation() {
        let error = check_switch(&NEXORA_SONNET, true, false).unwrap_err();

        assert_eq!(error.code, "busy");
        assert!(error.message.contains("Nexora Sonnet"));
        assert!(error.message.contains("Stop"));
    }

    #[test]
    fn the_selection_cannot_change_during_a_load() {
        let error = check_switch(&NEXORA_MINI, false, true).unwrap_err();

        assert_eq!(error.code, "busy");
        assert!(error.message.contains("loading"));
    }

    #[test]
    fn a_model_cannot_be_loaded_while_a_reply_streams() {
        let error = check_load_allowed(true).unwrap_err();

        assert_eq!(error.code, "busy");
        assert!(error.message.contains("Stop it"));
        assert!(check_load_allowed(false).is_ok());
    }

    #[test]
    fn a_message_cannot_be_sent_while_a_model_loads() {
        let error = check_generation_allowed(true).unwrap_err();

        assert_eq!(error.code, "busy");
        assert!(error.message.contains("still loading"));
        assert!(check_generation_allowed(false).is_ok());
    }

    #[test]
    fn the_sonnet_profile_serialises_its_artefact() {
        let value = serde_json::to_value(NEXORA_SONNET).expect("serialise profile");

        assert_eq!(value["artifact"]["state"], "approved");
        assert_eq!(
            value["artifact"]["fileName"],
            "qwen2.5-coder-14b-instruct-q4_k_m.gguf"
        );
        assert_eq!(value["artifact"]["sizeBytes"], 8_988_110_272u64);
        assert_eq!(
            value["artifact"]["sha256"],
            "c1e659736d89ac1065fb495330fb824d94001974a4bfa78e7270e43476a8d940"
        );
        assert_eq!(
            value["upstreamRevision"],
            "aedcc2d42b622764e023cf882b6652e646b95671"
        );
    }
}
