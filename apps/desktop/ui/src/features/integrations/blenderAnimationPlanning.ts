/** TypeScript mirrors of the Rust animation DTOs; Rust validation is authoritative. */
import type { SceneObjectTarget, SceneVector3 } from "./blenderScenePlanning";

export const BLENDER_ANIMATION_REQUEST_SCHEMA_VERSION = 1 as const;
export const BLENDER_ANIMATION_REQUEST_MAX_BYTES = 8 * 1024;
export const BLENDER_ANIMATION_MAX_FRAME = 100_000;
export const BLENDER_ANIMATION_MAX_INCLUSIVE_FRAME_COUNT = 2_500;
export const BLENDER_ANIMATION_FRAME_RATES = [24, 25, 30, 48, 50, 60] as const;
export const BLENDER_ANIMATION_MAX_ENDPOINT_KEYFRAMES = 2 as const;
export const BLENDER_ANIMATION_SEQUENCE_REQUEST_SCHEMA_VERSION = 1 as const;
export const BLENDER_ANIMATION_SEQUENCE_MAX_KEYFRAMES = 8 as const;
export const BLENDER_ANIMATION_SEQUENCE_MAX_REQUEST_BYTES = 8 * 1024;
export const BLENDER_ANIMATION_MAX_DESCRIPTION_CHARS = 1_024;

export type BlenderAnimationOperation = "translation" | "rotation" | "scale";
export type BlenderAnimationInterpolation = "linear" | "bezier" | "constant";
export type BlenderAnimationUnits =
  | "blender_units_local"
  | "euler_radians_local"
  | "dimensionless_local_scale";

/** Frames are 1-based integers; the range is inclusive and interpolation spans the endpoints. */
export interface BlenderAnimationRequest {
  schemaVersion: typeof BLENDER_ANIMATION_REQUEST_SCHEMA_VERSION;
  requestId: string;
  sceneReference: string;
  expectedSnapshotRevision: string | null;
  target: SceneObjectTarget | null;
  operation: BlenderAnimationOperation;
  startFrame: number;
  endFrame: number;
  frameRate: (typeof BLENDER_ANIMATION_FRAME_RATES)[number];
  interpolation: BlenderAnimationInterpolation;
  /** Local object channel value at startFrame. */
  startValue: SceneVector3;
  /** Local object channel value at endFrame. */
  endValue: SceneVector3;
  description: string | null;
}

export interface BlenderAnimationKeyframeIntent {
  frame: number;
  value: SceneVector3;
}

export interface BlenderAnimationTarget {
  objectId: string;
  objectName: string;
}

export type BlenderAnimationPlanState = "plan_created" | "awaiting_confirmation";
export type BlenderAnimationExecutionStatus = "connector_unavailable" | "executed";
export type BlenderAnimationActionClass = "read_only" | "mutation";
export type BlenderAnimationPermission = "scene_inspect" | "scene_modify";
export type BlenderAnimationConfirmation = "none" | "explicit_user_confirmation" | "destructive_confirmation";

/** Informational plan only; no execute/apply method or control is represented. */
export interface BlenderAnimationPlanPreview {
  request: BlenderAnimationRequest;
  actionId: "blender.animate_object";
  target: BlenderAnimationTarget;
  operation: BlenderAnimationOperation;
  units: BlenderAnimationUnits;
  startFrame: number;
  endFrame: number;
  frameRate: number;
  interpolation: BlenderAnimationInterpolation;
  keyframes: [BlenderAnimationKeyframeIntent, BlenderAnimationKeyframeIntent];
  actionClass: BlenderAnimationActionClass;
  validationStatus: "valid";
  requiredPermission: BlenderAnimationPermission;
  confirmationPolicy: BlenderAnimationConfirmation;
  planState: BlenderAnimationPlanState;
  executionStatus: BlenderAnimationExecutionStatus;
  snapshotRevision: string | null;
  freshnessCheckedByRevision: boolean;
  snapshotComplete: boolean;
  snapshotTruncated: boolean;
  limitations: string[];
}

export type BlenderAnimationPlanningErrorCode =
  | "snapshot_unavailable"
  | "invalid_snapshot"
  | "stale_snapshot"
  | "scene_mismatch"
  | "snapshot_revision_required"
  | "invalid_request"
  | "missing_target"
  | "target_not_found"
  | "ambiguous_target"
  | "incomplete_snapshot_for_name_resolution"
  | "invalid_frame_range"
  | "unsupported_frame_rate"
  | "invalid_value"
  | "invalid_candidate";

export interface BlenderAnimationPlanningError {
  code: BlenderAnimationPlanningErrorCode;
  path: string;
}

function formatVector(value: SceneVector3): string {
  return `(${value.x}, ${value.y}, ${value.z})`;
}

/** Safe chat-ready copy for a plan produced from a validated supplied snapshot. */
export function formatBlenderAnimationPlan(plan: BlenderAnimationPlanPreview): string {
  const operationLabel = {
    translation: "Translation",
    rotation: "Rotation",
    scale: "Scale",
  }[plan.operation];
  const unitLabel = {
    blender_units_local: "local Blender units",
    euler_radians_local: "local Euler radians",
    dimensionless_local_scale: "dimensionless local scale factors",
  }[plan.units];
  const keyframes = plan.keyframes
    .map((keyframe) => `frame ${keyframe.frame}: ${formatVector(keyframe.value)}`)
    .join("; ");
  const confirmation = plan.confirmationPolicy === "none"
    ? "No mutation confirmation required."
    : "Would require explicit confirmation; no confirmation was granted.";
  return [
    `Animation plan: ${plan.planState.replaceAll("_", " ")}.`,
    `Target: ${JSON.stringify(plan.target.objectName)} (${plan.target.objectId}).`,
    `Type: ${operationLabel}; values use ${unitLabel}.`,
    `Frames ${plan.startFrame}–${plan.endFrame} inclusive at ${plan.frameRate} fps.`,
    `Keyframe intent (exactly two endpoints): ${keyframes}.`,
    `Interpolation: ${plan.interpolation}.`,
    `Validation: ${plan.validationStatus}; permission: ${plan.requiredPermission}; ${confirmation}`,
    `Blender execution: ${plan.executionStatus.replaceAll("_", " ")}. No keyframes were created, applied, or rendered.`,
    ...(plan.request.description ? [`Description: ${plan.request.description}`] : []),
    ...plan.limitations,
  ].join("\n");
}

/** Phase 11 bounded object transform curve; rig/armature targets are unsupported. */
export interface BlenderAnimationSequenceRequest {
  schemaVersion: typeof BLENDER_ANIMATION_SEQUENCE_REQUEST_SCHEMA_VERSION;
  requestId: string;
  sceneReference: string;
  expectedSnapshotRevision: string;
  target: { kind: "object_id"; objectId: string };
  operation: BlenderAnimationOperation;
  frameRate: (typeof BLENDER_ANIMATION_FRAME_RATES)[number];
  interpolation: BlenderAnimationInterpolation;
  /** Rust enforces 2–8 strictly increasing, bounded keyframes. */
  keyframes: BlenderAnimationKeyframeIntent[];
}

export type BlenderAnimationSequencePlanningErrorCode =
  | "payload_too_large"
  | "malformed_request"
  | "unsupported_schema_version"
  | "invalid_request"
  | "snapshot_unavailable"
  | "stale_snapshot"
  | "snapshot_revision_unavailable"
  | "scene_mismatch"
  | "target_not_found"
  | "unsupported_target_type"
  | "invalid_frame_rate"
  | "invalid_keyframes"
  | "invalid_candidate";

export interface BlenderAnimationSequencePlanningError {
  code: BlenderAnimationSequencePlanningErrorCode;
  path: string;
}

export interface BlenderAnimationSequencePlanPreview {
  request: BlenderAnimationSequenceRequest;
  actionId: "blender.animate_object_keyframes";
  target: BlenderAnimationTarget;
  operation: BlenderAnimationOperation;
  units: BlenderAnimationUnits;
  frameRate: number;
  interpolation: BlenderAnimationInterpolation;
  keyframes: BlenderAnimationKeyframeIntent[];
  actionClass: "mutation";
  validationStatus: "valid";
  requiredPermission: "scene_modify";
  confirmationPolicy: "explicit_user_confirmation";
  planState: "awaiting_confirmation";
  executionStatus: "connector_unavailable";
  snapshotRevision: string;
  limitations: string[];
}

/** Review copy for a bounded curve candidate; never claims keys were inserted. */
export function formatBlenderAnimationSequencePlan(plan: BlenderAnimationSequencePlanPreview): string {
  const keyframes = plan.keyframes
    .map((keyframe) => `frame ${keyframe.frame}: ${formatVector(keyframe.value)}`)
    .join("; ");
  return [
    `Animation sequence plan: ${plan.planState.replaceAll("_", " ")}.`,
    `Target: ${JSON.stringify(plan.target.objectName)} (${plan.target.objectId}); revision ${JSON.stringify(plan.snapshotRevision)}.`,
    `Operation: ${plan.operation}; ${plan.keyframes.length} bounded keyframe intents at ${plan.frameRate} fps (${plan.interpolation}).`,
    `Keyframes: ${keyframes}.`,
    `Permission: ${plan.requiredPermission}; explicit confirmation is required and has not been granted.`,
    `Blender execution: ${plan.executionStatus.replaceAll("_", " ")}. No keyframes were created or applied.`,
    ...plan.limitations,
  ].join("\n");
}
