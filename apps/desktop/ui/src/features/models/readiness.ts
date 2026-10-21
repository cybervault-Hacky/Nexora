import type { ReadinessState, ResourceEstimate } from "../../lib/ipc";

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

/** Short label for a backend state. Never a guess: unknown input is null. */
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
