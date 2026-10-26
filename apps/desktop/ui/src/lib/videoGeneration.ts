import { invoke } from "@tauri-apps/api/core";
import {
  isDesktopShell,
  type InferenceError,
  type VideoGenerationCapabilityStatus,
} from "./ipc";
import { toInferenceError } from "./inference";

const DESKTOP_ONLY: InferenceError = {
  code: "desktop_only",
  message: "Local video-generation capability status is available only in the Nexora desktop app.",
};

/** Fetches static Phase 5 capability metadata; this performs no runtime or hardware probe. */
export async function fetchVideoGenerationStatus(): Promise<VideoGenerationCapabilityStatus> {
  if (!isDesktopShell()) throw DESKTOP_ONLY;
  try {
    return await invoke<VideoGenerationCapabilityStatus>("get_local_video_generation_status");
  } catch (cause) {
    throw toInferenceError(cause);
  }
}
