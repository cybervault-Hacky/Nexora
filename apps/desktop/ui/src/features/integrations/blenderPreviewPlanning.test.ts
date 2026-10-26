import { describe, expect, it } from "vitest";
import { formatBlenderPreviewPlan, type BlenderPreviewPlanPreview } from "./blenderPreviewPlanning";

const blockedRender: BlenderPreviewPlanPreview = {
  schemaVersion: 1,
  request: {
    schemaVersion: 1,
    requestId: "preview-1",
    sceneReference: "scene-main",
    expectedSceneRevision: "revision-1",
    preview: {
      kind: "render",
      frame: 1,
      cameraObjectId: "camera-main",
      width: 1_280,
      height: 720,
      samples: 16,
    },
  },
  status: "blocked",
  executionStatus: "connector_unavailable",
  requiredPermission: "scene_inspect",
  confirmationPolicy: "explicit_user_confirmation",
  confirmationGranted: false,
  expectedSceneRevision: "revision-1",
  details: {
    kind: "render",
    details: {
      frame: 1,
      cameraObjectId: "camera-main",
      cameraName: "Camera",
      width: 1_280,
      height: 720,
      samples: 16,
    },
  },
  resourceAdmission: {
    status: "unknown",
    reason: "No validated Blender workload measurements are available.",
    activeSharedReservations: 0,
  },
  error: { code: "resource_admission_unknown", path: "$.resourceAdmission" },
  limitations: ["No render was generated."],
};

describe("Blender preview-intent summary", () => {
  it("shows render parameters and conservative admission without claiming an image", () => {
    const text = formatBlenderPreviewPlan(blockedRender);
    expect(text).toContain("render frame 1 at 1280×720, 16 samples");
    expect(text).toContain("resource admission: unknown");
    expect(text).toContain("explicit_user_confirmation; granted: no");
    expect(text).toContain("No image or render was generated");
    expect(text).not.toContain("render completed");
  });
});
