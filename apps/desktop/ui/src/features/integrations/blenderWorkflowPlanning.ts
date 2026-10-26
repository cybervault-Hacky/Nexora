/** Phase 10 workflow preview contract; the Rust planner is authoritative. */
export const BLENDER_WORKFLOW_SCHEMA_VERSION = 1 as const;
export const BLENDER_WORKFLOW_MAX_REQUEST_BYTES = 32 * 1024;
export const BLENDER_WORKFLOW_MAX_ID_CHARS = 64;
export const BLENDER_WORKFLOW_MAX_DESCRIPTION_CHARS = 2_048;
export const BLENDER_WORKFLOW_MAX_STEPS = 8;
export const BLENDER_WORKFLOW_MAX_DEPENDENCIES_PER_STEP = 7;
export const BLENDER_WORKFLOW_MAX_IMAGE_REFERENCES = 4;
export const BLENDER_WORKFLOW_MAX_IMAGE_DESCRIPTION_CHARS = 1_024;
export const BLENDER_WORKFLOW_MAX_IMAGE_METADATA_BYTES = 4 * 1024;
export const BLENDER_WORKFLOW_MAX_PREVIEW_BYTES = 128 * 1024;

export type BlenderWorkflowStepType =
  | "inspect_supplied_snapshot"
  | "plan_scene_edit"
  | "plan_animation"
  | "plan_preview"
  | "associate_image_reference";
export type BlenderWorkflowPlanStatus =
  | "planned"
  | "invalid"
  | "blocked"
  | "awaiting_confirmation";
export type BlenderWorkflowStepStatus = BlenderWorkflowPlanStatus;
export type BlenderWorkflowExecutionStatus = "not_requested" | "connector_unavailable";
export type BlenderWorkflowValidationStatus = "valid" | "invalid" | "blocked";
export type BlenderWorkflowPermission = "scene_inspect" | "scene_modify";
export type BlenderWorkflowConfirmationPolicy =
  | "none"
  | "explicit_user_confirmation"
  | "destructive_confirmation";

export interface BlenderWorkflowStepError {
  code: string;
  path: string;
}

/** Minimal summary-facing shape of the Rust-validated workflow preview. */
export interface BlenderWorkflowStepPreview {
  stepId: string;
  stepType: BlenderWorkflowStepType;
  dependsOn: string[];
  status: BlenderWorkflowStepStatus;
  validationStatus: BlenderWorkflowValidationStatus;
  executionStatus: BlenderWorkflowExecutionStatus;
  requiredPermission: BlenderWorkflowPermission | null;
  confirmationPolicy: BlenderWorkflowConfirmationPolicy | null;
  error: BlenderWorkflowStepError | null;
}

/** No execution result or Apply/Run/Execute operation is represented. */
export interface BlenderWorkflowPlanPreview {
  schemaVersion: typeof BLENDER_WORKFLOW_SCHEMA_VERSION;
  workflowId: string;
  taskDescription: string;
  sceneReference: string | null;
  expectedSceneRevision: string | null;
  status: BlenderWorkflowPlanStatus;
  executionStatus: BlenderWorkflowExecutionStatus;
  requiredPermissions: BlenderWorkflowPermission[];
  confirmationRequired: boolean;
  steps: BlenderWorkflowStepPreview[];
  limitations: string[];
}

/** Safe, read-only chat/UI summary that preserves the planner's returned order. */
export function formatBlenderWorkflowSummary(plan: BlenderWorkflowPlanPreview): string {
  const lines = [
    `Workflow ${JSON.stringify(plan.workflowId)}: ${plan.status.replaceAll("_", " ")}.`,
    `Task: ${JSON.stringify(plan.taskDescription)}.`,
    `Scene scope: ${plan.sceneReference ? JSON.stringify(plan.sceneReference) : "not supplied"}; expected revision: ${plan.expectedSceneRevision ? JSON.stringify(plan.expectedSceneRevision) : "not supplied"}.`,
    `Execution: ${plan.executionStatus.replaceAll("_", " ")}; no workflow step was executed.`,
    `Required permissions: ${plan.requiredPermissions.length ? plan.requiredPermissions.join(", ") : "none"}.`,
    plan.confirmationRequired
      ? "Explicit confirmation would be required for the exact validated mutation plan; no confirmation has been granted."
      : "This preview does not request or grant workflow-level mutation confirmation.",
    "Dependencies define planning and review order only; they do not mean earlier steps were applied.",
    "Proposed steps:",
  ];

  plan.steps.forEach((step, index) => {
    const dependencies = step.dependsOn.length
      ? `; depends on ${step.dependsOn.map((id) => JSON.stringify(id)).join(", ")}`
      : "";
    const permission = step.requiredPermission ? `; permission ${step.requiredPermission}` : "";
    const confirmation = step.confirmationPolicy && step.confirmationPolicy !== "none"
      ? "; explicit confirmation is required but not granted"
      : "";
    const error = step.error ? `; ${step.error.code} at ${step.error.path}` : "";
    lines.push(
      `${index + 1}. ${JSON.stringify(step.stepId)} (${step.stepType}): ${step.status.replaceAll("_", " ")}${dependencies}${permission}${confirmation}${error}.`,
    );
  });

  lines.push(...plan.limitations);
  return lines.join("\n");
}
