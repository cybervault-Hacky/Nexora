import { Channel, invoke } from "@tauri-apps/api/core";
import {
  isDesktopShell,
  type MediaModelCatalog,
  type MediaModelDownloadProgress,
  type MediaModelDownloadResult,
  type MediaModelError,
  type MediaModelValidationResult,
  type MediaModelView,
} from "./ipc";
import { toInferenceError } from "./inference";
const DESKTOP_ONLY: MediaModelError = {
  code: "desktop_only",
  message: "Local media-model management is available in the Nexora desktop app.",
};

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (!isDesktopShell()) throw DESKTOP_ONLY;
  try {
    return await invoke<T>(command, args);
  } catch (cause) {
    throw toInferenceError(cause);
  }
}

export function fetchMediaModelCatalog(): Promise<MediaModelCatalog> {
  return call<MediaModelCatalog>("list_media_models");
}

export function fetchMediaModel(modelId: string): Promise<MediaModelView> {
  return call<MediaModelView>("get_media_model", { modelId });
}

export async function downloadMediaModel(
  modelId: string,
  onEvent: (progress: MediaModelDownloadProgress) => void,
): Promise<MediaModelDownloadResult> {
  if (!isDesktopShell()) throw DESKTOP_ONLY;
  const channel = new Channel<MediaModelDownloadProgress>();
  let closed = false;
  channel.onmessage = (progress) => {
    if (!closed) onEvent(progress);
  };
  try {
    return await invoke<MediaModelDownloadResult>("download_media_model", {
      modelId,
      onEvent: channel,
    });
  } catch (cause) {
    throw toInferenceError(cause);
  } finally {
    closed = true;
    channel.onmessage = () => undefined;
  }
}

export function cancelMediaModelDownload(modelId: string): Promise<boolean> {
  return call<boolean>("cancel_media_model_download", { modelId });
}

export function validateMediaModel(modelId: string): Promise<MediaModelValidationResult> {
  return call<MediaModelValidationResult>("validate_media_model", { modelId });
}

export function removeMediaModel(modelId: string): Promise<boolean> {
  return call<boolean>("remove_media_model", { modelId });
}
