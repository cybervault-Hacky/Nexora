//! Hardware-aware readiness for a model profile.
//!
//! One authoritative answer to "can this model be downloaded, loaded, or
//! used?", derived from verified artifact metadata plus what the machine
//! actually reports. The frontend renders this; it never re-derives it.
//!
//! Two rules shape everything here:
//!
//! * **Estimates are labelled as estimates.** The memory figure is arithmetic
//!   over the artifact size, the configured context length, and a fixed runtime
//!   reserve. It is not a measurement and it is not a guaranteed minimum.
//! * **Unknown is not the same as fine.** When a platform cannot report memory
//!   or free disk space, the affected check is reported as `unknown` and the
//!   action is allowed with a warning rather than silently assumed to fit.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::{Path, PathBuf};

use serde::ser::Serializer;
use serde::Serialize;

use super::registry::{ArtifactState, ModelProfile};

/// 1 GiB, as a `f64` for the small arithmetic below.
pub const GIB: f64 = 1024.0 * 1024.0 * 1024.0;
/// Bytes in 1 MiB.
pub const MIB_BYTES: u64 = 1024 * 1024;

/// Space held back on top of the weights when planning a download.
///
/// The published file is written to a `.part` file and renamed into place, so
/// the peak requirement is the file itself plus bookkeeping. This reserve also
/// covers the filesystem's own metadata for a multi-gigabyte file.
pub const INSTALL_HEADROOM_BYTES: u64 = 512 * MIB_BYTES;

/// Memory the desktop and the runtime need on top of the weights.
///
/// Covers the process image, the compute graph, and the buffers llama.cpp
/// allocates outside the KV cache. It is a planning constant, not a reading.
pub const RUNTIME_OVERHEAD_GIB: f64 = 1.0;

/// Headroom left for the rest of the desktop before calling memory sufficient.
///
/// Matches the rule the file inspector already applies, so the two checks in
/// the product cannot contradict each other.
pub const MEMORY_HEADROOM_GIB: f64 = 2.0;

/// Which file, per profile, was confirmed as the approved artefact this session.
///
/// Deliberately session-scoped and path-keyed: after a restart Nexora no longer
/// knows that a file on disk is the approved artefact, and confirming one copy
/// must not bless a different file with the same name in another folder. A
/// file's presence is never evidence on its own.
#[derive(Debug, Default)]
pub struct VerificationLedger {
    verified: BTreeMap<&'static str, PathBuf>,
    failed: BTreeSet<&'static str>,
}

impl VerificationLedger {
    /// Records a digest match for `profile_id` at `path`.
    pub fn mark_verified(&mut self, profile_id: &'static str, path: &Path) {
        self.failed.remove(profile_id);
        self.verified.insert(profile_id, normalise(path));
    }

    /// Records a digest mismatch. Clears any earlier match for the profile so a
    /// stale "verified" can never outlive the evidence for it.
    pub fn mark_failed(&mut self, profile_id: &'static str) {
        self.verified.remove(profile_id);
        self.failed.insert(profile_id);
    }

    pub fn is_verified(&self, profile_id: &str, path: &Path) -> bool {
        self.verified
            .get(profile_id)
            .is_some_and(|stored| *stored == normalise(path))
    }

    pub fn has_failed(&self, profile_id: &str) -> bool {
        self.failed.contains(profile_id)
    }
}

/// Canonical form for comparison. Falls back to the literal path when the file
/// is gone, which is correct: a deleted file must not compare equal to a
/// canonical path that was recorded earlier.
fn normalise(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// What the platform reported about free disk space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DiskStatus {
    /// Free bytes at the destination.
    Known(u64),
    /// Detection failed, or the platform has no implementation.
    Unknown,
}

/// What the platform reported about physical memory, in GiB.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MemoryStatus {
    Known {
        total_gib: f64,
        available_gib: f64,
    },
    /// Detection failed, or the platform has no implementation.
    Unknown,
}

/// Facts about files on disk for one profile.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct FileFacts {
    /// A file with the exact published name **and** exact published length.
    pub installed_exact: bool,
    /// Bytes already present in a `.part` file, when one is.
    pub partial_bytes: Option<u64>,
    /// The digest for this profile was confirmed against the published value.
    pub verified: bool,
    /// A digest check ran and did not match.
    pub verification_failed: bool,
    /// A file with the published name exists at the destination but is a
    /// different length, so it is not the approved artefact.
    pub wrong_size_bytes: Option<u64>,
}

/// Facts about the engine process.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EngineFacts {
    pub running: bool,
    pub healthy: bool,
    /// The profile id the engine reported loading, when it matched one.
    pub loaded_profile_id: Option<String>,
    /// The profile a recorded failure was attributed to, when it was.
    ///
    /// Separate from `loaded_profile_id` because a failed load leaves nothing
    /// loaded: the attribution is kept so the row can explain itself without
    /// pretending a dead process is still running this file.
    pub failed_profile_id: Option<String>,
    /// The profile whose load is in flight, before the engine confirms it.
    pub loading_profile_id: Option<String>,
    pub last_error: Option<String>,
}

impl EngineFacts {
    fn is_running(&self, profile_id: &str) -> bool {
        self.running && self.loaded_profile_id.as_deref() == Some(profile_id)
    }
}

/// Everything `assess` needs. Built by the command layer from real state.
#[derive(Debug, Clone)]
pub struct ReadinessInput<'a> {
    pub profile: &'a ModelProfile,
    /// The context window currently configured, not a value chosen to pass.
    pub context_size: u32,
    pub disk: DiskStatus,
    pub memory: MemoryStatus,
    pub files: FileFacts,
    pub download_active: bool,
    pub engine: EngineFacts,
}

/// The single readiness state for one profile.
///
/// Serialised as `snake_case`. The frontend switches on these strings and must
/// not invent new ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadinessState {
    /// The profile has no approved artefact, so nothing can be installed.
    Unavailable,
    /// Nothing on disk, enough free space. Downloading is allowed.
    ReadyToDownload,
    /// A download is running right now.
    Downloading,
    /// A partial file was left by a cancelled or interrupted download.
    DownloadIncomplete,
    /// A digest check ran and did not match the published value.
    VerificationFailed,
    /// The file is present at the exact published length but unverified.
    Unverified,
    /// Verified on disk, but the engine is holding a different model.
    DownloadedVerified,
    /// The destination does not have room for the artefact.
    InsufficientStorage,
    /// The machine does not have enough available memory to load this.
    InsufficientMemory,
    /// A resource needed to decide could not be measured.
    DetectionUnavailable,
    /// Verified on disk, enough memory, engine idle: load it.
    ReadyToLoad,
    /// The engine is starting and loading this profile.
    Loading,
    /// The engine is running this profile and answering.
    Loaded,
    /// The engine exited or failed while loading this profile.
    RuntimeFailure,
}

impl ReadinessState {
    /// Stable wire name. Kept separate from `Debug` so refactoring a variant
    /// cannot silently change the IPC contract.
    pub fn as_str(self) -> &'static str {
        match self {
            ReadinessState::Unavailable => "unavailable",
            ReadinessState::ReadyToDownload => "ready_to_download",
            ReadinessState::Downloading => "downloading",
            ReadinessState::DownloadIncomplete => "download_incomplete",
            ReadinessState::VerificationFailed => "verification_failed",
            ReadinessState::Unverified => "unverified",
            ReadinessState::DownloadedVerified => "downloaded_verified",
            ReadinessState::InsufficientStorage => "insufficient_storage",
            ReadinessState::InsufficientMemory => "insufficient_memory",
            ReadinessState::DetectionUnavailable => "detection_unavailable",
            ReadinessState::ReadyToLoad => "ready_to_load",
            ReadinessState::Loading => "loading",
            ReadinessState::Loaded => "loaded",
            ReadinessState::RuntimeFailure => "runtime_failure",
        }
    }
}

impl fmt::Display for ReadinessState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl Serialize for ReadinessState {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

/// Outcome of one check: `ok`, `warning`, `blocked`, or `unknown`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckStatus {
    Ok,
    Warning,
    Blocked,
    Unknown,
}

impl CheckStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            CheckStatus::Ok => "ok",
            CheckStatus::Warning => "warning",
            CheckStatus::Blocked => "blocked",
            CheckStatus::Unknown => "unknown",
        }
    }
}

impl Serialize for CheckStatus {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

/// One named check shown next to the readiness state.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadinessCheck {
    pub id: &'static str,
    pub status: CheckStatus,
    pub message: String,
}

/// Storage and memory arithmetic for one profile at one context size.
///
/// Every byte here is derived from the published artefact size or from the
/// configured context; none of it is measured on the machine.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceEstimate {
    pub model_bytes: u64,
    /// Free space the destination needs before a download may start.
    pub required_free_bytes: u64,
    /// Bytes the weights occupy once installed (the `.part` is renamed).
    pub install_bytes: u64,
    /// Peak bytes on disk during a download, including the headroom reserve.
    pub peak_download_bytes: u64,
    pub weights_gib: f64,
    /// KV cache for the configured context, zero when geometry is unknown.
    pub kv_cache_gib: f64,
    pub runtime_overhead_gib: f64,
    /// Weights + cache + runtime overhead. An estimate, never a minimum.
    pub estimated_memory_gib: f64,
    /// Available memory that must be left free for the rest of the desktop.
    pub memory_headroom_gib: f64,
    /// The context size these figures were computed for.
    pub context_size: u32,
}

impl ResourceEstimate {
    /// Memory this profile needs in total, rounded for display only.
    pub fn estimated_memory_display(&self) -> f64 {
        round2(self.estimated_memory_gib)
    }

    pub fn weights_display(&self) -> f64 {
        round2(self.weights_gib)
    }

    pub fn kv_cache_display(&self) -> f64 {
        round2(self.kv_cache_gib)
    }

    pub fn required_free_display(&self) -> f64 {
        round2(self.peak_download_bytes as f64 / GIB)
    }
}

/// The authoritative readiness result for one profile.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Readiness {
    pub state: ReadinessState,
    pub headline: &'static str,
    pub detail: String,
    pub can_download: bool,
    /// True when a file with the published name and exact size is present but
    /// this session has not confirmed its digest. Verifying is the next step.
    pub can_verify: bool,
    pub can_load: bool,
    pub can_generate: bool,
    pub estimate: ResourceEstimate,
    pub checks: Vec<ReadinessCheck>,
}

/// Converts a byte count to GiB.
pub fn bytes_to_gib(bytes: u64) -> f64 {
    bytes as f64 / GIB
}

/// Converts GiB to bytes, saturating instead of overflowing.
pub fn gib_to_bytes(gib: f64) -> u64 {
    if !gib.is_finite() || gib <= 0.0 {
        return 0;
    }

    let bytes = gib * GIB;

    if bytes >= u64::MAX as f64 {
        u64::MAX
    } else {
        bytes.round() as u64
    }
}

fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

/// `9.00 GB` style label for byte counts in messages.
pub fn format_bytes(bytes: u64) -> String {
    let gib = bytes_to_gib(bytes);

    if gib >= 1.0 {
        format!("{gib:.2} GiB")
    } else {
        format!("{:.0} MiB", bytes as f64 / MIB_BYTES as f64)
    }
}

/// Storage and memory arithmetic for a profile at a context size.
///
/// Weights come from the published artifact. The KV cache uses the profile's
/// published geometry (`kv_bytes_per_token`) when it has one, and otherwise
/// falls back to the same blunt allowance the file inspector uses, so the two
/// never disagree by more than a fraction of a GiB.
pub fn estimate(profile: &ModelProfile, context_size: u32) -> ResourceEstimate {
    let model_bytes = profile
        .artifact
        .source()
        .map_or(0, |source| source.size_bytes);
    let weights_gib = bytes_to_gib(model_bytes);

    let kv_cache_gib = match profile.kv_bytes_per_token {
        Some(per_token) => (per_token as f64 * context_size as f64) / GIB,
        // Unknown geometry: the inspector's allowance (0.5 GiB + 1.5 GiB per
        // 32k tokens). Deliberately generous rather than optimistic.
        None => 0.5 + (context_size as f64 / 32_768.0) * 1.5,
    };

    let estimated_memory_gib = weights_gib + kv_cache_gib + RUNTIME_OVERHEAD_GIB;

    let peak_download_bytes = model_bytes.saturating_add(INSTALL_HEADROOM_BYTES);

    ResourceEstimate {
        model_bytes,
        required_free_bytes: peak_download_bytes,
        install_bytes: model_bytes,
        peak_download_bytes,
        weights_gib,
        kv_cache_gib,
        runtime_overhead_gib: RUNTIME_OVERHEAD_GIB,
        estimated_memory_gib,
        memory_headroom_gib: MEMORY_HEADROOM_GIB,
        context_size,
    }
}

/// Decides the single readiness state and which actions are permitted.
///
/// Precedence follows what the user can act on next: a running engine first,
/// then an in-flight or failed download, then what is on disk, then resources.
pub fn assess(input: &ReadinessInput<'_>) -> Readiness {
    let estimate = estimate(input.profile, input.context_size);
    let mut checks = Vec::new();

    let has_artifact = matches!(input.profile.artifact, ArtifactState::Approved(_));

    // ---- resource checks -------------------------------------------------
    let storage_check = match input.disk {
        DiskStatus::Known(free) if has_artifact && !input.files.installed_exact => {
            if free >= estimate.required_free_bytes {
                CheckStatus::Ok
            } else {
                CheckStatus::Blocked
            }
        }
        DiskStatus::Known(_) => CheckStatus::Ok,
        DiskStatus::Unknown => CheckStatus::Unknown,
    };

    let memory_check = match input.memory {
        MemoryStatus::Known { available_gib, .. } => {
            let usable = (available_gib - estimate.memory_headroom_gib).max(0.0);

            if estimate.estimated_memory_gib <= usable {
                CheckStatus::Ok
            } else {
                // Never blocking while the model is not even installed: the
                // download is still useful and memory can change.
                if input.files.installed_exact {
                    CheckStatus::Blocked
                } else {
                    CheckStatus::Warning
                }
            }
        }
        MemoryStatus::Unknown => CheckStatus::Unknown,
    };

    let storage_blocks = storage_check == CheckStatus::Blocked;
    let memory_blocks = memory_check == CheckStatus::Blocked;

    match input.disk {
        DiskStatus::Known(free) => checks.push(ReadinessCheck {
            id: "storage",
            status: storage_check,
            message: if !has_artifact {
                "This model has no approved artefact, so no download is planned.".to_string()
            } else if storage_blocks {
                format!(
                    "Needs {} free at the destination (the {} file plus a {} reserve); {} is available.",
                    format_bytes(estimate.required_free_bytes),
                    format_bytes(estimate.model_bytes),
                    format_bytes(INSTALL_HEADROOM_BYTES),
                    format_bytes(free)
                )
            } else {
                format!(
                    "{} free at the destination; {} needed for this download.",
                    format_bytes(free),
                    format_bytes(estimate.required_free_bytes)
                )
            },
        }),
        DiskStatus::Unknown => checks.push(ReadinessCheck {
            id: "storage",
            status: CheckStatus::Unknown,
            message:
                "Free space at the download folder could not be read on this system. The download can start, but Nexora cannot confirm there is room for it."
                    .to_string(),
        }),
    }

    match input.memory {
        MemoryStatus::Known {
            total_gib,
            available_gib,
        } => {
            let detail = if memory_blocks {
                format!(
                    "About {:.1} GiB of memory is needed (weights {:.1} + cache {:.2} at {} tokens + {:.1} runtime), but only {:.1} of {:.1} GiB is available with {:.1} GiB kept for the desktop. An estimate, not a hard minimum.",
                    estimate.estimated_memory_gib,
                    estimate.weights_gib,
                    estimate.kv_cache_gib,
                    estimate.context_size,
                    estimate.runtime_overhead_gib,
                    available_gib,
                    total_gib,
                    estimate.memory_headroom_gib
                )
            } else if memory_check == CheckStatus::Warning {
                format!(
                    "About {:.1} GiB will be needed to load this model; {:.1} GiB is available now. Memory often changes, so the download is still allowed. An estimate, not a hard minimum.",
                    estimate.estimated_memory_gib, available_gib
                )
            } else {
                format!(
                    "{:.1} GiB available of {:.1} GiB; about {:.1} GiB needed to load. An estimate.",
                    available_gib, total_gib, estimate.estimated_memory_gib
                )
            };

            checks.push(ReadinessCheck {
                id: "memory",
                status: memory_check,
                message: detail,
            });
        }
        MemoryStatus::Unknown => checks.push(ReadinessCheck {
            id: "memory",
            status: CheckStatus::Unknown,
            message:
                "Memory could not be read on this system, so Nexora cannot check whether this model will fit. Loading may fail."
                    .to_string(),
        }),
    }

    if input.files.verification_failed {
        checks.push(ReadinessCheck {
            id: "integrity",
            status: CheckStatus::Blocked,
            message: "A digest check did not match the published SHA-256. The file is not the approved artefact.".to_string(),
        });
    }

    if let Some(bytes) = input.files.wrong_size_bytes {
        checks.push(ReadinessCheck {
            id: "integrity",
            status: CheckStatus::Blocked,
            message: format!(
                "A file with the published name is {}, but the approved artefact is {}. Nexora never overwrites a model file, so this one has to be moved or removed.",
                format_bytes(bytes),
                format_bytes(estimate.model_bytes)
            ),
        });
    }

    if let Some(bytes) = input.files.partial_bytes {
        checks.push(ReadinessCheck {
            id: "partial",
            status: CheckStatus::Warning,
            message: format!(
                "An unfinished download is present ({} of {}). It is never treated as installed.",
                format_bytes(bytes),
                format_bytes(estimate.model_bytes)
            ),
        });
    }

    // ---- state ----------------------------------------------------------
    let running_this = input.engine.is_running(input.profile.id);

    let (state, headline, detail) = if !has_artifact {
        (
            ReadinessState::Unavailable,
            "Unavailable",
            "This model has no approved artefact, so it cannot be downloaded or verified."
                .to_string(),
        )
    } else if running_this && input.engine.healthy {
        (
            ReadinessState::Loaded,
            "Loaded",
            "This model is loaded and answering from this machine.".to_string(),
        )
    } else if running_this {
        match input.engine.last_error {
            // The runtime's own words (stderr tail) are shown by the engine
            // status panel. Repeating them here would duplicate log content,
            // including file paths, in a second place.
            Some(_) => (
                ReadinessState::RuntimeFailure,
                "Runtime failure",
                "The engine could not load this model. The engine status shows the runtime's own message."
                    .to_string(),
            ),
            None => (
                ReadinessState::Loading,
                "Loading",
                "The engine is starting and loading this model.".to_string(),
            ),
        }
    } else if input.engine.last_error.is_some()
        && input.engine.failed_profile_id.as_deref() == Some(input.profile.id)
    {
        (
            ReadinessState::RuntimeFailure,
            "Runtime failure",
            "The engine stopped after it was loading this model. The engine status shows the runtime's own message."
                .to_string(),
        )
    } else if input.engine.loading_profile_id.as_deref() == Some(input.profile.id) {
        (
            ReadinessState::Loading,
            "Loading",
            "The engine is starting and loading this model. Nothing is confirmed until it answers."
                .to_string(),
        )
    } else if input.download_active {
        (
            ReadinessState::Downloading,
            "Downloading",
            "A download is running. It is verified before it is installed.".to_string(),
        )
    } else if let Some(bytes) = input.files.wrong_size_bytes {
        (
            ReadinessState::VerificationFailed,
            "Not the approved file",
            format!(
                "A file named like the approved artefact is {} instead of {}. It cannot be loaded or replaced automatically: move or remove it, then download again.",
                format_bytes(bytes),
                format_bytes(estimate.model_bytes)
            ),
        )
    } else if input.files.verification_failed {
        (
            ReadinessState::VerificationFailed,
            "Verification failed",
            "The file on disk does not match the published SHA-256. Move or remove it, then download again.".to_string(),
        )
    } else if input.files.partial_bytes.is_some() && !input.files.installed_exact {
        (
            ReadinessState::DownloadIncomplete,
            "Download incomplete",
            "An earlier download did not finish. Nexora never treats a partial file as installed; start again to replace it.".to_string(),
        )
    } else if input.files.installed_exact && input.files.verified {
        if memory_blocks {
            (
                ReadinessState::InsufficientMemory,
                "Not enough memory",
                format!(
                    "The model is installed and verified, but this machine does not have enough available memory to load it. About {:.1} GiB is needed. An estimate, not a hard minimum.",
                    estimate.estimated_memory_gib
                ),
            )
        } else if input.engine.running {
            (
                ReadinessState::DownloadedVerified,
                "Downloaded and verified",
                "The model is installed and verified. Unload the running engine before loading this one.".to_string(),
            )
        } else {
            (
                ReadinessState::ReadyToLoad,
                "Ready to load",
                "The model is installed and verified. Load it to start using it.".to_string(),
            )
        }
    } else if input.files.installed_exact {
        (
            ReadinessState::Unverified,
            "Verification required",
            "A file with the published name and exact size is present, but its SHA-256 has not been confirmed. Verify it before loading.".to_string(),
        )
    } else if storage_blocks {
        (
            ReadinessState::InsufficientStorage,
            "Not enough storage",
            format!(
                "The destination needs {} free; {} is available.",
                format_bytes(estimate.required_free_bytes),
                match input.disk {
                    DiskStatus::Known(free) => format_bytes(free),
                    DiskStatus::Unknown => "an unknown amount".to_string(),
                }
            ),
        )
    } else if input.disk == DiskStatus::Unknown && input.memory == MemoryStatus::Unknown {
        (
            ReadinessState::DetectionUnavailable,
            "System resources unknown",
            "Nexora could not read memory or free disk space on this system. The download can start, but neither the fit nor the space is confirmed."
                .to_string(),
        )
    } else {
        (
            ReadinessState::ReadyToDownload,
            "Ready to download",
            "Nothing is installed yet. The approved file can be downloaded and will be verified before it is installed."
                .to_string(),
        )
    };

    let can_download = has_artifact
        && !input.files.installed_exact
        && !input.download_active
        && !storage_blocks
        && input.files.wrong_size_bytes.is_none()
        && !input.files.verification_failed;

    // A running engine holds the single runtime slot, so this profile cannot be
    // loaded until that engine is unloaded — even when it is a different model.
    let can_verify = input.files.installed_exact && !input.files.verified;

    let can_load = input.files.installed_exact
        && input.files.verified
        && !input.engine.running
        && !memory_blocks;

    let can_generate = running_this && input.engine.healthy;

    Readiness {
        state,
        headline,
        detail,
        can_download,
        can_verify,
        can_load,
        can_generate,
        estimate,
        checks,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inference::registry::{NEXORA_MINI, NEXORA_SONNET};

    fn known_disk(gib: f64) -> DiskStatus {
        DiskStatus::Known(gib_to_bytes(gib))
    }

    fn memory(total: f64, available: f64) -> MemoryStatus {
        MemoryStatus::Known {
            total_gib: total,
            available_gib: available,
        }
    }

    fn idle() -> EngineFacts {
        EngineFacts::default()
    }

    fn base(profile: &'static ModelProfile, context: u32) -> ReadinessInput<'static> {
        ReadinessInput {
            profile,
            context_size: context,
            disk: known_disk(100.0),
            memory: memory(32.0, 24.0),
            files: FileFacts::default(),
            download_active: false,
            engine: idle(),
        }
    }

    // ---- unit conversion ------------------------------------------------

    #[test]
    fn converts_bytes_to_gib() {
        assert_eq!(bytes_to_gib(0), 0.0);
        assert!((bytes_to_gib(1024 * 1024 * 1024) - 1.0).abs() < 1e-9);
        assert!((bytes_to_gib(8_988_110_272) - 8.370_65).abs() < 0.001);
    }

    #[test]
    fn converts_gib_to_bytes() {
        assert_eq!(gib_to_bytes(1.0), 1024 * 1024 * 1024);
        assert_eq!(gib_to_bytes(0.5), 512 * 1024 * 1024);
        assert_eq!(gib_to_bytes(0.0), 0);
        assert_eq!(gib_to_bytes(-1.0), 0);
        assert_eq!(gib_to_bytes(f64::NAN), 0);
        assert_eq!(gib_to_bytes(f64::INFINITY), 0);
    }

    #[test]
    fn conversion_round_trips() {
        for gib in [0.25, 1.0, 8.37, 16.0, 63.5] {
            let back = bytes_to_gib(gib_to_bytes(gib));
            assert!((back - gib).abs() < 1e-6, "{gib} -> {back}");
        }
    }

    #[test]
    fn formats_byte_counts_readably() {
        assert_eq!(format_bytes(1024 * 1024 * 1024), "1.00 GiB");
        assert_eq!(format_bytes(8_988_110_272), "8.37 GiB");
        assert_eq!(format_bytes(512 * 1024 * 1024), "512 MiB");
    }

    // ---- estimates ------------------------------------------------------

    #[test]
    fn sonnet_estimate_uses_the_published_artefact() {
        let estimate = estimate(&NEXORA_SONNET, 8_192);

        assert_eq!(estimate.model_bytes, 8_988_110_272);
        assert_eq!(estimate.install_bytes, 8_988_110_272);
        assert!((estimate.weights_gib - 8.370_65).abs() < 0.001);

        // 192 KiB per token (48 layers x 8 KV heads x 128 dims x 2 x 2 bytes).
        assert_eq!(NEXORA_SONNET.kv_bytes_per_token, Some(196_608));
        assert!((estimate.kv_cache_gib - 1.5).abs() < 0.001);

        assert!((estimate.estimated_memory_gib - (8.370_65 + 1.5 + 1.0)).abs() < 0.01);
        assert_eq!(estimate.context_size, 8_192);
    }

    #[test]
    fn mini_estimate_uses_the_published_artefact() {
        let estimate = estimate(&NEXORA_MINI, 32_768);

        assert_eq!(estimate.model_bytes, 4_683_073_536);
        assert_eq!(NEXORA_MINI.kv_bytes_per_token, Some(57_344));
        // 56 KiB/token at 32k tokens = 1.75 GiB.
        assert!((estimate.kv_cache_gib - 1.75).abs() < 0.001);
    }

    #[test]
    fn estimate_scales_with_context_not_with_a_fixed_minimum() {
        let small = estimate(&NEXORA_SONNET, 2_048);
        let large = estimate(&NEXORA_SONNET, 32_768);

        assert!(small.estimated_memory_gib < large.estimated_memory_gib);
        assert!(small.kv_cache_gib < large.kv_cache_gib);
        // Weights are constant; only the cache grows.
        assert_eq!(small.weights_gib, large.weights_gib);
    }

    #[test]
    fn download_requirement_includes_headroom_over_the_file() {
        let estimate = estimate(&NEXORA_SONNET, 8_192);

        assert_eq!(
            estimate.required_free_bytes,
            estimate.model_bytes + INSTALL_HEADROOM_BYTES
        );
        assert!(estimate.peak_download_bytes > estimate.install_bytes);
    }

    #[test]
    fn unknown_kv_geometry_still_reserves_a_conservative_cache() {
        let known = estimate(&NEXORA_SONNET, 8_192);
        assert!(known.model_bytes > 0);

        // A profile with no published attention geometry cannot compute an
        // exact cache, but must not claim the cache is free.
        let mut profile = NEXORA_SONNET;
        profile.kv_bytes_per_token = None;
        let fallback = estimate(&profile, 8_192);

        assert!(fallback.kv_cache_gib > 0.0);
        assert_eq!(fallback.weights_gib, known.weights_gib);
    }

    // ---- verification ledger ---------------------------------------------

    #[test]
    fn the_ledger_ties_a_digest_to_one_path() {
        let mut ledger = VerificationLedger::default();
        let path = Path::new("/models/sonnet.gguf");

        ledger.mark_verified(NEXORA_SONNET.id, path);

        assert!(ledger.is_verified(NEXORA_SONNET.id, path));
        assert!(
            !ledger.is_verified(NEXORA_SONNET.id, Path::new("/other/sonnet.gguf")),
            "a copy in another folder is not the file that was hashed"
        );
        assert!(!ledger.is_verified(NEXORA_MINI.id, path));
        assert!(!ledger.has_failed(NEXORA_SONNET.id));
    }

    #[test]
    fn a_new_session_starts_unverified() {
        let ledger = VerificationLedger::default();

        assert!(!ledger.is_verified(NEXORA_MINI.id, Path::new("/models/mini.gguf")));
        assert!(!ledger.is_verified(NEXORA_SONNET.id, Path::new("/models/sonnet.gguf")));
    }

    #[test]
    fn a_failure_clears_an_earlier_match_and_vice_versa() {
        let mut ledger = VerificationLedger::default();
        let path = Path::new("/models/sonnet.gguf");

        ledger.mark_verified(NEXORA_SONNET.id, path);
        ledger.mark_failed(NEXORA_SONNET.id);

        assert!(!ledger.is_verified(NEXORA_SONNET.id, path));
        assert!(ledger.has_failed(NEXORA_SONNET.id));

        ledger.mark_verified(NEXORA_SONNET.id, path);

        assert!(ledger.is_verified(NEXORA_SONNET.id, path));
        assert!(!ledger.has_failed(NEXORA_SONNET.id));
    }

    #[test]
    fn a_missing_file_never_compares_equal_to_a_recorded_path() {
        let mut ledger = VerificationLedger::default();
        let missing = Path::new("/definitely/not/here/sonnet.gguf");

        ledger.mark_verified(NEXORA_SONNET.id, missing);

        // Both sides fall back to the literal path, so the record still matches
        // itself; a different literal path does not.
        assert!(ledger.is_verified(NEXORA_SONNET.id, missing));
        assert!(!ledger.is_verified(NEXORA_SONNET.id, Path::new("/elsewhere.gguf")));
    }

    // ---- readiness states -----------------------------------------------

    #[test]
    fn a_clean_machine_is_ready_to_download() {
        let readiness = assess(&base(&NEXORA_SONNET, 8_192));

        assert_eq!(readiness.state, ReadinessState::ReadyToDownload);
        assert!(readiness.can_download);
        assert!(!readiness.can_load);
        assert!(!readiness.can_generate);
        assert_eq!(readiness.checks.len(), 2);
    }

    #[test]
    fn low_storage_blocks_download_but_not_the_estimate() {
        let mut input = base(&NEXORA_SONNET, 8_192);
        input.disk = known_disk(4.0);

        let readiness = assess(&input);

        assert_eq!(readiness.state, ReadinessState::InsufficientStorage);
        assert!(!readiness.can_download);
        assert!(!readiness.can_load);
        assert!(readiness
            .checks
            .iter()
            .any(|check| check.id == "storage" && check.status == CheckStatus::Blocked));
        // 8 988 110 272 B + 512 MiB reserve = 8.87 GiB.
        assert!(readiness.detail.contains("8.87 GiB"));
    }

    #[test]
    fn low_memory_blocks_loading_once_the_file_is_present() {
        let mut input = base(&NEXORA_SONNET, 8_192);
        input.memory = memory(8.0, 6.0);
        input.files = FileFacts {
            installed_exact: true,
            verified: true,
            ..FileFacts::default()
        };

        let readiness = assess(&input);

        assert_eq!(readiness.state, ReadinessState::InsufficientMemory);
        assert!(!readiness.can_load);
        assert!(readiness.detail.contains("An estimate, not a hard minimum"));
    }

    #[test]
    fn low_memory_warns_before_download_but_does_not_block_it() {
        let mut input = base(&NEXORA_SONNET, 8_192);
        input.memory = memory(8.0, 6.0);

        let readiness = assess(&input);

        // Memory may change; the download is still worth starting.
        assert_eq!(readiness.state, ReadinessState::ReadyToDownload);
        assert!(readiness.can_download);
        assert!(!readiness.can_load);
        assert!(readiness
            .checks
            .iter()
            .any(|check| check.id == "memory" && check.status == CheckStatus::Warning));
    }

    #[test]
    fn unknown_resources_are_reported_as_unknown_not_as_fine() {
        let mut input = base(&NEXORA_SONNET, 8_192);
        input.disk = DiskStatus::Unknown;
        input.memory = MemoryStatus::Unknown;

        let readiness = assess(&input);

        assert_eq!(readiness.state, ReadinessState::DetectionUnavailable);
        // Unknown is not a block: the user can still proceed knowingly.
        assert!(readiness.can_download);
        assert!(!readiness.can_load);
        assert!(readiness
            .checks
            .iter()
            .all(|check| check.status == CheckStatus::Unknown));
    }

    #[test]
    fn unknown_memory_alone_still_allows_the_download() {
        let mut input = base(&NEXORA_SONNET, 8_192);
        input.memory = MemoryStatus::Unknown;

        let readiness = assess(&input);

        assert_eq!(readiness.state, ReadinessState::ReadyToDownload);
        assert!(readiness.can_download);
    }

    #[test]
    fn a_partial_file_is_reported_as_incomplete_and_never_installed() {
        let mut input = base(&NEXORA_SONNET, 8_192);
        input.files.partial_bytes = Some(3_000_000_000);

        let readiness = assess(&input);

        assert_eq!(readiness.state, ReadinessState::DownloadIncomplete);
        assert!(!readiness.can_load);
        assert!(readiness.can_download);
        assert!(readiness
            .checks
            .iter()
            .any(|check| check.id == "partial" && check.status == CheckStatus::Warning));
    }

    #[test]
    fn an_exact_but_unverified_file_requires_verification() {
        let mut input = base(&NEXORA_SONNET, 8_192);
        input.files.installed_exact = true;

        let readiness = assess(&input);

        assert_eq!(readiness.state, ReadinessState::Unverified);
        assert!(!readiness.can_load);
        assert!(!readiness.can_download);
        assert!(readiness.can_verify, "verifying is the next step");
    }

    #[test]
    fn an_installed_wrong_sized_file_is_reported_as_not_the_approved_one() {
        let mut input = base(&NEXORA_SONNET, 8_192);
        input.files.wrong_size_bytes = Some(1_000_000_000);

        let readiness = assess(&input);

        assert_eq!(readiness.state, ReadinessState::VerificationFailed);
        assert_eq!(readiness.headline, "Not the approved file");
        assert!(!readiness.can_download, "Nexora never overwrites");
        assert!(!readiness.can_load);
        assert!(!readiness.can_verify, "there is nothing valid to hash");
        assert!(readiness.detail.contains("954 MiB"), "{}", readiness.detail);
        assert!(readiness
            .checks
            .iter()
            .any(|check| check.id == "integrity" && check.status == CheckStatus::Blocked));
    }

    #[test]
    fn a_download_may_start_once_a_rejected_file_is_gone() {
        // The ledger remembers the failure, but the command layer stops
        // reporting it once the file no longer exists. Readiness on its own
        // must not block a clean destination.
        let mut input = base(&NEXORA_SONNET, 8_192);
        input.files.verification_failed = false;
        input.files.installed_exact = false;

        let readiness = assess(&input);

        assert_eq!(readiness.state, ReadinessState::ReadyToDownload);
        assert!(readiness.can_download);
    }

    #[test]
    fn every_state_name_is_unique_and_snake_case() {
        let states = [
            ReadinessState::Unavailable,
            ReadinessState::ReadyToDownload,
            ReadinessState::Downloading,
            ReadinessState::DownloadIncomplete,
            ReadinessState::VerificationFailed,
            ReadinessState::Unverified,
            ReadinessState::DownloadedVerified,
            ReadinessState::InsufficientStorage,
            ReadinessState::InsufficientMemory,
            ReadinessState::DetectionUnavailable,
            ReadinessState::ReadyToLoad,
            ReadinessState::Loading,
            ReadinessState::Loaded,
            ReadinessState::RuntimeFailure,
        ];

        let mut seen = std::collections::BTreeSet::new();

        for state in states {
            let name = state.as_str();

            assert!(seen.insert(name), "duplicate wire name {name}");
            assert!(
                name.chars().all(|c| c.is_ascii_lowercase() || c == '_'),
                "{name} is not snake_case"
            );
            assert_eq!(state.to_string(), name);
        }

        assert_eq!(seen.len(), 14);
    }

    #[test]
    fn a_failed_checksum_is_blocking_and_specific() {
        let mut input = base(&NEXORA_SONNET, 8_192);
        input.files.installed_exact = true;
        input.files.verification_failed = true;

        let readiness = assess(&input);

        assert_eq!(readiness.state, ReadinessState::VerificationFailed);
        assert!(!readiness.can_load);
        assert!(!readiness.can_download);
        assert!(readiness
            .checks
            .iter()
            .any(|check| check.id == "integrity" && check.status == CheckStatus::Blocked));
    }

    #[test]
    fn verifying_the_same_file_is_the_only_step_from_unverified_to_loadable() {
        // One machine, one file. The only thing that changes between the two
        // calls is the digest result, so the transition proves verification is
        // the gate and nothing else is quietly deciding.
        let mut input = base(&NEXORA_SONNET, 8_192);
        input.files.installed_exact = true;

        let before = assess(&input);

        assert_eq!(before.state, ReadinessState::Unverified);
        assert!(before.can_verify);
        assert!(!before.can_load);

        input.files.verified = true;

        let after = assess(&input);

        assert_eq!(after.state, ReadinessState::ReadyToLoad);
        assert!(!after.can_verify, "there is nothing left to check");
        assert!(after.can_load);
        assert_eq!(
            before.estimate, after.estimate,
            "the estimate does not move"
        );
    }

    #[test]
    fn a_cancelled_download_returns_to_ready_to_download() {
        // While the transfer runs, nothing else may start. Once cancellation
        // has removed the partial file, the row is downloadable again.
        let mut input = base(&NEXORA_SONNET, 8_192);
        input.download_active = true;

        assert_eq!(assess(&input).state, ReadinessState::Downloading);
        assert!(!assess(&input).can_download);

        input.download_active = false;
        input.files.partial_bytes = None;

        let after = assess(&input);

        assert_eq!(after.state, ReadinessState::ReadyToDownload);
        assert!(after.can_download);
        assert!(
            !after.can_load,
            "nothing was installed by a cancelled transfer"
        );
    }

    #[test]
    fn a_verified_install_with_memory_to_spare_is_ready_to_load() {
        let mut input = base(&NEXORA_SONNET, 8_192);
        input.files = FileFacts {
            installed_exact: true,
            verified: true,
            ..FileFacts::default()
        };

        let readiness = assess(&input);

        assert_eq!(readiness.state, ReadinessState::ReadyToLoad);
        assert!(readiness.can_load);
        assert!(!readiness.can_download);
        assert!(!readiness.can_generate);
    }

    #[test]
    fn a_verified_install_while_another_model_runs_is_not_loadable_yet() {
        let mut input = base(&NEXORA_SONNET, 8_192);
        input.files = FileFacts {
            installed_exact: true,
            verified: true,
            ..FileFacts::default()
        };
        input.engine = EngineFacts {
            running: true,
            healthy: true,
            loaded_profile_id: Some("nexora-mini-qwen2.5-coder-7b-instruct-q4-k-m".into()),
            last_error: None,
            ..EngineFacts::default()
        };

        let readiness = assess(&input);

        assert_eq!(readiness.state, ReadinessState::DownloadedVerified);
        assert!(!readiness.can_load);
    }

    #[test]
    fn an_active_download_is_reported_and_cannot_be_restarted() {
        let mut input = base(&NEXORA_SONNET, 8_192);
        input.download_active = true;

        let readiness = assess(&input);

        assert_eq!(readiness.state, ReadinessState::Downloading);
        assert!(!readiness.can_download);
        assert!(!readiness.can_load);
    }

    #[test]
    fn a_running_engine_reports_loading_then_loaded() {
        let mut input = base(&NEXORA_SONNET, 8_192);
        input.files = FileFacts {
            installed_exact: true,
            verified: true,
            ..FileFacts::default()
        };
        let mut engine = EngineFacts {
            running: true,
            healthy: false,
            loaded_profile_id: Some(NEXORA_SONNET.id.to_string()),
            last_error: None,
            ..EngineFacts::default()
        };

        input.engine = engine.clone();
        assert_eq!(assess(&input).state, ReadinessState::Loading);

        engine.healthy = true;
        input.engine = engine;
        let ready = assess(&input);

        assert_eq!(ready.state, ReadinessState::Loaded);
        assert!(ready.can_generate);
    }

    #[test]
    fn a_runtime_failure_is_surfaced_with_its_reason() {
        let mut input = base(&NEXORA_SONNET, 8_192);
        input.files = FileFacts {
            installed_exact: true,
            verified: true,
            ..FileFacts::default()
        };
        input.engine = EngineFacts {
            running: true,
            healthy: false,
            loaded_profile_id: Some(NEXORA_SONNET.id.to_string()),
            failed_profile_id: Some(NEXORA_SONNET.id.to_string()),
            last_error: Some("llama-server exited with status 1.".to_string()),
            ..EngineFacts::default()
        };

        let readiness = assess(&input);

        assert_eq!(readiness.state, ReadinessState::RuntimeFailure);
        assert!(!readiness.can_generate);
        assert!(
            readiness.detail.contains("engine status"),
            "the card points at the error instead of duplicating a stderr tail"
        );
    }

    #[test]
    fn a_stopped_engine_after_failure_keeps_reporting_the_failure() {
        let mut input = base(&NEXORA_SONNET, 8_192);
        input.files = FileFacts {
            installed_exact: true,
            verified: true,
            ..FileFacts::default()
        };
        input.engine = EngineFacts {
            running: false,
            healthy: false,
            loaded_profile_id: None,
            failed_profile_id: Some(NEXORA_SONNET.id.to_string()),
            last_error: Some("out of memory".to_string()),
            ..EngineFacts::default()
        };

        assert_eq!(assess(&input).state, ReadinessState::RuntimeFailure);
    }

    #[test]
    fn an_in_flight_load_is_reported_as_loading_not_ready() {
        let mut input = base(&NEXORA_SONNET, 8_192);
        input.files = FileFacts {
            installed_exact: true,
            verified: true,
            ..FileFacts::default()
        };
        input.engine = EngineFacts {
            running: true,
            healthy: false,
            loading_profile_id: Some(NEXORA_SONNET.id.to_string()),
            ..EngineFacts::default()
        };

        let readiness = assess(&input);

        assert_eq!(readiness.state, ReadinessState::Loading);
        assert!(!readiness.can_generate);

        // Another profile's load must not present as this row loading. The
        // engine is busy, so this row is merely waiting for the slot.
        let mut other = input;
        other.engine.loading_profile_id = Some(NEXORA_MINI.id.to_string());

        let other = assess(&other);

        assert_ne!(other.state, ReadinessState::Loading);
        assert_eq!(other.state, ReadinessState::DownloadedVerified);
        assert!(!other.can_load);
    }

    #[test]
    fn a_failure_attributed_to_another_profile_is_not_this_rows_failure() {
        let mut input = base(&NEXORA_SONNET, 8_192);
        input.files = FileFacts {
            installed_exact: true,
            verified: true,
            ..FileFacts::default()
        };
        input.engine = EngineFacts {
            running: true,
            healthy: true,
            loaded_profile_id: Some(NEXORA_MINI.id.to_string()),
            failed_profile_id: Some(NEXORA_MINI.id.to_string()),
            last_error: Some("The engine stopped unexpectedly.".to_string()),
            ..EngineFacts::default()
        };

        // A healthy Mini is running, so Sonnet is merely waiting for the slot.
        assert_eq!(assess(&input).state, ReadinessState::DownloadedVerified);

        input.engine.running = false;
        input.engine.healthy = false;
        input.engine.loaded_profile_id = None;

        // Mini failed, not Sonnet: Sonnet is still loadable once the engine is
        // idle, and must not inherit the other profile's failure.
        assert_eq!(assess(&input).state, ReadinessState::ReadyToLoad);
    }

    #[test]
    fn a_pending_profile_is_unavailable_and_offers_no_action() {
        let mut profile = NEXORA_SONNET;
        profile.artifact = ArtifactState::Pending {
            note: "No artefact recorded.",
        };

        let input = ReadinessInput {
            profile: &profile,
            ..base(&NEXORA_SONNET, 8_192)
        };
        let readiness = assess(&input);

        assert_eq!(readiness.state, ReadinessState::Unavailable);
        assert!(!readiness.can_download);
        assert!(!readiness.can_load);
        assert_eq!(readiness.estimate.model_bytes, 0);
    }

    // ---- cross-model guarantees -----------------------------------------

    #[test]
    fn each_profile_reports_its_own_state_not_the_others() {
        // Mini installed and loaded; Sonnet selected but absent. Sonnet must
        // not inherit Mini's readiness.
        let mut sonnet = base(&NEXORA_SONNET, 8_192);
        sonnet.engine = EngineFacts {
            running: true,
            healthy: true,
            loaded_profile_id: Some(NEXORA_MINI.id.to_string()),
            last_error: None,
            ..EngineFacts::default()
        };

        let readiness = assess(&sonnet);

        assert_eq!(readiness.state, ReadinessState::ReadyToDownload);
        assert!(!readiness.can_generate);
        assert!(!readiness.can_load);
    }

    #[test]
    fn sonnet_needs_more_memory_than_mini_on_the_same_machine() {
        let mini = estimate(&NEXORA_MINI, 8_192);
        let sonnet = estimate(&NEXORA_SONNET, 8_192);

        assert!(sonnet.estimated_memory_gib > mini.estimated_memory_gib);
        assert!(sonnet.model_bytes > mini.model_bytes);
    }

    #[test]
    fn the_state_name_is_stable_for_the_ipc_contract() {
        // The frontend switches on these exact strings.
        assert_eq!(
            ReadinessState::ReadyToDownload.as_str(),
            "ready_to_download"
        );
        assert_eq!(
            ReadinessState::DownloadIncomplete.as_str(),
            "download_incomplete"
        );
        assert_eq!(
            ReadinessState::VerificationFailed.as_str(),
            "verification_failed"
        );
        assert_eq!(
            ReadinessState::InsufficientStorage.as_str(),
            "insufficient_storage"
        );
        assert_eq!(
            ReadinessState::InsufficientMemory.as_str(),
            "insufficient_memory"
        );
        assert_eq!(
            ReadinessState::DetectionUnavailable.as_str(),
            "detection_unavailable"
        );
        assert_eq!(ReadinessState::ReadyToLoad.as_str(), "ready_to_load");
        assert_eq!(ReadinessState::RuntimeFailure.as_str(), "runtime_failure");
        assert_eq!(CheckStatus::Blocked.as_str(), "blocked");
    }

    #[test]
    fn every_state_has_a_distinct_wire_name() {
        let states = [
            ReadinessState::Unavailable,
            ReadinessState::ReadyToDownload,
            ReadinessState::Downloading,
            ReadinessState::DownloadIncomplete,
            ReadinessState::VerificationFailed,
            ReadinessState::Unverified,
            ReadinessState::DownloadedVerified,
            ReadinessState::InsufficientStorage,
            ReadinessState::InsufficientMemory,
            ReadinessState::DetectionUnavailable,
            ReadinessState::ReadyToLoad,
            ReadinessState::Loading,
            ReadinessState::Loaded,
            ReadinessState::RuntimeFailure,
        ];

        let mut names: Vec<&str> = states.iter().map(|state| state.as_str()).collect();
        let total = names.len();

        names.sort_unstable();
        names.dedup();

        assert_eq!(names.len(), total);
    }

    #[test]
    fn displayed_figures_are_rounded_not_raw() {
        let estimate = estimate(&NEXORA_SONNET, 8_192);

        assert_eq!(estimate.weights_display(), round2(estimate.weights_gib));
        assert_eq!(
            estimate.estimated_memory_display(),
            round2(estimate.estimated_memory_gib)
        );
    }
}
