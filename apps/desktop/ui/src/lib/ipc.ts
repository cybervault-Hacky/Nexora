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
  "download_cancelled",
  "verifying",
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

/** Confidence attached to best-effort native hardware readings. */
export type DetectionConfidence = "confirmed" | "estimated" | "unavailable" | "unknown";

export interface HardwareDetection<T> {
  value: T | null;
  confidence: DetectionConfidence;
  detail: string | null;
}

export interface AdaptiveCapabilities {
  contractVersion: number;
  hardware: {
    operatingSystem: {
      name: HardwareDetection<string>;
      version: HardwareDetection<string>;
      architecture: HardwareDetection<string>;
    };
    processor: {
      architecture: HardwareDetection<string>;
      availableLogicalProcessors: HardwareDetection<number>;
      model: HardwareDetection<string>;
    };
    memory: {
      totalPhysicalBytes: HardwareDetection<number>;
      availablePhysicalBytes: HardwareDetection<number>;
      sharedGpuBytes: HardwareDetection<number>;
    };
    graphicsAdapters: HardwareDetection<{
      vendor: HardwareDetection<string>;
      model: HardwareDetection<string>;
      dedicatedVramBytes: HardwareDetection<number>;
      sharedMemoryBytes: HardwareDetection<number>;
    }[]>;
    modelStorage: {
      freeBytes: HardwareDetection<number>;
      totalBytes: HardwareDetection<number>;
    };
    accelerationBackends: {
      backend: "cpu" | "cuda" | "direct_ml" | "vulkan";
      availability: DetectionConfidence;
      detail: string;
    }[];
    refreshedAtUnixMs: number | null;
  };
  resources: {
    totalMemoryBytes: HardwareDetection<number>;
    availableMemoryBytes: HardwareDetection<number>;
    dedicatedVramBytes: HardwareDetection<number>;
    modelStorageFreeBytes: HardwareDetection<number>;
    accelerationConfirmed: boolean;
  };
  selectedProfile: "light" | "balanced" | "quality";
  profileConfidence: DetectionConfidence;
  profileReasons: string[];
  imageWorkload: PreliminaryWorkloadFeasibility;
  videoWorkload: PreliminaryWorkloadFeasibility;
}

export interface PreliminaryWorkloadFeasibility {
  status: "resource_constrained" | "resource_headroom_observed" | "unknown";
  generationSupportConfirmed: boolean;
  reasons: string[];
}

export type MediaModelCategory =
  | "image_generation"
  | "image_editing"
  | "video_generation"
  | "video_editing_enhancement";
export type MediaModelLifecycleState =
  | "not_installed"
  | "awaiting_compatibility_assessment"
  | "ready_to_download"
  | "downloading"
  | "download_interrupted"
  | "downloaded_not_validated"
  | "installed_validated"
  | "incompatible"
  | "failed"
  | "removing";
export type MediaModelCompatibilityStatus =
  | "awaiting_assessment"
  | "resource_constrained"
  | "resource_headroom_observed"
  | "unknown";
export type MediaModelBackendRequirement = "cpu" | "cuda" | "direct_ml" | "vulkan";
export type MediaModelFileRole =
  | "weights"
  | "text_encoder"
  | "vae"
  | "configuration"
  | "tokenizer"
  | "auxiliary";

export interface MediaModelFileDefinition {
  id: string;
  fileName: string;
  role: MediaModelFileRole;
  required: boolean;
  expectedSizeBytes: number | null;
  expectedSha256: string | null;
  downloadUrl: string | null;
}

export interface MediaModelDefinition {
  id: string;
  displayName: string;
  category: MediaModelCategory;
  family: string | null;
  version: string | null;
  source: { officialReference: string } | null;
  license: {
    identifier: string | null;
    reference: string | null;
    restrictions: string | null;
  };
  files: MediaModelFileDefinition[];
  expectedTotalSizeBytes: number | null;
  requirements: {
    minimumRamBytes: number | null;
    recommendedRamBytes: number | null;
    minimumVramBytes: number | null;
    recommendedVramBytes: number | null;
    requiredBackend: MediaModelBackendRequirement | null;
  };
  inferenceRuntime: {
    name: string;
    officialReference: string;
    supportedBackends: MediaModelBackendRequirement[];
  } | null;
  compatibility: "awaiting_assessment" | "experiment_only";
  downloadable: boolean;
  experimental: boolean;
}

export interface MediaModelInstallationRecord {
  schemaVersion: number;
  modelId: string;
  state: MediaModelLifecycleState;
  files: {
    fileId: string;
    fileName: string;
    bytes: number;
    sha256: string;
    integrityVerified: boolean;
  }[];
  lastError: string | null;
  updatedAtUnixMs: number;
}

export interface MediaModelCompatibilityAssessment {
  status: MediaModelCompatibilityStatus;
  reasons: string[];
  runtimeCompatibilityConfirmed: false;
}

export interface MediaModelDownloadProgress {
  modelId: string;
  fileId: string | null;
  receivedBytes: number;
  totalBytes: number | null;
}

export interface MediaModelView {
  definition: MediaModelDefinition;
  state: MediaModelLifecycleState;
  progress: MediaModelDownloadProgress | null;
  usable: boolean;
  compatibility: MediaModelCompatibilityAssessment;
  installation: MediaModelInstallationRecord | null;
  integrityNote: string;
}

export interface MediaModelCatalog {
  categories: MediaModelCategory[];
  models: MediaModelView[];
  storageDirectoryAvailable: boolean;
  catalogNote: string;
}

export interface MediaModelDownloadResult {
  modelId: string;
  state: MediaModelLifecycleState;
  downloadedFileCount: number;
  integrityComplete: boolean;
  message: string;
}

export type ImageQualityProfile = "light" | "balanced" | "quality";
export type ImageGenerationStage = "preparing" | "running" | "cancelling";

export interface GenerateImageRequestPayload {
  prompt: string;
  profile?: ImageQualityProfile;
  seed?: number | null;
}

export interface ActiveImageGenerationStatus {
  requestId: string;
  stage: ImageGenerationStage;
  detail: string;
}

export interface ImageGenerationStatus {
  modelId: string;
  modelState: MediaModelLifecycleState;
  modelValidated: boolean;
  runtimeAvailable: boolean;
  runtimeCpuBackendConfirmed: boolean;
  runtimeVersionVerified: boolean;
  modelRuntimeCompatibilityConfirmed: boolean;
  usable: boolean;
  selectedBackend: string | null;
  availableRamBytes: number | null;
  modelStorageFreeBytes: number | null;
  outputStorageFreeBytes: number | null;
  canAttemptGeneration: boolean;
  activeGeneration: ActiveImageGenerationStatus | null;
  limitations: string[];
}

/** Status-only Sonnet media capability; this is never a coding-model profile. */
export interface SonnetImageGenerationCapabilityStatus {
  candidateModelId: string;
  candidateModelName: string;
  parameterCount: number;
  officialModelCard: string;
  modelCardSourceRevision: string;
  officialModelRepository: string;
  officialRepositoryReportedMd5: string;
  modelWeightsRevision: string | null;
  modelWeightsSizeBytes: number | null;
  modelWeightsSha256: string | null;
  registeredWithMediaModelManager: boolean;
  requiredComponentCount: number;
  requiredAssetManifestComplete: boolean;
  downloadable: boolean;
  licenseIdentifier: string;
  licenseReference: string;
  publisherUseLimitations: string[];
  runtimeName: string;
  runtimeSourceRevision: string;
  runtimeSourceGuide: string;
  upstreamRuntimeSupportDocumented: boolean;
  upstreamWindowsSupportDocumented: boolean;
  upstreamBackendsDocumented: string[];
  runtimeBundleConfigured: boolean;
  runtimeProbePerformed: boolean;
  runtimeBinaryRevisionVerified: boolean;
  modelRuntimeCompatibilityConfirmed: boolean;
  hardwareCompatibility: "unknown";
  minimumRamBytes: number | null;
  minimumVramBytes: number | null;
  resourceEvidence: string;
  generationCommandAvailable: boolean;
  outputValidationPipelineAvailable: boolean;
  inferenceUsable: boolean;
  limitations: string[];
}

export interface GeneratedImageArtifact {
  schemaVersion: number;
  artifactId: string;
  modelId: string;
  modelVersion: string;
  mimeType: "image/png";
  width: number;
  height: number;
  bytes: number;
  sha256: string;
  seed: number;
  profile: ImageQualityProfile;
  steps: number;
  backend: "cpu";
  createdAtUnixMs: number;
}

/** Candidate metadata is reported; editing usability remains unavailable until a compatible local runtime is integrated. */
export interface ImageEditingCapabilityStatus {
  candidateModelId: string;
  candidateModelName: string;
  candidateModelCard: string;
  modelWeightsRevision: string;
  modelWeightsFile: string;
  modelWeightsBytes: number;
  modelWeightsSha256: string;
  modelRegisteredWithMediaManager: boolean;
  runtimeName: string;
  runtimeReference: string;
  licenseReference: string;
  licenseNote: string;
  runtimeBundleAvailable: boolean;
  runtimeProbePerformed: boolean;
  modelRuntimeCompatibilityConfirmed: boolean;
  hardwareCompatibility: "unknown";
  sourceImageImportAvailable: boolean;
  editCommandAvailable: boolean;
  limitations: string[];
  /** Additive fields are optional for compatibility with older desktop cores. */
  modelWeightsFormat?: string;
  requiredAssetManifestComplete?: boolean;
  requiredAssetManifestNote?: string;
  modelDownloaded?: boolean | null;
  modelIntegrityValidated?: boolean | null;
  modelLoaded?: boolean;
  runtimeSourceRevision?: string;
  runtimePackageVersionPinned?: boolean;
  upstreamInstructionEditingSupported?: boolean;
  runtimeCompatibilityStatus?: "unknown" | "incompatible" | "compatible";
  windowsCompatibility?: "unknown" | "incompatible" | "compatible";
  minimumRamBytes?: number | null;
  minimumVramBytes?: number | null;
  resourceEvidence?: string;
  supportsInstructionGuidedEditing?: boolean;
  supportsMaskInput?: boolean;
  supportsRegionSelection?: boolean;
  outputValidationPipelineAvailable?: boolean;
  usableForInstructionEditing?: boolean;
}

/** Status-only video-editing boundary; no operation is currently implemented. */
export interface VideoEditingCapabilityStatus {
  conventionalProcessingAvailable: boolean;
  supportedOperationCount: number;
  sourceVideoImportAvailable: boolean;
  inputFormatContractEstablished: boolean;
  externalMediaRuntimeConfigured: boolean;
  runtimeVersionPinned: boolean;
  runtimeProbePerformed: boolean;
  runtimeCompatibilityStatus: "unknown" | "incompatible" | "compatible";
  windowsCompatibility: "unknown" | "incompatible" | "compatible";
  supportedInputFormats: string[];
  supportedOutputFormats: string[];
  outputArtifactStoreAvailable: boolean;
  outputValidationAvailable: boolean;
  processingProgressAvailable: boolean;
  cancellationAvailable: boolean;
  aiEnhancementCandidateModelId: string | null;
  aiEnhancementModelRegistered: boolean;
  aiEnhancementRuntimeAvailable: boolean;
  aiEnhancementUsable: boolean;
  hardwareCompatibility: "unknown";
  resourceEvidence: string;
  limitations: string[];
}

/** Read-only Phase 5 boundary; none of these flags imply a usable runtime. */
export interface VideoGenerationCapabilityStatus {
  candidateModelId: string;
  candidateModelName: string;
  task: "text_to_video_only";
  modelRegisteredWithMediaManager: boolean;
  /** Additive lifecycle fields remain optional for older desktop cores. */
  modelLifecycleState?: MediaModelLifecycleState | null;
  managerReportedDownloaded?: boolean | null;
  managerReportedIntegrityValidated?: boolean | null;
  managerRecordedRequiredFileCount?: number | null;
  managerStateNote?: string;
  modelLoaded?: boolean;
  usableForTextToVideo?: boolean;
  runtimeCompatibilityStatus?: "unknown" | "incompatible" | "compatible";
  requiredModelFileCount: number;
  registeredModelSizeBytes: number;
  modelLicense: string;
  modelLicenseReference: string;
  runtimeName: string;
  runtimeSourceRevision: string;
  runtimeReference: string;
  upstreamWindowsSupportDocumented: boolean;
  upstreamBackendsDocumented: string[];
  runtimeBundleAvailable: boolean;
  runtimePackageVersionPinned?: boolean;
  runtimeBinaryRevisionVerified: boolean;
  runtimeProbePerformed: boolean;
  modelRuntimeCompatibilityConfirmed: boolean;
  modelSpecificResourceFitConfirmed: boolean;
  hardwareCompatibility: "unknown";
  generationCommandAvailable: boolean;
  nexoraOutputPipelineAvailable: boolean;
  upstreamVideoContainersDocumented: string[];
  selectedDiffusionWeights: string;
  selectedTextEncoder: string;
  resourceEvidence: string;
  limitations: string[];
}

export interface ImageEditSourceArtifact {
  schemaVersion: number;
  sourceId: string;
  mimeType: "image/png";
  width: number;
  height: number;
  bytes: number;
  sha256: string;
  createdAtUnixMs: number;
}

export interface ImportImageEditSourcePayload {
  /** Bytes from a user-selected PNG; never a renderer-supplied filesystem path. */
  sourceBytes: number[];
}

export interface MediaModelValidationResult {
  modelId: string;
  state: MediaModelLifecycleState;
  verifiedFileCount: number;
  integrityComplete: boolean;
  message: string;
}

export interface MediaModelError {
  code: string;
  message: string;
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

export interface UsageWarning {
  code: string;
  message: string;
}

export interface GenerationOutcome {
  requestId: string;
  finishReason: string | null;
  cancelled: boolean;
  usageWarnings: UsageWarning[];
}

export interface InferenceError {
  code: string;
  message: string;
}

/**
 * Device-local generation accounting, as reported by `src-tauri/src/usage/mod.rs`.
 * Missing quota fields mean there is no approved local limit configured; a
 * missing `used` value means the persisted counter could not be trusted.
 */
export interface UsageStatus {
  policyConfigured: boolean;
  allowance: number | null;
  used: number | null;
  remaining: number | null;
  /** UTC calendar day, `YYYY-MM-DD`. */
  day: string;
  resetAtMs: number | null;
  policyId: string | null;
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
