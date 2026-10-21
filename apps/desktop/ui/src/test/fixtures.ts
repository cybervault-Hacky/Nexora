import type {
  AppStatus,
  EngineState,
  Readiness,
  ReadinessState,
  UsageStatus,
} from "../lib/ipc";

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

export interface UsageOverrides {
  allowance?: number;
  used?: number;
  remaining?: number;
  day?: string;
  resetAtMs?: number;
  provisional?: boolean;
  exhausted?: boolean;
  notice?: string | null;
  error?: string | null;
}

/** Builds the exact `UsageStatus` shape the Rust core serialises. */
export function makeUsage(overrides: UsageOverrides = {}): UsageStatus {
  const allowance = overrides.allowance ?? 50;
  const used = overrides.used ?? 0;

  return {
    allowance,
    used,
    remaining: overrides.remaining ?? Math.max(0, allowance - used),
    day: overrides.day ?? "2026-10-20",
    resetAtMs: overrides.resetAtMs ?? Date.UTC(2026, 9, 21),
    policyId: "nexora-mini-free-generations-v1",
    provisional: overrides.provisional ?? true,
    exhausted: overrides.exhausted ?? allowance - used <= 0,
    notice: overrides.notice ?? null,
    error: overrides.error ?? null,
  };
}

export interface ReadinessOverrides {
  state?: ReadinessState;
  headline?: string;
  detail?: string;
  canDownload?: boolean;
  canVerify?: boolean;
  canLoad?: boolean;
  canGenerate?: boolean;
  estimate?: Partial<Readiness["estimate"]>;
  checks?: Readiness["checks"];
}

/**
 * Builds the exact `Readiness` shape the Rust core serialises.
 *
 * Defaults describe a clean machine looking at the Sonnet artefact, which is
 * the case the UI most often has to explain: nothing installed, room to fetch.
 */
export function makeReadiness(overrides: ReadinessOverrides = {}): Readiness {
  const state = overrides.state ?? "ready_to_download";

  const downloadable = state === "ready_to_download" || state === "download_incomplete";

  return {
    state,
    headline: overrides.headline ?? "Ready to download",
    detail:
      overrides.detail ??
      "Nothing is installed yet. The approved file can be downloaded and will be verified before it is installed.",
    canDownload: overrides.canDownload ?? downloadable,
    canVerify: overrides.canVerify ?? state === "unverified",
    canLoad: overrides.canLoad ?? (state === "ready_to_load" || state === "loaded"),
    canGenerate: overrides.canGenerate ?? state === "loaded",
    estimate: {
      modelBytes: 8_988_110_272,
      requiredFreeBytes: 9_525_018_624,
      installBytes: 8_988_110_272,
      peakDownloadBytes: 9_525_018_624,
      weightsGib: 8.3709,
      kvCacheGib: 1.5,
      runtimeOverheadGib: 1,
      estimatedMemoryGib: 10.8709,
      memoryHeadroomGib: 2,
      contextSize: 8_192,
      ...overrides.estimate,
    },
    checks: overrides.checks ?? [
      {
        id: "storage",
        status: "ok",
        message: "100.00 GiB free at the destination; 8.87 GiB needed for this download.",
      },
      {
        id: "memory",
        status: "ok",
        message: "24.0 GiB available of 32.0 GiB; about 10.9 GiB needed to load. An estimate.",
      },
    ],
  };
}
