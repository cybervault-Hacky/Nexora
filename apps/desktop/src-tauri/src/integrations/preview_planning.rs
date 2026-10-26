//! Bounded, transport-independent Blender preview intent and admission screen.
//!
//! This module describes requested viewport/render previews only. It never
//! opens Blender, renders, writes a preview file, or reserves resources.
use serde::{Deserialize, Serialize};

use crate::resource_management::{
    BlenderAdmissionStatus, BlenderPreviewKind, BlenderWorkloadProfile, MediaResourceCoordinator,
};

use super::blender_actions::{ActionPermission, ConfirmationRequirement, MAX_ANIMATION_FRAME};
use super::scene_planning::{
    valid_identifier, validate_scene_scope, SceneObjectType, ScenePlanningErrorCode,
    ValidatedSceneSnapshot,
};

pub const BLENDER_PREVIEW_REQUEST_SCHEMA_VERSION: u16 = 1;
pub const MAX_BLENDER_PREVIEW_REQUEST_BYTES: usize = 4 * 1024;
pub const MAX_PREVIEW_RENDER_WIDTH: u32 = 1_920;
pub const MAX_PREVIEW_RENDER_HEIGHT: u32 = 1_080;
pub const MAX_PREVIEW_RENDER_PIXELS: u64 = 1_920 * 1_080;
pub const MAX_PREVIEW_RENDER_SAMPLES: u16 = 64;
pub const MIN_PREVIEW_RENDER_SIDE: u32 = 64;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum BlenderPreviewKindRequest {
    Viewport {
        frame: u32,
    },
    Render {
        frame: u32,
        camera_object_id: String,
        width: u32,
        height: u32,
        samples: u16,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BlenderPreviewRequest {
    pub schema_version: u16,
    pub request_id: String,
    pub scene_reference: String,
    pub expected_scene_revision: String,
    pub preview: BlenderPreviewKindRequest,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BlenderPreviewPlanStatus {
    Planned,
    Blocked,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BlenderPreviewExecutionStatus {
    ConnectorUnavailable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BlenderPreviewAdmissionStatus {
    Unknown,
    Deferred,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "kind", content = "details", rename_all = "snake_case")]
pub enum BlenderPreviewDetails {
    Viewport {
        frame: u32,
    },
    Render {
        frame: u32,
        camera_object_id: String,
        camera_name: String,
        width: u32,
        height: u32,
        samples: u16,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlenderPreviewAdmission {
    pub status: BlenderPreviewAdmissionStatus,
    pub reason: String,
    pub active_shared_reservations: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BlenderPreviewErrorCode {
    PayloadTooLarge,
    MalformedRequest,
    UnsupportedSchemaVersion,
    InvalidRequest,
    SnapshotUnavailable,
    SnapshotRevisionUnavailable,
    StaleSnapshot,
    SceneMismatch,
    InvalidFrame,
    InvalidResolution,
    InvalidSamples,
    CameraNotFound,
    InvalidCameraTarget,
    ResourceAdmissionUnknown,
    ResourceAdmissionDeferred,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct BlenderPreviewError {
    pub code: BlenderPreviewErrorCode,
    pub path: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlenderPreviewPlanPreview {
    pub schema_version: u16,
    pub request: BlenderPreviewRequest,
    pub status: BlenderPreviewPlanStatus,
    pub execution_status: BlenderPreviewExecutionStatus,
    pub required_permission: ActionPermission,
    pub confirmation_policy: ConfirmationRequirement,
    pub confirmation_granted: bool,
    pub expected_scene_revision: String,
    pub details: BlenderPreviewDetails,
    pub resource_admission: BlenderPreviewAdmission,
    pub error: Option<BlenderPreviewError>,
    pub limitations: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BlenderPreviewPlan {
    preview: BlenderPreviewPlanPreview,
}

impl BlenderPreviewPlan {
    pub fn preview(&self) -> &BlenderPreviewPlanPreview {
        &self.preview
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlenderPreviewPlanningError {
    pub code: BlenderPreviewErrorCode,
    pub path: String,
}

impl BlenderPreviewPlanningError {
    fn new(code: BlenderPreviewErrorCode, path: impl Into<String>) -> Self {
        Self {
            code,
            path: path.into(),
        }
    }
}

pub fn plan_blender_preview_json(
    snapshot: Option<&ValidatedSceneSnapshot>,
    coordinator: Option<&MediaResourceCoordinator>,
    request_json: &str,
) -> Result<BlenderPreviewPlan, BlenderPreviewPlanningError> {
    if request_json.len() > MAX_BLENDER_PREVIEW_REQUEST_BYTES {
        return Err(BlenderPreviewPlanningError::new(
            BlenderPreviewErrorCode::PayloadTooLarge,
            "$",
        ));
    }
    let request: BlenderPreviewRequest = serde_json::from_str(request_json).map_err(|_| {
        BlenderPreviewPlanningError::new(BlenderPreviewErrorCode::MalformedRequest, "$")
    })?;
    plan_blender_preview(snapshot, coordinator, &request)
}

pub fn plan_blender_preview(
    snapshot: Option<&ValidatedSceneSnapshot>,
    coordinator: Option<&MediaResourceCoordinator>,
    request: &BlenderPreviewRequest,
) -> Result<BlenderPreviewPlan, BlenderPreviewPlanningError> {
    let request_size = serde_json::to_vec(request)
        .map_err(|_| {
            BlenderPreviewPlanningError::new(BlenderPreviewErrorCode::InvalidRequest, "$")
        })?
        .len();
    if request_size > MAX_BLENDER_PREVIEW_REQUEST_BYTES {
        return Err(BlenderPreviewPlanningError::new(
            BlenderPreviewErrorCode::PayloadTooLarge,
            "$",
        ));
    }
    if request.schema_version != BLENDER_PREVIEW_REQUEST_SCHEMA_VERSION {
        return Err(BlenderPreviewPlanningError::new(
            BlenderPreviewErrorCode::UnsupportedSchemaVersion,
            "$.schemaVersion",
        ));
    }
    if !valid_identifier(&request.request_id)
        || !valid_identifier(&request.scene_reference)
        || !valid_identifier(&request.expected_scene_revision)
    {
        return Err(BlenderPreviewPlanningError::new(
            BlenderPreviewErrorCode::InvalidRequest,
            "$.requestId",
        ));
    }
    let snapshot = snapshot.ok_or_else(|| {
        BlenderPreviewPlanningError::new(BlenderPreviewErrorCode::SnapshotUnavailable, "$.snapshot")
    })?;
    let scene = snapshot.snapshot();
    if scene.scene_reference != request.scene_reference {
        return Err(BlenderPreviewPlanningError::new(
            BlenderPreviewErrorCode::SceneMismatch,
            "$.sceneReference",
        ));
    }
    let Some(actual_revision) = scene.revision.as_deref() else {
        return Err(BlenderPreviewPlanningError::new(
            BlenderPreviewErrorCode::SnapshotRevisionUnavailable,
            "$.expectedSceneRevision",
        ));
    };
    if actual_revision != request.expected_scene_revision.as_str() {
        return Err(BlenderPreviewPlanningError::new(
            BlenderPreviewErrorCode::StaleSnapshot,
            "$.expectedSceneRevision",
        ));
    }
    validate_scene_scope(
        snapshot,
        &request.scene_reference,
        Some(&request.expected_scene_revision),
        false,
    )
    .map_err(|error| {
        let code = match error.code {
            ScenePlanningErrorCode::SnapshotUnavailable => {
                BlenderPreviewErrorCode::SnapshotUnavailable
            }
            ScenePlanningErrorCode::StaleSnapshot => BlenderPreviewErrorCode::StaleSnapshot,
            ScenePlanningErrorCode::SceneMismatch => BlenderPreviewErrorCode::SceneMismatch,
            _ => BlenderPreviewErrorCode::InvalidRequest,
        };
        BlenderPreviewPlanningError::new(code, error.path)
    })?;

    let (details, profile, confirmation_policy) = match &request.preview {
        BlenderPreviewKindRequest::Viewport { frame } => {
            validate_frame(*frame)?;
            (
                BlenderPreviewDetails::Viewport { frame: *frame },
                BlenderWorkloadProfile::viewport(),
                ConfirmationRequirement::None,
            )
        }
        BlenderPreviewKindRequest::Render {
            frame,
            camera_object_id,
            width,
            height,
            samples,
        } => {
            validate_frame(*frame)?;
            if !valid_identifier(camera_object_id) {
                return Err(BlenderPreviewPlanningError::new(
                    BlenderPreviewErrorCode::InvalidRequest,
                    "$.preview.cameraObjectId",
                ));
            }
            if *width < MIN_PREVIEW_RENDER_SIDE
                || *height < MIN_PREVIEW_RENDER_SIDE
                || *width > MAX_PREVIEW_RENDER_WIDTH
                || *height > MAX_PREVIEW_RENDER_HEIGHT
                || u64::from(*width) * u64::from(*height) > MAX_PREVIEW_RENDER_PIXELS
            {
                return Err(BlenderPreviewPlanningError::new(
                    BlenderPreviewErrorCode::InvalidResolution,
                    "$.preview.width",
                ));
            }
            if *samples == 0 || *samples > MAX_PREVIEW_RENDER_SAMPLES {
                return Err(BlenderPreviewPlanningError::new(
                    BlenderPreviewErrorCode::InvalidSamples,
                    "$.preview.samples",
                ));
            }
            let camera = scene
                .objects
                .iter()
                .find(|object| object.object_id.as_str() == camera_object_id.as_str())
                .ok_or_else(|| {
                    BlenderPreviewPlanningError::new(
                        BlenderPreviewErrorCode::CameraNotFound,
                        "$.preview.cameraObjectId",
                    )
                })?;
            if camera.object_type != SceneObjectType::Camera {
                return Err(BlenderPreviewPlanningError::new(
                    BlenderPreviewErrorCode::InvalidCameraTarget,
                    "$.preview.cameraObjectId",
                ));
            }
            (
                BlenderPreviewDetails::Render {
                    frame: *frame,
                    camera_object_id: camera.object_id.clone(),
                    camera_name: camera.name.clone(),
                    width: *width,
                    height: *height,
                    samples: *samples,
                },
                BlenderWorkloadProfile::render(*width, *height, *samples),
                ConfirmationRequirement::ExplicitUserConfirmation,
            )
        }
    };

    let admission = match coordinator {
        Some(coordinator) => coordinator.assess_blender_preview(&profile),
        None => crate::resource_management::BlenderAdmissionDecision {
            status: BlenderAdmissionStatus::Unknown,
            reason: "The shared resource coordinator was not supplied; no workload is admitted."
                .to_string(),
            active_shared_reservations: 0,
        },
    };
    let admission_status = match admission.status {
        BlenderAdmissionStatus::Deferred => BlenderPreviewAdmissionStatus::Deferred,
        BlenderAdmissionStatus::Unknown => BlenderPreviewAdmissionStatus::Unknown,
    };
    let blocking_error = match (&request.preview, admission_status) {
        (BlenderPreviewKindRequest::Render { .. }, BlenderPreviewAdmissionStatus::Deferred) => {
            Some(BlenderPreviewError {
                code: BlenderPreviewErrorCode::ResourceAdmissionDeferred,
                path: "$.preview".to_string(),
            })
        }
        (BlenderPreviewKindRequest::Render { .. }, BlenderPreviewAdmissionStatus::Unknown) => {
            Some(BlenderPreviewError {
                code: BlenderPreviewErrorCode::ResourceAdmissionUnknown,
                path: "$.preview".to_string(),
            })
        }
        _ => None,
    };
    let status = if blocking_error.is_some() {
        BlenderPreviewPlanStatus::Blocked
    } else {
        BlenderPreviewPlanStatus::Planned
    };
    Ok(BlenderPreviewPlan {
        preview: BlenderPreviewPlanPreview {
            schema_version: BLENDER_PREVIEW_REQUEST_SCHEMA_VERSION,
            request: request.clone(),
            status,
            execution_status: BlenderPreviewExecutionStatus::ConnectorUnavailable,
            required_permission: ActionPermission::SceneInspect,
            confirmation_policy,
            confirmation_granted: false,
            expected_scene_revision: request.expected_scene_revision.clone(),
            details,
            resource_admission: BlenderPreviewAdmission {
                status: admission_status,
                reason: admission.reason,
                active_shared_reservations: admission.active_shared_reservations,
            },
            error: blocking_error,
            limitations: vec![
                "No viewport image or render was generated; this is a bounded preview intent only.".to_string(),
                "Blender execution is unavailable and no preview output file was written.".to_string(),
                "The preview is scoped to the exact supplied scene revision; a changed revision invalidates the plan.".to_string(),
            ],
        },
    })
}

fn validate_frame(frame: u32) -> Result<(), BlenderPreviewPlanningError> {
    if !(1..=MAX_ANIMATION_FRAME).contains(&frame) {
        return Err(BlenderPreviewPlanningError::new(
            BlenderPreviewErrorCode::InvalidFrame,
            "$.preview.frame",
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "preview_planning_tests.rs"]
mod tests;
