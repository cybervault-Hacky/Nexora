//! Strict, transport-independent scene snapshot validation and edit planning.
//!
//! Snapshot values are untrusted data. This module never reads Blender, infers
//! a scene from installation state, authorizes a mutation, or executes a plan.
use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::blender_actions::{
    parse_candidate_json, ActionClass, ActionPermission, ConfirmationRequirement, ObjectTransform,
    PrimitiveType, ValidatedBatch, MAX_CANDIDATE_BYTES, MAX_OBJECT_NAME_CHARS, MAX_OBJECT_SCALE,
    MAX_ROTATION_RADIANS, MAX_SCENE_LOCATION, MIN_OBJECT_SCALE,
};

pub const SCENE_SNAPSHOT_SCHEMA_VERSION: u16 = 1;
pub const SCENE_EDIT_REQUEST_SCHEMA_VERSION: u16 = 1;
pub const MAX_SCENE_SNAPSHOT_BYTES: usize = 256 * 1024;
pub const MAX_SCENE_EDIT_REQUEST_BYTES: usize = MAX_CANDIDATE_BYTES;
pub const MAX_SNAPSHOT_OBJECTS: usize = 1_000;
pub const MAX_SNAPSHOT_COLLECTIONS: usize = 256;
pub const MAX_COLLECTIONS_PER_OBJECT: usize = 32;
pub const MAX_SCENE_NAME_CHARS: usize = 128;
pub const MAX_SCENE_EDIT_DESCRIPTION_CHARS: usize = 4_096;
pub const MAX_SNAPSHOT_REPORTED_ITEMS: u32 = 10_000_000;
const MAX_SAFE_UNIX_MILLIS: u64 = 8_640_000_000_000_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SceneObjectType {
    Mesh,
    Curve,
    Camera,
    Light,
    Empty,
    Other,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SceneObjectSnapshot {
    pub object_id: String,
    pub name: String,
    pub object_type: SceneObjectType,
    pub transform: ObjectTransform,
    #[serde(default)]
    pub parent_object_id: Option<String>,
    pub collection_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SceneCollectionSnapshot {
    pub collection_id: String,
    pub name: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SnapshotCompleteness {
    pub complete: bool,
    pub truncated: bool,
    pub reported_object_count: u32,
    pub reported_collection_count: u32,
}

/// A future, read-only scene snapshot. `capturedAtUnixMs` records source
/// metadata; it does not by itself prove that the scene is current or genuine.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SceneSnapshot {
    pub schema_version: u16,
    pub scene_reference: String,
    pub scene_name: String,
    #[serde(default)]
    pub revision: Option<String>,
    pub captured_at_unix_ms: u64,
    pub completeness: SnapshotCompleteness,
    pub objects: Vec<SceneObjectSnapshot>,
    pub collections: Vec<SceneCollectionSnapshot>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SnapshotValidationErrorCode {
    PayloadTooLarge,
    MalformedSnapshot,
    UnsupportedSchemaVersion,
    InvalidIdentifier,
    InvalidString,
    InvalidTimestamp,
    TooManyObjects,
    TooManyCollections,
    InvalidCompleteness,
    DuplicateIdentifier,
    InvalidTransform,
    InvalidRelationship,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SnapshotValidationError {
    pub code: SnapshotValidationErrorCode,
    /// Static/index-based path; untrusted values are never copied into errors.
    pub path: String,
}

impl SnapshotValidationError {
    fn new(code: SnapshotValidationErrorCode, path: impl Into<String>) -> Self {
        Self {
            code,
            path: path.into(),
        }
    }
}

/// Constructible only after strict deserialization and all semantic checks.
#[derive(Clone, Debug, PartialEq)]
pub struct ValidatedSceneSnapshot {
    snapshot: SceneSnapshot,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SceneSnapshotState {
    SnapshotUnavailable,
    InvalidSnapshot(SnapshotValidationError),
    StaleSnapshot,
    /// Schema-valid source data only; this does not prove provenance or currentness.
    ValidSnapshot(ValidatedSceneSnapshot),
}

impl ValidatedSceneSnapshot {
    pub fn snapshot(&self) -> &SceneSnapshot {
        &self.snapshot
    }
}

fn assess_snapshot_revision(
    snapshot: &ValidatedSceneSnapshot,
    expected_revision: Option<&str>,
) -> SceneSnapshotState {
    if expected_revision
        .is_some_and(|expected| snapshot.snapshot.revision.as_deref() != Some(expected))
    {
        SceneSnapshotState::StaleSnapshot
    } else {
        SceneSnapshotState::ValidSnapshot(snapshot.clone())
    }
}

pub(crate) fn validate_scene_scope(
    snapshot: &ValidatedSceneSnapshot,
    scene_reference: &str,
    expected_revision: Option<&str>,
    mutation: bool,
) -> Result<bool, ScenePlanningError> {
    let scene = snapshot.snapshot();
    if scene_reference != scene.scene_reference {
        return Err(ScenePlanningError::new(
            ScenePlanningErrorCode::SceneMismatch,
            "$.sceneReference",
        ));
    }
    if matches!(
        assess_snapshot_revision(snapshot, expected_revision),
        SceneSnapshotState::StaleSnapshot
    ) {
        return Err(ScenePlanningError::new(
            ScenePlanningErrorCode::StaleSnapshot,
            "$.expectedSnapshotRevision",
        ));
    }
    if mutation && scene.revision.is_some() && expected_revision.is_none() {
        return Err(ScenePlanningError::new(
            ScenePlanningErrorCode::SnapshotRevisionRequired,
            "$.expectedSnapshotRevision",
        ));
    }
    Ok(expected_revision.is_some() && scene.revision.is_some())
}

pub fn assess_scene_snapshot_json(input: Option<&str>) -> SceneSnapshotState {
    let Some(input) = input else {
        return SceneSnapshotState::SnapshotUnavailable;
    };
    match parse_scene_snapshot_json(input) {
        Ok(snapshot) => SceneSnapshotState::ValidSnapshot(snapshot),
        Err(error) => SceneSnapshotState::InvalidSnapshot(error),
    }
}

pub fn parse_scene_snapshot_json(
    input: &str,
) -> Result<ValidatedSceneSnapshot, SnapshotValidationError> {
    if input.len() > MAX_SCENE_SNAPSHOT_BYTES {
        return Err(SnapshotValidationError::new(
            SnapshotValidationErrorCode::PayloadTooLarge,
            "$",
        ));
    }
    let snapshot: SceneSnapshot = serde_json::from_str(input).map_err(|_| {
        SnapshotValidationError::new(SnapshotValidationErrorCode::MalformedSnapshot, "$")
    })?;
    validate_scene_snapshot(snapshot)
}

pub fn validate_scene_snapshot(
    snapshot: SceneSnapshot,
) -> Result<ValidatedSceneSnapshot, SnapshotValidationError> {
    let fail = |code, path: String| Err(SnapshotValidationError::new(code, path));
    if snapshot.schema_version != SCENE_SNAPSHOT_SCHEMA_VERSION {
        return fail(
            SnapshotValidationErrorCode::UnsupportedSchemaVersion,
            "$.schemaVersion".into(),
        );
    }
    if !valid_identifier(&snapshot.scene_reference) {
        return fail(
            SnapshotValidationErrorCode::InvalidIdentifier,
            "$.sceneReference".into(),
        );
    }
    if !valid_text(&snapshot.scene_name, MAX_SCENE_NAME_CHARS) {
        return fail(
            SnapshotValidationErrorCode::InvalidString,
            "$.sceneName".into(),
        );
    }
    if snapshot.captured_at_unix_ms == 0 || snapshot.captured_at_unix_ms > MAX_SAFE_UNIX_MILLIS {
        return fail(
            SnapshotValidationErrorCode::InvalidTimestamp,
            "$.capturedAtUnixMs".into(),
        );
    }
    if let Some(revision) = &snapshot.revision {
        if !valid_identifier(revision) {
            return fail(
                SnapshotValidationErrorCode::InvalidIdentifier,
                "$.revision".into(),
            );
        }
    }
    if snapshot.objects.len() > MAX_SNAPSHOT_OBJECTS {
        return fail(
            SnapshotValidationErrorCode::TooManyObjects,
            "$.objects".into(),
        );
    }
    if snapshot.collections.len() > MAX_SNAPSHOT_COLLECTIONS {
        return fail(
            SnapshotValidationErrorCode::TooManyCollections,
            "$.collections".into(),
        );
    }
    let included_objects = snapshot.objects.len() as u32;
    let included_collections = snapshot.collections.len() as u32;
    let completeness = snapshot.completeness;
    if completeness.reported_object_count < included_objects
        || completeness.reported_collection_count < included_collections
        || completeness.reported_object_count > MAX_SNAPSHOT_REPORTED_ITEMS
        || completeness.reported_collection_count > MAX_SNAPSHOT_REPORTED_ITEMS
        || (completeness.complete
            && (completeness.truncated
                || completeness.reported_object_count != included_objects
                || completeness.reported_collection_count != included_collections))
        || (completeness.truncated && completeness.complete)
    {
        return fail(
            SnapshotValidationErrorCode::InvalidCompleteness,
            "$.completeness".into(),
        );
    }

    let mut object_ids = BTreeSet::new();
    for (index, object) in snapshot.objects.iter().enumerate() {
        let path = format!("$.objects[{index}]");
        if !valid_identifier(&object.object_id) {
            return fail(
                SnapshotValidationErrorCode::InvalidIdentifier,
                format!("{path}.objectId"),
            );
        }
        if !object_ids.insert(object.object_id.as_str()) {
            return fail(
                SnapshotValidationErrorCode::DuplicateIdentifier,
                format!("{path}.objectId"),
            );
        }
        if !valid_text(&object.name, MAX_OBJECT_NAME_CHARS) {
            return fail(
                SnapshotValidationErrorCode::InvalidString,
                format!("{path}.name"),
            );
        }
        if !valid_transform(&object.transform) {
            return fail(
                SnapshotValidationErrorCode::InvalidTransform,
                format!("{path}.transform"),
            );
        }
        if let Some(parent) = &object.parent_object_id {
            if !valid_identifier(parent)
                || (completeness.complete
                    && !snapshot
                        .objects
                        .iter()
                        .any(|candidate| candidate.object_id == *parent))
            {
                return fail(
                    SnapshotValidationErrorCode::InvalidRelationship,
                    format!("{path}.parentObjectId"),
                );
            }
        }
        if object.collection_ids.len() > MAX_COLLECTIONS_PER_OBJECT {
            return fail(
                SnapshotValidationErrorCode::InvalidRelationship,
                format!("{path}.collectionIds"),
            );
        }
        let mut memberships = BTreeSet::new();
        for (membership_index, collection_id) in object.collection_ids.iter().enumerate() {
            if !valid_identifier(collection_id) || !memberships.insert(collection_id.as_str()) {
                return fail(
                    SnapshotValidationErrorCode::InvalidRelationship,
                    format!("{path}.collectionIds[{membership_index}]"),
                );
            }
        }
    }

    let mut collection_ids = BTreeSet::new();
    for (index, collection) in snapshot.collections.iter().enumerate() {
        let path = format!("$.collections[{index}]");
        if !valid_identifier(&collection.collection_id) {
            return fail(
                SnapshotValidationErrorCode::InvalidIdentifier,
                format!("{path}.collectionId"),
            );
        }
        if !collection_ids.insert(collection.collection_id.as_str()) {
            return fail(
                SnapshotValidationErrorCode::DuplicateIdentifier,
                format!("{path}.collectionId"),
            );
        }
        if !valid_text(&collection.name, MAX_OBJECT_NAME_CHARS) {
            return fail(
                SnapshotValidationErrorCode::InvalidString,
                format!("{path}.name"),
            );
        }
    }
    if completeness.complete {
        for (index, object) in snapshot.objects.iter().enumerate() {
            if object
                .collection_ids
                .iter()
                .any(|id| !collection_ids.contains(id.as_str()))
            {
                return fail(
                    SnapshotValidationErrorCode::InvalidRelationship,
                    format!("$.objects[{index}].collectionIds"),
                );
            }
        }
    }
    if has_parent_cycle(&snapshot.objects) {
        return fail(
            SnapshotValidationErrorCode::InvalidRelationship,
            "$.objects".into(),
        );
    }
    Ok(ValidatedSceneSnapshot { snapshot })
}

fn has_parent_cycle(objects: &[SceneObjectSnapshot]) -> bool {
    let parent_by_id: BTreeMap<&str, Option<&str>> = objects
        .iter()
        .map(|object| {
            (
                object.object_id.as_str(),
                object.parent_object_id.as_deref(),
            )
        })
        .collect();
    for object in objects {
        let mut visited = BTreeSet::new();
        let mut current = Some(object.object_id.as_str());
        while let Some(id) = current {
            if !visited.insert(id) {
                return true;
            }
            current = parent_by_id.get(id).copied().flatten();
        }
    }
    false
}

pub(crate) fn valid_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    value.chars().count() <= 64
        && chars.next().is_some_and(|c| c.is_ascii_alphanumeric())
        && chars.all(|c| c.is_ascii_alphanumeric() || "._:-".contains(c))
}

pub(crate) fn valid_text(value: &str, max_chars: usize) -> bool {
    !value.trim().is_empty()
        && value.chars().count() <= max_chars
        && !value.chars().any(char::is_control)
}

fn valid_transform(transform: &ObjectTransform) -> bool {
    let in_range = |value: f64, minimum: f64, maximum: f64| {
        value.is_finite() && value >= minimum && value <= maximum
    };
    [
        transform.location.x,
        transform.location.y,
        transform.location.z,
    ]
    .into_iter()
    .all(|v| in_range(v, -MAX_SCENE_LOCATION, MAX_SCENE_LOCATION))
        && [
            transform.rotation.x,
            transform.rotation.y,
            transform.rotation.z,
        ]
        .into_iter()
        .all(|v| in_range(v, -MAX_ROTATION_RADIANS, MAX_ROTATION_RADIANS))
        && [transform.scale.x, transform.scale.y, transform.scale.z]
            .into_iter()
            .all(|v| in_range(v, MIN_OBJECT_SCALE, MAX_OBJECT_SCALE))
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ObjectTarget {
    ObjectId { object_id: String },
    ObjectName { name: String },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum SceneEditIntent {
    SceneSummary,
    RenameObject {
        target: ObjectTarget,
        new_name: String,
    },
    SetObjectTransform {
        target: ObjectTarget,
        transform: ObjectTransform,
    },
    CreatePrimitive {
        primitive: PrimitiveType,
        name: Option<String>,
        transform: ObjectTransform,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SceneEditRequest {
    pub schema_version: u16,
    pub request_id: String,
    pub scene_reference: String,
    pub user_request: String,
    #[serde(default)]
    pub expected_snapshot_revision: Option<String>,
    pub intent: SceneEditIntent,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ScenePlanState {
    PlanCreated,
    AwaitingConfirmation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SceneExecutionStatus {
    ConnectorUnavailable,
    /// Reserved for a future reviewed connector; no Phase 8 path emits this.
    Executed,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum PlannedSceneAction {
    SceneSummary,
    RenameObject {
        object_id: String,
        current_name: String,
        new_name: String,
    },
    SetObjectTransform {
        object_id: String,
        object_name: String,
        transform: ObjectTransform,
    },
    CreatePrimitive {
        primitive: PrimitiveType,
        name: Option<String>,
        transform: ObjectTransform,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneEditPlanPreview {
    pub request: SceneEditRequest,
    pub action_id: String,
    pub action_class: ActionClass,
    pub validation_status: &'static str,
    pub required_permission: ActionPermission,
    pub confirmation_policy: ConfirmationRequirement,
    pub plan_state: ScenePlanState,
    pub execution_status: SceneExecutionStatus,
    pub snapshot_revision: Option<String>,
    pub snapshot_complete: bool,
    pub snapshot_truncated: bool,
    pub freshness_checked_by_revision: bool,
    pub planned_action: PlannedSceneAction,
    pub limitations: Vec<String>,
}

/// Carries a Phase 6-validated candidate but is not an `AuthorizedBatch` and
/// cannot be passed to the executor. The preview is safe to display as data.
#[derive(Clone, Debug, PartialEq)]
pub struct SceneEditPlan {
    preview: SceneEditPlanPreview,
    candidate: ValidatedBatch,
}

impl SceneEditPlan {
    pub fn preview(&self) -> &SceneEditPlanPreview {
        &self.preview
    }
    pub fn validated_candidate(&self) -> &ValidatedBatch {
        &self.candidate
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ScenePlanningErrorCode {
    SnapshotUnavailable,
    InvalidSnapshot,
    StaleSnapshot,
    SceneMismatch,
    InvalidRequest,
    SnapshotRevisionRequired,
    TargetNotFound,
    AmbiguousTarget,
    InvalidCandidate,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ScenePlanningError {
    pub code: ScenePlanningErrorCode,
    pub path: String,
}

impl ScenePlanningError {
    fn new(code: ScenePlanningErrorCode, path: impl Into<String>) -> Self {
        Self {
            code,
            path: path.into(),
        }
    }
}

pub fn plan_scene_edit_json(
    snapshot_json: Option<&str>,
    request_json: &str,
) -> Result<SceneEditPlan, ScenePlanningError> {
    if request_json.len() > MAX_SCENE_EDIT_REQUEST_BYTES {
        return Err(ScenePlanningError::new(
            ScenePlanningErrorCode::InvalidRequest,
            "$",
        ));
    }
    let request: SceneEditRequest = serde_json::from_str(request_json)
        .map_err(|_| ScenePlanningError::new(ScenePlanningErrorCode::InvalidRequest, "$"))?;
    let snapshot = match assess_scene_snapshot_json(snapshot_json) {
        SceneSnapshotState::SnapshotUnavailable => {
            return Err(ScenePlanningError::new(
                ScenePlanningErrorCode::SnapshotUnavailable,
                "$.snapshot",
            ));
        }
        SceneSnapshotState::InvalidSnapshot(_) => {
            return Err(ScenePlanningError::new(
                ScenePlanningErrorCode::InvalidSnapshot,
                "$.snapshot",
            ));
        }
        SceneSnapshotState::StaleSnapshot => {
            return Err(ScenePlanningError::new(
                ScenePlanningErrorCode::StaleSnapshot,
                "$.snapshot",
            ));
        }
        SceneSnapshotState::ValidSnapshot(snapshot) => snapshot,
    };
    plan_scene_edit(Some(&snapshot), &request)
}

pub fn plan_scene_edit(
    snapshot: Option<&ValidatedSceneSnapshot>,
    request: &SceneEditRequest,
) -> Result<SceneEditPlan, ScenePlanningError> {
    let Some(snapshot) = snapshot else {
        return Err(ScenePlanningError::new(
            ScenePlanningErrorCode::SnapshotUnavailable,
            "$.snapshot",
        ));
    };
    validate_request(request)?;
    let scene = snapshot.snapshot();
    let mutation = matches!(
        &request.intent,
        SceneEditIntent::RenameObject { .. }
            | SceneEditIntent::SetObjectTransform { .. }
            | SceneEditIntent::CreatePrimitive { .. }
    );
    let freshness_checked_by_revision = validate_scene_scope(
        snapshot,
        &request.scene_reference,
        request.expected_snapshot_revision.as_deref(),
        mutation,
    )?;

    let (action_id, parameters, planned_action) = match &request.intent {
        SceneEditIntent::SceneSummary => (
            super::blender_actions::GET_SCENE_SUMMARY_ID,
            json!({}),
            PlannedSceneAction::SceneSummary,
        ),
        SceneEditIntent::RenameObject { target, new_name } => {
            let object = resolve_target(scene, target)?;
            (
                super::blender_actions::RENAME_OBJECT_ID,
                json!({ "object_id": object.object_id, "new_name": new_name }),
                PlannedSceneAction::RenameObject {
                    object_id: object.object_id.clone(),
                    current_name: object.name.clone(),
                    new_name: new_name.clone(),
                },
            )
        }
        SceneEditIntent::SetObjectTransform { target, transform } => {
            let object = resolve_target(scene, target)?;
            (
                super::blender_actions::SET_OBJECT_TRANSFORM_ID,
                transform_parameters(&object.object_id, transform),
                PlannedSceneAction::SetObjectTransform {
                    object_id: object.object_id.clone(),
                    object_name: object.name.clone(),
                    transform: *transform,
                },
            )
        }
        SceneEditIntent::CreatePrimitive {
            primitive,
            name,
            transform,
        } => (
            super::blender_actions::CREATE_PRIMITIVE_ID,
            create_primitive_parameters(*primitive, name.as_deref(), transform),
            PlannedSceneAction::CreatePrimitive {
                primitive: *primitive,
                name: name.clone(),
                transform: *transform,
            },
        ),
    };
    let candidate_value = json!({
        "contract_version": super::blender_actions::ACTION_CONTRACT_VERSION,
        "request_id": request.request_id,
        "actions": [{
            "action_id": action_id,
            "contract_version": super::blender_actions::ACTION_CONTRACT_VERSION,
            "parameters": parameters,
        }],
    });
    let candidate_json = serde_json::to_string(&candidate_value)
        .map_err(|_| ScenePlanningError::new(ScenePlanningErrorCode::InvalidCandidate, "$"))?;
    if candidate_json.len() > MAX_CANDIDATE_BYTES {
        return Err(ScenePlanningError::new(
            ScenePlanningErrorCode::InvalidCandidate,
            "$",
        ));
    }
    let candidate = parse_candidate_json(&candidate_json).map_err(|_| {
        ScenePlanningError::new(ScenePlanningErrorCode::InvalidCandidate, "$.actions")
    })?;
    let descriptor = candidate.actions()[0].descriptor();
    let plan_state = if descriptor.class == ActionClass::Mutation {
        ScenePlanState::AwaitingConfirmation
    } else {
        ScenePlanState::PlanCreated
    };
    let mut limitations = Vec::new();
    if scene.revision.is_none() {
        limitations.push(
            "The snapshot provides no revision; stale-scene detection is limited.".to_string(),
        );
    }
    if !scene.completeness.complete || scene.completeness.truncated {
        limitations.push("The supplied scene snapshot is incomplete or truncated; the plan is scoped only to included data.".to_string());
    }
    limitations.push(
        "Plan only: nothing has been applied. The Blender executor is unavailable.".to_string(),
    );
    Ok(SceneEditPlan {
        preview: SceneEditPlanPreview {
            request: request.clone(),
            action_id: descriptor.action_id.to_string(),
            action_class: descriptor.class,
            validation_status: "valid",
            required_permission: descriptor.permission,
            confirmation_policy: descriptor.confirmation,
            plan_state,
            execution_status: SceneExecutionStatus::ConnectorUnavailable,
            snapshot_revision: scene.revision.clone(),
            snapshot_complete: scene.completeness.complete,
            snapshot_truncated: scene.completeness.truncated,
            freshness_checked_by_revision,
            planned_action,
            limitations,
        },
        candidate,
    })
}

fn validate_request(request: &SceneEditRequest) -> Result<(), ScenePlanningError> {
    let invalid = |path| {
        Err(ScenePlanningError::new(
            ScenePlanningErrorCode::InvalidRequest,
            path,
        ))
    };
    if request.schema_version != SCENE_EDIT_REQUEST_SCHEMA_VERSION {
        return invalid("$.schemaVersion");
    }
    if !valid_identifier(&request.request_id) || request.request_id.len() > 64 {
        return invalid("$.requestId");
    }
    if !valid_identifier(&request.scene_reference) {
        return invalid("$.sceneReference");
    }
    if !valid_text(&request.user_request, MAX_SCENE_EDIT_DESCRIPTION_CHARS) {
        return invalid("$.userRequest");
    }
    if let Some(revision) = &request.expected_snapshot_revision {
        if !valid_identifier(revision) {
            return invalid("$.expectedSnapshotRevision");
        }
    }
    match &request.intent {
        SceneEditIntent::SceneSummary => {}
        SceneEditIntent::RenameObject { target, new_name } => {
            validate_target(target)?;
            if !valid_text(new_name, MAX_OBJECT_NAME_CHARS) {
                return invalid("$.intent.newName");
            }
        }
        SceneEditIntent::SetObjectTransform { target, transform } => {
            validate_target(target)?;
            if !valid_transform(transform) {
                return invalid("$.intent.transform");
            }
        }
        SceneEditIntent::CreatePrimitive {
            name, transform, ..
        } => {
            if name
                .as_ref()
                .is_some_and(|value| !valid_text(value, MAX_OBJECT_NAME_CHARS))
            {
                return invalid("$.intent.name");
            }
            if !valid_transform(transform) {
                return invalid("$.intent.transform");
            }
        }
    }
    Ok(())
}

fn validate_target(target: &ObjectTarget) -> Result<(), ScenePlanningError> {
    let invalid =
        || ScenePlanningError::new(ScenePlanningErrorCode::InvalidRequest, "$.intent.target");
    match target {
        ObjectTarget::ObjectId { object_id } if valid_identifier(object_id) => Ok(()),
        ObjectTarget::ObjectName { name } if valid_text(name, MAX_OBJECT_NAME_CHARS) => Ok(()),
        _ => Err(invalid()),
    }
}

pub(crate) fn resolve_target<'a>(
    snapshot: &'a SceneSnapshot,
    target: &ObjectTarget,
) -> Result<&'a SceneObjectSnapshot, ScenePlanningError> {
    match target {
        ObjectTarget::ObjectId { object_id } => snapshot
            .objects
            .iter()
            .find(|object| object.object_id == *object_id)
            .ok_or_else(|| {
                ScenePlanningError::new(
                    ScenePlanningErrorCode::TargetNotFound,
                    "$.intent.target.objectId",
                )
            }),
        ObjectTarget::ObjectName { name } => {
            let mut matches = snapshot
                .objects
                .iter()
                .filter(|object| object.name == *name);
            let Some(found) = matches.next() else {
                return Err(ScenePlanningError::new(
                    ScenePlanningErrorCode::TargetNotFound,
                    "$.intent.target.name",
                ));
            };
            if matches.next().is_some() {
                return Err(ScenePlanningError::new(
                    ScenePlanningErrorCode::AmbiguousTarget,
                    "$.intent.target.name",
                ));
            }
            Ok(found)
        }
    }
}

fn vector_values(vector: super::blender_actions::Vector3) -> [f64; 3] {
    [vector.x, vector.y, vector.z]
}

fn transform_parameters(object_id: &str, transform: &ObjectTransform) -> Value {
    json!({
        "object_id": object_id,
        "location": vector_values(transform.location),
        "rotation": vector_values(transform.rotation),
        "scale": vector_values(transform.scale),
    })
}

fn create_primitive_parameters(
    primitive: PrimitiveType,
    name: Option<&str>,
    transform: &ObjectTransform,
) -> Value {
    let mut parameters = json!({
        "primitive": primitive,
        "location": vector_values(transform.location),
        "rotation": vector_values(transform.rotation),
        "scale": vector_values(transform.scale),
    });
    if let Some(name) = name {
        parameters["name"] = json!(name);
    }
    parameters
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::integrations::blender_actions::{
        ActionClass, ActionPermission, ConfirmationRequirement, Vector3, CREATE_PRIMITIVE_ID,
        GET_SCENE_SUMMARY_ID, RENAME_OBJECT_ID, SET_OBJECT_TRANSFORM_ID,
    };

    fn transform() -> ObjectTransform {
        ObjectTransform {
            location: Vector3 {
                x: 0.0,
                y: 1.0,
                z: 2.0,
            },
            rotation: Vector3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            scale: Vector3 {
                x: 1.0,
                y: 1.0,
                z: 1.0,
            },
        }
    }

    fn snapshot_json(two_named: bool) -> String {
        let duplicate_name = if two_named { "Cube" } else { "Sphere" };
        json!({
            "schemaVersion": 1,
            "sceneReference": "scene-opaque-1",
            "sceneName": "Example",
            "revision": "rev-42",
            "capturedAtUnixMs": 1_720_000_000_000u64,
            "completeness": { "complete": true, "truncated": false, "reportedObjectCount": 2, "reportedCollectionCount": 1 },
            "objects": [
                { "objectId": "obj-cube", "name": "Cube", "objectType": "mesh", "transform": transform(), "parentObjectId": null, "collectionIds": ["collection-main"] },
                { "objectId": "obj-second", "name": duplicate_name, "objectType": "mesh", "transform": transform(), "parentObjectId": "obj-cube", "collectionIds": ["collection-main"] }
            ],
            "collections": [{ "collectionId": "collection-main", "name": "Collection" }]
        }).to_string()
    }

    fn request_json(intent: Value, expected_revision: Option<&str>) -> String {
        json!({
            "schemaVersion": 1,
            "requestId": "request-1",
            "sceneReference": "scene-opaque-1",
            "userRequest": "Apply the requested typed scene change",
            "expectedSnapshotRevision": expected_revision,
            "intent": intent,
        })
        .to_string()
    }

    fn summary_request(revision: Option<&str>) -> String {
        request_json(json!({ "kind": "scene_summary" }), revision)
    }

    fn rename_request(name: Value, revision: Option<&str>) -> String {
        request_json(
            json!({
                "kind": "rename_object",
                "target": name,
                "newName": "Renamed",
            }),
            revision,
        )
    }

    #[test]
    fn validates_bounded_snapshot_and_rejects_unknown_or_duplicate_data() {
        assert!(parse_scene_snapshot_json(&snapshot_json(false)).is_ok());
        let unknown = snapshot_json(false).replace(
            "\"sceneName\":\"Example\",",
            "\"sceneName\":\"Example\",\"path\":\"C:/private\",",
        );
        assert_eq!(
            parse_scene_snapshot_json(&unknown).unwrap_err().code,
            SnapshotValidationErrorCode::MalformedSnapshot
        );
        let duplicate_key = snapshot_json(false).replace(
            "\"schemaVersion\":1,",
            "\"schemaVersion\":1,\"schemaVersion\":1,",
        );
        assert_eq!(
            parse_scene_snapshot_json(&duplicate_key).unwrap_err().code,
            SnapshotValidationErrorCode::MalformedSnapshot
        );
        assert_eq!(
            parse_scene_snapshot_json(&" ".repeat(MAX_SCENE_SNAPSHOT_BYTES + 1))
                .unwrap_err()
                .code,
            SnapshotValidationErrorCode::PayloadTooLarge
        );
    }

    #[test]
    fn distinguishes_unavailable_invalid_stale_and_valid_snapshots() {
        assert!(matches!(
            assess_scene_snapshot_json(None),
            SceneSnapshotState::SnapshotUnavailable
        ));
        assert!(matches!(
            assess_scene_snapshot_json(Some("{}")),
            SceneSnapshotState::InvalidSnapshot(_)
        ));
        let valid_snapshot =
            parse_scene_snapshot_json(&snapshot_json(false)).expect("valid snapshot");
        assert!(matches!(
            assess_snapshot_revision(&valid_snapshot, Some("rev-42")),
            SceneSnapshotState::ValidSnapshot(_)
        ));
        assert!(matches!(
            assess_snapshot_revision(&valid_snapshot, Some("rev-old")),
            SceneSnapshotState::StaleSnapshot
        ));
        assert_eq!(
            plan_scene_edit_json(None, &summary_request(Some("rev-42")))
                .unwrap_err()
                .code,
            ScenePlanningErrorCode::SnapshotUnavailable
        );
        assert_eq!(
            plan_scene_edit_json(Some("{}"), &summary_request(None))
                .unwrap_err()
                .code,
            ScenePlanningErrorCode::InvalidSnapshot
        );
        assert_eq!(
            plan_scene_edit_json(
                Some(&snapshot_json(false)),
                &summary_request(Some("rev-old"))
            )
            .unwrap_err()
            .code,
            ScenePlanningErrorCode::StaleSnapshot
        );
    }

    #[test]
    fn plans_read_only_summary_without_claiming_execution() {
        let plan = plan_scene_edit_json(
            Some(&snapshot_json(false)),
            &summary_request(Some("rev-42")),
        )
        .expect("plan");
        let preview = plan.preview();
        assert_eq!(preview.action_id, GET_SCENE_SUMMARY_ID);
        assert_eq!(preview.action_class, ActionClass::ReadOnly);
        assert_eq!(preview.required_permission, ActionPermission::SceneInspect);
        assert_eq!(preview.confirmation_policy, ConfirmationRequirement::None);
        assert_eq!(preview.plan_state, ScenePlanState::PlanCreated);
        assert_eq!(
            preview.execution_status,
            SceneExecutionStatus::ConnectorUnavailable
        );
        assert_eq!(plan.validated_candidate().actions().len(), 1);
    }

    #[test]
    fn plans_mutations_only_after_unique_snapshot_target_resolution() {
        let target = json!({ "kind": "object_name", "name": "Cube" });
        let plan = plan_scene_edit_json(
            Some(&snapshot_json(false)),
            &rename_request(target.clone(), Some("rev-42")),
        )
        .expect("plan");
        let preview = plan.preview();
        assert_eq!(preview.action_id, RENAME_OBJECT_ID);
        assert_eq!(preview.plan_state, ScenePlanState::AwaitingConfirmation);
        assert_eq!(
            preview.confirmation_policy,
            ConfirmationRequirement::ExplicitUserConfirmation
        );
        assert_eq!(
            preview.execution_status,
            SceneExecutionStatus::ConnectorUnavailable
        );
        assert!(
            matches!(&preview.planned_action, PlannedSceneAction::RenameObject { object_id, .. } if object_id == "obj-cube")
        );
        assert_eq!(
            plan.validated_candidate().actions()[0].action_id(),
            RENAME_OBJECT_ID
        );

        assert_eq!(
            plan_scene_edit_json(
                Some(&snapshot_json(true)),
                &rename_request(target, Some("rev-42"))
            )
            .unwrap_err()
            .code,
            ScenePlanningErrorCode::AmbiguousTarget
        );
        let missing = json!({ "kind": "object_id", "objectId": "not-in-snapshot" });
        assert_eq!(
            plan_scene_edit_json(
                Some(&snapshot_json(false)),
                &rename_request(missing, Some("rev-42"))
            )
            .unwrap_err()
            .code,
            ScenePlanningErrorCode::TargetNotFound
        );
    }

    #[test]
    fn transform_and_create_candidates_pass_phase_six_validation() {
        let target = json!({ "kind": "object_id", "objectId": "obj-cube" });
        let transform_intent =
            json!({ "kind": "set_object_transform", "target": target, "transform": transform() });
        let transform_plan = plan_scene_edit_json(
            Some(&snapshot_json(false)),
            &request_json(transform_intent, Some("rev-42")),
        )
        .expect("transform plan");
        assert_eq!(
            transform_plan.validated_candidate().actions()[0].action_id(),
            SET_OBJECT_TRANSFORM_ID
        );

        let create_intent = json!({ "kind": "create_primitive", "primitive": "cube", "name": "Lamp stand", "transform": transform() });
        let create_plan = plan_scene_edit_json(
            Some(&snapshot_json(false)),
            &request_json(create_intent, Some("rev-42")),
        )
        .expect("create plan");
        assert_eq!(
            create_plan.validated_candidate().actions()[0].action_id(),
            CREATE_PRIMITIVE_ID
        );
        assert_eq!(
            create_plan.preview().confirmation_policy,
            ConfirmationRequirement::ExplicitUserConfirmation
        );
    }

    #[test]
    fn rejects_missing_expected_revision_for_versioned_mutations_and_scene_mismatch() {
        let target = json!({ "kind": "object_id", "objectId": "obj-cube" });
        let rename = rename_request(target, None);
        assert_eq!(
            plan_scene_edit_json(Some(&snapshot_json(false)), &rename)
                .unwrap_err()
                .code,
            ScenePlanningErrorCode::SnapshotRevisionRequired
        );
        let mut request: SceneEditRequest =
            serde_json::from_str(&summary_request(None)).expect("request");
        request.scene_reference = "different-scene".into();
        let snapshot = parse_scene_snapshot_json(&snapshot_json(false)).expect("snapshot");
        assert_eq!(
            plan_scene_edit(Some(&snapshot), &request).unwrap_err().code,
            ScenePlanningErrorCode::SceneMismatch
        );
    }

    #[test]
    fn rejects_phase_six_out_of_bounds_and_unknown_request_fields() {
        let invalid_transform = json!({ "location": { "x": 10_001.0, "y": 0.0, "z": 0.0 }, "rotation": { "x": 0.0, "y": 0.0, "z": 0.0 }, "scale": { "x": 1.0, "y": 1.0, "z": 1.0 } });
        let intent = json!({ "kind": "create_primitive", "primitive": "cube", "transform": invalid_transform });
        assert_eq!(
            plan_scene_edit_json(
                Some(&snapshot_json(false)),
                &request_json(intent, Some("rev-42"))
            )
            .unwrap_err()
            .code,
            ScenePlanningErrorCode::InvalidRequest
        );
        let unknown =
            summary_request(None).replace("\"intent\":", "\"unexpected\":true,\"intent\":");
        assert_eq!(
            plan_scene_edit_json(Some(&snapshot_json(false)), &unknown)
                .unwrap_err()
                .code,
            ScenePlanningErrorCode::InvalidRequest
        );
        assert_eq!(
            plan_scene_edit_json(
                Some(&snapshot_json(false)),
                &" ".repeat(MAX_SCENE_EDIT_REQUEST_BYTES + 1)
            )
            .unwrap_err()
            .code,
            ScenePlanningErrorCode::InvalidRequest
        );
    }

    #[test]
    fn rejects_snapshot_relationship_cycles_and_duplicate_ids() {
        let cycle = snapshot_json(false).replace(
            "\"parentObjectId\":\"obj-cube\"",
            "\"parentObjectId\":\"obj-second\"",
        );
        assert_eq!(
            parse_scene_snapshot_json(&cycle).unwrap_err().code,
            SnapshotValidationErrorCode::InvalidRelationship
        );
        let duplicate = snapshot_json(false)
            .replace("\"objectId\":\"obj-second\"", "\"objectId\":\"obj-cube\"");
        assert_eq!(
            parse_scene_snapshot_json(&duplicate).unwrap_err().code,
            SnapshotValidationErrorCode::DuplicateIdentifier
        );
    }
}
