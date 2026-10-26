import type { ImageEditSourceArtifact } from "../../lib/ipc";

export const IMAGE_REFERENCE_SCHEMA_VERSION = 1 as const;
export const IMAGE_TO_3D_TASK_SCHEMA_VERSION = 1 as const;
export const IMAGE_REFERENCE_MAX_BYTES = 8 * 1024 * 1024;
export const IMAGE_REFERENCE_MAX_DIMENSION = 2_048;
export const IMAGE_REFERENCE_MAX_PIXELS = 4 * 1024 * 1024;

export type ImageReferenceProcessingState = "validated";

/** Versioned, path-free metadata for a locally managed reference asset. */
export interface ImageReferenceMetadata {
  schemaVersion: typeof IMAGE_REFERENCE_SCHEMA_VERSION;
  referenceId: string;
  format: "image/png";
  width: number;
  height: number;
  sizeBytes: number;
  description?: string;
  processingState: ImageReferenceProcessingState;
  validationErrors: string[];
}

export interface PreparedImageTo3dTask {
  schemaVersion: typeof IMAGE_TO_3D_TASK_SCHEMA_VERSION;
  taskType: "image_to_3d";
  userRequest: string;
  references: ImageReferenceMetadata[];
  knownConstraints: string[];
  missingInformation: string[];
  analysisAvailable: false;
  nextStage: "awaiting_supported_vision_capability";
}

export function imageReferenceFromArtifact(
  artifact: ImageEditSourceArtifact,
): ImageReferenceMetadata {
  return {
    schemaVersion: IMAGE_REFERENCE_SCHEMA_VERSION,
    referenceId: artifact.sourceId,
    format: artifact.mimeType,
    width: artifact.width,
    height: artifact.height,
    sizeBytes: artifact.bytes,
    processingState: "validated",
    validationErrors: [],
  };
}

/** Prepares data only; it does not interpret the image or generate Blender actions. */
export function prepareImageTo3dTask(
  userRequest: string,
  reference: ImageReferenceMetadata,
): PreparedImageTo3dTask {
  return {
    schemaVersion: IMAGE_TO_3D_TASK_SCHEMA_VERSION,
    taskType: "image_to_3d",
    userRequest: userRequest.trim(),
    references: [{ ...reference, description: userRequest.trim() || undefined }],
    knownConstraints: [
      "A user-selected PNG reference is locally validated.",
      "The reference has not been analyzed by a vision-capable model.",
      "Blender execution is unavailable.",
    ],
    missingInformation: [
      "Image understanding and a supported image-to-3D model are unavailable.",
      "Object geometry, style, and output constraints have not been analyzed or validated from the image.",
      "Future generation should confirm any output constraints not included in the user's written request.",
    ],
    analysisAvailable: false,
    nextStage: "awaiting_supported_vision_capability",
  };
}

/** Bounded in-renderer checks before passing at most 8 MiB to the Rust PNG validator. */
export function preflightReferenceFile(file: Pick<File, "size">): string | null {
  if (file.size === 0) return "The selected image is empty.";
  if (file.size > IMAGE_REFERENCE_MAX_BYTES) return "The image exceeds the 8 MiB limit.";
  return null;
}
