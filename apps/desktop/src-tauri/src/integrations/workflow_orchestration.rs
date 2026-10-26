//! Bounded orchestration of Phase 6–11 planning contracts.
//!
//! This module plans and summarizes typed inputs only. Dependencies express
//! review/planning order; they never imply that earlier actions were applied.
use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::image_references::{valid_reference, ImageReferenceMetadata};
use crate::resource_management::MediaResourceCoordinator;

use super::animation_planning::{
    plan_animation, AnimationPlanPreview, AnimationPlanningErrorCode, AnimationRequest,
};
use super::blender_actions::{
    ActionClass, ActionPermission, ConfirmationRequirement, ValidatedBatch,
};
use super::preview_planning::{
    plan_blender_preview, BlenderPreviewErrorCode, BlenderPreviewPlanPreview,
    BlenderPreviewPlanStatus, BlenderPreviewRequest,
};
use super::scene_planning::{
    plan_scene_edit, validate_scene_scope, SceneEditPlanPreview, SceneEditRequest,
    ScenePlanningErrorCode, ValidatedSceneSnapshot,
};

pub const WORKFLOW_CONTRACT_VERSION: u16 = 1;
pub const MAX_WORKFLOW_REQUEST_BYTES: usize = 32 * 1024;
pub const MAX_WORKFLOW_ID_CHARS: usize = 64;
pub const MAX_WORKFLOW_DESCRIPTION_CHARS: usize = 2_048;
pub const MAX_WORKFLOW_STEPS: usize = 8;
pub const MAX_DEPENDENCIES_PER_STEP: usize = MAX_WORKFLOW_STEPS - 1;
pub const MAX_WORKFLOW_IMAGE_REFERENCES: usize = 4;
pub const MAX_WORKFLOW_REFERENCE_DESCRIPTION_CHARS: usize = 1_024;
pub const MAX_WORKFLOW_REFERENCE_METADATA_BYTES: usize = 4 * 1024;
pub const MAX_WORKFLOW_PREVIEW_BYTES: usize = 128 * 1024;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkflowRequest {
    pub schema_version: u16,
    pub workflow_id: String,
    pub task_description: String,
    #[serde(default)]
    pub scene_reference: Option<String>,
    #[serde(default)]
    pub expected_scene_revision: Option<String>,
    pub steps: Vec<WorkflowStep>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkflowStep {
    pub step_id: String,
    #[serde(default)]
    pub depends_on: Vec<String>,
    pub step: WorkflowStepKind,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum WorkflowStepKind {
    InspectSuppliedSnapshot,
    PlanSceneEdit { request: SceneEditRequest },
    PlanAnimation { request: AnimationRequest },
    PlanPreview { request: BlenderPreviewRequest },
    AssociateImageReference { reference_id: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowStepType {
    InspectSuppliedSnapshot,
    PlanSceneEdit,
    PlanAnimation,
    PlanPreview,
    AssociateImageReference,
}

impl WorkflowStepKind {
    fn step_type(&self) -> WorkflowStepType {
        match self {
            Self::InspectSuppliedSnapshot => WorkflowStepType::InspectSuppliedSnapshot,
            Self::PlanSceneEdit { .. } => WorkflowStepType::PlanSceneEdit,
            Self::PlanAnimation { .. } => WorkflowStepType::PlanAnimation,
            Self::PlanPreview { .. } => WorkflowStepType::PlanPreview,
            Self::AssociateImageReference { .. } => WorkflowStepType::AssociateImageReference,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowPlanStatus {
    Planned,
    Invalid,
    Blocked,
    AwaitingConfirmation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowExecutionStatus {
    NotRequested,
    ConnectorUnavailable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowStepStatus {
    Planned,
    Invalid,
    Blocked,
    AwaitingConfirmation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowValidationStatus {
    Valid,
    Invalid,
    Blocked,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowStepExecutionStatus {
    NotRequested,
    ConnectorUnavailable,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotInspectionPreview {
    pub scene_reference: String,
    pub scene_name: String,
    pub snapshot_revision: Option<String>,
    pub object_count: u32,
    pub collection_count: u32,
    pub complete: bool,
    pub truncated: bool,
    pub freshness_checked_by_revision: bool,
    pub note: &'static str,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageReferenceAssociationPreview {
    pub reference_id: String,
    pub format: String,
    pub width: u32,
    pub height: u32,
    pub size_bytes: u64,
    pub user_task_description: String,
    pub analysis_available: bool,
    pub note: &'static str,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "kind", content = "plan", rename_all = "snake_case")]
pub enum WorkflowStepDetails {
    SnapshotInspection(SnapshotInspectionPreview),
    SceneEdit(SceneEditPlanPreview),
    Animation(AnimationPlanPreview),
    Preview(BlenderPreviewPlanPreview),
    ImageReference(ImageReferenceAssociationPreview),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowStepErrorCode {
    InvalidRequest,
    SnapshotUnavailable,
    StaleSnapshot,
    SceneMismatch,
    SnapshotRevisionRequired,
    SnapshotRevisionUnavailable,
    InvalidCandidate,
    MissingTarget,
    TargetNotFound,
    AmbiguousTarget,
    IncompleteSnapshotForNameResolution,
    MissingImageReference,
    AmbiguousImageReference,
    InvalidImageReference,
    PreviewResourceAdmissionUnknown,
    PreviewResourceAdmissionDeferred,
    DependencyFailed,
    DependencyUnavailable,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct WorkflowStepError {
    pub code: WorkflowStepErrorCode,
    /// Fixed JSON path; never contains an untrusted value.
    pub path: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowStepPreview {
    pub step_id: String,
    pub step_type: WorkflowStepType,
    pub depends_on: Vec<String>,
    pub status: WorkflowStepStatus,
    pub validation_status: WorkflowValidationStatus,
    pub execution_status: WorkflowStepExecutionStatus,
    pub required_permission: Option<ActionPermission>,
    pub confirmation_policy: Option<ConfirmationRequirement>,
    pub details: Option<WorkflowStepDetails>,
    pub error: Option<WorkflowStepError>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowPlanPreview {
    pub schema_version: u16,
    pub workflow_id: String,
    pub task_description: String,
    pub scene_reference: Option<String>,
    pub expected_scene_revision: Option<String>,
    pub status: WorkflowPlanStatus,
    pub execution_status: WorkflowExecutionStatus,
    pub required_permissions: Vec<ActionPermission>,
    pub confirmation_required: bool,
    pub steps: Vec<WorkflowStepPreview>,
    pub limitations: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WorkflowPlan {
    preview: WorkflowPlanPreview,
    /// Exact Phase 6-validated candidates, retained only in memory for
    /// confirmation comparison. They are not authorized and are not serialized.
    validated_candidates: Vec<(String, ValidatedBatch)>,
}

impl WorkflowPlan {
    pub fn preview(&self) -> &WorkflowPlanPreview {
        &self.preview
    }

    /// Returns a non-serializable comparison binding, not a consent grant.
    pub fn confirmation_binding(&self) -> Option<WorkflowConfirmationBinding> {
        (self.preview.status == WorkflowPlanStatus::AwaitingConfirmation).then(|| {
            WorkflowConfirmationBinding {
                preview: self.preview.clone(),
                validated_candidates: self.validated_candidates.clone(),
            }
        })
    }
}

/// Snapshot of the exact preview and validated action batches shown for review.
/// The host must still obtain consent; this value never authorizes execution.
#[derive(Clone, Debug, PartialEq)]
pub struct WorkflowConfirmationBinding {
    preview: WorkflowPlanPreview,
    validated_candidates: Vec<(String, ValidatedBatch)>,
}

impl WorkflowConfirmationBinding {
    pub fn matches(&self, current_plan: &WorkflowPlan) -> bool {
        self.preview == current_plan.preview
            && self.validated_candidates == current_plan.validated_candidates
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowContractErrorCode {
    PayloadTooLarge,
    MalformedWorkflow,
    UnsupportedContractVersion,
    InvalidWorkflowId,
    InvalidDescription,
    InvalidSceneReference,
    InvalidSceneRevision,
    InvalidStepCount,
    InvalidStepId,
    DuplicateStepId,
    TooManyDependencies,
    InvalidDependencyId,
    DuplicateDependency,
    MissingDependency,
    DependencyCycle,
    TooManyImageReferences,
    InvalidInputs,
    PreviewTooLarge,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct WorkflowContractError {
    pub code: WorkflowContractErrorCode,
    pub path: String,
}

impl WorkflowContractError {
    fn new(code: WorkflowContractErrorCode, path: impl Into<String>) -> Self {
        Self {
            code,
            path: path.into(),
        }
    }
}

pub fn plan_workflow_json(
    snapshot: Option<&ValidatedSceneSnapshot>,
    image_references: &[ImageReferenceMetadata],
    workflow_json: &str,
) -> Result<WorkflowPlan, WorkflowContractError> {
    plan_workflow_json_with_resources(snapshot, image_references, workflow_json, None)
}

pub fn plan_workflow_json_with_resources(
    snapshot: Option<&ValidatedSceneSnapshot>,
    image_references: &[ImageReferenceMetadata],
    workflow_json: &str,
    coordinator: Option<&MediaResourceCoordinator>,
) -> Result<WorkflowPlan, WorkflowContractError> {
    if workflow_json.len() > MAX_WORKFLOW_REQUEST_BYTES {
        return Err(WorkflowContractError::new(
            WorkflowContractErrorCode::PayloadTooLarge,
            "$",
        ));
    }
    let request: WorkflowRequest = serde_json::from_str(workflow_json).map_err(|_| {
        WorkflowContractError::new(WorkflowContractErrorCode::MalformedWorkflow, "$")
    })?;
    plan_workflow_with_resources(snapshot, image_references, &request, coordinator)
}

pub fn plan_workflow(
    snapshot: Option<&ValidatedSceneSnapshot>,
    image_references: &[ImageReferenceMetadata],
    request: &WorkflowRequest,
) -> Result<WorkflowPlan, WorkflowContractError> {
    plan_workflow_with_resources(snapshot, image_references, request, None)
}

pub fn plan_workflow_with_resources(
    snapshot: Option<&ValidatedSceneSnapshot>,
    image_references: &[ImageReferenceMetadata],
    request: &WorkflowRequest,
    coordinator: Option<&MediaResourceCoordinator>,
) -> Result<WorkflowPlan, WorkflowContractError> {
    let request_size = serde_json::to_vec(request)
        .map_err(|_| WorkflowContractError::new(WorkflowContractErrorCode::InvalidInputs, "$"))?
        .len();
    if request_size > MAX_WORKFLOW_REQUEST_BYTES {
        return Err(WorkflowContractError::new(
            WorkflowContractErrorCode::PayloadTooLarge,
            "$",
        ));
    }
    validate_workflow_request(request)?;
    if image_references.len() > MAX_WORKFLOW_IMAGE_REFERENCES {
        return Err(WorkflowContractError::new(
            WorkflowContractErrorCode::TooManyImageReferences,
            "$.inputs.imageReferences",
        ));
    }
    for (index, reference) in image_references.iter().enumerate() {
        let metadata_size = serde_json::to_vec(reference)
            .map_err(|_| WorkflowContractError::new(WorkflowContractErrorCode::InvalidInputs, "$"))?
            .len();
        if !valid_reference(reference)
            || reference.description.as_deref().is_some_and(|description| {
                description.chars().count() > MAX_WORKFLOW_REFERENCE_DESCRIPTION_CHARS
            })
            || metadata_size > MAX_WORKFLOW_REFERENCE_METADATA_BYTES
        {
            return Err(WorkflowContractError::new(
                WorkflowContractErrorCode::InvalidInputs,
                format!("$.inputs.imageReferences[{index}]"),
            ));
        }
    }
    let order = stable_topological_order(&request.steps)?;
    let mut status_by_id = BTreeMap::<String, WorkflowStepStatus>::new();
    let mut execution_by_id = BTreeMap::<String, WorkflowStepExecutionStatus>::new();
    let mut step_previews = Vec::with_capacity(request.steps.len());
    let mut validated_candidates = Vec::new();
    let mut permissions = BTreeSet::new();

    for index in order {
        let step = &request.steps[index];
        let path = format!("$.steps[{index}]");
        let failed_dependency = step.depends_on.iter().any(|dependency| {
            matches!(
                status_by_id.get(dependency),
                Some(WorkflowStepStatus::Invalid | WorkflowStepStatus::Blocked)
            )
        });
        let unavailable_dependency = step.depends_on.iter().any(|dependency| {
            execution_by_id.get(dependency)
                == Some(&WorkflowStepExecutionStatus::ConnectorUnavailable)
        });
        let (preview, candidate) = if failed_dependency {
            (
                blocked_step_preview(step, WorkflowStepErrorCode::DependencyFailed, &path),
                None,
            )
        } else if unavailable_dependency {
            (
                blocked_step_preview(step, WorkflowStepErrorCode::DependencyUnavailable, &path),
                None,
            )
        } else {
            plan_step(
                snapshot,
                image_references,
                request,
                step,
                index,
                coordinator,
            )
        };
        if let Some(permission) = preview.required_permission {
            permissions.insert(permission);
        }
        status_by_id.insert(step.step_id.clone(), preview.status);
        execution_by_id.insert(step.step_id.clone(), preview.execution_status);
        if let Some(candidate) = candidate {
            validated_candidates.push((step.step_id.clone(), candidate));
        }
        step_previews.push(preview);
    }

    // `step_previews` already follows the stable topological order. Status lookup
    // and candidate order are deterministic for identical inputs.
    let has_invalid = step_previews
        .iter()
        .any(|step| step.status == WorkflowStepStatus::Invalid);
    let has_blocked = step_previews
        .iter()
        .any(|step| step.status == WorkflowStepStatus::Blocked);
    let has_pending_confirmation = step_previews
        .iter()
        .any(|step| step.status == WorkflowStepStatus::AwaitingConfirmation);
    let status = if has_invalid {
        WorkflowPlanStatus::Invalid
    } else if has_blocked {
        WorkflowPlanStatus::Blocked
    } else if has_pending_confirmation {
        WorkflowPlanStatus::AwaitingConfirmation
    } else {
        WorkflowPlanStatus::Planned
    };
    let confirmation_required = status == WorkflowPlanStatus::AwaitingConfirmation;
    let execution_status = if step_previews
        .iter()
        .any(|step| step.execution_status == WorkflowStepExecutionStatus::ConnectorUnavailable)
    {
        WorkflowExecutionStatus::ConnectorUnavailable
    } else {
        WorkflowExecutionStatus::NotRequested
    };
    let mut limitations = vec![
        "Workflow planning is data-only; dependencies define review order and do not mean prior steps were executed or applied.".to_string(),
        "Each planning step uses the supplied snapshot independently; proposed outputs are not composed into later inputs.".to_string(),
        "The current Blender executor is unavailable; no scene, timeline, image, or rendered output was changed.".to_string(),
    ];
    if step_previews.iter().any(|step| {
        step.step_type == WorkflowStepType::AssociateImageReference
            && step.status == WorkflowStepStatus::Planned
    }) {
        limitations.push(
            "Associated image references were not analyzed by a vision-capable model.".to_string(),
        );
    }
    let preview = WorkflowPlanPreview {
        schema_version: WORKFLOW_CONTRACT_VERSION,
        workflow_id: request.workflow_id.clone(),
        task_description: request.task_description.clone(),
        scene_reference: request.scene_reference.clone(),
        expected_scene_revision: request.expected_scene_revision.clone(),
        status,
        execution_status,
        required_permissions: permissions.into_iter().collect(),
        confirmation_required,
        steps: step_previews,
        limitations,
    };
    let preview_size = serde_json::to_vec(&preview)
        .map_err(|_| WorkflowContractError::new(WorkflowContractErrorCode::InvalidInputs, "$"))?
        .len();
    if preview_size > MAX_WORKFLOW_PREVIEW_BYTES {
        return Err(WorkflowContractError::new(
            WorkflowContractErrorCode::PreviewTooLarge,
            "$",
        ));
    }
    Ok(WorkflowPlan {
        preview,
        validated_candidates,
    })
}

fn validate_workflow_request(request: &WorkflowRequest) -> Result<(), WorkflowContractError> {
    if request.schema_version != WORKFLOW_CONTRACT_VERSION {
        return Err(WorkflowContractError::new(
            WorkflowContractErrorCode::UnsupportedContractVersion,
            "$.schemaVersion",
        ));
    }
    if !valid_token(&request.workflow_id) {
        return Err(WorkflowContractError::new(
            WorkflowContractErrorCode::InvalidWorkflowId,
            "$.workflowId",
        ));
    }
    if !valid_description(&request.task_description, MAX_WORKFLOW_DESCRIPTION_CHARS) {
        return Err(WorkflowContractError::new(
            WorkflowContractErrorCode::InvalidDescription,
            "$.taskDescription",
        ));
    }
    if request
        .scene_reference
        .as_deref()
        .is_some_and(|value| !super::scene_planning::valid_identifier(value))
    {
        return Err(WorkflowContractError::new(
            WorkflowContractErrorCode::InvalidSceneReference,
            "$.sceneReference",
        ));
    }
    if request
        .expected_scene_revision
        .as_deref()
        .is_some_and(|value| !super::scene_planning::valid_identifier(value))
    {
        return Err(WorkflowContractError::new(
            WorkflowContractErrorCode::InvalidSceneRevision,
            "$.expectedSceneRevision",
        ));
    }
    if request.expected_scene_revision.is_some() && request.scene_reference.is_none() {
        return Err(WorkflowContractError::new(
            WorkflowContractErrorCode::InvalidSceneReference,
            "$.sceneReference",
        ));
    }
    if request.steps.is_empty() || request.steps.len() > MAX_WORKFLOW_STEPS {
        return Err(WorkflowContractError::new(
            WorkflowContractErrorCode::InvalidStepCount,
            "$.steps",
        ));
    }
    for (index, step) in request.steps.iter().enumerate() {
        if !valid_token(&step.step_id) {
            return Err(WorkflowContractError::new(
                WorkflowContractErrorCode::InvalidStepId,
                format!("$.steps[{index}].stepId"),
            ));
        }
    }
    Ok(())
}

fn stable_topological_order(steps: &[WorkflowStep]) -> Result<Vec<usize>, WorkflowContractError> {
    let mut index_by_id = BTreeMap::new();
    for (index, step) in steps.iter().enumerate() {
        if index_by_id.insert(step.step_id.as_str(), index).is_some() {
            return Err(WorkflowContractError::new(
                WorkflowContractErrorCode::DuplicateStepId,
                format!("$.steps[{index}].stepId"),
            ));
        }
    }
    let mut indegree = vec![0usize; steps.len()];
    let mut dependents = vec![Vec::<usize>::new(); steps.len()];
    for (index, step) in steps.iter().enumerate() {
        if step.depends_on.len() > MAX_DEPENDENCIES_PER_STEP {
            return Err(WorkflowContractError::new(
                WorkflowContractErrorCode::TooManyDependencies,
                format!("$.steps[{index}].dependsOn"),
            ));
        }
        let mut unique_dependencies = BTreeSet::new();
        for (dependency_index, dependency) in step.depends_on.iter().enumerate() {
            if !valid_token(dependency) {
                return Err(WorkflowContractError::new(
                    WorkflowContractErrorCode::InvalidDependencyId,
                    format!("$.steps[{index}].dependsOn[{dependency_index}]"),
                ));
            }
            if !unique_dependencies.insert(dependency.as_str()) {
                return Err(WorkflowContractError::new(
                    WorkflowContractErrorCode::DuplicateDependency,
                    format!("$.steps[{index}].dependsOn[{dependency_index}]"),
                ));
            }
            let Some(&dependency_step_index) = index_by_id.get(dependency.as_str()) else {
                return Err(WorkflowContractError::new(
                    WorkflowContractErrorCode::MissingDependency,
                    format!("$.steps[{index}].dependsOn[{dependency_index}]"),
                ));
            };
            indegree[index] += 1;
            dependents[dependency_step_index].push(index);
        }
    }

    let mut emitted = vec![false; steps.len()];
    let mut order = Vec::with_capacity(steps.len());
    while order.len() < steps.len() {
        let Some(next) =
            (0..steps.len()).find(|&candidate| !emitted[candidate] && indegree[candidate] == 0)
        else {
            return Err(WorkflowContractError::new(
                WorkflowContractErrorCode::DependencyCycle,
                "$.steps",
            ));
        };
        emitted[next] = true;
        order.push(next);
        for dependent in &dependents[next] {
            indegree[*dependent] -= 1;
        }
    }
    Ok(order)
}

fn plan_step(
    snapshot: Option<&ValidatedSceneSnapshot>,
    image_references: &[ImageReferenceMetadata],
    workflow: &WorkflowRequest,
    step: &WorkflowStep,
    index: usize,
    coordinator: Option<&MediaResourceCoordinator>,
) -> (WorkflowStepPreview, Option<ValidatedBatch>) {
    let path = format!("$.steps[{index}]");
    let mut base = empty_step_preview(step);
    match &step.step {
        WorkflowStepKind::InspectSuppliedSnapshot => {
            let Some(scene_reference) = workflow.scene_reference.as_deref() else {
                return (
                    invalid_step_preview(
                        step,
                        WorkflowStepErrorCode::InvalidRequest,
                        format!("{path}.sceneReference"),
                    ),
                    None,
                );
            };
            let Some(snapshot) = snapshot else {
                return (
                    invalid_step_preview(
                        step,
                        WorkflowStepErrorCode::SnapshotUnavailable,
                        "$.inputs.snapshot",
                    ),
                    None,
                );
            };
            let fresh = match validate_scene_scope(
                snapshot,
                scene_reference,
                workflow.expected_scene_revision.as_deref(),
                false,
            ) {
                Ok(fresh) => fresh,
                Err(error) => {
                    return (
                        invalid_step_preview(step, map_scene_error(error), &path),
                        None,
                    )
                }
            };
            let scene = snapshot.snapshot();
            base.status = WorkflowStepStatus::Planned;
            base.validation_status = WorkflowValidationStatus::Valid;
            base.required_permission = Some(ActionPermission::SceneInspect);
            base.execution_status = WorkflowStepExecutionStatus::NotRequested;
            base.details = Some(WorkflowStepDetails::SnapshotInspection(
                SnapshotInspectionPreview {
                    scene_reference: scene.scene_reference.clone(),
                    scene_name: scene.scene_name.clone(),
                    snapshot_revision: scene.revision.clone(),
                    object_count: scene.objects.len() as u32,
                    collection_count: scene.collections.len() as u32,
                    complete: scene.completeness.complete,
                    truncated: scene.completeness.truncated,
                    freshness_checked_by_revision: fresh,
                    note: "Summarized supplied validated snapshot data; no Blender scene query was made.",
                },
            ));
            (base, None)
        }
        WorkflowStepKind::PlanSceneEdit { request } => {
            if !scope_matches(
                workflow,
                &request.scene_reference,
                request.expected_snapshot_revision.as_deref(),
            ) {
                return (
                    invalid_step_preview(
                        step,
                        WorkflowStepErrorCode::InvalidRequest,
                        format!("{path}.request.sceneReference"),
                    ),
                    None,
                );
            }
            if !matches!(
                &request.intent,
                super::scene_planning::SceneEditIntent::SceneSummary
            ) {
                if let Err(code) = require_exact_mutation_revision(
                    snapshot,
                    workflow.expected_scene_revision.as_deref(),
                ) {
                    return (
                        invalid_step_preview(
                            step,
                            code,
                            format!("{path}.request.expectedSnapshotRevision"),
                        ),
                        None,
                    );
                }
            }
            match plan_scene_edit(snapshot, request) {
                Ok(plan) => {
                    let plan_preview = plan.preview().clone();
                    let candidate = plan.validated_candidate().clone();
                    let mutation = plan_preview.action_class == ActionClass::Mutation;
                    base.status = if mutation {
                        WorkflowStepStatus::AwaitingConfirmation
                    } else {
                        WorkflowStepStatus::Planned
                    };
                    base.validation_status = WorkflowValidationStatus::Valid;
                    base.execution_status = WorkflowStepExecutionStatus::ConnectorUnavailable;
                    base.required_permission = Some(plan_preview.required_permission);
                    base.confirmation_policy = Some(plan_preview.confirmation_policy);
                    base.details = Some(WorkflowStepDetails::SceneEdit(plan_preview));
                    (base, Some(candidate))
                }
                Err(error) => (
                    invalid_step_preview(step, map_scene_error(error), &path),
                    None,
                ),
            }
        }
        WorkflowStepKind::PlanAnimation { request } => {
            if !scope_matches(
                workflow,
                &request.scene_reference,
                request.expected_snapshot_revision.as_deref(),
            ) {
                return (
                    invalid_step_preview(
                        step,
                        WorkflowStepErrorCode::InvalidRequest,
                        format!("{path}.request.sceneReference"),
                    ),
                    None,
                );
            }
            if let Err(code) = require_exact_mutation_revision(
                snapshot,
                workflow.expected_scene_revision.as_deref(),
            ) {
                return (
                    invalid_step_preview(
                        step,
                        code,
                        format!("{path}.request.expectedSnapshotRevision"),
                    ),
                    None,
                );
            }
            match plan_animation(snapshot, request) {
                Ok(plan) => {
                    let plan_preview = plan.preview().clone();
                    let candidate = plan.validated_candidate().clone();
                    base.status = WorkflowStepStatus::AwaitingConfirmation;
                    base.validation_status = WorkflowValidationStatus::Valid;
                    base.execution_status = WorkflowStepExecutionStatus::ConnectorUnavailable;
                    base.required_permission = Some(plan_preview.required_permission);
                    base.confirmation_policy = Some(plan_preview.confirmation_policy);
                    base.details = Some(WorkflowStepDetails::Animation(plan_preview));
                    (base, Some(candidate))
                }
                Err(error) => (
                    invalid_step_preview(step, map_animation_error(error), &path),
                    None,
                ),
            }
        }
        WorkflowStepKind::PlanPreview { request } => {
            if !scope_matches(
                workflow,
                &request.scene_reference,
                Some(&request.expected_scene_revision),
            ) {
                return (
                    invalid_step_preview(
                        step,
                        WorkflowStepErrorCode::InvalidRequest,
                        format!("{path}.request.sceneReference"),
                    ),
                    None,
                );
            }
            match plan_blender_preview(snapshot, coordinator, request) {
                Ok(plan) => {
                    let plan_preview = plan.preview().clone();
                    let blocked = plan_preview.status == BlenderPreviewPlanStatus::Blocked;
                    base.status = if blocked {
                        WorkflowStepStatus::Blocked
                    } else {
                        WorkflowStepStatus::Planned
                    };
                    base.validation_status = if blocked {
                        WorkflowValidationStatus::Blocked
                    } else {
                        WorkflowValidationStatus::Valid
                    };
                    base.execution_status = WorkflowStepExecutionStatus::ConnectorUnavailable;
                    base.required_permission = Some(plan_preview.required_permission);
                    base.confirmation_policy = Some(plan_preview.confirmation_policy);
                    base.error = plan_preview.error.as_ref().map(|error| WorkflowStepError {
                        code: map_preview_error(error.code),
                        path: error.path.clone(),
                    });
                    base.details = Some(WorkflowStepDetails::Preview(plan_preview));
                    (base, None)
                }
                Err(error) => (
                    invalid_step_preview(step, map_preview_error(error.code), error.path),
                    None,
                ),
            }
        }
        WorkflowStepKind::AssociateImageReference { reference_id } => {
            if !valid_reference_id(reference_id) {
                return (
                    invalid_step_preview(
                        step,
                        WorkflowStepErrorCode::InvalidImageReference,
                        format!("{path}.referenceId"),
                    ),
                    None,
                );
            }
            let matching = image_references
                .iter()
                .filter(|reference| reference.reference_id == *reference_id)
                .collect::<Vec<_>>();
            if matching.is_empty() {
                return (
                    invalid_step_preview(
                        step,
                        WorkflowStepErrorCode::MissingImageReference,
                        format!("{path}.referenceId"),
                    ),
                    None,
                );
            }
            if matching.len() > 1 {
                return (
                    invalid_step_preview(
                        step,
                        WorkflowStepErrorCode::AmbiguousImageReference,
                        format!("{path}.referenceId"),
                    ),
                    None,
                );
            }
            let reference = matching[0];
            let encoded_size =
                serde_json::to_vec(reference).map_or(usize::MAX, |bytes| bytes.len());
            if !valid_reference(reference)
                || reference.description.as_deref().is_some_and(|description| {
                    description.chars().count() > MAX_WORKFLOW_REFERENCE_DESCRIPTION_CHARS
                })
                || encoded_size > MAX_WORKFLOW_REFERENCE_METADATA_BYTES
            {
                return (
                    invalid_step_preview(
                        step,
                        WorkflowStepErrorCode::InvalidImageReference,
                        format!("{path}.referenceId"),
                    ),
                    None,
                );
            }
            base.status = WorkflowStepStatus::Planned;
            base.validation_status = WorkflowValidationStatus::Valid;
            base.execution_status = WorkflowStepExecutionStatus::NotRequested;
            base.details = Some(WorkflowStepDetails::ImageReference(
                ImageReferenceAssociationPreview {
                    reference_id: reference.reference_id.clone(),
                    format: reference.format.clone(),
                    width: reference.width,
                    height: reference.height,
                    size_bytes: reference.size_bytes,
                    user_task_description: workflow.task_description.clone(),
                    analysis_available: false,
                    note: "Associated with user-written text only; image content was not analyzed.",
                },
            ));
            (base, None)
        }
    }
}

fn scope_matches(
    workflow: &WorkflowRequest,
    scene_reference: &str,
    expected_revision: Option<&str>,
) -> bool {
    workflow.scene_reference.as_deref() == Some(scene_reference)
        && workflow.expected_scene_revision.as_deref() == expected_revision
}

fn require_exact_mutation_revision(
    snapshot: Option<&ValidatedSceneSnapshot>,
    expected_revision: Option<&str>,
) -> Result<(), WorkflowStepErrorCode> {
    let snapshot = snapshot.ok_or(WorkflowStepErrorCode::SnapshotUnavailable)?;
    let expected = expected_revision.ok_or(WorkflowStepErrorCode::SnapshotRevisionRequired)?;
    let actual = snapshot
        .snapshot()
        .revision
        .as_deref()
        .ok_or(WorkflowStepErrorCode::SnapshotRevisionUnavailable)?;
    if actual != expected {
        return Err(WorkflowStepErrorCode::StaleSnapshot);
    }
    Ok(())
}

fn empty_step_preview(step: &WorkflowStep) -> WorkflowStepPreview {
    WorkflowStepPreview {
        step_id: step.step_id.clone(),
        step_type: step.step.step_type(),
        depends_on: step.depends_on.clone(),
        status: WorkflowStepStatus::Planned,
        validation_status: WorkflowValidationStatus::Valid,
        execution_status: WorkflowStepExecutionStatus::NotRequested,
        required_permission: None,
        confirmation_policy: None,
        details: None,
        error: None,
    }
}

fn invalid_step_preview(
    step: &WorkflowStep,
    code: WorkflowStepErrorCode,
    path: impl Into<String>,
) -> WorkflowStepPreview {
    let mut preview = empty_step_preview(step);
    preview.status = WorkflowStepStatus::Invalid;
    preview.validation_status = WorkflowValidationStatus::Invalid;
    preview.error = Some(WorkflowStepError {
        code,
        path: path.into(),
    });
    preview
}

fn blocked_step_preview(
    step: &WorkflowStep,
    code: WorkflowStepErrorCode,
    path: &str,
) -> WorkflowStepPreview {
    let mut preview = empty_step_preview(step);
    preview.status = WorkflowStepStatus::Blocked;
    preview.validation_status = WorkflowValidationStatus::Blocked;
    preview.error = Some(WorkflowStepError {
        code,
        path: format!("{path}.dependsOn"),
    });
    preview
}

fn map_scene_error(error: super::scene_planning::ScenePlanningError) -> WorkflowStepErrorCode {
    match error.code {
        ScenePlanningErrorCode::SnapshotUnavailable => WorkflowStepErrorCode::SnapshotUnavailable,
        ScenePlanningErrorCode::StaleSnapshot => WorkflowStepErrorCode::StaleSnapshot,
        ScenePlanningErrorCode::SceneMismatch => WorkflowStepErrorCode::SceneMismatch,
        ScenePlanningErrorCode::SnapshotRevisionRequired => {
            WorkflowStepErrorCode::SnapshotRevisionRequired
        }
        ScenePlanningErrorCode::TargetNotFound => WorkflowStepErrorCode::TargetNotFound,
        ScenePlanningErrorCode::AmbiguousTarget => WorkflowStepErrorCode::AmbiguousTarget,
        ScenePlanningErrorCode::InvalidCandidate => WorkflowStepErrorCode::InvalidCandidate,
        ScenePlanningErrorCode::InvalidSnapshot | ScenePlanningErrorCode::InvalidRequest => {
            WorkflowStepErrorCode::InvalidRequest
        }
    }
}

fn map_animation_error(
    error: super::animation_planning::AnimationPlanningError,
) -> WorkflowStepErrorCode {
    match error.code {
        AnimationPlanningErrorCode::SnapshotUnavailable => {
            WorkflowStepErrorCode::SnapshotUnavailable
        }
        AnimationPlanningErrorCode::InvalidSnapshot => WorkflowStepErrorCode::InvalidRequest,
        AnimationPlanningErrorCode::StaleSnapshot => WorkflowStepErrorCode::StaleSnapshot,
        AnimationPlanningErrorCode::SceneMismatch => WorkflowStepErrorCode::SceneMismatch,
        AnimationPlanningErrorCode::SnapshotRevisionRequired => {
            WorkflowStepErrorCode::SnapshotRevisionRequired
        }
        AnimationPlanningErrorCode::MissingTarget => WorkflowStepErrorCode::MissingTarget,
        AnimationPlanningErrorCode::TargetNotFound => WorkflowStepErrorCode::TargetNotFound,
        AnimationPlanningErrorCode::AmbiguousTarget => WorkflowStepErrorCode::AmbiguousTarget,
        AnimationPlanningErrorCode::IncompleteSnapshotForNameResolution => {
            WorkflowStepErrorCode::IncompleteSnapshotForNameResolution
        }
        AnimationPlanningErrorCode::InvalidCandidate => WorkflowStepErrorCode::InvalidCandidate,
        AnimationPlanningErrorCode::InvalidRequest
        | AnimationPlanningErrorCode::InvalidFrameRange
        | AnimationPlanningErrorCode::UnsupportedFrameRate
        | AnimationPlanningErrorCode::InvalidValue => WorkflowStepErrorCode::InvalidRequest,
    }
}

fn map_preview_error(code: BlenderPreviewErrorCode) -> WorkflowStepErrorCode {
    match code {
        BlenderPreviewErrorCode::SnapshotUnavailable => WorkflowStepErrorCode::SnapshotUnavailable,
        BlenderPreviewErrorCode::SnapshotRevisionUnavailable => {
            WorkflowStepErrorCode::SnapshotRevisionUnavailable
        }
        BlenderPreviewErrorCode::StaleSnapshot => WorkflowStepErrorCode::StaleSnapshot,
        BlenderPreviewErrorCode::SceneMismatch => WorkflowStepErrorCode::SceneMismatch,
        BlenderPreviewErrorCode::CameraNotFound => WorkflowStepErrorCode::MissingTarget,
        BlenderPreviewErrorCode::ResourceAdmissionUnknown => {
            WorkflowStepErrorCode::PreviewResourceAdmissionUnknown
        }
        BlenderPreviewErrorCode::ResourceAdmissionDeferred => {
            WorkflowStepErrorCode::PreviewResourceAdmissionDeferred
        }
        BlenderPreviewErrorCode::PayloadTooLarge
        | BlenderPreviewErrorCode::MalformedRequest
        | BlenderPreviewErrorCode::UnsupportedSchemaVersion
        | BlenderPreviewErrorCode::InvalidRequest
        | BlenderPreviewErrorCode::InvalidFrame
        | BlenderPreviewErrorCode::InvalidResolution
        | BlenderPreviewErrorCode::InvalidSamples
        | BlenderPreviewErrorCode::InvalidCameraTarget => WorkflowStepErrorCode::InvalidRequest,
    }
}

fn valid_token(value: &str) -> bool {
    value.len() <= MAX_WORKFLOW_ID_CHARS && super::scene_planning::valid_identifier(value)
}

fn valid_reference_id(value: &str) -> bool {
    value.len() == 32 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn valid_description(value: &str, maximum: usize) -> bool {
    !value.trim().is_empty()
        && value.chars().count() <= maximum
        && !value.chars().any(char::is_control)
}

#[cfg(test)]
#[path = "workflow_orchestration_tests.rs"]
mod tests;
