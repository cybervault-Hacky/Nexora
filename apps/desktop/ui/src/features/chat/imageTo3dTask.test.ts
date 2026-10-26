import { describe, expect, it } from "vitest";
import type { ImageEditSourceArtifact } from "../../lib/ipc";
import {
  IMAGE_REFERENCE_MAX_BYTES,
  imageReferenceFromArtifact,
  preflightReferenceFile,
  prepareImageTo3dTask,
} from "./imageTo3dTask";

const artifact: ImageEditSourceArtifact = {
  schemaVersion: 1,
  sourceId: "0123456789abcdef0123456789abcdef",
  mimeType: "image/png",
  width: 512,
  height: 512,
  bytes: 128_000,
  sha256: "0".repeat(64),
  createdAtUnixMs: 1,
};

describe("image-to-3D reference preparation", () => {
  it("creates versioned, path-free metadata from the local validator result", () => {
    expect(imageReferenceFromArtifact(artifact)).toEqual({
      schemaVersion: 1,
      referenceId: artifact.sourceId,
      format: "image/png",
      width: 512,
      height: 512,
      sizeBytes: 128_000,
      processingState: "validated",
      validationErrors: [],
    });
  });

  it("prepares task data without claiming analysis or generating executable instructions", () => {
    const reference = imageReferenceFromArtifact(artifact);
    const task = prepareImageTo3dTask("Create a low-poly stone arch", reference);
    expect(task).toMatchObject({
      schemaVersion: 1,
      taskType: "image_to_3d",
      userRequest: "Create a low-poly stone arch",
      analysisAvailable: false,
      nextStage: "awaiting_supported_vision_capability",
    });
    expect(task.references[0]?.description).toBe(task.userRequest);
    expect(task).not.toHaveProperty("script");
    expect(task).not.toHaveProperty("pythonCode");
    expect(task).not.toHaveProperty("filePath");
  });

  it("bounds empty and oversized file selection before byte reads", () => {
    expect(preflightReferenceFile({ size: 0 })).toMatch(/empty/);
    expect(preflightReferenceFile({ size: IMAGE_REFERENCE_MAX_BYTES + 1 })).toMatch(/8 MiB/);
    expect(preflightReferenceFile({ size: IMAGE_REFERENCE_MAX_BYTES })).toBeNull();
  });
});
