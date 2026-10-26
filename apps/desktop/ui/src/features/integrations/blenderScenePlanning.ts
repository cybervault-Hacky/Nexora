/** TypeScript mirrors of the Rust transport-independent Blender scene-planning DTOs.
 * Renderer types are descriptive only; Rust remains authoritative for validation.
 */
export const BLENDER_SCENE_SNAPSHOT_SCHEMA_VERSION = 1 as const;
export const BLENDER_SCENE_EDIT_SCHEMA_VERSION = 1 as const;
export const BLENDER_SCENE_SNAPSHOT_MAX_BYTES = 256 * 1024;
export const BLENDER_SCENE_MAX_OBJECTS = 1_000;
export const BLENDER_SCENE_MAX_COLLECTIONS = 256;

export interface SceneVector3 {
  x: number;
  y: number;
  z: number;
}

export interface SceneObjectTransform {
  location: SceneVector3;
  rotation: SceneVector3;
  scale: SceneVector3;
}

export type SceneObjectType = "mesh" | "curve" | "camera" | "light" | "empty" | "other";

export interface SceneObjectSnapshot {
  objectId: string;
  name: string;
  objectType: SceneObjectType;
  transform: SceneObjectTransform;
  parentObjectId: string | null;
  collectionIds: string[];
}

export interface SceneCollectionSnapshot {
  collectionId: string;
  name: string;
}

export interface SceneSnapshotCompleteness {
  complete: boolean;
  truncated: boolean;
  reportedObjectCount: number;
  reportedCollectionCount: number;
}

/** A timestamp is source metadata, not proof of currentness or provenance. */
export interface SceneSnapshot {
  schemaVersion: typeof BLENDER_SCENE_SNAPSHOT_SCHEMA_VERSION;
  sceneReference: string;
  sceneName: string;
  revision: string | null;
  capturedAtUnixMs: number;
  completeness: SceneSnapshotCompleteness;
  objects: SceneObjectSnapshot[];
  collections: SceneCollectionSnapshot[];
}

export type SceneSnapshotState =
  | { state: "snapshot_unavailable" }
  | { state: "invalid_snapshot"; code: string }
  | { state: "stale_snapshot"; code: string }
  | { state: "valid_snapshot"; snapshot: SceneSnapshot };

export type SceneObjectTarget =
  | { kind: "object_id"; objectId: string }
  | { kind: "object_name"; name: string };

export type ScenePrimitiveType = "cube" | "uv_sphere" | "cylinder" | "cone" | "torus" | "plane";

export type SceneEditIntent =
  | { kind: "scene_summary" }
  | { kind: "rename_object"; target: SceneObjectTarget; newName: string }
  | { kind: "set_object_transform"; target: SceneObjectTarget; transform: SceneObjectTransform }
  | { kind: "create_primitive"; primitive: ScenePrimitiveType; name?: string | null; transform: SceneObjectTransform };

export interface SceneEditRequest {
  schemaVersion: typeof BLENDER_SCENE_EDIT_SCHEMA_VERSION;
  requestId: string;
  sceneReference: string;
  userRequest: string;
  expectedSnapshotRevision: string | null;
  intent: SceneEditIntent;
}

export type ScenePlanState = "plan_created" | "awaiting_confirmation";
export type SceneExecutionStatus = "connector_unavailable" | "executed";
export type SceneActionClass = "read_only" | "mutation";
export type SceneActionPermission = "scene_inspect" | "scene_modify";
export type SceneConfirmationPolicy = "none" | "explicit_user_confirmation" | "destructive_confirmation";

export type PlannedSceneAction =
  | { kind: "scene_summary" }
  | { kind: "rename_object"; objectId: string; currentName: string; newName: string }
  | { kind: "set_object_transform"; objectId: string; objectName: string; transform: SceneObjectTransform }
  | { kind: "create_primitive"; primitive: ScenePrimitiveType; name: string | null; transform: SceneObjectTransform };

/** Informational preview DTO; no Apply/Execute operation is represented here. */
export interface SceneEditPlanPreview {
  request: SceneEditRequest;
  actionId: "blender.get_scene_summary" | "blender.rename_object" | "blender.set_object_transform" | "blender.create_primitive";
  actionClass: SceneActionClass;
  validationStatus: "valid";
  requiredPermission: SceneActionPermission;
  confirmationPolicy: SceneConfirmationPolicy;
  planState: ScenePlanState;
  executionStatus: SceneExecutionStatus;
  snapshotRevision: string | null;
  snapshotComplete: boolean;
  snapshotTruncated: boolean;
  freshnessCheckedByRevision: boolean;
  plannedAction: PlannedSceneAction;
  limitations: string[];
}

export type ScenePlanningErrorCode =
  | "snapshot_unavailable"
  | "invalid_snapshot"
  | "stale_snapshot"
  | "scene_mismatch"
  | "invalid_request"
  | "snapshot_revision_required"
  | "target_not_found"
  | "ambiguous_target"
  | "invalid_candidate";

export interface ScenePlanningError {
  code: ScenePlanningErrorCode;
  path: string;
}

/** Safe copy for an existing-chat plan preview; no success/apply language. */
export function formatSceneEditPlanPreview(plan: SceneEditPlanPreview): string {
  const proposed = (() => {
    switch (plan.plannedAction.kind) {
      case "scene_summary": return "Request a read-only scene summary; no Blender request was made.";
      case "rename_object": return `Rename ${JSON.stringify(plan.plannedAction.currentName)} (${plan.plannedAction.objectId}) to ${JSON.stringify(plan.plannedAction.newName)}.`;
      case "set_object_transform": return `Set transform for ${JSON.stringify(plan.plannedAction.objectName)} (${plan.plannedAction.objectId}): ${JSON.stringify(plan.plannedAction.transform)}.`;
      case "create_primitive": return `Create ${plan.plannedAction.primitive}${plan.plannedAction.name ? ` named ${JSON.stringify(plan.plannedAction.name)}` : ""} with transform ${JSON.stringify(plan.plannedAction.transform)}.`;
    }
  })();
  const target = "objectId" in plan.plannedAction
    ? plan.plannedAction.objectId
    : "scene";
  const confirmation = plan.confirmationPolicy === "none"
    ? "No mutation confirmation required for this read-only plan."
    : "Would require explicit confirmation of this exact validated action; no confirmation was granted.";
  return [
    `Plan: ${plan.planState.replaceAll("_", " ")}.`,
    `Requested: ${plan.request.userRequest}`,
    `Action: ${plan.actionId}; target reference: ${target}.`,
    `Proposed parameters: ${proposed}`,
    `Validation: ${plan.validationStatus}; permission: ${plan.requiredPermission}; ${confirmation}`,
    `Blender execution: ${plan.executionStatus.replaceAll("_", " ")}. Nothing has been applied.`,
    ...plan.limitations,
  ].join("\n");
}
