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
  /** Registered profile id. `null` means the default (Nexora Mini). */
  modelId: string | null;
}

/**
 * The approved model artefact described by the Rust registry.
 *
 * Metadata only: it does not imply the file is present on this machine, and
 * `locallyVerified` stays false until a recorded inference run exists.
 */
export type ModelArtifact =
  | {
      state: "approved";
      repository: string;
      revision: string;
      fileName: string;
      sizeBytes: number;
      sha256: string;
    }
  | { state: "pending"; note: string };

export interface ModelProfile {
  id: string;
  displayName: string;
  publisher: string;
  upstreamModel: string;
  /** Null until the upstream revision has been recorded. */
  upstreamRevision: string | null;
  format: string;
  quantization: string;
  licence: { id: string; reference: string };
  /**
   * `pending` means the artefact has no recorded revision, length, or digest,
   * so Nexora refuses to download or verify it. The UI must show that state
   * rather than treating the profile as installable.
   */
  artifact: ModelArtifact;
  context: { recommended: number; documentedMax: number };
  compatibilityNotes: string[];
  locallyVerified: boolean;
}

/**
 * The backend's readiness state for one profile.
 *
 * These strings are the complete set the Rust core can emit. The UI switches on
 * them and must not invent new ones or derive a state of its own.
 */
export const READINESS_STATES = [
  "unavailable",
  "ready_to_download",
  "downloading",
  "download_incomplete",
  "verification_failed",
  "unverified",
  "downloaded_verified",
  "insufficient_storage",
  "insufficient_memory",
  "detection_unavailable",
  "ready_to_load",
  "loading",
  "loaded",
  "runtime_failure",
] as const;

export type ReadinessState = (typeof READINESS_STATES)[number];

/** Outcome of one readiness check. */
export type CheckStatus = "ok" | "warning" | "blocked" | "unknown";

export interface ReadinessCheck {
  id: string;
  status: CheckStatus;
  message: string;
}

/**
 * Storage and memory arithmetic derived from the published artefact size and
 * the configured context. Estimates, never measurements of this machine.
 */
export interface ResourceEstimate {
  modelBytes: number;
  requiredFreeBytes: number;
  installBytes: number;
  peakDownloadBytes: number;
  weightsGib: number;
  kvCacheGib: number;
  runtimeOverheadGib: number;
  estimatedMemoryGib: number;
  memoryHeadroomGib: number;
  contextSize: number;
}

/**
 * The single authoritative answer for what can be done with a model. The core
 * computes it from real files, real memory, and real disk space; the UI only
 * renders it.
 */
export interface Readiness {
  state: ReadinessState;
  headline: string;
  detail: string;
  canDownload: boolean;
  canVerify: boolean;
  canLoad: boolean;
  canGenerate: boolean;
  estimate: ResourceEstimate;
  checks: ReadinessCheck[];
}

/**
 * One row of the model selector. Every field is authoritative: the UI never
 * infers availability from a file path or from the profile list.
 */
export interface ModelCatalog {
  entries: ModelCatalogEntry[];
  running: boolean;
  loadedProfileId: string | null;
  /** Non-null when sending would be refused because of the model selection. */
  selectionError: string | null;
}

export interface ModelCatalogEntry {
  profile: ModelProfile;
  selected: boolean;
  /** True only when the engine is running this profile. */
  active: boolean;
  installedPath: string | null;
  /** False when the artefact is not approved, so downloading is refused. */
  downloadable: boolean;
  /** Mirrors `readiness.state`; kept flat for simple rendering. */
  status: ReadinessState;
  detail: string;
  readiness: Readiness;
}

export interface ModelInfo {
  path: string;
  fileName: string;
  sizeBytes: number;
  sizeGib: number;
  ggufVersion: number;
  tensorCount: number;
  /** `general.architecture` from the file's metadata table, when readable. */
  architecture: string | null;
  /** `general.name`, the publisher's own label for the file. */
  name: string | null;
  /** The context window the file declares, when it declares one. */
  contextLength: number | null;
  estimatedMinRamGib: number;
  fitsAvailableRam: boolean | null;
  warnings: string[];
}

/** Progress from an explicit, user-started model download. */
export type ModelDownloadEvent =
  | { type: "progress"; received: number; total: number }
  | { type: "verifying"; total: number };

export interface ModelDownloadOutcome {
  /** Null when the user cancelled; no file was published in that case. */
  path: string | null;
  fileName: string;
  sizeBytes: number;
  sha256: string;
  cancelled: boolean;
}

/** The result of hashing a file the user chose and comparing it to the profile. */
export interface IntegrityReport {
  path: string;
  fileName: string;
  profileId: string;
  sizeBytes: number;
  sizeMatches: boolean;
  sha256: string;
  expectedSizeBytes: number;
  expectedSha256: string;
  digestMatches: boolean;
}

export interface RuntimeInfo {
  path: string;
  versionLine: string;
  /** False when the executable does not report the llama.cpp build in docs/phase-2. */
  pinnedBuildMatched: boolean;
}

/** Free space at one filesystem location, all optional by design. */
export interface DiskSpace {
  freeBytes: number | null;
  totalBytes: number | null;
  /** Why the values are null, when they are. */
  unavailableReason: string | null;
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
  /**
   * The profile this message was composed under.
   *
   * The backend reads the selection from its own settings; this field lets it
   * refuse a message that was prepared before the user switched models instead
   * of answering it with a different one.
   */
  profileId?: string | null;
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

/**
 * Daily free usage, as reported by `src-tauri/src/usage/mod.rs`.
 *
 * Every field is authoritative. The UI never derives remaining usage or the
 * reset instant; it only formats them.
 */
export interface UsageStatus {
  allowance: number;
  used: number;
  remaining: number;
  /** UTC calendar day, `YYYY-MM-DD`. */
  day: string;
  resetAtMs: number;
  policyId: string;
  provisional: boolean;
  exhausted: boolean;
  notice: string | null;
  error: string | null;
}

/** Export DTOs. Keep in sync with `src-tauri/src/export/mod.rs`. */

export interface ExportError {
  code: string;
  message: string;
}

export interface ExportCandidate {
  /** Slash-separated path inside the project. */
  path: string;
  size: number;
}

export interface ExportPlan {
  rootName: string;
  files: ExportCandidate[];
  excludedSensitive: number;
  excludedSymlink: number;
  excludedUnsafe: number;
  totalBytes: number;
  truncated: boolean;
}

export interface ExportProgress {
  done: number;
  total: number;
  bytesDone: number;
  bytesTotal: number;
  current: string;
}

export type ExportEvent =
  | { type: "preparing"; total: number }
  | ({ type: "progress" } & ExportProgress)
  | { type: "finalizing"; total: number };

export interface ExportOutcome {
  /** Absolute destination, or an empty string when nothing was written. */
  path: string;
  files: number;
  bytes: number;
  cancelled: boolean;
  replaced: boolean;
}

export interface SingleExportRequest {
  relativePath: string;
  destination: string;
  overwrite: boolean;
}

export interface ZipExportRequest {
  selection: string[] | null;
  destination: string;
  overwrite: boolean;
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
