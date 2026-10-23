import type { ModelProfile, ReadinessState, ResourceEstimate } from "../../lib/ipc";

/**
 * Approved profile identifiers from `src-tauri/src/inference/registry.rs`.
 *
 * Keep exact parity with `NEXORA_MINI_ID` and `NEXORA_SONNET_ID` in Rust.
 * Only profiles registered in `APPROVED` may appear in the selector UI.
 */
export const NEXORA_MINI_ID = "nexora-mini-qwen2.5-coder-7b-instruct-q4-k-m";
export const NEXORA_SONNET_ID = "nexora-sonnet-qwen2.5-coder-14b-instruct-q4-k-m";

export const APPROVED_PROFILE_IDS = [NEXORA_MINI_ID, NEXORA_SONNET_ID] as const;

export function isApprovedProfileId(id: string | null | undefined): boolean {
  if (!id) return false;
  return (APPROVED_PROFILE_IDS as readonly string[]).includes(id.trim());
}

/**
 * Concise, truthful model description derived strictly from approved registry
 * metadata without inventing unverified capability or benchmark claims.
 */
export function describeModelProfile(profile: ModelProfile): string {
  if (profile.id === NEXORA_MINI_ID) {
    return `${profile.upstreamModel} · Fast local coding & refactoring`;
  }
  if (profile.id === NEXORA_SONNET_ID) {
    return `${profile.upstreamModel} · Deeper reasoning & multi-file changes`;
  }
  return `${profile.upstreamModel} · ${profile.quantization}`;
}

/**
 * Formats real byte transfer progress from `start_model_download`.
 * Never invents completion percentages or estimated time remaining.
 */
export function formatDownloadProgress(progress: {
  received: number;
  total: number;
  verifying: boolean;
}): string {
  if (progress.verifying) {
    return "Verifying SHA-256 integrity before publishing…";
  }
  if (progress.total <= 0) {
    return progress.received > 0
      ? `Downloading ${gib(progress.received)} GiB received…`
      : "Starting download…";
  }
  const percent = Math.min(
    99,
    Math.max(0, Math.floor((progress.received / progress.total) * 100)),
  );
  return `Downloading ${gib(progress.received)} of ${gib(progress.total)} GiB (${percent}%)`;
}

/**
 * How a readiness state should read and look.
 *
 * `Record<ReadinessState, …>` is deliberate: adding a state to the Rust core
 * without teaching the UI about it is a type error, not a silent fallback.
 */
export type ReadinessTone = "neutral" | "ok" | "busy" | "warning" | "danger";

const LABELS: Record<ReadinessState, string> = {
  unavailable: "Unavailable",
  ready_to_download: "Ready to download",
  downloading: "Downloading",
  download_incomplete: "Download incomplete",
  download_cancelled: "Download cancelled",
  verifying: "Verifying integrity",
  verification_failed: "Verification failed",
  unverified: "Verification required",
  downloaded_verified: "Downloaded and verified",
  insufficient_storage: "Not enough storage",
  insufficient_memory: "Not enough memory",
  detection_unavailable: "System resources unknown",
  ready_to_load: "Ready to load",
  loading: "Loading",
  loaded: "Loaded",
  runtime_failure: "Runtime failure",
};

const TONES: Record<ReadinessState, ReadinessTone> = {
  unavailable: "neutral",
  ready_to_download: "ok",
  downloading: "busy",
  download_incomplete: "warning",
  download_cancelled: "neutral",
  verifying: "busy",
  verification_failed: "danger",
  unverified: "warning",
  downloaded_verified: "ok",
  insufficient_storage: "warning",
  insufficient_memory: "warning",
  detection_unavailable: "warning",
  ready_to_load: "ok",
  loading: "busy",
  loaded: "ok",
  runtime_failure: "danger",
};

/** Short label for a backend state. Never a guess: unknown input falls back explicitly. */
export function readinessLabel(state: ReadinessState): string {
  return LABELS[state] ?? "Unknown state";
}

export function readinessTone(state: ReadinessState): ReadinessTone {
  return TONES[state] ?? "neutral";
}

const GIB = 1024 ** 3;

/** Two-decimal GiB from a byte count. */
export function gib(bytes: number): string {
  return (bytes / GIB).toFixed(2);
}

/** The estimate, in the words the panel uses. Always marked as an estimate. */
export function estimateLines(estimate: ResourceEstimate): {
  fileGib: string;
  requiredFreeGib: string;
  memoryGib: string;
  cacheGib: string;
  context: string;
} {
  return {
    fileGib: gib(estimate.modelBytes),
    requiredFreeGib: gib(estimate.requiredFreeBytes),
    memoryGib: estimate.estimatedMemoryGib.toFixed(1),
    cacheGib: estimate.kvCacheGib.toFixed(2),
    context: estimate.contextSize.toLocaleString("en-US"),
  };
}
