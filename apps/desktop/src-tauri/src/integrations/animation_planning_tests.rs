use super::super::scene_planning::SCENE_SNAPSHOT_SCHEMA_VERSION;
use super::*;
use serde_json::{json, Value};

fn vector(x: f64, y: f64, z: f64) -> Value {
    json!({ "x": x, "y": y, "z": z })
}

fn snapshot_json(duplicate_name: bool, revision: Option<&str>) -> String {
    let second_object = duplicate_name.then(|| {
        json!({
            "objectId": "object-second",
            "name": "Cube",
            "objectType": "mesh",
            "transform": {
                "location": vector(1.0, 0.0, 0.0),
                "rotation": vector(0.0, 0.0, 0.0),
                "scale": vector(1.0, 1.0, 1.0)
            },
            "parentObjectId": null,
            "collectionIds": []
        })
    });
    let mut objects = vec![json!({
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
    })];
    if let Some(second) = second_object {
        objects.push(second);
    }
    json!({
        "schemaVersion": SCENE_SNAPSHOT_SCHEMA_VERSION,
        "sceneReference": "scene-main",
        "sceneName": "Scene",
        "revision": revision,
        "capturedAtUnixMs": 1_720_000_000_000u64,
        "completeness": {
            "complete": true,
            "truncated": false,
            "reportedObjectCount": objects.len(),
            "reportedCollectionCount": 0
        },
        "objects": objects,
        "collections": []
    })
    .to_string()
}

fn request_value(operation: &str, target: Value) -> Value {
    json!({
        "schemaVersion": ANIMATION_REQUEST_SCHEMA_VERSION,
        "requestId": "animation-test-1",
        "sceneReference": "scene-main",
        "expectedSnapshotRevision": "revision-1",
        "target": target,
        "operation": operation,
        "startFrame": 1,
        "endFrame": 48,
        "frameRate": 24,
        "interpolation": "linear",
        "startValue": vector(0.0, 0.0, 0.0),
        "endValue": vector(10.0, 2.0, 1.0),
        "description": "Move the object locally"
    })
}

fn target_id() -> Value {
    json!({ "kind": "object_id", "objectId": "object-cube" })
}

fn sequence_request_value(keyframes: Value) -> Value {
    json!({
        "schemaVersion": ANIMATION_SEQUENCE_REQUEST_SCHEMA_VERSION,
        "requestId": "sequence-request-1",
        "sceneReference": "scene-main",
        "expectedSnapshotRevision": "revision-1",
        "target": target_id(),
        "operation": "translation",
        "frameRate": 24,
        "interpolation": "bezier",
        "keyframes": keyframes
    })
}

fn plan_from_value(
    snapshot: Option<&str>,
    request: Value,
) -> Result<AnimationPlan, AnimationPlanningError> {
    plan_animation_json(snapshot, &request.to_string())
}

#[test]
fn creates_a_deterministic_two_keyframe_plan_with_phase_six_policy() {
    let snapshot = snapshot_json(false, Some("revision-1"));
    let plan = plan_from_value(Some(&snapshot), request_value("translation", target_id()))
        .expect("valid animation plan");
    let preview = plan.preview();
    assert_eq!(preview.action_id, ANIMATE_OBJECT_ID);
    assert_eq!(preview.target.object_id, "object-cube");
    assert_eq!(preview.target.object_name, "Cube");
    assert_eq!(preview.operation, AnimationOperation::Translation);
    assert_eq!(preview.units, AnimationValueUnits::BlenderUnitsLocal);
    assert_eq!((preview.start_frame, preview.end_frame), (1, 48));
    assert_eq!(preview.frame_rate, 24);
    assert_eq!(preview.interpolation, InterpolationMode::Linear);
    assert_eq!(preview.keyframes.len(), 2);
    assert_eq!(preview.keyframes[0].frame, 1);
    assert_eq!(preview.keyframes[1].frame, 48);
    assert_eq!(preview.validation_status, "valid");
    assert_eq!(preview.action_class, ActionClass::Mutation);
    assert_eq!(preview.required_permission, ActionPermission::SceneModify);
    assert_eq!(
        preview.confirmation_policy,
        ConfirmationRequirement::ExplicitUserConfirmation
    );
    assert_eq!(preview.plan_state, AnimationPlanState::AwaitingConfirmation);
    assert_eq!(
        preview.execution_status,
        AnimationExecutionStatus::ConnectorUnavailable
    );
    assert!(preview.freshness_checked_by_revision);
    assert_eq!(plan.validated_candidate().actions().len(), 1);
    assert!(matches!(
        &plan.validated_candidate().actions()[0],
        ValidatedAction::AnimateObject { .. }
    ));
    assert!(preview
        .limitations
        .iter()
        .any(|message| message.contains("no keyframes were created")));
}

#[test]
fn supports_rotation_and_scale_with_explicit_units_and_ranges() {
    let snapshot = snapshot_json(false, Some("revision-1"));
    let mut rotation = request_value("rotation", target_id());
    rotation["startValue"] = vector(0.0, 0.0, 0.0);
    rotation["endValue"] = vector(0.0, 0.0, 3.14);
    let rotation = plan_from_value(Some(&snapshot), rotation).expect("rotation plan");
    assert_eq!(
        rotation.preview().units,
        AnimationValueUnits::EulerRadiansLocal
    );

    let mut scale = request_value("scale", target_id());
    scale["startValue"] = vector(1.0, 1.0, 1.0);
    scale["endValue"] = vector(2.0, 0.5, 3.0);
    scale["interpolation"] = json!("bezier");
    let scale = plan_from_value(Some(&snapshot), scale).expect("scale plan");
    assert_eq!(
        scale.preview().units,
        AnimationValueUnits::DimensionlessLocalScale
    );
    assert_eq!(scale.preview().interpolation, InterpolationMode::Bezier);
}

#[test]
fn rejects_missing_ambiguous_and_unknown_targets_without_inventing_ids() {
    let snapshot = snapshot_json(false, Some("revision-1"));
    let mut missing = request_value("translation", Value::Null);
    missing.as_object_mut().unwrap().remove("target");
    assert_eq!(
        plan_from_value(Some(&snapshot), missing).unwrap_err().code,
        AnimationPlanningErrorCode::MissingTarget
    );

    let by_missing_id = request_value(
        "translation",
        json!({ "kind": "object_id", "objectId": "not-present" }),
    );
    assert_eq!(
        plan_from_value(Some(&snapshot), by_missing_id)
            .unwrap_err()
            .code,
        AnimationPlanningErrorCode::TargetNotFound
    );

    let ambiguous_snapshot = snapshot_json(true, Some("revision-1"));
    let by_name = request_value(
        "translation",
        json!({ "kind": "object_name", "name": "Cube" }),
    );
    assert_eq!(
        plan_from_value(Some(&ambiguous_snapshot), by_name.clone())
            .unwrap_err()
            .code,
        AnimationPlanningErrorCode::AmbiguousTarget
    );

    let mut partial: Value = serde_json::from_str(&snapshot).expect("snapshot JSON");
    partial["completeness"]["complete"] = json!(false);
    partial["completeness"]["truncated"] = json!(true);
    partial["completeness"]["reportedObjectCount"] = json!(2);
    assert_eq!(
        plan_from_value(Some(&partial.to_string()), by_name)
            .unwrap_err()
            .code,
        AnimationPlanningErrorCode::IncompleteSnapshotForNameResolution
    );
}

#[test]
fn distinguishes_unavailable_invalid_stale_and_mismatched_scene() {
    let request = request_value("translation", target_id());
    assert_eq!(
        plan_from_value(None, request.clone()).unwrap_err().code,
        AnimationPlanningErrorCode::SnapshotUnavailable
    );
    assert_eq!(
        plan_from_value(Some("{}"), request.clone())
            .unwrap_err()
            .code,
        AnimationPlanningErrorCode::InvalidSnapshot
    );
    let snapshot = snapshot_json(false, Some("revision-1"));
    let mut stale = request.clone();
    stale["expectedSnapshotRevision"] = json!("revision-old");
    assert_eq!(
        plan_from_value(Some(&snapshot), stale).unwrap_err().code,
        AnimationPlanningErrorCode::StaleSnapshot
    );
    let mut mismatched = request;
    mismatched["sceneReference"] = json!("other-scene");
    assert_eq!(
        plan_from_value(Some(&snapshot), mismatched)
            .unwrap_err()
            .code,
        AnimationPlanningErrorCode::SceneMismatch
    );
}

#[test]
fn rejects_missing_revision_for_mutations_and_enforces_timing_values_and_fields() {
    let snapshot = snapshot_json(false, Some("revision-1"));
    let mut missing_revision = request_value("translation", target_id());
    missing_revision["expectedSnapshotRevision"] = Value::Null;
    assert_eq!(
        plan_from_value(Some(&snapshot), missing_revision)
            .unwrap_err()
            .code,
        AnimationPlanningErrorCode::SnapshotRevisionRequired
    );

    for (field, value, expected) in [
        (
            "startFrame",
            json!(0),
            AnimationPlanningErrorCode::InvalidFrameRange,
        ),
        (
            "endFrame",
            json!(2501),
            AnimationPlanningErrorCode::InvalidFrameRange,
        ),
        (
            "frameRate",
            json!(23),
            AnimationPlanningErrorCode::UnsupportedFrameRate,
        ),
        (
            "interpolation",
            json!("cubic"),
            AnimationPlanningErrorCode::InvalidRequest,
        ),
        (
            "operation",
            json!("location"),
            AnimationPlanningErrorCode::InvalidRequest,
        ),
    ] {
        let mut invalid = request_value("translation", target_id());
        invalid[field] = value;
        assert_eq!(
            plan_from_value(Some(&snapshot), invalid).unwrap_err().code,
            expected
        );
    }

    let mut out_of_range = request_value("translation", target_id());
    out_of_range["endValue"] = vector(MAX_SCENE_LOCATION + 1.0, 0.0, 0.0);
    assert_eq!(
        plan_from_value(Some(&snapshot), out_of_range)
            .unwrap_err()
            .code,
        AnimationPlanningErrorCode::InvalidValue
    );
    let mut invalid_scale = request_value("scale", target_id());
    invalid_scale["startValue"] = vector(0.0, 1.0, 1.0);
    assert_eq!(
        plan_from_value(Some(&snapshot), invalid_scale)
            .unwrap_err()
            .code,
        AnimationPlanningErrorCode::InvalidValue
    );
    let mut oversized_description = request_value("translation", target_id());
    oversized_description["description"] = json!("a".repeat(MAX_ANIMATION_DESCRIPTION_CHARS + 1));
    assert_eq!(
        plan_from_value(Some(&snapshot), oversized_description)
            .unwrap_err()
            .code,
        AnimationPlanningErrorCode::InvalidRequest
    );
    let mut unknown = request_value("translation", target_id());
    unknown["python"] = json!("bpy.ops.object.keyframe_insert()");
    assert_eq!(
        plan_from_value(Some(&snapshot), unknown).unwrap_err().code,
        AnimationPlanningErrorCode::InvalidRequest
    );
}

#[test]
fn plans_bounded_multi_key_object_transform_sequences_with_exact_confirmation_policy() {
    let snapshot = snapshot_json(false, Some("revision-1"));
    let request = sequence_request_value(json!([
        { "frame": 1, "value": vector(0.0, 0.0, 0.0) },
        { "frame": 12, "value": vector(2.0, 0.0, 0.0) },
        { "frame": 48, "value": vector(10.0, 2.0, 1.0) }
    ]));
    let plan = plan_animation_sequence_json(Some(&snapshot), &request.to_string())
        .expect("bounded sequence plan");
    assert_eq!(plan.preview().action_id, ANIMATE_OBJECT_KEYFRAMES_ID);
    assert_eq!(plan.preview().target.object_id, "object-cube");
    assert_eq!(plan.preview().keyframes.len(), 3);
    assert_eq!(plan.preview().keyframes[1].frame, 12);
    assert_eq!(
        plan.preview().required_permission,
        ActionPermission::SceneModify
    );
    assert_eq!(
        plan.preview().confirmation_policy,
        ConfirmationRequirement::ExplicitUserConfirmation
    );
    assert_eq!(
        plan.preview().execution_status,
        AnimationExecutionStatus::ConnectorUnavailable
    );
    assert!(matches!(
        &plan.validated_candidate().actions()[0],
        ValidatedAction::AnimateObjectKeyframes { keyframes, .. } if keyframes.len() == 3
    ));

    let non_increasing = sequence_request_value(json!([
        { "frame": 1, "value": vector(0.0, 0.0, 0.0) },
        { "frame": 1, "value": vector(1.0, 0.0, 0.0) }
    ]));
    assert_eq!(
        plan_animation_sequence_json(Some(&snapshot), &non_increasing.to_string())
            .unwrap_err()
            .code,
        AnimationSequencePlanningErrorCode::InvalidKeyframes
    );

    let too_many = sequence_request_value(json!((0..=MAX_ANIMATION_SEQUENCE_KEYFRAMES)
        .map(|frame| json!({ "frame": frame + 1, "value": vector(frame as f64, 0.0, 0.0) }))
        .collect::<Vec<_>>()));
    assert_eq!(
        plan_animation_sequence_json(Some(&snapshot), &too_many.to_string())
            .unwrap_err()
            .code,
        AnimationSequencePlanningErrorCode::InvalidKeyframes
    );

    let unsupported_type_snapshot =
        snapshot.replace("\"objectType\":\"mesh\"", "\"objectType\":\"other\"");
    assert_eq!(
        plan_animation_sequence_json(Some(&unsupported_type_snapshot), &request.to_string())
            .unwrap_err()
            .code,
        AnimationSequencePlanningErrorCode::UnsupportedTargetType
    );

    assert_eq!(
        plan_animation_sequence_json(Some(&snapshot), "{")
            .unwrap_err()
            .code,
        AnimationSequencePlanningErrorCode::MalformedRequest
    );
    let oversized = " ".repeat(MAX_ANIMATION_SEQUENCE_REQUEST_BYTES + 1);
    assert_eq!(
        plan_animation_sequence_json(Some(&snapshot), &oversized)
            .unwrap_err()
            .code,
        AnimationSequencePlanningErrorCode::PayloadTooLarge
    );
}
