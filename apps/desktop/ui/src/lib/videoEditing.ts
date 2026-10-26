import { invoke } from "@tauri-apps/api/core";
import {
  isDesktopShell,
  type InferenceError,
  type VideoEditingCapabilityStatus,
} from "./ipc";
import { toInferenceError } from "./inference";

const DESKTOP_ONLY: InferenceError = {
  code: "desktop_only",
  message: "Local video-editing capability status is available only in the Nexora desktop app.",
};

/** Fetches static Phase 10 capability metadata; this performs no runtime or hardware probe. */
export async function fetchVideoEditingStatus(): Promise<VideoEditingCapabilityStatus> {
  if (!isDesktopShell()) throw DESKTOP_ONLY;
  try {
    return await invoke<VideoEditingCapabilityStatus>("get_local_video_editing_status");
  } catch (cause) {
    throw toInferenceError(cause);
  }
}
