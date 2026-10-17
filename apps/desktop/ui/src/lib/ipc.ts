/** Types shared with the Rust inference layer. Keep in sync with `inference/mod.rs`. */

export type EngineState =
  | "not_configured"
  | "stopped"
  | "loading"
  | "ready"
  | "generating"
  | "error";

export interface EngineStatus {
  state: EngineState;
  runtime: string | null;
  model: string | null;
  detail: string;
}

export interface AppStatus {
  name: string;
  version: string;
  engine: EngineStatus;
  modelPath: string | null;
  runtimePath: string | null;
  contextSize: number;
}

export interface Settings {
  modelPath: string | null;
  runtimePath: string | null;
  contextSize: number;
}

export interface ModelInfo {
  path: string;
  fileName: string;
  sizeBytes: number;
  sizeGib: number;
  ggufVersion: number;
  tensorCount: number;
  estimatedMinRamGib: number;
  fitsAvailableRam: boolean | null;
  warnings: string[];
}

export interface RuntimeInfo {
  path: string;
  versionLine: string;
  /** False when the executable does not report the llama.cpp build in docs/phase-2. */
  pinnedBuildMatched: boolean;
}

export interface SystemInfo {
  totalRamGib: number | null;
  availableRamGib: number | null;
  platform: string;
}

export interface ChatTurn {
  role: "system" | "user" | "assistant";
  content: string;
}

export interface GenerateRequestPayload {
  requestId: string;
  messages: ChatTurn[];
  maxTokens?: number;
  temperature?: number;
}

export interface GenerationEvent {
  type: "started" | "delta";
  requestId: string;
  text?: string;
}

export interface GenerationOutcome {
  requestId: string;
  finishReason: string | null;
  cancelled: boolean;
}

export interface InferenceError {
  code: string;
  message: string;
}

/** True only inside the Tauri webview, never in a plain browser tab. */
export function isDesktopShell(): boolean {
  return (
    typeof window !== "undefined" && Reflect.has(window, "__TAURI_INTERNALS__")
  );
}

export function isEngineReady(status: AppStatus | null): boolean {
  return status?.engine.state === "ready" || status?.engine.state === "generating";
}

export function isEngineBusy(status: AppStatus | null): boolean {
  return status?.engine.state === "generating";
}

/** Status used when the UI runs in a plain browser, never pretending otherwise. */
export function previewStatus(): AppStatus {
  return {
    name: "Nexora",
    version: "0.1.0",
    engine: {
      state: "not_configured",
      runtime: null,
      model: null,
      detail:
        "Browser preview. Local inference is only available in the Tauri desktop build.",
    },
    modelPath: null,
    runtimePath: null,
    contextSize: 32768,
  };
}
