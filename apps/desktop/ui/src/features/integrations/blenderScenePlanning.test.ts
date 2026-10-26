import { describe, expect, it } from "vitest";
import {
  formatSceneEditPlanPreview,
  type SceneEditPlanPreview,
} from "./blenderScenePlanning";

const preview: SceneEditPlanPreview = {
  request: {
    schemaVersion: 1,
    requestId: "request-1",
    sceneReference: "scene-opaque-1",
    userRequest: "Rename Cube to Base",
    expectedSnapshotRevision: "rev-42",
    intent: { kind: "rename_object", target: { kind: "object_id", objectId: "obj-cube" }, newName: "Base" },
  },
  actionId: "blender.rename_object",
  actionClass: "mutation",
  validationStatus: "valid",
  requiredPermission: "scene_modify",
  confirmationPolicy: "explicit_user_confirmation",
  planState: "awaiting_confirmation",
  executionStatus: "connector_unavailable",
  snapshotRevision: "rev-42",
  snapshotComplete: true,
  snapshotTruncated: false,
  freshnessCheckedByRevision: true,
  plannedAction: { kind: "rename_object", objectId: "obj-cube", currentName: "Cube", newName: "Base" },
  limitations: ["Plan only; not applied."],
};

describe("Blender scene-plan preview copy", () => {
  it("shows target, proposed edit, confirmation policy, and unavailable execution", () => {
    const text = formatSceneEditPlanPreview(preview);
    expect(text).toContain("awaiting confirmation");
    expect(text).toContain("Rename \"Cube\" (obj-cube) to \"Base\"");
    expect(text).toContain("scene_modify");
    expect(text).toContain("explicit confirmation");
    expect(text).toContain("connector unavailable");
    expect(text).toContain("Nothing has been applied");
    expect(text).not.toContain("executed");
  });
});
