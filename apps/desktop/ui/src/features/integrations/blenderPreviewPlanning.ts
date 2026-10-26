/** Phase 11 preview-intent contracts; Rust validation and admission are authoritative. */
export const BLENDER_PREVIEW_REQUEST_SCHEMA_VERSION = 1 as const;
export const BLENDER_PREVIEW_REQUEST_MAX_BYTES = 4 * 1024;
export const BLENDER_PREVIEW_MAX_RENDER_WIDTH = 1_920;
export const BLENDER_PREVIEW_MAX_RENDER_HEIGHT = 1_080;
export const BLENDER_PREVIEW_MAX_RENDER_SAMPLES = 64;

export type BlenderPreviewPermission = "scene_inspect";
export type BlenderPreviewConfirmation = "none" | "explicit_user_confirmation";
export type BlenderPreviewExecutionStatus = "connector_unavailable";
export type BlenderPreviewAdmissionStatus = "unknown" | "deferred";

export type BlenderPreviewKindRequest =
  | { kind: "viewport"; frame: number }
  | {
      kind: "render";
      frame: number;
      cameraObjectId: string;
      width: number;
      height: number;
      samples: number;
    };

export interface BlenderPreviewRequest {
  schemaVersion: typeof BLENDER_PREVIEW_REQUEST_SCHEMA_VERSION;
  requestId: string;
  sceneReference: string;
  expectedSceneRevision: string;
  preview: BlenderPreviewKindRequest;
}

export type BlenderPreviewDetails =
  | { kind: "viewport"; details: { frame: number } }
  | {
      kind: "render";
      details: {
        frame: number;
        cameraObjectId: string;
        cameraName: string;
        width: number;
        height: number;
        samples: number;
      };
    };

export type BlenderPreviewErrorCode =
  | "payload_too_large"
  | "malformed_request"
  | "unsupported_schema_version"
  | "snapshot_unavailable"
  | "snapshot_revision_unavailable"
  | "stale_snapshot"
  | "scene_mismatch"
  | "camera_not_found"
  | "invalid_camera_target"
  | "resource_admission_unknown"
  | "resource_admission_deferred"
  | "invalid_request"
  | "invalid_frame"
  | "invalid_resolution"
  | "invalid_samples";

export interface BlenderPreviewPlanPreview {
  schemaVersion: typeof BLENDER_PREVIEW_REQUEST_SCHEMA_VERSION;
  request: BlenderPreviewRequest;
  status: "planned" | "blocked";
  executionStatus: BlenderPreviewExecutionStatus;
  requiredPermission: BlenderPreviewPermission;
  confirmationPolicy: BlenderPreviewConfirmation;
  confirmationGranted: false;
  expectedSceneRevision: string;
  details: BlenderPreviewDetails;
  resourceAdmission: {
    status: BlenderPreviewAdmissionStatus;
    reason: string;
    activeSharedReservations: number;
  };
  error: { code: BlenderPreviewErrorCode; path: string } | null;
  limitations: string[];
}

/** User-safe intent summary; no preview bitmap, render result, or output path is claimed. */
export function formatBlenderPreviewPlan(plan: BlenderPreviewPlanPreview): string {
  const preview = plan.details.kind === "viewport"
    ? `viewport frame ${plan.details.details.frame}`
    : `render frame ${plan.details.details.frame} at ${plan.details.details.width}×${plan.details.details.height}, ${plan.details.details.samples} samples, camera ${JSON.stringify(plan.details.details.cameraObjectId)}`;
  const error = plan.error ? `; ${plan.error.code} at ${plan.error.path}` : "";
  return [
    `Blender ${preview} preview intent: ${plan.status}${error}.`,
    `Scene revision: ${JSON.stringify(plan.expectedSceneRevision)}; resource admission: ${plan.resourceAdmission.status}.`,
    `Permission: ${plan.requiredPermission}; confirmation: ${plan.confirmationPolicy}; granted: no.`,
    `Execution: ${plan.executionStatus}. No image or render was generated.`,
    ...plan.limitations,
  ].join("\n");
}
