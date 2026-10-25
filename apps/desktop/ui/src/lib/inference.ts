import { Channel, invoke } from "@tauri-apps/api/core";
import {
  isDesktopShell,
  previewStatus,
  type AppStatus,
  type GenerateRequestPayload,
  type GenerationEvent,
  type GenerationOutcome,
  type InferenceError,
  type IntegrityReport,
  type ModelDownloadEvent,
  type ModelDownloadOutcome,
  type DiskSpace,
  type ModelCatalog,
  type ModelInfo,
  type Readiness,
  type ModelProfile,
  type RuntimeInfo,
  type Settings,
  type SystemInfo,
} from "./ipc";

const DESKTOP_ONLY: InferenceError = {
  code: "desktop_only",
  message:
    "Local inference runs in the Nexora desktop app. This browser preview cannot reach the engine.",
};

/**
 * Tauri rejects a command with the serialised error value. Normalise every
 * failure — including transport and serialisation errors — into one shape so
 * the UI never has to guess.
 */
export function toInferenceError(cause: unknown): InferenceError {
  if (
    typeof cause === "object" &&
    cause !== null &&
    "code" in cause &&
    "message" in cause &&
    typeof (cause as InferenceError).code === "string" &&
    typeof (cause as InferenceError).message === "string"
  ) {
    return cause as InferenceError;
  }

  if (typeof cause === "string" && cause.trim().length > 0) {
    return { code: "ipc", message: cause };
  }

  if (cause instanceof Error && cause.message.trim().length > 0) {
    return { code: "ipc", message: cause.message };
  }

  return { code: "ipc", message: "The desktop core did not answer." };
}

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (!isDesktopShell()) {
    throw DESKTOP_ONLY;
  }

  try {
    return await invoke<T>(command, args);
  } catch (cause) {
    throw toInferenceError(cause);
  }
}

export async function fetchStatus(): Promise<AppStatus> {
  if (!isDesktopShell()) {
    return previewStatus();
  }

  return call<AppStatus>("get_app_status");
}

export function fetchSettings(): Promise<Settings> {
  return call<Settings>("get_settings");
}

export function saveSettings(settings: Settings): Promise<Settings> {
  return call<Settings>("save_settings", { settings });
}

/**
 * Metadata for the selected model profile. Read-only: it does not report
 * whether the model is installed or running — use `fetchModelCatalog`.
 */
export function fetchModelProfile(): Promise<ModelProfile> {
  return call<ModelProfile>("get_model_profile");
}

/**
 * Every registered profile with its real availability.
 *
 * The core decides `status`; the UI must not derive readiness from a path.
 */
export function fetchModelCatalog(): Promise<ModelCatalog> {
  return call<ModelCatalog>("get_model_catalog");
}

/**
 * The backend's readiness for the selected profile, re-measured on every call.
 *
 * Used by the model manager so a row can refresh its state without re-reading
 * the whole catalogue. The UI must render this result, never recompute it.
 */
export function fetchModelReadiness(): Promise<Readiness> {
  return call<Readiness>("check_model_readiness");
}

/**
 * Free space where models are stored, or at `targetDir` when given.
 *
 * `null` fields mean the platform could not measure it. Never substitute a
 * guess for a null here.
 */
export function fetchDiskSpace(targetDir?: string | null): Promise<DiskSpace> {
  return call<DiskSpace>("get_disk_space", { targetDir: targetDir ?? null });
}

/**
 * Persists the selected profile. Does not download, load, or change paths —
 * the chat guard refuses a request when the loaded model is not the chosen one.
 */
export function selectModel(id: string): Promise<Settings> {
  return call<Settings>("select_model", { id });
}

/**
 * Downloads a profile's approved artefact and reports progress.
 *
 * Only ever called from an explicit user action. The file is verified against
 * the registry before it is published; `cancelled` means nothing was installed.
 * A profile whose artefact is `pending` is refused by the core.
 */
export async function startModelDownload(
  profileId: string,
  onEvent: (event: ModelDownloadEvent) => void,
): Promise<ModelDownloadOutcome> {
  if (!isDesktopShell()) {
    throw DESKTOP_ONLY;
  }

  let closed = false;
  const channel = new Channel<ModelDownloadEvent>();
  channel.onmessage = (event) => {
    if (!closed) {
      onEvent(event);
    }
  };

  try {
    return await invoke<ModelDownloadOutcome>("start_model_download", {
      profileId,
      onEvent: channel,
    });
  } catch (cause) {
    throw toInferenceError(cause);
  } finally {
    closed = true;
    channel.onmessage = () => undefined;
  }
}

export async function cancelModelDownload(): Promise<boolean> {
  if (!isDesktopShell()) {
    return false;
  }

  try {
    return await invoke<boolean>("cancel_model_download");
  } catch {
    return false;
  }
}

/**
 * Hashes a chosen file and compares it with a profile's approved digest.
 *
 * Omitting `profileId` uses the selected profile in the desktop core.
 */
export function verifyModel(path: string, profileId?: string): Promise<IntegrityReport> {
  return call<IntegrityReport>("verify_model", { path, profileId: profileId ?? null });
}

export function validateModel(
  path: string,
  contextSize: number,
): Promise<ModelInfo> {
  return call<ModelInfo>("validate_model", { path, contextSize });
}

export function probeRuntime(path: string): Promise<RuntimeInfo> {
  return call<RuntimeInfo>("probe_runtime", { path });
}

export function loadModel(): Promise<AppStatus> {
  return call<AppStatus>("load_model");
}

export function unloadModel(): Promise<AppStatus> {
  return call<AppStatus>("unload_model");
}

export function fetchSystemInfo(): Promise<SystemInfo> {
  return call<SystemInfo>("get_system_info");
}

/**
 * Starts a streamed completion. Each token arrives through a Tauri channel;
 * the promise resolves when generation ends, is stopped, or fails.
 */
export async function generate(
  request: GenerateRequestPayload,
  onEvent: (event: GenerationEvent) => void,
): Promise<GenerationOutcome> {
  if (!isDesktopShell()) {
    throw DESKTOP_ONLY;
  }

  let closed = false;
  const channel = new Channel<GenerationEvent>();
  channel.onmessage = (event) => {
    if (!closed) {
      onEvent(event);
    }
  };

  try {
    return await invoke<GenerationOutcome>("start_generation", {
      request,
      onEvent: channel,
    });
  } catch (cause) {
    throw toInferenceError(cause);
  } finally {
    closed = true;
    channel.onmessage = () => undefined;
  }
}

export async function cancelGeneration(requestId: string): Promise<boolean> {
  if (!isDesktopShell()) {
    return false;
  }

  try {
    return await invoke<boolean>("cancel_generation", { requestId });
  } catch {
    return false;
  }
}
