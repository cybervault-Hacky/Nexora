//! Shared, conservative resource admission and reservation primitives for
//! local media workloads. This module does not probe hardware, start runtimes,
//! load models, or claim that admission makes a capability usable.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::adaptive::{AdaptiveCapabilities, Detection, DetectionConfidence};
use crate::media_models::{self, BackendRequirement, LifecycleState, MediaModelLifecycleSnapshot};

/// Evidence attached to a value. `Unknown` is never represented by zero.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ResourceEvidence<T> {
    Confirmed { value: T, source: String },
    Estimated { value: T, method: String },
    Unknown { reason: String },
}

impl<T> ResourceEvidence<T> {
    fn from_detection(reading: &Detection<T>, subject: &str) -> Self
    where
        T: Clone,
    {
        let detail = reading
            .detail
            .clone()
            .unwrap_or_else(|| format!("No reliable {subject} evidence is available."));
        match (&reading.value, reading.confidence) {
            (Some(value), DetectionConfidence::Confirmed) => Self::Confirmed {
                value: value.clone(),
                source: detail,
            },
            (Some(value), DetectionConfidence::Estimated) => Self::Estimated {
                value: value.clone(),
                method: detail,
            },
            _ => Self::Unknown { reason: detail },
        }
    }

    fn unknown(reason: impl Into<String>) -> Self {
        Self::Unknown {
            reason: reason.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ResourceRequirement<T> {
    Required(ResourceEvidence<T>),
    NotRequired { reason: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum MediaCapability {
    ImageGeneration,
    ImageEditing,
    VideoGeneration,
    VideoEditingEnhancement,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BlenderPreviewKind {
    Viewport,
    Render,
}

/// Data-only Blender workload profile. Request dimensions/settings are known
/// inputs; measured runtime requirements remain explicitly unknown.
#[derive(Debug, Clone)]
pub(crate) struct BlenderWorkloadProfile {
    pub kind: BlenderPreviewKind,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub samples: Option<u16>,
    pub frame_count: u16,
    pub peak_system_memory_bytes: ResourceEvidence<u64>,
    pub peak_gpu_memory_bytes: ResourceEvidence<u64>,
    pub temporary_storage_bytes: ResourceEvidence<u64>,
}

impl BlenderWorkloadProfile {
    pub(crate) fn viewport() -> Self {
        Self::new(BlenderPreviewKind::Viewport, None, None, None)
    }

    pub(crate) fn render(width: u32, height: u32, samples: u16) -> Self {
        Self::new(
            BlenderPreviewKind::Render,
            Some(width),
            Some(height),
            Some(samples),
        )
    }

    fn new(
        kind: BlenderPreviewKind,
        width: Option<u32>,
        height: Option<u32>,
        samples: Option<u16>,
    ) -> Self {
        let reason = "No approved Windows/Blender workload measurements establish peak resource requirements.";
        Self {
            kind,
            width,
            height,
            samples,
            frame_count: 1,
            peak_system_memory_bytes: ResourceEvidence::unknown(reason),
            peak_gpu_memory_bytes: ResourceEvidence::unknown(reason),
            temporary_storage_bytes: ResourceEvidence::unknown(
                "No preview output path or measured temporary-render storage profile is available.",
            ),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BlenderAdmissionStatus {
    Deferred,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BlenderAdmissionDecision {
    pub status: BlenderAdmissionStatus,
    pub reason: String,
    pub active_shared_reservations: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MediaBackend {
    Cpu,
    Cuda,
    DirectMl,
    Vulkan,
}

impl From<BackendRequirement> for MediaBackend {
    fn from(value: BackendRequirement) -> Self {
        match value {
            BackendRequirement::Cpu => Self::Cpu,
            BackendRequirement::Cuda => Self::Cuda,
            BackendRequirement::DirectMl => Self::DirectMl,
            BackendRequirement::Vulkan => Self::Vulkan,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AssetReadiness {
    ManagerReportedValidated,
    NotInstalled,
    DownloadedNotValidated,
    Busy,
    FailedOrIncompatible,
    Unknown,
}

impl AssetReadiness {
    pub(crate) fn from_lifecycle(snapshot: &MediaModelLifecycleSnapshot) -> Self {
        match snapshot.state {
            LifecycleState::InstalledValidated
                if snapshot.manager_reported_downloaded == Some(true)
                    && snapshot.manager_reported_integrity_validated == Some(true) =>
            {
                Self::ManagerReportedValidated
            }
            LifecycleState::InstalledValidated
            | LifecycleState::DownloadedNotValidated
            | LifecycleState::DownloadInterrupted => Self::DownloadedNotValidated,
            LifecycleState::NotInstalled
            | LifecycleState::AwaitingCompatibilityAssessment
            | LifecycleState::ReadyToDownload => Self::NotInstalled,
            LifecycleState::Downloading | LifecycleState::Removing => Self::Busy,
            LifecycleState::Incompatible | LifecycleState::Failed => Self::FailedOrIncompatible,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WorkloadShape {
    Image {
        width: u32,
        height: u32,
        steps: u32,
    },
    Video {
        width: u32,
        height: u32,
        frames: u32,
    },
}

#[derive(Debug, Clone)]
pub(crate) struct WorkloadLimits {
    /// A fixed supported image size is known for the current SD-Turbo adapter.
    pub image_size: Option<(u32, u32)>,
    /// `None` means the adapter has not established a supported step contract.
    pub allowed_image_steps: Option<Vec<u32>>,
    /// Video limits remain absent until an executor establishes them.
    pub max_video_size: Option<(u32, u32)>,
    pub max_video_frames: Option<u32>,
    pub max_concurrent_operations: usize,
    pub max_concurrent_per_model: usize,
}

/// Model/operation profile assembled from the trusted media registry plus
/// operation-specific facts. Weight storage is intentionally separate from
/// system/GPU working-memory requirements.
#[derive(Debug, Clone)]
pub(crate) struct MediaWorkloadProfile {
    pub capability: MediaCapability,
    pub model_id: String,
    pub model_registered: bool,
    pub model_weight_storage_bytes: ResourceEvidence<u64>,
    pub minimum_system_memory_bytes: ResourceEvidence<u64>,
    pub recommended_system_memory_bytes: ResourceEvidence<u64>,
    pub minimum_gpu_memory_bytes: ResourceRequirement<u64>,
    /// An adapter-enforced temporary/output-storage floor, not a measured peak.
    pub temporary_disk_bytes: ResourceEvidence<u64>,
    pub minimum_available_memory_screening_floor_bytes: ResourceEvidence<u64>,
    pub required_backend: ResourceEvidence<MediaBackend>,
    pub workload_limits: WorkloadLimits,
}

impl MediaWorkloadProfile {
    /// Builds an image-generation profile from a registered model definition.
    /// Missing registry values stay unknown; no defaults are inferred.
    pub(crate) fn registered_image_generation(
        model_id: &str,
        temporary_disk_floor_bytes: u64,
        available_memory_screening_floor_bytes: u64,
        image_size: (u32, u32),
        allowed_steps: Vec<u32>,
    ) -> Self {
        let definition = media_models::registered_definition(model_id);
        let (registered, weight_bytes, min_ram, recommended_ram, min_vram, backend) = match definition {
            Some(definition) => (
                true,
                definition.expected_total_size_bytes.map(|value| ResourceEvidence::Confirmed {
                    value,
                    source: "Exact required-artifact total in the pinned Phase 2 media-model registry.".to_string(),
                }).unwrap_or_else(|| ResourceEvidence::unknown("The registered model has no exact total artifact size.")),
                registry_requirement(definition.requirements.minimum_ram_bytes, "No model-specific minimum system-memory requirement is established in the media-model registry."),
                registry_requirement(definition.requirements.recommended_ram_bytes, "No model-specific recommended system-memory requirement is established in the media-model registry."),
                definition.requirements.minimum_vram_bytes.map(|value| ResourceRequirement::Required(ResourceEvidence::Confirmed {
                    value,
                    source: "Model-specific minimum GPU-memory requirement recorded in the reviewed media-model registry.".to_string(),
                })).unwrap_or_else(|| ResourceRequirement::Required(ResourceEvidence::unknown(
                    "No model-specific GPU-memory requirement is established in the media-model registry.",
                ))),
                definition.requirements.required_backend.map(|backend| ResourceEvidence::Confirmed {
                    value: backend.into(),
                    source: "Required backend recorded in the registered media-model definition.".to_string(),
                }).unwrap_or_else(|| ResourceEvidence::unknown("No required backend is established in the media-model registry.")),
            ),
            None => (
                false,
                ResourceEvidence::unknown("The model ID is not present in the trusted media-model registry."),
                ResourceEvidence::unknown("The model is not registered; system-memory requirements are unavailable."),
                ResourceEvidence::unknown("The model is not registered; recommended memory is unavailable."),
                ResourceRequirement::Required(ResourceEvidence::unknown("The model is not registered; GPU-memory requirements are unavailable.")),
                ResourceEvidence::unknown("The model is not registered; required backend is unavailable."),
            ),
        };

        Self {
            capability: MediaCapability::ImageGeneration,
            model_id: model_id.to_string(),
            model_registered: registered,
            model_weight_storage_bytes: weight_bytes,
            minimum_system_memory_bytes: min_ram,
            recommended_system_memory_bytes: recommended_ram,
            // The existing SD-Turbo request explicitly selects CPU. This says
            // GPU memory is not budgeted by this invocation; it is not runtime
            // compatibility evidence and does not establish CPU memory fit.
            minimum_gpu_memory_bytes: if backend_is_cpu(&backend) {
                ResourceRequirement::NotRequired {
                    reason: "The current adapter requests CPU execution only; this does not confirm that a runtime honors or supports that request.".to_string(),
                }
            } else {
                min_vram
            },
            temporary_disk_bytes: ResourceEvidence::Confirmed {
                value: temporary_disk_floor_bytes,
                source: "The existing image adapter's enforced free-space reserve for staging and bounded output.".to_string(),
            },
            minimum_available_memory_screening_floor_bytes: ResourceEvidence::Confirmed {
                value: available_memory_screening_floor_bytes,
                source: "The Phase 3 lower-bound screen equals the registered raw checkpoint size; it is not a complete runtime-memory requirement or a sufficient-fit estimate.".to_string(),
            },
            required_backend: backend,
            workload_limits: WorkloadLimits {
                image_size: Some(image_size),
                allowed_image_steps: Some(allowed_steps),
                max_video_size: None,
                max_video_frames: None,
                max_concurrent_operations: 1,
                max_concurrent_per_model: 1,
            },
        }
    }
}

fn registry_requirement(value: Option<u64>, unknown_reason: &str) -> ResourceEvidence<u64> {
    match value {
        Some(value) => ResourceEvidence::Confirmed {
            value,
            source: "Model-specific requirement recorded in the reviewed media-model registry."
                .to_string(),
        },
        None => ResourceEvidence::unknown(unknown_reason),
    }
}

fn backend_is_cpu(backend: &ResourceEvidence<MediaBackend>) -> bool {
    matches!(
        backend,
        ResourceEvidence::Confirmed {
            value: MediaBackend::Cpu,
            ..
        }
    )
}

#[derive(Debug, Clone)]
pub(crate) struct HostResourceSnapshot {
    pub total_system_memory_bytes: ResourceEvidence<u64>,
    pub available_system_memory_bytes: ResourceEvidence<u64>,
    pub dedicated_gpu_memory_bytes: ResourceEvidence<u64>,
    pub model_storage_free_bytes: ResourceEvidence<u64>,
    pub temporary_storage_free_bytes: ResourceEvidence<u64>,
    pub sampled_at_unix_ms: Option<u64>,
    pub age_ms: Option<u64>,
}

impl HostResourceSnapshot {
    /// Converts the existing Phase 1 contract. This performs no probes or file
    /// scans; callers supply the already-collected output-volume reading.
    pub(crate) fn from_adaptive(
        capabilities: &AdaptiveCapabilities,
        temporary_storage_free_bytes: Option<u64>,
    ) -> Self {
        let sampled_at_unix_ms = capabilities.hardware.refreshed_at_unix_ms;
        let age_ms = sampled_at_unix_ms
            .and_then(|sampled| unix_millis().and_then(|now| now.checked_sub(sampled)));
        let temporary_storage_free_bytes = match temporary_storage_free_bytes {
            Some(value) => ResourceEvidence::Confirmed {
                value,
                source: "Free bytes reported for the image-output volume by the existing Phase 1 disk-space interface.".to_string(),
            },
            None => ResourceEvidence::unknown("Free space for the image-output volume could not be read."),
        };
        Self {
            total_system_memory_bytes: ResourceEvidence::from_detection(
                &capabilities.hardware.memory.total_physical_bytes,
                "total system memory",
            ),
            available_system_memory_bytes: ResourceEvidence::from_detection(
                &capabilities.hardware.memory.available_physical_bytes,
                "available system memory",
            ),
            dedicated_gpu_memory_bytes: ResourceEvidence::from_detection(
                &capabilities.resources.dedicated_vram_bytes,
                "dedicated GPU memory",
            ),
            model_storage_free_bytes: ResourceEvidence::from_detection(
                &capabilities.resources.model_storage_free_bytes,
                "model-storage free space",
            ),
            temporary_storage_free_bytes,
            sampled_at_unix_ms,
            age_ms,
        }
    }

    fn fresh(&self) -> bool {
        // A short freshness window is an application admission policy, not a
        // hardware requirement. Missing/future timestamps fail closed.
        self.age_ms
            .is_some_and(|age| age <= RESOURCE_SNAPSHOT_MAX_AGE_MS)
    }
}

const RESOURCE_SNAPSHOT_MAX_AGE_MS: u64 = 30_000;
const MAX_ACTIVE_MEDIA_INFERENCE: usize = 1;

#[derive(Debug, Clone)]
pub(crate) enum RuntimeAvailabilityEvidence {
    Available {
        runtime: String,
        version_pinned: bool,
    },
    Unavailable {
        reason: String,
    },
    Unknown {
        reason: String,
    },
}

#[derive(Debug, Clone)]
pub(crate) enum RuntimeCompatibilityEvidence {
    ConfirmedCompatible { evidence: String },
    ConfirmedIncompatible { reason: String },
    Unknown { reason: String },
}

#[derive(Debug, Clone)]
pub(crate) struct AdmissionContext {
    pub assets: AssetReadiness,
    pub runtime_availability: RuntimeAvailabilityEvidence,
    pub runtime_compatibility: RuntimeCompatibilityEvidence,
    pub host: HostResourceSnapshot,
    pub workload: WorkloadShape,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AdmissionStatus {
    AllowedByKnownConstraints,
    Rejected,
    Deferred,
    Unknown,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AdmissionReasonCode {
    ModelNotRegistered,
    ModelAssetsUnavailable,
    ModelAssetsNotValidated,
    ModelLifecycleUnknown,
    ModelOperationBusy,
    RuntimeUnavailable,
    RuntimeAvailabilityUnknown,
    RuntimeIncompatible,
    RuntimeCompatibilityUnknown,
    UnsupportedWorkload,
    WorkloadLimitsUnknown,
    InsufficientSystemMemory,
    CheckpointSizeScreeningFloor,
    InsufficientGpuMemory,
    InsufficientTemporaryDisk,
    ResourceRequirementUnknown,
    HostResourceUnknown,
    ResourceSnapshotStale,
    RequiredBackendUnknown,
    ConcurrencyLimit,
    DuplicateOperationId,
    ReservationStateUnavailable,
    ReservationTokenExhausted,
}

#[derive(Debug, Clone)]
pub(crate) struct AdmissionReason {
    pub code: AdmissionReasonCode,
    pub detail: String,
}

#[derive(Debug, Clone)]
pub(crate) struct AdmissionDecision {
    pub status: AdmissionStatus,
    pub reasons: Vec<AdmissionReason>,
    pub resource_snapshot_age_ms: Option<u64>,
}

impl AdmissionDecision {
    fn new(status: AdmissionStatus, reasons: Vec<AdmissionReason>, age_ms: Option<u64>) -> Self {
        Self {
            status,
            reasons,
            resource_snapshot_age_ms: age_ms,
        }
    }

    pub(crate) fn user_message(&self) -> String {
        let detail = self
            .reasons
            .iter()
            .map(|reason| reason.detail.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        let age = self
            .resource_snapshot_age_ms
            .map(|age| format!(" The Phase 1 resource snapshot was {age} ms old at admission."))
            .unwrap_or_else(|| " The age of the Phase 1 resource snapshot is unknown.".to_string());
        match self.status {
            AdmissionStatus::Rejected => format!("Local media workload rejected: {detail} No media process was started.{age}"),
            AdmissionStatus::Deferred => format!("Local media workload deferred: {detail} No media process was started.{age}"),
            AdmissionStatus::Unknown => format!("Local media workload admission is unknown: {detail} No media process was started.{age}"),
            AdmissionStatus::Unavailable => format!("Local media workload is unavailable: {detail} No media process was started.{age}"),
            AdmissionStatus::AllowedByKnownConstraints => format!("Known resource constraints permit this workload; this does not guarantee runtime compatibility or successful inference.{age}"),
        }
    }
}

#[derive(Debug)]
pub(crate) enum ReservationOutcome {
    Reserved {
        decision: AdmissionDecision,
        reservation: ResourceReservation,
    },
    Denied(AdmissionDecision),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct ResourceOperationId(String);

impl ResourceOperationId {
    /// Accepts only the native generation manager's 128-bit random hex ID.
    pub(crate) fn from_native_id(value: &str) -> Option<Self> {
        (value.len() == 32 && value.bytes().all(|byte| byte.is_ascii_hexdigit()))
            .then(|| Self(value.to_ascii_lowercase()))
    }
}

#[derive(Debug)]
struct ActiveReservation {
    token: u64,
    capability: MediaCapability,
    model_id: String,
}

#[derive(Debug, Default)]
struct ReservationState {
    next_token: u64,
    active: HashMap<ResourceOperationId, ActiveReservation>,
}

/// Cloneable shared coordinator. Active reservations are in-memory, per-app
/// state; no scheduler, worker, persistence, or UI-visible mutable state exists.
#[derive(Debug, Clone, Default)]
pub(crate) struct MediaResourceCoordinator {
    state: Arc<Mutex<ReservationState>>,
}

#[derive(Debug)]
pub(crate) struct ResourceReservation {
    state: Arc<Mutex<ReservationState>>,
    operation_id: ResourceOperationId,
    token: u64,
}

impl Drop for ResourceReservation {
    fn drop(&mut self) {
        // Recover a poisoned lock only to remove this exact lease. A stale guard
        // cannot release a newer reservation with the same opaque operation ID.
        let mut state = match self.state.lock() {
            Ok(state) => state,
            Err(poisoned) => poisoned.into_inner(),
        };
        if state
            .active
            .get(&self.operation_id)
            .is_some_and(|active| active.token == self.token)
        {
            state.active.remove(&self.operation_id);
        }
    }
}

impl MediaResourceCoordinator {
    /// Read-only admission screen for future Blender previews. It shares the
    /// media inference reservation mutex but never reserves or starts work.
    /// Until Blender runtime/peak-resource evidence is measured, idle capacity
    /// remains `Unknown`, not admitted.
    pub(crate) fn assess_blender_preview(
        &self,
        profile: &BlenderWorkloadProfile,
    ) -> BlenderAdmissionDecision {
        let state = match self.state.lock() {
            Ok(state) => state,
            Err(_) => {
                return BlenderAdmissionDecision {
                    status: BlenderAdmissionStatus::Unknown,
                    reason: "Shared workload reservation state is unavailable.".to_string(),
                    active_shared_reservations: 0,
                };
            }
        };
        let active_shared_reservations = state.active.len();
        if active_shared_reservations > 0 {
            return BlenderAdmissionDecision {
                status: BlenderAdmissionStatus::Deferred,
                reason: "A local media workload currently holds the shared resource reservation; preview admission is deferred.".to_string(),
                active_shared_reservations,
            };
        }
        let resource_evidence_unknown = matches!(
            &profile.peak_system_memory_bytes,
            ResourceEvidence::Unknown { .. }
        ) || matches!(
            &profile.peak_gpu_memory_bytes,
            ResourceEvidence::Unknown { .. }
        ) || matches!(
            &profile.temporary_storage_bytes,
            ResourceEvidence::Unknown { .. }
        );
        let workload_metadata_known = profile.frame_count == 1
            && match profile.kind {
                BlenderPreviewKind::Viewport => {
                    profile.width.is_none() && profile.height.is_none() && profile.samples.is_none()
                }
                BlenderPreviewKind::Render => {
                    profile.width.is_some() && profile.height.is_some() && profile.samples.is_some()
                }
            };
        BlenderAdmissionDecision {
            status: BlenderAdmissionStatus::Unknown,
            reason: if resource_evidence_unknown || !workload_metadata_known {
                "Blender peak memory, runtime compatibility, and supported-hardware evidence are not established; no workload is admitted.".to_string()
            } else {
                "Blender resource measurements and runtime compatibility have not been approved; no workload is admitted.".to_string()
            },
            active_shared_reservations,
        }
    }

    /// Non-reserving preview for existing capability status. The result is a
    /// point-in-time assessment only; a request must call `reserve` again.
    pub(crate) fn assess(
        &self,
        profile: &MediaWorkloadProfile,
        context: &AdmissionContext,
    ) -> AdmissionDecision {
        let decision = evaluate(profile, context);
        if decision.status != AdmissionStatus::AllowedByKnownConstraints {
            return decision;
        }
        let state = match self.state.lock() {
            Ok(state) => state,
            Err(_) => return AdmissionDecision::new(
                AdmissionStatus::Unavailable,
                vec![reason(AdmissionReasonCode::ReservationStateUnavailable, "Shared media reservation state is unavailable; capacity cannot be assessed safely.")],
                context.host.age_ms,
            ),
        };
        if capacity_exhausted(&state, profile) {
            return AdmissionDecision::new(
                AdmissionStatus::Deferred,
                vec![reason(AdmissionReasonCode::ConcurrencyLimit, "The shared media inference limit is one active operation; retry after the current workload releases its reservation.")],
                context.host.age_ms,
            );
        }
        decision
    }

    /// Evaluate conservative limits and atomically reserve capacity only when
    /// every required fact is established. `operation_id` is created by the
    /// native operation owner, never supplied by the renderer.
    pub(crate) fn reserve(
        &self,
        operation_id: ResourceOperationId,
        profile: &MediaWorkloadProfile,
        context: &AdmissionContext,
    ) -> ReservationOutcome {
        let decision = evaluate(profile, context);
        if decision.status != AdmissionStatus::AllowedByKnownConstraints {
            return ReservationOutcome::Denied(decision);
        }

        let mut state = match self.state.lock() {
            Ok(state) => state,
            Err(_) => {
                return ReservationOutcome::Denied(AdmissionDecision::new(
                    AdmissionStatus::Unavailable,
                    vec![reason(
                        AdmissionReasonCode::ReservationStateUnavailable,
                        "Shared media reservation state is unavailable; no workload was admitted.",
                    )],
                    context.host.age_ms,
                ))
            }
        };
        if state.active.contains_key(&operation_id) {
            return ReservationOutcome::Denied(AdmissionDecision::new(
                AdmissionStatus::Deferred,
                vec![reason(
                    AdmissionReasonCode::DuplicateOperationId,
                    "This operation identifier already owns a media reservation.",
                )],
                context.host.age_ms,
            ));
        }
        if capacity_exhausted(&state, profile) {
            return ReservationOutcome::Denied(AdmissionDecision::new(
                AdmissionStatus::Deferred,
                vec![reason(AdmissionReasonCode::ConcurrencyLimit, "The shared media inference limit is one active operation; retry after the current workload releases its reservation.")],
                context.host.age_ms,
            ));
        }
        let Some(token) = state.next_token.checked_add(1) else {
            return ReservationOutcome::Denied(AdmissionDecision::new(
                AdmissionStatus::Unavailable,
                vec![reason(
                    AdmissionReasonCode::ReservationTokenExhausted,
                    "A unique reservation token could not be allocated; no workload was admitted.",
                )],
                context.host.age_ms,
            ));
        };
        state.next_token = token;
        state.active.insert(
            operation_id.clone(),
            ActiveReservation {
                token,
                capability: profile.capability,
                model_id: profile.model_id.clone(),
            },
        );
        drop(state);

        ReservationOutcome::Reserved {
            decision,
            reservation: ResourceReservation {
                state: self.state.clone(),
                operation_id,
                token,
            },
        }
    }
}

fn capacity_exhausted(state: &ReservationState, profile: &MediaWorkloadProfile) -> bool {
    let capability_active = state
        .active
        .values()
        .filter(|active| active.capability == profile.capability)
        .count();
    let model_active = state
        .active
        .values()
        .filter(|active| active.model_id == profile.model_id)
        .count();
    state.active.len() >= MAX_ACTIVE_MEDIA_INFERENCE
        || capability_active >= profile.workload_limits.max_concurrent_operations
        || model_active >= profile.workload_limits.max_concurrent_per_model
}

fn evaluate(profile: &MediaWorkloadProfile, context: &AdmissionContext) -> AdmissionDecision {
    let age = context.host.age_ms;
    if !profile.model_registered {
        return decision(
            AdmissionStatus::Unavailable,
            AdmissionReasonCode::ModelNotRegistered,
            "The requested model ID is not registered in the Phase 2 media-model manager.",
            age,
        );
    }
    match context.assets {
        AssetReadiness::NotInstalled => {
            return decision(
                AdmissionStatus::Unavailable,
                AdmissionReasonCode::ModelAssetsUnavailable,
                "The registered model has no manager-reported complete installation.",
                age,
            )
        }
        AssetReadiness::DownloadedNotValidated => {
            return decision(
                AdmissionStatus::Unavailable,
                AdmissionReasonCode::ModelAssetsNotValidated,
                "Downloaded assets are not manager-recorded as integrity-validated.",
                age,
            )
        }
        AssetReadiness::Busy => {
            return decision(
                AdmissionStatus::Deferred,
                AdmissionReasonCode::ModelOperationBusy,
                "The media-model manager is downloading, removing, or otherwise using this model.",
                age,
            )
        }
        AssetReadiness::FailedOrIncompatible => {
            return decision(
                AdmissionStatus::Unavailable,
                AdmissionReasonCode::ModelAssetsUnavailable,
                "The media-model manager reports failed or incompatible model assets.",
                age,
            )
        }
        AssetReadiness::Unknown => {
            return decision(
                AdmissionStatus::Unknown,
                AdmissionReasonCode::ModelLifecycleUnknown,
                "The model installation lifecycle could not be established.",
                age,
            )
        }
        AssetReadiness::ManagerReportedValidated => {}
    }

    let mut rejected = Vec::new();
    let mut unknown = Vec::new();
    match &context.runtime_availability {
        RuntimeAvailabilityEvidence::Unavailable { reason: detail } => {
            return decision(
                AdmissionStatus::Unavailable,
                AdmissionReasonCode::RuntimeUnavailable,
                detail,
                age,
            );
        }
        RuntimeAvailabilityEvidence::Unknown { reason: detail } => {
            unknown.push(reason(
                AdmissionReasonCode::RuntimeAvailabilityUnknown,
                detail,
            ));
        }
        RuntimeAvailabilityEvidence::Available {
            version_pinned: false,
            ..
        } => {
            unknown.push(reason(AdmissionReasonCode::RuntimeAvailabilityUnknown, "A runtime was located, but its version is not pinned; it is not admitted as a known runtime."));
        }
        RuntimeAvailabilityEvidence::Available {
            version_pinned: true,
            ..
        } => {}
    }
    match &context.runtime_compatibility {
        RuntimeCompatibilityEvidence::ConfirmedIncompatible { reason: detail } => {
            return decision(
                AdmissionStatus::Unavailable,
                AdmissionReasonCode::RuntimeIncompatible,
                detail,
                age,
            );
        }
        RuntimeCompatibilityEvidence::Unknown { reason: detail } => {
            unknown.push(reason(
                AdmissionReasonCode::RuntimeCompatibilityUnknown,
                detail,
            ));
        }
        RuntimeCompatibilityEvidence::ConfirmedCompatible { .. } => {}
    }
    let host_fresh = context.host.fresh();
    if !host_fresh {
        let (code, detail) = if context.host.sampled_at_unix_ms.is_some() {
            (AdmissionReasonCode::ResourceSnapshotStale, "The Phase 1 resource snapshot is missing a usable age or exceeds the 30-second admission freshness window.")
        } else {
            (AdmissionReasonCode::HostResourceUnknown, "The Phase 1 resource snapshot has no refresh timestamp; current resource availability is unknown.")
        };
        unknown.push(reason(code, detail));
    } else {
        compare_requirement(
            &profile.minimum_system_memory_bytes,
            &context.host.total_system_memory_bytes,
            AdmissionReasonCode::InsufficientSystemMemory,
            "Known total system memory is below the registered model minimum.",
            "System-memory requirements or current total-memory evidence are unknown or estimated.",
            &mut rejected,
            &mut unknown,
        );
        compare_requirement(
            &profile.minimum_available_memory_screening_floor_bytes,
            &context.host.available_system_memory_bytes,
            AdmissionReasonCode::CheckpointSizeScreeningFloor,
            "Available system memory is below the adapter's lower-bound checkpoint-size screening floor; this is not a model-fit estimate.",
            "Available system memory is unknown or estimated; the checkpoint-size screen cannot be evaluated.",
            &mut rejected,
            &mut unknown,
        );
        match &profile.minimum_gpu_memory_bytes {
            ResourceRequirement::NotRequired { .. } => {}
            ResourceRequirement::Required(requirement) => compare_requirement(
                requirement,
                &context.host.dedicated_gpu_memory_bytes,
                AdmissionReasonCode::InsufficientGpuMemory,
                "Known dedicated GPU memory is below the registered model minimum.",
                "GPU-memory requirements or current dedicated GPU-memory evidence are unknown or estimated.",
                &mut rejected,
                &mut unknown,
            ),
        }
        compare_requirement(
            &profile.temporary_disk_bytes,
            &context.host.temporary_storage_free_bytes,
            AdmissionReasonCode::InsufficientTemporaryDisk,
            "Known free output-volume space is below the adapter's bounded staging reserve.",
            "Required temporary disk or output-volume free space is unknown or estimated.",
            &mut rejected,
            &mut unknown,
        );
    }
    match &profile.required_backend {
        ResourceEvidence::Confirmed { .. } => {}
        ResourceEvidence::Estimated { .. } => unknown.push(reason(
            AdmissionReasonCode::RequiredBackendUnknown,
            "The required execution backend is only estimated; compatibility cannot be admitted.",
        )),
        ResourceEvidence::Unknown { reason: detail } => {
            unknown.push(reason(AdmissionReasonCode::RequiredBackendUnknown, detail))
        }
    }
    match workload_support(profile, context.workload) {
        WorkloadSupport::Supported => {}
        WorkloadSupport::Unsupported => rejected.push(reason(AdmissionReasonCode::UnsupportedWorkload, "The requested dimensions or operation settings exceed the adapter's explicitly supported workload contract.")),
        WorkloadSupport::Unknown => unknown.push(reason(AdmissionReasonCode::WorkloadLimitsUnknown, "The adapter has not established supported dimensions, frame count, or operation settings for this workload.")),
    }

    if !rejected.is_empty() {
        return AdmissionDecision::new(AdmissionStatus::Rejected, rejected, age);
    }
    if !unknown.is_empty() {
        return AdmissionDecision::new(AdmissionStatus::Unknown, unknown, age);
    }
    AdmissionDecision::new(AdmissionStatus::AllowedByKnownConstraints, Vec::new(), age)
}

fn compare_requirement(
    requirement: &ResourceEvidence<u64>,
    available: &ResourceEvidence<u64>,
    insufficient_code: AdmissionReasonCode,
    insufficient_detail: &str,
    unknown_detail: &str,
    rejected: &mut Vec<AdmissionReason>,
    unknown: &mut Vec<AdmissionReason>,
) {
    let required = match requirement {
        ResourceEvidence::Confirmed { value, .. } => Some(*value),
        ResourceEvidence::Estimated { .. } => {
            unknown.push(reason(AdmissionReasonCode::ResourceRequirementUnknown, "A resource requirement is only estimated; this admission policy requires confirmed limits."));
            None
        }
        ResourceEvidence::Unknown { reason: detail } => {
            unknown.push(reason(
                AdmissionReasonCode::ResourceRequirementUnknown,
                detail,
            ));
            None
        }
    };
    let Some(required) = required else {
        return;
    };
    match available {
        ResourceEvidence::Confirmed { value, .. } if *value < required => {
            rejected.push(reason(insufficient_code, insufficient_detail));
        }
        ResourceEvidence::Confirmed { .. } => {}
        ResourceEvidence::Estimated { .. } => unknown.push(reason(
            AdmissionReasonCode::HostResourceUnknown,
            unknown_detail,
        )),
        ResourceEvidence::Unknown { reason: detail } => {
            unknown.push(reason(AdmissionReasonCode::HostResourceUnknown, detail))
        }
    }
}

enum WorkloadSupport {
    Supported,
    Unsupported,
    Unknown,
}

fn workload_support(profile: &MediaWorkloadProfile, workload: WorkloadShape) -> WorkloadSupport {
    match workload {
        WorkloadShape::Image {
            width,
            height,
            steps,
        } => {
            let (Some(size), Some(allowed_steps)) = (
                profile.workload_limits.image_size,
                profile.workload_limits.allowed_image_steps.as_ref(),
            ) else {
                return WorkloadSupport::Unknown;
            };
            if size == (width, height) && allowed_steps.contains(&steps) {
                WorkloadSupport::Supported
            } else {
                WorkloadSupport::Unsupported
            }
        }
        WorkloadShape::Video {
            width,
            height,
            frames,
        } => {
            let (Some((max_width, max_height)), Some(max_frames)) = (
                profile.workload_limits.max_video_size,
                profile.workload_limits.max_video_frames,
            ) else {
                return WorkloadSupport::Unknown;
            };
            if width <= max_width && height <= max_height && frames > 0 && frames <= max_frames {
                WorkloadSupport::Supported
            } else {
                WorkloadSupport::Unsupported
            }
        }
    }
}

fn decision(
    status: AdmissionStatus,
    code: AdmissionReasonCode,
    detail: &str,
    age_ms: Option<u64>,
) -> AdmissionDecision {
    AdmissionDecision::new(status, vec![reason(code, detail)], age_ms)
}

fn reason(code: AdmissionReasonCode, detail: impl AsRef<str>) -> AdmissionReason {
    AdmissionReason {
        code,
        detail: detail.as_ref().to_string(),
    }
}

fn unix_millis() -> Option<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| u64::try_from(duration.as_millis()).ok())
}

#[cfg(test)]
mod blender_resource_tests {
    use super::*;

    #[test]
    fn blender_preview_resource_profile_never_admits_without_measurements() {
        let coordinator = MediaResourceCoordinator::default();
        let profile = BlenderWorkloadProfile::render(1_280, 720, 16);
        let assessment = coordinator.assess_blender_preview(&profile);
        assert_eq!(assessment.status, BlenderAdmissionStatus::Unknown);
        assert_eq!(assessment.active_shared_reservations, 0);
        assert!(assessment.reason.contains("not established"));
        assert!(coordinator
            .state
            .lock()
            .expect("reservation lock")
            .active
            .is_empty());
    }

    #[test]
    fn active_shared_media_reservation_defers_blender_preview_assessment() {
        let coordinator = MediaResourceCoordinator::default();
        {
            let mut state = coordinator.state.lock().expect("reservation lock");
            state.active.insert(
                ResourceOperationId("0123456789abcdef0123456789abcdef".into()),
                ActiveReservation {
                    token: 1,
                    capability: MediaCapability::ImageGeneration,
                    model_id: "test-model".into(),
                },
            );
        }
        let assessment = coordinator.assess_blender_preview(&BlenderWorkloadProfile::viewport());
        assert_eq!(assessment.status, BlenderAdmissionStatus::Deferred);
        assert_eq!(assessment.active_shared_reservations, 1);
        assert!(assessment.reason.contains("deferred"));
    }
}
