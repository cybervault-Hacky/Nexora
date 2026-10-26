import { invoke } from "@tauri-apps/api/core";
import {
  isDesktopShell,
  type GenerateImageRequestPayload,
  type GeneratedImageArtifact,
  type ImageGenerationStatus,
  type InferenceError,
} from "./ipc";
import { toInferenceError } from "./inference";

const DESKTOP_ONLY: InferenceError = {
  code: "desktop_only",
  message: "Local image generation is available only in the Nexora desktop app.",
};

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (!isDesktopShell()) throw DESKTOP_ONLY;
  try {
    return await invoke<T>(command, args);
  } catch (cause) {
    throw toInferenceError(cause);
  }
}

/** Reports local SD-Turbo lifecycle/runtime/resource status without loading a model. */
export function fetchImageGenerationStatus(): Promise<ImageGenerationStatus> {
  return call<ImageGenerationStatus>("get_local_image_generation_status");
}

/** Requests local SD-Turbo generation; native admission may reject before runtime probing. Model/runtime/output paths are never accepted from the renderer. */
export function generateLocalImage(
  request: GenerateImageRequestPayload,
): Promise<GeneratedImageArtifact> {
  return call<GeneratedImageArtifact>("generate_local_image", { request });
}

/** Cancels the one active local image process, if any. */
export async function cancelLocalImageGeneration(): Promise<boolean> {
  if (!isDesktopShell()) return false;
  try {
    return await invoke<boolean>("cancel_local_image_generation");
  } catch {
    return false;
  }
}

/** Retrieves verified metadata by opaque manager-owned artifact ID only. */
export function fetchGeneratedImageArtifact(
  artifactId: string,
): Promise<GeneratedImageArtifact> {
  return call<GeneratedImageArtifact>("get_generated_image_artifact", { artifactId });
}
