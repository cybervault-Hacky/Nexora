use super::*;
use serde_json::{json, Value};

use super::super::blender_actions::{
    AnimationOperation, InterpolationMode, ObjectTransform, PrimitiveType, Vector3,
};
use super::super::preview_planning::{BlenderPreviewKindRequest, BlenderPreviewRequest};
use super::super::scene_planning::{
    parse_scene_snapshot_json, ObjectTarget, SceneEditIntent, SceneEditRequest,
    SCENE_EDIT_REQUEST_SCHEMA_VERSION,
};

fn vector(x: f64, y: f64, z: f64) -> Value {
    json!({ "x": x, "y": y, "z": z })
}

fn snapshot() -> ValidatedSceneSnapshot {
    snapshot_with_revision(Some("revision-1"))
}

fn snapshot_with_revision(revision: Option<&str>) -> ValidatedSceneSnapshot {
    let input = json!({
        "schemaVersion": 1,
        "sceneReference": "scene-main",
        "sceneName": "Scene",
        "revision": revision,
        "capturedAtUnixMs": 1_720_000_000_000u64,
        "completeness": {
            "complete": true,
            "truncated": false,
            "reportedObjectCount": 2,
            "reportedCollectionCount": 0
        },
        "objects": [{
            "objectId": "object-cube",
            "name": "Cube",
            "objectType": "mesh",
            "transform": {
                "location": vector(0.0, 0.0, 0.0),
                "rotation": vector(0.0, 0.0, 0.0),
                "scale": vector(1.0, 1.0, 1.0)
            },
            "parentObjectId": null,
            "collectionIds": []
        }, {
            "objectId": "camera-main",
            "name": "Camera",
            "objectType": "camera",
            "transform": {
                "location": vector(0.0, -5.0, 2.0),
                "rotation": vector(0.0, 0.0, 0.0),
                "scale": vector(1.0, 1.0, 1.0)
            },
            "parentObjectId": null,
            "collectionIds": []
        }],
        "collections": []
    })
    .to_string();
    parse_scene_snapshot_json(&input).expect("valid supplied fixture snapshot")
}

fn image_reference() -> ImageReferenceMetadata {
    ImageReferenceMetadata {
        schema_version: 1,
        reference_id: "0123456789abcdef0123456789abcdef".into(),
        format: "image/png".into(),
        width: 512,
        height: 512,
        size_bytes: 128_000,
        description: Some("a stylized stone arch".into()),
        processing_state: "validated".into(),
        validation_errors: vec![],
    }
}

fn base_workflow(steps: Vec<WorkflowStep>) -> WorkflowRequest {
    WorkflowRequest {
        schema_version: WORKFLOW_CONTRACT_VERSION,
        workflow_id: "workflow-1".into(),
        task_description: "Plan a scene based on this reference".into(),
        scene_reference: Some("scene-main".into()),
        expected_scene_revision: Some("revision-1".into()),
        steps,
    }
}

fn inspect_step(step_id: &str) -> WorkflowStep {
    WorkflowStep {
        step_id: step_id.into(),
        depends_on: vec![],
        step: WorkflowStepKind::InspectSuppliedSnapshot,
    }
}

fn image_step(step_id: &str, depends_on: Vec<&str>) -> WorkflowStep {
    WorkflowStep {
        step_id: step_id.into(),
        depends_on: depends_on.into_iter().map(str::to_string).collect(),
        step: WorkflowStepKind::AssociateImageReference {
            reference_id: image_reference().reference_id,
        },
    }
}

fn render_preview_step(step_id: &str) -> WorkflowStep {
    WorkflowStep {
        step_id: step_id.into(),
        depends_on: vec![],
        step: WorkflowStepKind::PlanPreview {
            request: BlenderPreviewRequest {
                schema_version: 1,
                request_id: "render-preview-1".into(),
                scene_reference: "scene-main".into(),
                expected_scene_revision: "revision-1".into(),
                preview: BlenderPreviewKindRequest::Render {
                    frame: 1,
                    camera_object_id: "camera-main".into(),
                    width: 1_280,
                    height: 720,
                    samples: 16,
                },
            },
        },
    }
}

fn rename_step(step_id: &str, new_name: &str) -> WorkflowStep {
    WorkflowStep {
        step_id: step_id.into(),
        depends_on: vec![],
        step: WorkflowStepKind::PlanSceneEdit {
            request: SceneEditRequest {
                schema_version: SCENE_EDIT_REQUEST_SCHEMA_VERSION,
                request_id: "edit-request-1".into(),
                scene_reference: "scene-main".into(),
                user_request: "Rename the supplied object".into(),
                expected_snapshot_revision: Some("revision-1".into()),
                intent: SceneEditIntent::RenameObject {
                    target: ObjectTarget::ObjectId {
                        object_id: "object-cube".into(),
                    },
                    new_name: new_name.into(),
                },
            },
        },
    }
}

#[test]
fn plans_typed_steps_deterministically_and_keeps_dependency_order_explicit() {
    let workflow = base_workflow(vec![
        image_step("image-after-inspection", vec!["inspect"]),
        image_step("independent-reference", vec![]),
        inspect_step("inspect"),
    ]);
    let inputs = [image_reference()];
    let first = plan_workflow(Some(&snapshot()), &inputs, &workflow).expect("workflow plan");
    let second = plan_workflow(Some(&snapshot()), &inputs, &workflow).expect("repeat plan");

    assert_eq!(first.preview(), second.preview());
    let steps = &first.preview().steps;
    assert_eq!(
        steps
            .iter()
            .map(|step| step.step_id.as_str())
            .collect::<Vec<_>>(),
        vec!["independent-reference", "inspect", "image-after-inspection"]
    );
    assert_eq!(steps[2].depends_on, vec!["inspect".to_string()]);
    assert_eq!(
        steps[1].required_permission,
        Some(ActionPermission::SceneInspect)
    );
    assert_eq!(steps[0].status, WorkflowStepStatus::Planned);
    assert_eq!(
        first.preview().execution_status,
        WorkflowExecutionStatus::NotRequested
    );
    assert!(!first.preview().confirmation_required);
}

#[test]
fn invalid_upstream_step_blocks_dependents_but_not_independent_steps() {
    let workflow = base_workflow(vec![
        inspect_step("inspection"),
        image_step("dependent-reference", vec!["inspection"]),
        image_step("independent-reference", vec![]),
    ]);
    let references = [image_reference()];
    let plan = plan_workflow(None, &references, &workflow).expect("invalid steps are previewed");

    assert_eq!(plan.preview().status, WorkflowPlanStatus::Invalid);
    assert_eq!(plan.preview().steps[0].status, WorkflowStepStatus::Invalid);
    assert_eq!(plan.preview().steps[1].status, WorkflowStepStatus::Blocked);
    assert_eq!(
        plan.preview().steps[1]
            .error
            .as_ref()
            .map(|error| error.code),
        Some(WorkflowStepErrorCode::DependencyFailed)
    );
    assert_eq!(plan.preview().steps[2].status, WorkflowStepStatus::Planned);
    assert!(!plan.preview().confirmation_required);
    assert!(plan.confirmation_binding().is_none());
}

#[test]
fn unavailable_mutation_dependency_blocks_later_steps_without_promoting_them() {
    let workflow = base_workflow(vec![
        rename_step("rename", "Renamed Cube"),
        WorkflowStep {
            step_id: "inspect-after-rename".into(),
            depends_on: vec!["rename".into()],
            step: WorkflowStepKind::InspectSuppliedSnapshot,
        },
    ]);
    let scene = snapshot();
    let plan = plan_workflow(Some(&scene), &[], &workflow).expect("workflow preview");

    assert_eq!(plan.preview().status, WorkflowPlanStatus::Blocked);
    assert_eq!(
        plan.preview().steps[0].status,
        WorkflowStepStatus::AwaitingConfirmation
    );
    assert_eq!(plan.preview().steps[1].status, WorkflowStepStatus::Blocked);
    assert_eq!(
        plan.preview().steps[1]
            .error
            .as_ref()
            .map(|error| error.code),
        Some(WorkflowStepErrorCode::DependencyUnavailable)
    );
    assert!(!plan.preview().confirmation_required);
    assert!(plan.confirmation_binding().is_none());
    assert_eq!(
        plan.preview().execution_status,
        WorkflowExecutionStatus::ConnectorUnavailable
    );
}

#[test]
fn mutation_uses_phase_six_permission_and_exact_confirmation_binding() {
    let workflow = base_workflow(vec![rename_step("rename", "Renamed Cube")]);
    let scene = snapshot();
    let plan = plan_workflow(Some(&scene), &[], &workflow).expect("mutation plan");
    let preview = plan.preview();
    assert_eq!(preview.status, WorkflowPlanStatus::AwaitingConfirmation);
    assert!(preview.confirmation_required);
    assert_eq!(
        preview.required_permissions,
        vec![ActionPermission::SceneModify]
    );
    assert_eq!(
        preview.steps[0].status,
        WorkflowStepStatus::AwaitingConfirmation
    );
    assert_eq!(
        preview.steps[0].confirmation_policy,
        Some(ConfirmationRequirement::ExplicitUserConfirmation)
    );
    assert_eq!(
        preview.execution_status,
        WorkflowExecutionStatus::ConnectorUnavailable
    );

    let binding = plan.confirmation_binding().expect("review binding");
    assert!(binding.matches(&plan));
    let changed_workflow = base_workflow(vec![rename_step("rename", "Different Target Name")]);
    let changed_plan = plan_workflow(Some(&scene), &[], &changed_workflow).expect("changed plan");
    assert!(!binding.matches(&changed_plan));
    assert!(changed_plan.confirmation_binding().is_some());
}

#[test]
fn animation_step_reuses_phase_nine_planner_and_phase_six_policy() {
    let request = AnimationRequest {
        schema_version: 1,
        request_id: "animation-request-1".into(),
        scene_reference: "scene-main".into(),
        expected_snapshot_revision: Some("revision-1".into()),
        target: Some(ObjectTarget::ObjectId {
            object_id: "object-cube".into(),
        }),
        operation: AnimationOperation::Translation,
        start_frame: 1,
        end_frame: 48,
        frame_rate: 24,
        interpolation: InterpolationMode::Linear,
        start_value: Vector3 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        },
        end_value: Vector3 {
            x: 10.0,
            y: 0.0,
            z: 0.0,
        },
        description: Some("Move the supplied object".into()),
    };
    let workflow = base_workflow(vec![WorkflowStep {
        step_id: "animate".into(),
        depends_on: vec![],
        step: WorkflowStepKind::PlanAnimation { request },
    }]);
    let plan = plan_workflow(Some(&snapshot()), &[], &workflow).expect("animation workflow plan");

    assert_eq!(
        plan.preview().status,
        WorkflowPlanStatus::AwaitingConfirmation
    );
    assert_eq!(
        plan.preview().required_permissions,
        vec![ActionPermission::SceneModify]
    );
    assert_eq!(
        plan.preview().steps[0].confirmation_policy,
        Some(ConfirmationRequirement::ExplicitUserConfirmation)
    );
    let Some(WorkflowStepDetails::Animation(details)) = &plan.preview().steps[0].details else {
        panic!("expected Phase 9 animation plan preview");
    };
    assert_eq!(details.keyframes.len(), 2);
    assert_eq!(
        plan.preview().steps[0].execution_status,
        WorkflowStepExecutionStatus::ConnectorUnavailable
    );
}

#[test]
fn supplied_image_association_does_not_claim_vision_analysis() {
    let workflow = base_workflow(vec![image_step("reference", vec![])]);
    let plan = plan_workflow(None, &[image_reference()], &workflow).expect("reference plan");
    let Some(WorkflowStepDetails::ImageReference(details)) = &plan.preview().steps[0].details
    else {
        panic!("expected image association details");
    };
    assert!(!details.analysis_available);
    assert!(details.note.contains("not analyzed"));
    assert!(plan
        .preview()
        .limitations
        .iter()
        .any(|limitation| limitation.contains("not analyzed")));
}

#[test]
fn rejects_cycles_duplicates_unknown_fields_versions_and_oversized_contracts() {
    let mut cyclic = base_workflow(vec![
        image_step("first", vec!["second"]),
        image_step("second", vec!["first"]),
    ]);
    assert_eq!(
        plan_workflow(None, &[image_reference()], &cyclic)
            .expect_err("cycle rejected")
            .code,
        WorkflowContractErrorCode::DependencyCycle
    );

    cyclic.steps[1].depends_on = vec!["second".into()];
    assert_eq!(
        plan_workflow(None, &[image_reference()], &cyclic)
            .expect_err("self-dependency is a cycle")
            .code,
        WorkflowContractErrorCode::DependencyCycle
    );

    let duplicate = base_workflow(vec![inspect_step("same"), inspect_step("same")]);
    assert_eq!(
        plan_workflow(Some(&snapshot()), &[], &duplicate)
            .expect_err("duplicate IDs rejected")
            .code,
        WorkflowContractErrorCode::DuplicateStepId
    );

    let unknown_field = r#"{"schemaVersion":1,"workflowId":"wf","taskDescription":"task","steps":[{"stepId":"a","step":{"kind":"inspect_supplied_snapshot"},"extra":true}]}"#;
    assert_eq!(
        plan_workflow_json(None, &[], unknown_field)
            .expect_err("strict fields")
            .code,
        WorkflowContractErrorCode::MalformedWorkflow
    );

    let unsupported_version = r#"{"schemaVersion":99,"workflowId":"wf","taskDescription":"task","steps":[{"stepId":"a","step":{"kind":"inspect_supplied_snapshot"}}]}"#;
    assert_eq!(
        plan_workflow_json(None, &[], unsupported_version)
            .expect_err("unsupported version")
            .code,
        WorkflowContractErrorCode::UnsupportedContractVersion
    );

    let too_large = " ".repeat(MAX_WORKFLOW_REQUEST_BYTES + 1);
    assert_eq!(
        plan_workflow_json(None, &[], &too_large)
            .expect_err("oversized JSON rejected")
            .code,
        WorkflowContractErrorCode::PayloadTooLarge
    );
}

#[test]
fn validates_scene_revision_and_rejects_unbounded_image_metadata() {
    let mut stale_workflow = base_workflow(vec![inspect_step("inspect")]);
    stale_workflow.expected_scene_revision = Some("revision-old".into());
    let stale = plan_workflow(Some(&snapshot()), &[], &stale_workflow).expect("step error preview");
    assert_eq!(stale.preview().steps[0].status, WorkflowStepStatus::Invalid);
    assert_eq!(
        stale.preview().steps[0]
            .error
            .as_ref()
            .map(|error| error.code),
        Some(WorkflowStepErrorCode::StaleSnapshot)
    );

    let mut no_expected_revision = base_workflow(vec![rename_step("rename", "Renamed Cube")]);
    no_expected_revision.expected_scene_revision = None;
    if let WorkflowStepKind::PlanSceneEdit { request } = &mut no_expected_revision.steps[0].step {
        request.expected_snapshot_revision = None;
    }
    let missing_revision = plan_workflow(Some(&snapshot()), &[], &no_expected_revision)
        .expect("missing mutation revision is reported on the step");
    assert_eq!(
        missing_revision.preview().steps[0]
            .error
            .as_ref()
            .map(|error| error.code),
        Some(WorkflowStepErrorCode::SnapshotRevisionRequired)
    );

    let unavailable_revision = plan_workflow(
        Some(&snapshot_with_revision(None)),
        &[],
        &base_workflow(vec![rename_step("rename", "Renamed Cube")]),
    )
    .expect("unavailable source revision is reported on the step");
    assert_eq!(
        unavailable_revision.preview().steps[0]
            .error
            .as_ref()
            .map(|error| error.code),
        Some(WorkflowStepErrorCode::SnapshotRevisionUnavailable)
    );

    let mut reference = image_reference();
    reference.description = Some("x".repeat(MAX_WORKFLOW_REFERENCE_DESCRIPTION_CHARS + 1));
    let invalid_inputs = plan_workflow(
        None,
        &[reference],
        &base_workflow(vec![image_step("reference", vec![])]),
    )
    .expect_err("oversized image metadata rejected before planning");
    assert_eq!(
        invalid_inputs.code,
        WorkflowContractErrorCode::InvalidInputs
    );
}

#[test]
fn creates_no_binding_for_a_workflow_with_blocked_or_invalid_steps() {
    let plan = plan_workflow(
        None,
        &[],
        &base_workflow(vec![
            inspect_step("inspect"),
            image_step("later", vec!["inspect"]),
        ]),
    )
    .expect("statuses are returned for review");
    assert_eq!(plan.preview().status, WorkflowPlanStatus::Invalid);
    assert!(plan.confirmation_binding().is_none());
}

#[test]
fn typed_contract_enforces_step_count_bounds() {
    let many_steps = (0..=MAX_WORKFLOW_STEPS)
        .map(|index| inspect_step(&format!("step-{index}")))
        .collect();
    let error =
        plan_workflow(None, &[], &base_workflow(many_steps)).expect_err("step count bounded");
    assert_eq!(error.code, WorkflowContractErrorCode::InvalidStepCount);

    let request = base_workflow(vec![inspect_step("single")]);
    let mut oversized = request.clone();
    oversized.task_description = "x".repeat(MAX_WORKFLOW_REQUEST_BYTES + 1);
    assert_eq!(
        plan_workflow(None, &[], &oversized)
            .expect_err("typed input also has serialized size bound")
            .code,
        WorkflowContractErrorCode::PayloadTooLarge
    );
}

#[test]
fn scene_edit_test_fixture_types_remain_explicitly_planning_only() {
    let transform = ObjectTransform {
        location: Vector3 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
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
    };
    let mut step = rename_step("create", "unused");
    step.step = WorkflowStepKind::PlanSceneEdit {
        request: SceneEditRequest {
            schema_version: SCENE_EDIT_REQUEST_SCHEMA_VERSION,
            request_id: "edit-create-1".into(),
            scene_reference: "scene-main".into(),
            user_request: "Plan a cube".into(),
            expected_snapshot_revision: Some("revision-1".into()),
            intent: SceneEditIntent::CreatePrimitive {
                primitive: PrimitiveType::Cube,
                name: Some("Proposed Cube".into()),
                transform,
            },
        },
    };
    let plan = plan_workflow(Some(&snapshot()), &[], &base_workflow(vec![step]))
        .expect("create action is still only planned");
    assert_eq!(
        plan.preview().status,
        WorkflowPlanStatus::AwaitingConfirmation
    );
    assert_eq!(
        plan.preview().steps[0].execution_status,
        WorkflowStepExecutionStatus::ConnectorUnavailable
    );
}

#[test]
fn render_preview_workflow_step_exposes_blocked_admission_without_execution() {
    let workflow = base_workflow(vec![render_preview_step("render-preview")]);
    let plan = plan_workflow(Some(&snapshot()), &[], &workflow).expect("preview-only workflow");
    let step = &plan.preview().steps[0];
    assert_eq!(step.step_type, WorkflowStepType::PlanPreview);
    assert_eq!(step.status, WorkflowStepStatus::Blocked);
    assert_eq!(
        step.execution_status,
        WorkflowStepExecutionStatus::ConnectorUnavailable
    );
    assert_eq!(
        step.error.as_ref().map(|error| error.code),
        Some(WorkflowStepErrorCode::PreviewResourceAdmissionUnknown)
    );
    assert_eq!(
        step.required_permission,
        Some(ActionPermission::SceneInspect)
    );
    assert_eq!(
        step.confirmation_policy,
        Some(ConfirmationRequirement::ExplicitUserConfirmation)
    );
    assert!(matches!(
        &step.details,
        Some(WorkflowStepDetails::Preview(preview))
            if preview.status == BlenderPreviewPlanStatus::Blocked
                && !preview.confirmation_granted
    ));
    assert_eq!(
        plan.preview().execution_status,
        WorkflowExecutionStatus::ConnectorUnavailable
    );
}

#[test]
fn preview_workflow_step_must_match_declared_scene_and_revision_scope() {
    let mut step = render_preview_step("render-preview");
    let WorkflowStepKind::PlanPreview { request } = &mut step.step else {
        unreachable!();
    };
    request.expected_scene_revision = "revision-from-another-plan".into();
    let workflow = base_workflow(vec![step]);
    let plan =
        plan_workflow(Some(&snapshot()), &[], &workflow).expect("invalid intent is summarized");
    assert_eq!(plan.preview().steps[0].status, WorkflowStepStatus::Invalid);
    assert_eq!(
        plan.preview().steps[0]
            .error
            .as_ref()
            .map(|error| error.code),
        Some(WorkflowStepErrorCode::InvalidRequest)
    );
}
