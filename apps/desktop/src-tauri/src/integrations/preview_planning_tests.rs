use super::*;
use serde_json::json;

use super::super::scene_planning::parse_scene_snapshot_json;

fn vector(x: f64, y: f64, z: f64) -> serde_json::Value {
    json!({ "x": x, "y": y, "z": z })
}

fn snapshot() -> ValidatedSceneSnapshot {
    let input = json!({
        "schemaVersion": 1,
        "sceneReference": "scene-main",
        "sceneName": "Scene",
        "revision": "revision-1",
        "capturedAtUnixMs": 1_720_000_000_000u64,
        "completeness": {
            "complete": true,
            "truncated": false,
            "reportedObjectCount": 2,
            "reportedCollectionCount": 0
        },
        "objects": [
            {
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
            },
            {
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
            }
        ],
        "collections": []
    })
    .to_string();
    parse_scene_snapshot_json(&input).expect("validated fixture snapshot")
}

fn viewport_request() -> BlenderPreviewRequest {
    BlenderPreviewRequest {
        schema_version: BLENDER_PREVIEW_REQUEST_SCHEMA_VERSION,
        request_id: "preview-request-1".into(),
        scene_reference: "scene-main".into(),
        expected_scene_revision: "revision-1".into(),
        preview: BlenderPreviewKindRequest::Viewport { frame: 1 },
    }
}

fn render_request() -> BlenderPreviewRequest {
    BlenderPreviewRequest {
        schema_version: BLENDER_PREVIEW_REQUEST_SCHEMA_VERSION,
        request_id: "render-request-1".into(),
        scene_reference: "scene-main".into(),
        expected_scene_revision: "revision-1".into(),
        preview: BlenderPreviewKindRequest::Render {
            frame: 1,
            camera_object_id: "camera-main".into(),
            width: 1_280,
            height: 720,
            samples: 16,
        },
    }
}

#[test]
fn viewport_preview_is_only_a_revision_bound_intent() {
    let plan = plan_blender_preview(Some(&snapshot()), None, &viewport_request())
        .expect("bounded viewport intent");
    assert_eq!(plan.preview().status, BlenderPreviewPlanStatus::Planned);
    assert_eq!(
        plan.preview().execution_status,
        BlenderPreviewExecutionStatus::ConnectorUnavailable
    );
    assert_eq!(
        plan.preview().required_permission,
        ActionPermission::SceneInspect
    );
    assert!(!plan.preview().confirmation_granted);
    assert_eq!(
        plan.preview().resource_admission.status,
        BlenderPreviewAdmissionStatus::Unknown
    );
    assert!(plan.preview().limitations[0].contains("No viewport image or render was generated"));
}

#[test]
fn render_preview_is_blocked_when_resource_evidence_is_unknown() {
    let plan = plan_blender_preview(Some(&snapshot()), None, &render_request())
        .expect("render intent is previewed, not executed");
    assert_eq!(plan.preview().status, BlenderPreviewPlanStatus::Blocked);
    assert_eq!(
        plan.preview().error.as_ref().map(|error| error.code),
        Some(BlenderPreviewErrorCode::ResourceAdmissionUnknown)
    );
    assert_eq!(
        plan.preview().confirmation_policy,
        ConfirmationRequirement::ExplicitUserConfirmation
    );
    assert!(!plan.preview().confirmation_granted);
    assert!(matches!(
        &plan.preview().details,
        BlenderPreviewDetails::Render { camera_object_id, width: 1_280, .. }
            if camera_object_id == "camera-main"
    ));
}

#[test]
fn rejects_malformed_and_oversized_preview_requests() {
    assert_eq!(
        plan_blender_preview_json(Some(&snapshot()), None, "{")
            .unwrap_err()
            .code,
        BlenderPreviewErrorCode::MalformedRequest
    );
    let oversized = " ".repeat(MAX_BLENDER_PREVIEW_REQUEST_BYTES + 1);
    assert_eq!(
        plan_blender_preview_json(Some(&snapshot()), None, &oversized)
            .unwrap_err()
            .code,
        BlenderPreviewErrorCode::PayloadTooLarge
    );
}

#[test]
fn rejects_stale_revisions_bad_camera_and_unbounded_render_settings() {
    let mut stale = viewport_request();
    stale.expected_scene_revision = "revision-old".into();
    assert_eq!(
        plan_blender_preview(Some(&snapshot()), None, &stale)
            .unwrap_err()
            .code,
        BlenderPreviewErrorCode::StaleSnapshot
    );

    let mut bad_camera = render_request();
    if let BlenderPreviewKindRequest::Render {
        camera_object_id, ..
    } = &mut bad_camera.preview
    {
        *camera_object_id = "object-cube".into();
    }
    assert_eq!(
        plan_blender_preview(Some(&snapshot()), None, &bad_camera)
            .unwrap_err()
            .code,
        BlenderPreviewErrorCode::InvalidCameraTarget
    );

    let mut excessive = render_request();
    if let BlenderPreviewKindRequest::Render { width, .. } = &mut excessive.preview {
        *width = MAX_PREVIEW_RENDER_WIDTH + 1;
    }
    assert_eq!(
        plan_blender_preview(Some(&snapshot()), None, &excessive)
            .unwrap_err()
            .code,
        BlenderPreviewErrorCode::InvalidResolution
    );
}
