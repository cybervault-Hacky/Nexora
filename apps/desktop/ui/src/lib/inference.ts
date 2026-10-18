import { Channel, invoke } from "@tauri-apps/api/core";
import {
  isDesktopShell,
  previewStatus,
  type AppStatus,
  type GenerateRequestPayload,
  type GenerationEvent,
  type GenerationOutcome,
  type InferenceError,
  type ModelInfo,
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

  const channel = new Channel<GenerationEvent>();
  channel.onmessage = (event) => onEvent(event);

  try {
    return await invoke<GenerationOutcome>("start_generation", {
      request,
      onEvent: channel,
    });
  } catch (cause) {
    throw toInferenceError(cause);
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
