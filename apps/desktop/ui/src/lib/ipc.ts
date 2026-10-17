import { invoke } from "@tauri-apps/api/core";

export type EngineState = "not_connected" | "unavailable";

export interface EngineStatus {
  state: EngineState;
  inferenceAvailable: boolean;
  runtime: string;
  model: string | null;
  detail: string;
}

export interface AppStatus {
  name: string;
  version: string;
  engine: EngineStatus;
}

/** True only inside the Tauri webview, never in a plain browser tab. */
export function isDesktopShell(): boolean {
  return typeof window !== "undefined" && Reflect.has(window, "__TAURI_INTERNALS__");
}

export function browserPreviewStatus(): AppStatus {
  return {
    name: "Nexora",
    version: "0.1.0",
    engine: {
      state: "unavailable",
      inferenceAvailable: false,
      runtime: "llama.cpp (planned)",
      model: null,
      detail:
        "Running in a browser preview. Open the Tauri desktop build to reach the Rust core.",
    },
  };
}

/**
 * The only Rust call in the foundation: a read-only status snapshot.
 * It cannot read files, spawn processes, or change application state.
 */
export async function loadAppStatus(): Promise<AppStatus> {
  if (!isDesktopShell()) {
    return browserPreviewStatus();
  }

  return invoke<AppStatus>("get_app_status");
}
