//! Transport-independent validation and planning for bounded object animation.
//!
//! This module accepts only typed animation intent and a Phase 8 validated
//! snapshot. It does not inspect Blender, create keyframes, render, or execute.
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::blender_actions::{
    parse_candidate_json, ActionClass, ActionPermission, AnimationOperation,
    ConfirmationRequirement, InterpolationMode, ObjectKeyframe, ValidatedAction, ValidatedBatch,
    ACTION_CONTRACT_VERSION, ANIMATE_OBJECT_ID, ANIMATE_OBJECT_KEYFRAMES_ID,
    MAX_ANIMATION_DESCRIPTION_CHARS, MAX_ANIMATION_FRAME, MAX_ANIMATION_FRAME_COUNT,
    MAX_ANIMATION_KEYFRAMES, MAX_CANDIDATE_BYTES, MAX_OBJECT_NAME_CHARS, MAX_OBJECT_SCALE,
    MAX_ROTATION_RADIANS, MAX_SCENE_LOCATION, MIN_ANIMATION_FRAME, MIN_OBJECT_SCALE,
    SUPPORTED_ANIMATION_FRAME_RATES,
};
use super::scene_planning::{
    assess_scene_snapshot_json, resolve_target, valid_identifier, valid_text, validate_scene_scope,
    ObjectTarget, SceneObjectType, ScenePlanningErrorCode, SceneSnapshotState,
    ValidatedSceneSnapshot,
};

pub const ANIMATION_REQUEST_SCHEMA_VERSION: u16 = 1;
pub const MAX_ANIMATION_REQUEST_BYTES: usize = 8 * 1024;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnimationRequest {
    pub schema_version: u16,
    pub request_id: String,
    pub scene_reference: String,
    #[serde(default)]
    pub expected_snapshot_revision: Option<String>,
    #[serde(default)]
    pub target: Option<ObjectTarget>,
    pub operation: AnimationOperation,
    pub start_frame: u32,
    pub end_frame: u32,
    pub frame_rate: u16,
    pub interpolation: InterpolationMode,
    /// Local object-channel value at `start_frame`.
    pub start_value: super::blender_actions::Vector3,
    /// Local object-channel value at `end_frame`.
    pub end_value: super::blender_actions::Vector3,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AnimationValueUnits {
    BlenderUnitsLocal,
    EulerRadiansLocal,
    DimensionlessLocalScale,
}

impl AnimationValueUnits {
    fn for_operation(operation: AnimationOperation) -> Self {
        match operation {
            AnimationOperation::Translation => Self::BlenderUnitsLocal,
            AnimationOperation::Rotation => Self::EulerRadiansLocal,
            AnimationOperation::Scale => Self::DimensionlessLocalScale,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnimationKeyframeIntent {
    pub frame: u32,
    pub value: super::blender_actions::Vector3,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnimationTarget {
    pub object_id: String,
    pub object_name: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AnimationPlanState {
    PlanCreated,
    AwaitingConfirmation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AnimationExecutionStatus {
    ConnectorUnavailable,
    /// Reserved for a future reviewed connector; Phase 9 never emits this.
    Executed,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnimationPlanPreview {
    pub request: AnimationRequest,
    pub action_id: String,
    pub target: AnimationTarget,
    pub operation: AnimationOperation,
    pub units: AnimationValueUnits,
    /// Inclusive integer frames; exactly two endpoint keyframes are planned.
    pub start_frame: u32,
    pub end_frame: u32,
    pub frame_rate: u16,
    pub interpolation: InterpolationMode,
    pub keyframes: [AnimationKeyframeIntent; 2],
    pub action_class: ActionClass,
    pub validation_status: &'static str,
    pub required_permission: ActionPermission,
    pub confirmation_policy: ConfirmationRequirement,
    pub plan_state: AnimationPlanState,
    pub execution_status: AnimationExecutionStatus,
    pub snapshot_revision: Option<String>,
    pub freshness_checked_by_revision: bool,
    pub snapshot_complete: bool,
    pub snapshot_truncated: bool,
    pub limitations: Vec<String>,
}

/// The candidate stays private and is not an `AuthorizedBatch`.
#[derive(Clone, Debug, PartialEq)]
pub struct AnimationPlan {
    preview: AnimationPlanPreview,
    candidate: ValidatedBatch,
}

impl AnimationPlan {
    pub fn preview(&self) -> &AnimationPlanPreview {
        &self.preview
    }

    pub fn validated_candidate(&self) -> &ValidatedBatch {
        &self.candidate
    }
}

pub const ANIMATION_SEQUENCE_REQUEST_SCHEMA_VERSION: u16 = 1;
pub const MAX_ANIMATION_SEQUENCE_REQUEST_BYTES: usize = 8 * 1024;
pub const MAX_ANIMATION_SEQUENCE_KEYFRAMES: usize = MAX_ANIMATION_KEYFRAMES;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnimationSequenceRequest {
    pub schema_version: u16,
    pub request_id: String,
    pub scene_reference: String,
    pub expected_snapshot_revision: String,
    pub target: ObjectTarget,
    pub operation: AnimationOperation,
    pub frame_rate: u16,
    pub interpolation: InterpolationMode,
    pub keyframes: Vec<AnimationKeyframeIntent>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AnimationSequencePlanState {
    AwaitingConfirmation,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnimationSequencePlanPreview {
    pub request: AnimationSequenceRequest,
    pub action_id: String,
    pub target: AnimationTarget,
    pub operation: AnimationOperation,
    pub units: AnimationValueUnits,
    pub frame_rate: u16,
    pub interpolation: InterpolationMode,
    pub keyframes: Vec<AnimationKeyframeIntent>,
    pub action_class: ActionClass,
    pub validation_status: &'static str,
    pub required_permission: ActionPermission,
    pub confirmation_policy: ConfirmationRequirement,
    pub plan_state: AnimationSequencePlanState,
    pub execution_status: AnimationExecutionStatus,
    pub snapshot_revision: Option<String>,
    pub limitations: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AnimationSequencePlan {
    preview: AnimationSequencePlanPreview,
    candidate: ValidatedBatch,
}

impl AnimationSequencePlan {
    pub fn preview(&self) -> &AnimationSequencePlanPreview {
        &self.preview
    }

    pub fn validated_candidate(&self) -> &ValidatedBatch {
        &self.candidate
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AnimationPlanningErrorCode {
    SnapshotUnavailable,
    InvalidSnapshot,
    StaleSnapshot,
    SceneMismatch,
    SnapshotRevisionRequired,
    InvalidRequest,
    MissingTarget,
    TargetNotFound,
    AmbiguousTarget,
    IncompleteSnapshotForNameResolution,
    InvalidFrameRange,
    UnsupportedFrameRate,
    InvalidValue,
    InvalidCandidate,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct AnimationPlanningError {
    pub code: AnimationPlanningErrorCode,
    pub path: String,
}

impl AnimationPlanningError {
    fn new(code: AnimationPlanningErrorCode, path: impl Into<String>) -> Self {
        Self {
            code,
            path: path.into(),
        }
    }
}

fn map_scene_error(error: super::scene_planning::ScenePlanningError) -> AnimationPlanningError {
    let code = match error.code {
        ScenePlanningErrorCode::SnapshotUnavailable => {
            AnimationPlanningErrorCode::SnapshotUnavailable
        }
        ScenePlanningErrorCode::InvalidSnapshot => AnimationPlanningErrorCode::InvalidSnapshot,
        ScenePlanningErrorCode::StaleSnapshot => AnimationPlanningErrorCode::StaleSnapshot,
        ScenePlanningErrorCode::SceneMismatch => AnimationPlanningErrorCode::SceneMismatch,
        ScenePlanningErrorCode::SnapshotRevisionRequired => {
            AnimationPlanningErrorCode::SnapshotRevisionRequired
        }
        ScenePlanningErrorCode::TargetNotFound => AnimationPlanningErrorCode::TargetNotFound,
        ScenePlanningErrorCode::AmbiguousTarget => AnimationPlanningErrorCode::AmbiguousTarget,
        _ => AnimationPlanningErrorCode::InvalidRequest,
    };
    AnimationPlanningError::new(code, error.path)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AnimationSequencePlanningErrorCode {
    PayloadTooLarge,
    MalformedRequest,
    UnsupportedSchemaVersion,
    InvalidRequest,
    SnapshotUnavailable,
    StaleSnapshot,
    SnapshotRevisionUnavailable,
    SceneMismatch,
    TargetNotFound,
    UnsupportedTargetType,
    InvalidFrameRate,
    InvalidKeyframes,
    InvalidCandidate,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct AnimationSequencePlanningError {
    pub code: AnimationSequencePlanningErrorCode,
    pub path: String,
}

impl AnimationSequencePlanningError {
    fn new(code: AnimationSequencePlanningErrorCode, path: impl Into<String>) -> Self {
        Self {
            code,
            path: path.into(),
        }
    }
}

pub fn plan_animation_sequence_json(
    snapshot_json: Option<&str>,
    request_json: &str,
) -> Result<AnimationSequencePlan, AnimationSequencePlanningError> {
    if request_json.len() > MAX_ANIMATION_SEQUENCE_REQUEST_BYTES {
        return Err(AnimationSequencePlanningError::new(
            AnimationSequencePlanningErrorCode::PayloadTooLarge,
            "$",
        ));
    }
    let request: AnimationSequenceRequest = serde_json::from_str(request_json).map_err(|_| {
        AnimationSequencePlanningError::new(
            AnimationSequencePlanningErrorCode::MalformedRequest,
            "$",
        )
    })?;
    let snapshot = match assess_scene_snapshot_json(snapshot_json) {
        SceneSnapshotState::SnapshotUnavailable => {
            return Err(AnimationSequencePlanningError::new(
                AnimationSequencePlanningErrorCode::SnapshotUnavailable,
                "$.snapshot",
            ));
        }
        SceneSnapshotState::InvalidSnapshot(_) | SceneSnapshotState::StaleSnapshot => {
            return Err(AnimationSequencePlanningError::new(
                AnimationSequencePlanningErrorCode::SnapshotUnavailable,
                "$.snapshot",
            ));
        }
        SceneSnapshotState::ValidSnapshot(snapshot) => snapshot,
    };
    plan_animation_sequence(Some(&snapshot), &request)
}

/// Plans a bounded 2–8 point object transform curve. The allowlist is limited
/// to mesh and empty objects; rig/armature animation remains unsupported until
/// named rigs and prerequisites are verified against real Blender builds.
pub fn plan_animation_sequence(
    snapshot: Option<&ValidatedSceneSnapshot>,
    request: &AnimationSequenceRequest,
) -> Result<AnimationSequencePlan, AnimationSequencePlanningError> {
    let snapshot = snapshot.ok_or_else(|| {
        AnimationSequencePlanningError::new(
            AnimationSequencePlanningErrorCode::SnapshotUnavailable,
            "$.snapshot",
        )
    })?;
    let request_size = serde_json::to_vec(request)
        .map_err(|_| {
            AnimationSequencePlanningError::new(
                AnimationSequencePlanningErrorCode::InvalidRequest,
                "$",
            )
        })?
        .len();
    if request_size > MAX_ANIMATION_SEQUENCE_REQUEST_BYTES {
        return Err(AnimationSequencePlanningError::new(
            AnimationSequencePlanningErrorCode::PayloadTooLarge,
            "$",
        ));
    }
    if request.schema_version != ANIMATION_SEQUENCE_REQUEST_SCHEMA_VERSION
        || !valid_identifier(&request.request_id)
        || !valid_identifier(&request.scene_reference)
        || !valid_identifier(&request.expected_snapshot_revision)
        || !matches!(&request.target, ObjectTarget::ObjectId { .. })
    {
        return Err(AnimationSequencePlanningError::new(
            if request.schema_version != ANIMATION_SEQUENCE_REQUEST_SCHEMA_VERSION {
                AnimationSequencePlanningErrorCode::UnsupportedSchemaVersion
            } else {
                AnimationSequencePlanningErrorCode::InvalidRequest
            },
            "$.request",
        ));
    }
    if !SUPPORTED_ANIMATION_FRAME_RATES.contains(&request.frame_rate) {
        return Err(AnimationSequencePlanningError::new(
            AnimationSequencePlanningErrorCode::InvalidFrameRate,
            "$.frameRate",
        ));
    }
    if !(2..=MAX_ANIMATION_SEQUENCE_KEYFRAMES).contains(&request.keyframes.len())
        || request.keyframes.iter().any(|point| {
            !(MIN_ANIMATION_FRAME..=MAX_ANIMATION_FRAME).contains(&point.frame)
                || !point.value.x.is_finite()
                || !point.value.y.is_finite()
                || !point.value.z.is_finite()
        })
        || request
            .keyframes
            .windows(2)
            .any(|pair| pair[1].frame <= pair[0].frame)
        || request
            .keyframes
            .last()
            .zip(request.keyframes.first())
            .map_or(true, |(last, first)| {
                last.frame.saturating_sub(first.frame).saturating_add(1) > MAX_ANIMATION_FRAME_COUNT
            })
    {
        return Err(AnimationSequencePlanningError::new(
            AnimationSequencePlanningErrorCode::InvalidKeyframes,
            "$.keyframes",
        ));
    }

    let scene = snapshot.snapshot();
    if scene.scene_reference != request.scene_reference {
        return Err(AnimationSequencePlanningError::new(
            AnimationSequencePlanningErrorCode::SceneMismatch,
            "$.sceneReference",
        ));
    }
    if scene.revision.as_deref() != Some(request.expected_snapshot_revision.as_str()) {
        return Err(AnimationSequencePlanningError::new(
            if scene.revision.is_some() {
                AnimationSequencePlanningErrorCode::StaleSnapshot
            } else {
                AnimationSequencePlanningErrorCode::SnapshotRevisionUnavailable
            },
            "$.expectedSnapshotRevision",
        ));
    }
    validate_scene_scope(
        snapshot,
        &request.scene_reference,
        Some(&request.expected_snapshot_revision),
        true,
    )
    .map_err(|error| {
        let code = match error.code {
            ScenePlanningErrorCode::SnapshotUnavailable => {
                AnimationSequencePlanningErrorCode::SnapshotUnavailable
            }
            ScenePlanningErrorCode::StaleSnapshot => {
                AnimationSequencePlanningErrorCode::StaleSnapshot
            }
            ScenePlanningErrorCode::SceneMismatch => {
                AnimationSequencePlanningErrorCode::SceneMismatch
            }
            _ => AnimationSequencePlanningErrorCode::InvalidRequest,
        };
        AnimationSequencePlanningError::new(code, error.path)
    })?;
    let object = resolve_target(scene, &request.target).map_err(|error| {
        let code = match error.code {
            ScenePlanningErrorCode::TargetNotFound => {
                AnimationSequencePlanningErrorCode::TargetNotFound
            }
            _ => AnimationSequencePlanningErrorCode::InvalidRequest,
        };
        AnimationSequencePlanningError::new(code, error.path)
    })?;
    if !matches!(
        object.object_type,
        SceneObjectType::Mesh | SceneObjectType::Empty
    ) {
        return Err(AnimationSequencePlanningError::new(
            AnimationSequencePlanningErrorCode::UnsupportedTargetType,
            "$.target",
        ));
    }

    let candidate_value = json!({
        "contract_version": ACTION_CONTRACT_VERSION,
        "request_id": request.request_id,
        "actions": [{
            "action_id": ANIMATE_OBJECT_KEYFRAMES_ID,
            "contract_version": ACTION_CONTRACT_VERSION,
            "parameters": {
                "object_id": object.object_id,
                "operation": request.operation,
                "frame_rate": request.frame_rate,
                "interpolation": request.interpolation,
                "keyframes": request.keyframes.iter().map(|point| json!({
                    "frame": point.frame,
                    "value": [point.value.x, point.value.y, point.value.z],
                })).collect::<Vec<_>>(),
            },
        }],
    });
    let candidate_json = serde_json::to_string(&candidate_value).map_err(|_| {
        AnimationSequencePlanningError::new(
            AnimationSequencePlanningErrorCode::InvalidCandidate,
            "$",
        )
    })?;
    let candidate = parse_candidate_json(&candidate_json).map_err(|_| {
        AnimationSequencePlanningError::new(
            AnimationSequencePlanningErrorCode::InvalidCandidate,
            "$.keyframes",
        )
    })?;
    let descriptor = candidate.actions()[0].descriptor();
    Ok(AnimationSequencePlan {
        preview: AnimationSequencePlanPreview {
            request: request.clone(),
            action_id: descriptor.action_id.to_string(),
            target: AnimationTarget {
                object_id: object.object_id.clone(),
                object_name: object.name.clone(),
            },
            operation: request.operation,
            units: AnimationValueUnits::for_operation(request.operation),
            frame_rate: request.frame_rate,
            interpolation: request.interpolation,
            keyframes: request.keyframes.clone(),
            action_class: descriptor.class,
            validation_status: "valid",
            required_permission: descriptor.permission,
            confirmation_policy: descriptor.confirmation,
            plan_state: AnimationSequencePlanState::AwaitingConfirmation,
            execution_status: AnimationExecutionStatus::ConnectorUnavailable,
            snapshot_revision: scene.revision.clone(),
            limitations: vec![
                "Bounded object transform curve only; no rigging, drivers, expressions, or executable code are supported.".to_string(),
                "This is a validated candidate for review; no keyframes were created or applied.".to_string(),
            ],
        },
        candidate,
    })
}

/// Strict JSON entry point for future host wiring. No snapshot source exists in
/// Phase 9; passing `None` returns `snapshot_unavailable` without fabricating it.
pub fn plan_animation_json(
    snapshot_json: Option<&str>,
    request_json: &str,
) -> Result<AnimationPlan, AnimationPlanningError> {
    if request_json.len() > MAX_ANIMATION_REQUEST_BYTES {
        return Err(AnimationPlanningError::new(
            AnimationPlanningErrorCode::InvalidRequest,
            "$",
        ));
    }
    let request: AnimationRequest = serde_json::from_str(request_json).map_err(|_| {
        AnimationPlanningError::new(AnimationPlanningErrorCode::InvalidRequest, "$")
    })?;
    let snapshot = match assess_scene_snapshot_json(snapshot_json) {
        SceneSnapshotState::SnapshotUnavailable => {
            return Err(AnimationPlanningError::new(
                AnimationPlanningErrorCode::SnapshotUnavailable,
                "$.snapshot",
            ));
        }
        SceneSnapshotState::InvalidSnapshot(_) => {
            return Err(AnimationPlanningError::new(
                AnimationPlanningErrorCode::InvalidSnapshot,
                "$.snapshot",
            ));
        }
        SceneSnapshotState::StaleSnapshot => {
            return Err(AnimationPlanningError::new(
                AnimationPlanningErrorCode::StaleSnapshot,
                "$.snapshot",
            ));
        }
        SceneSnapshotState::ValidSnapshot(snapshot) => snapshot,
    };
    plan_animation(Some(&snapshot), &request)
}

/// Deterministically plans a two-keyframe action from a validated Phase 8
/// snapshot. The request and entire candidate are checked before a plan exists.
pub fn plan_animation(
    snapshot: Option<&ValidatedSceneSnapshot>,
    request: &AnimationRequest,
) -> Result<AnimationPlan, AnimationPlanningError> {
    let Some(snapshot) = snapshot else {
        return Err(AnimationPlanningError::new(
            AnimationPlanningErrorCode::SnapshotUnavailable,
            "$.snapshot",
        ));
    };
    validate_request(request)?;
    let target = request.target.as_ref().ok_or_else(|| {
        AnimationPlanningError::new(AnimationPlanningErrorCode::MissingTarget, "$.target")
    })?;
    let freshness_checked_by_revision = validate_scene_scope(
        snapshot,
        &request.scene_reference,
        request.expected_snapshot_revision.as_deref(),
        true,
    )
    .map_err(map_scene_error)?;
    let scene = snapshot.snapshot();
    if matches!(target, ObjectTarget::ObjectName { .. })
        && (!scene.completeness.complete || scene.completeness.truncated)
    {
        return Err(AnimationPlanningError::new(
            AnimationPlanningErrorCode::IncompleteSnapshotForNameResolution,
            "$.target",
        ));
    }
    let object = resolve_target(scene, target).map_err(map_scene_error)?;

    let candidate_value = json!({
        "contract_version": super::blender_actions::ACTION_CONTRACT_VERSION,
        "request_id": request.request_id,
        "actions": [{
            "action_id": ANIMATE_OBJECT_ID,
            "contract_version": super::blender_actions::ACTION_CONTRACT_VERSION,
            "parameters": {
                "object_id": object.object_id,
                "operation": request.operation,
                "start_frame": request.start_frame,
                "end_frame": request.end_frame,
                "frame_rate": request.frame_rate,
                "interpolation": request.interpolation,
                "start_value": vector_values(request.start_value),
                "end_value": vector_values(request.end_value),
            },
        }],
    });
    let candidate_json = serde_json::to_string(&candidate_value).map_err(|_| {
        AnimationPlanningError::new(AnimationPlanningErrorCode::InvalidCandidate, "$")
    })?;
    if candidate_json.len() > MAX_CANDIDATE_BYTES {
        return Err(AnimationPlanningError::new(
            AnimationPlanningErrorCode::InvalidCandidate,
            "$",
        ));
    }
    let candidate = parse_candidate_json(&candidate_json).map_err(|_| {
        AnimationPlanningError::new(AnimationPlanningErrorCode::InvalidCandidate, "$.actions[0]")
    })?;
    let Some(ValidatedAction::AnimateObject {
        object_id,
        operation,
        start_frame,
        end_frame,
        frame_rate,
        interpolation,
        start_value,
        end_value,
    }) = candidate.actions().first()
    else {
        return Err(AnimationPlanningError::new(
            AnimationPlanningErrorCode::InvalidCandidate,
            "$.actions[0]",
        ));
    };
    let descriptor = candidate.actions()[0].descriptor();
    let mut limitations = vec![
        "Plan only: no Blender request was made and no keyframes were created.".to_string(),
        "The Blender executor is unavailable; execution status is connector_unavailable."
            .to_string(),
    ];
    if scene.revision.is_none() {
        limitations
            .push("The snapshot has no revision; stale-scene detection is limited.".to_string());
    }
    if !scene.completeness.complete || scene.completeness.truncated {
        limitations.push(
            "The supplied snapshot is incomplete or truncated; target resolution is scoped to included objects.".to_string(),
        );
    }
    Ok(AnimationPlan {
        preview: AnimationPlanPreview {
            request: request.clone(),
            action_id: descriptor.action_id.to_string(),
            target: AnimationTarget {
                object_id: object_id.clone(),
                object_name: object.name.clone(),
            },
            operation: *operation,
            units: AnimationValueUnits::for_operation(*operation),
            start_frame: *start_frame,
            end_frame: *end_frame,
            frame_rate: *frame_rate,
            interpolation: *interpolation,
            keyframes: [
                AnimationKeyframeIntent {
                    frame: *start_frame,
                    value: *start_value,
                },
                AnimationKeyframeIntent {
                    frame: *end_frame,
                    value: *end_value,
                },
            ],
            action_class: descriptor.class,
            validation_status: "valid",
            required_permission: descriptor.permission,
            confirmation_policy: descriptor.confirmation,
            plan_state: AnimationPlanState::AwaitingConfirmation,
            execution_status: AnimationExecutionStatus::ConnectorUnavailable,
            snapshot_revision: scene.revision.clone(),
            freshness_checked_by_revision,
            snapshot_complete: scene.completeness.complete,
            snapshot_truncated: scene.completeness.truncated,
            limitations,
        },
        candidate,
    })
}

fn validate_request(request: &AnimationRequest) -> Result<(), AnimationPlanningError> {
    let invalid = |path| {
        Err(AnimationPlanningError::new(
            AnimationPlanningErrorCode::InvalidRequest,
            path,
        ))
    };
    if request.schema_version != ANIMATION_REQUEST_SCHEMA_VERSION {
        return invalid("$.schemaVersion");
    }
    if !valid_identifier(&request.request_id) {
        return invalid("$.requestId");
    }
    if !valid_identifier(&request.scene_reference) {
        return invalid("$.sceneReference");
    }
    if let Some(revision) = &request.expected_snapshot_revision {
        if !valid_identifier(revision) {
            return invalid("$.expectedSnapshotRevision");
        }
    }
    if let Some(description) = &request.description {
        if !valid_text(description, MAX_ANIMATION_DESCRIPTION_CHARS) {
            return invalid("$.description");
        }
    }
    if let Some(target) = &request.target {
        match target {
            ObjectTarget::ObjectId { object_id } if valid_identifier(object_id) => {}
            ObjectTarget::ObjectName { name } if valid_text(name, MAX_OBJECT_NAME_CHARS) => {}
            _ => return invalid("$.target"),
        }
    }
    if request.start_frame < MIN_ANIMATION_FRAME
        || request.end_frame > MAX_ANIMATION_FRAME
        || request.end_frame <= request.start_frame
        || request.end_frame - request.start_frame + 1 > MAX_ANIMATION_FRAME_COUNT
    {
        return Err(AnimationPlanningError::new(
            AnimationPlanningErrorCode::InvalidFrameRange,
            "$.endFrame",
        ));
    }
    if !SUPPORTED_ANIMATION_FRAME_RATES.contains(&request.frame_rate) {
        return Err(AnimationPlanningError::new(
            AnimationPlanningErrorCode::UnsupportedFrameRate,
            "$.frameRate",
        ));
    }
    let (minimum, maximum) = match request.operation {
        AnimationOperation::Translation => (-MAX_SCENE_LOCATION, MAX_SCENE_LOCATION),
        AnimationOperation::Rotation => (-MAX_ROTATION_RADIANS, MAX_ROTATION_RADIANS),
        AnimationOperation::Scale => (MIN_OBJECT_SCALE, MAX_OBJECT_SCALE),
    };
    if !vector_in_range(request.start_value, minimum, maximum)
        || !vector_in_range(request.end_value, minimum, maximum)
    {
        return Err(AnimationPlanningError::new(
            AnimationPlanningErrorCode::InvalidValue,
            "$.startValue",
        ));
    }
    Ok(())
}

fn vector_in_range(value: super::blender_actions::Vector3, minimum: f64, maximum: f64) -> bool {
    [value.x, value.y, value.z]
        .into_iter()
        .all(|component| component.is_finite() && (minimum..=maximum).contains(&component))
}

fn vector_values(value: super::blender_actions::Vector3) -> [f64; 3] {
    [value.x, value.y, value.z]
}

#[cfg(test)]
#[path = "animation_planning_tests.rs"]
mod tests;
