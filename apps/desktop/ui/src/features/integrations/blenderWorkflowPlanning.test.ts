import { describe, expect, it } from "vitest";
import {
  BLENDER_WORKFLOW_MAX_DEPENDENCIES_PER_STEP,
  BLENDER_WORKFLOW_MAX_STEPS,
  formatBlenderWorkflowSummary,
  type BlenderWorkflowPlanPreview,
} from "./blenderWorkflowPlanning";

function preview(): BlenderWorkflowPlanPreview {
  return {
    schemaVersion: 1,
    workflowId: "review-01",
    taskDescription: "Inspect the supplied scene, then associate a reference",
    sceneReference: "scene-main",
    expectedSceneRevision: "revision-1",
    status: "invalid",
    executionStatus: "connector_unavailable",
    requiredPermissions: ["scene_inspect"],
    confirmationRequired: false,
    steps: [
      {
        stepId: "inspect",
        stepType: "inspect_supplied_snapshot",
        dependsOn: [],
        status: "invalid",
        validationStatus: "invalid",
        executionStatus: "not_requested",
        requiredPermission: null,
        confirmationPolicy: null,
        error: { code: "snapshot_unavailable", path: "$.inputs.snapshot" },
      },
      {
        stepId: "reference",
        stepType: "associate_image_reference",
        dependsOn: ["inspect"],
        status: "blocked",
        validationStatus: "blocked",
        executionStatus: "not_requested",
        requiredPermission: null,
        confirmationPolicy: null,
        error: { code: "dependency_failed", path: "$.steps[1].dependsOn" },
      },
    ],
    limitations: ["No scene or image content was changed or analyzed."],
  };
}

describe("Blender workflow planning summary", () => {
  it("mirrors the bounded Phase 10 contract", () => {
    expect(BLENDER_WORKFLOW_MAX_STEPS).toBe(8);
    expect(BLENDER_WORKFLOW_MAX_DEPENDENCIES_PER_STEP).toBe(7);
  });

  it("preserves planned step order and explains invalid/blocked propagation", () => {
    const summary = formatBlenderWorkflowSummary(preview());
    expect(summary.indexOf('1. "inspect"')).toBeLessThan(summary.indexOf('2. "reference"'));
    expect(summary).toContain("invalid");
    expect(summary).toContain("blocked");
    expect(summary).toContain('depends on "inspect"');
    expect(summary).toContain("snapshot_unavailable at $.inputs.snapshot");
  });

  it("never represents execution as completed or implies a requested task is consent", () => {
    const summary = formatBlenderWorkflowSummary(preview());
    expect(summary).toContain("connector unavailable");
    expect(summary).toContain("no workflow step was executed");
    expect(summary).toContain("planning and review order only");
    expect(summary).not.toContain("executed: true");
  });

  it("summarizes bounded preview steps without claiming a generated result", () => {
    const plan = preview();
    plan.status = "blocked";
    plan.steps = [
      {
        stepId: "render-preview",
        stepType: "plan_preview",
        dependsOn: [],
        status: "blocked",
        validationStatus: "blocked",
        executionStatus: "connector_unavailable",
        requiredPermission: "scene_inspect",
        confirmationPolicy: "explicit_user_confirmation",
        error: { code: "preview_resource_admission_unknown", path: "$.preview" },
      },
    ];
    const summary = formatBlenderWorkflowSummary(plan);
    expect(summary).toContain("plan_preview");
    expect(summary).toContain("blocked");
    expect(summary).toContain("preview_resource_admission_unknown");
    expect(summary).toContain("no workflow step was executed");
  });

  it("states that mutation confirmation is ungranted and tied to a validated plan", () => {
    const plan = preview();
    plan.status = "awaiting_confirmation";
    plan.confirmationRequired = true;
    plan.requiredPermissions = ["scene_modify"];
    plan.steps = [
      {
        stepId: "rename",
        stepType: "plan_scene_edit",
        dependsOn: [],
        status: "awaiting_confirmation",
        validationStatus: "valid",
        executionStatus: "connector_unavailable",
        requiredPermission: "scene_modify",
        confirmationPolicy: "explicit_user_confirmation",
        error: null,
      },
    ];
    const summary = formatBlenderWorkflowSummary(plan);
    expect(summary).toContain("exact validated mutation plan");
    expect(summary).toContain("no confirmation has been granted");
    expect(summary).toContain("permission scene_modify");
  });
});
