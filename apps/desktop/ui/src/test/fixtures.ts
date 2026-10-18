import type { AppStatus, EngineState } from "../lib/ipc";

export interface StatusOverrides {
  state?: EngineState;
  model?: string | null;
  detail?: string;
  modelPath?: string | null;
  runtimePath?: string | null;
  contextSize?: number;
}

/** Builds the exact `AppStatus` shape the Rust core serialises. */
export function makeStatus(overrides: StatusOverrides = {}): AppStatus {
  const state = overrides.state ?? "ready";

  return {
    name: "Nexora",
    version: "0.1.0",
    engine: {
      state,
      runtime: state === "not_configured" ? null : "llama-server b11714",
      model:
        overrides.model !== undefined
          ? overrides.model
          : state === "not_configured"
            ? null
            : "qwen2.5-coder-7b-instruct-q4_k_m.gguf",
      detail:
        overrides.detail ??
        (state === "ready"
          ? "Model loaded and answering on 127.0.0.1."
          : "No local model is configured yet."),
    },
    modelPath:
      overrides.modelPath !== undefined
        ? overrides.modelPath
        : state === "not_configured"
          ? null
          : "D:\\Models\\qwen2.5-coder-7b\\model.gguf",
    runtimePath:
      overrides.runtimePath !== undefined
        ? overrides.runtimePath
        : state === "not_configured"
          ? null
          : "C:\\Tools\\llama.cpp\\llama-server.exe",
    contextSize: overrides.contextSize ?? 32768,
  };
}

/** Marks the jsdom window as the Tauri webview, so the desktop path is used. */
export function markDesktop(value = true): void {
  if (value) {
    Object.defineProperty(window, "__TAURI_INTERNALS__", {
      value: {},
      configurable: true,
      writable: true,
    });
    return;
  }

  Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
}
