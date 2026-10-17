import { invoke } from "@tauri-apps/api/core";

/**
 * Engine states the interface can render. `ready` is part of the contract so
 * the UI can consume it unchanged when the supervisor lands, but the Rust core
 * in this build only ever returns `not_connected`.
 */
export type EngineState = "not_connected" | "ready";

export interface EngineStatus {
  state: EngineState;
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
  return (
    typeof window !== "undefined" && Reflect.has(window, "__TAURI_INTERNALS__")
  );
}

/** Single source of truth for "is inference usable right now?". */
export function isEngineReady(status: AppStatus | null): boolean {
  return status?.engine.state === "ready";
}

/**
 * Status shown when the UI runs outside Tauri (browser preview or a failed
 * IPC call). It never pretends that an engine exists.
 */
export function browserPreviewStatus(): AppStatus {
  return {
    name: "Nexora",
    version: "0.1.0",
    engine: {
      state: "not_connected",
      runtime: "llama.cpp (planned)",
      model: null,
      detail:
        "Running outside the desktop shell. Open the Tauri build to reach the Rust core.",
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
