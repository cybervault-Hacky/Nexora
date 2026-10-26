import { invoke } from "@tauri-apps/api/core";
import {
  isDesktopShell,
  type ImageEditSourceArtifact,
  type ImageEditingCapabilityStatus,
  type ImportImageEditSourcePayload,
  type InferenceError,
} from "./ipc";
import { toInferenceError } from "./inference";

const DESKTOP_ONLY: InferenceError = {
  code: "desktop_only",
  message: "Managed image-editing sources are available only in the Nexora desktop app.",
};

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (!isDesktopShell()) throw DESKTOP_ONLY;
  try {
    return await invoke<T>(command, args);
  } catch (cause) {
    throw toInferenceError(cause);
  }
}

/** Reports the reviewed candidate and the unavailable runtime boundary; it performs no probes. */
export function fetchImageEditingStatus(): Promise<ImageEditingCapabilityStatus> {
  return call<ImageEditingCapabilityStatus>("get_local_image_editing_status");
}

/** Copies a selected PNG's bytes into app-owned storage. File paths are never accepted. */
export function importImageEditSource(
  sourceBytes: Uint8Array,
): Promise<ImageEditSourceArtifact> {
  if (sourceBytes.byteLength > 8 * 1024 * 1024) {
    throw {
      code: "image_too_large",
      message: "The source image exceeds the 8 MiB import limit.",
    } satisfies InferenceError;
  }
  const request: ImportImageEditSourcePayload = {
    sourceBytes: Array.from(sourceBytes),
  };
  return call<ImageEditSourceArtifact>("import_image_edit_source", { request });
}

/** Revalidates a previously imported source by opaque manager-owned ID. */
export function fetchImageEditSource(
  sourceId: string,
): Promise<ImageEditSourceArtifact> {
  return call<ImageEditSourceArtifact>("get_image_edit_source", { sourceId });
}

/** Removes a managed reference by opaque app ID; filesystem paths are never passed. */
export function deleteImageEditSource(sourceId: string): Promise<void> {
  return call<void>("remove_image_edit_source", { sourceId });
}
