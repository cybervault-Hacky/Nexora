import { describe, expect, it } from "vitest";
import {
  BLENDER_ANIMATION_SEQUENCE_MAX_KEYFRAMES,
  formatBlenderAnimationPlan,
  formatBlenderAnimationSequencePlan,
  type BlenderAnimationPlanPreview,
  type BlenderAnimationSequencePlanPreview,
} from "./blenderAnimationPlanning";

const plan: BlenderAnimationPlanPreview = {
  request: {
    schemaVersion: 1,
    requestId: "animation-1",
    sceneReference: "scene-main",
    expectedSnapshotRevision: "revision-1",
    target: { kind: "object_id", objectId: "object-cube" },
    operation: "translation",
    startFrame: 1,
    endFrame: 48,
    frameRate: 24,
    interpolation: "linear",
    startValue: { x: 0, y: 0, z: 0 },
    endValue: { x: 10, y: 0, z: 0 },
    description: "Move locally",
  },
  actionId: "blender.animate_object",
  target: { objectId: "object-cube", objectName: "Cube" },
  operation: "translation",
  units: "blender_units_local",
  startFrame: 1,
  endFrame: 48,
  frameRate: 24,
  interpolation: "linear",
  keyframes: [
    { frame: 1, value: { x: 0, y: 0, z: 0 } },
    { frame: 48, value: { x: 10, y: 0, z: 0 } },
  ],
  actionClass: "mutation",
  validationStatus: "valid",
  requiredPermission: "scene_modify",
  confirmationPolicy: "explicit_user_confirmation",
  planState: "awaiting_confirmation",
  executionStatus: "connector_unavailable",
  snapshotRevision: "revision-1",
  freshnessCheckedByRevision: true,
  snapshotComplete: true,
  snapshotTruncated: false,
  limitations: ["Plan only."],
};

describe("Blender animation-plan summary", () => {
  it("summarizes endpoints and confirmation without implying execution", () => {
    const text = formatBlenderAnimationPlan(plan);
    expect(text).toContain("Target: \"Cube\" (object-cube)");
    expect(text).toContain("Translation");
    expect(text).toContain("Frames 1–48 inclusive at 24 fps");
    expect(text).toContain("frame 1: (0, 0, 0)");
    expect(text).toContain("frame 48: (10, 0, 0)");
    expect(text).toContain("Interpolation: linear");
    expect(text).toContain("explicit confirmation");
    expect(text).toContain("connector unavailable");
    expect(text).toContain("No keyframes were created, applied, or rendered");
    expect(text).not.toContain("executed");
  });
});

describe("Blender bounded animation-sequence summary", () => {
  it("summarizes sequence keyframes without implying that Blender created them", () => {
    const sequence: BlenderAnimationSequencePlanPreview = {
      request: {
        schemaVersion: 1,
        requestId: "sequence-1",
        sceneReference: "scene-main",
        expectedSnapshotRevision: "revision-1",
        target: { kind: "object_id", objectId: "object-cube" },
        operation: "translation",
        frameRate: 24,
        interpolation: "bezier",
        keyframes: [
          { frame: 1, value: { x: 0, y: 0, z: 0 } },
          { frame: 12, value: { x: 2, y: 0, z: 0 } },
          { frame: 48, value: { x: 10, y: 2, z: 1 } },
        ],
      },
      actionId: "blender.animate_object_keyframes",
      target: { objectId: "object-cube", objectName: "Cube" },
      operation: "translation",
      units: "blender_units_local",
      frameRate: 24,
      interpolation: "bezier",
      keyframes: [
        { frame: 1, value: { x: 0, y: 0, z: 0 } },
        { frame: 12, value: { x: 2, y: 0, z: 0 } },
        { frame: 48, value: { x: 10, y: 2, z: 1 } },
      ],
      actionClass: "mutation",
      validationStatus: "valid",
      requiredPermission: "scene_modify",
      confirmationPolicy: "explicit_user_confirmation",
      planState: "awaiting_confirmation",
      executionStatus: "connector_unavailable",
      snapshotRevision: "revision-1",
      limitations: ["No live Blender execution."],
    };
    const text = formatBlenderAnimationSequencePlan(sequence);
    expect(BLENDER_ANIMATION_SEQUENCE_MAX_KEYFRAMES).toBe(8);
    expect(text).toContain("3 bounded keyframe intents");
    expect(text).toContain("frame 12: (2, 0, 0)");
    expect(text).toContain("explicit confirmation is required and has not been granted");
    expect(text).toContain("No keyframes were created or applied");
    expect(text).not.toContain("executed");
  });
});
