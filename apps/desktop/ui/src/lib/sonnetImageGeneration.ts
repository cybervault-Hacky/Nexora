import { invoke } from "@tauri-apps/api/core";
import {
  isDesktopShell,
  type InferenceError,
  type SonnetImageGenerationCapabilityStatus,
} from "./ipc";
import { toInferenceError } from "./inference";

const DESKTOP_ONLY: InferenceError = {
  code: "desktop_only",
  message: "Sonnet image-generation capability status is available only in the Nexora desktop app.",
};

/** Reads the reviewed candidate and blockers; this calls no model or runtime. */
export async function fetchSonnetImageGenerationStatus(): Promise<SonnetImageGenerationCapabilityStatus> {
  if (!isDesktopShell()) throw DESKTOP_ONLY;
  try {
    return await invoke<SonnetImageGenerationCapabilityStatus>(
      "get_sonnet_image_generation_status",
    );
  } catch (cause) {
    throw toInferenceError(cause);
  }
}
