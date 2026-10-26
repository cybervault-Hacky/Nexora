import { useCallback, useEffect, useRef, useState, type RefObject } from "react";
import { Composer } from "../components/Composer";
import { EmptyState } from "../components/EmptyState";
import { Icon } from "../components/Icon";
import { MessageList } from "../components/MessageList";
import { ModelSelector } from "../components/ModelSelector";
import { RouterLink } from "../components/RouterLink";
import type { usePersistentConversation } from "../features/chat/usePersistentConversation";
import type { ModelCatalog } from "../features/models/useModelCatalog";
import type { UsageView } from "../features/usage/useUsage";
import { isApprovedProfileId } from "../features/models/readiness";
import { planContextBudget } from "../features/workspace/contextBudget";
import type { RetrievalResult } from "../features/workspace/retrieval";
import type { useWorkspace } from "../features/workspace/useWorkspace";
import type { ServerUsageSummary } from "../lib/auth";
import type { buildEvidenceReport } from "../lib/evidence";
import type {
  AppStatus,
  ImageEditingCapabilityStatus,
  ImageGenerationStatus,
  MediaModelLifecycleState,
  SonnetImageGenerationCapabilityStatus,
  VideoEditingCapabilityStatus,
  VideoGenerationCapabilityStatus,
} from "../lib/ipc";
import { deleteImageEditSource, fetchImageEditingStatus, importImageEditSource } from "../lib/imageEditing";
import {
  imageReferenceFromArtifact,
  preflightReferenceFile,
  prepareImageTo3dTask,
  type ImageReferenceMetadata,
} from "../features/chat/imageTo3dTask";
import { fetchImageGenerationStatus } from "../lib/imageGeneration";
import { fetchSonnetImageGenerationStatus } from "../lib/sonnetImageGeneration";
import { fetchVideoEditingStatus } from "../lib/videoEditing";
import { fetchVideoGenerationStatus } from "../lib/videoGeneration";
import { resolveMediaTask, type ComposerMode, type MediaTask } from "../features/chat/mediaMode";
import { isNearBottom } from "../lib/scroll";
import { navigateTo } from "../lib/router";

type ConversationState = ReturnType<typeof usePersistentConversation>;
type WorkspaceState = ReturnType<typeof useWorkspace>;
type ContextPlan = ReturnType<typeof planContextBudget>;
type EvidenceReport = ReturnType<typeof buildEvidenceReport>;

export interface ChatPageProps {
  conversation: ConversationState;
  workspace: WorkspaceState;
  engineStatus: AppStatus | null;
  statusError: string | null;
  engineReady: boolean;
  desktop: boolean;
  /** Prompt-grounding stays in the shell; diagnostic panels are hidden from chat. */
  contextPlan: ContextPlan | null;
  retrievalResult: RetrievalResult | null;
  evidenceReport: EvidenceReport | null;
  /** Submitted from the shell (Ctrl+L) after navigating here. */
  textareaRef: RefObject<HTMLTextAreaElement | null>;
  focusNonce: number;
  onSubmit: () => void;
  /** Authoritative daily allowance; `null` status means the browser preview. */
  usage: UsageView;
  /** Optional server-synchronized per-model plan usage when signed in. */
  serverUsage?: ServerUsageSummary | null;
  /** Registered models and their real availability, from the desktop core. */
  models: ModelCatalog;
  /** Refreshes top-level engine status after loading/downloading from selector. */
  onModelLoaded?: () => void;
}

function modelLifecycleLabel(state: MediaModelLifecycleState | null): string {
  switch (state) {
    case "not_installed": return "not installed";
    case "awaiting_compatibility_assessment": return "awaiting compatibility assessment";
    case "ready_to_download": return "registered; assets not installed";
    case "downloading": return "downloading";
    case "download_interrupted": return "download interrupted";
    case "downloaded_not_validated": return "downloaded; integrity not validated";
    case "installed_validated": return "installed and checksum-validated";
    case "incompatible": return "marked incompatible";
    case "failed": return "failed";
    case "removing": return "being removed";
    default: return "unknown";
  }
}

function miniImageStatusExplanation(status: ImageGenerationStatus): string {
  const runtime = !status.runtimeAvailable
    ? "no local runtime is detected"
    : !status.runtimeVersionVerified
      ? "a local runtime is detected, but its version is unverified"
      : "a local runtime is detected";
  const model = `model state ${modelLifecycleLabel(status.modelState)}, checksum validation ${status.modelValidated ? "complete" : "unconfirmed"}`;
  return `Mini / SD-Turbo: ${model}; ${runtime}; model/runtime compatibility ${status.modelRuntimeCompatibilityConfirmed ? "confirmed" : "unconfirmed"}; inference usable ${status.usable ? "yes" : "no"}.`;
}

function sonnetImageStatusExplanation(status: SonnetImageGenerationCapabilityStatus): string {
  const runtime = status.upstreamRuntimeSupportDocumented
    ? `upstream ${status.runtimeName} support is documented at source revision ${status.runtimeSourceRevision}`
    : "no compatible upstream runtime source is established";
  return `Sonnet / ${status.candidateModelName} (${(status.parameterCount / 1_000_000_000).toFixed(0)}B, ${status.licenseIdentifier}): model manager registration ${status.registeredWithMediaModelManager ? "yes" : "no"}; weight revision pinned ${status.modelWeightsRevision ? "yes" : "no"}; complete trusted asset manifest ${status.requiredAssetManifestComplete ? "yes" : "no"}; ${runtime}; Nexora runtime configured ${status.runtimeBundleConfigured ? "yes" : "no"}; model/runtime compatibility ${status.modelRuntimeCompatibilityConfirmed ? "confirmed" : "unconfirmed"}; resource fit ${status.hardwareCompatibility}; generation command ${status.generationCommandAvailable ? "available" : "unavailable"}; output validation ${status.outputValidationPipelineAvailable ? "available" : "unavailable"}; inference usable ${status.inferenceUsable ? "yes" : "no"}.`;
}

function editingStatusExplanation(status: ImageEditingCapabilityStatus): string {
  const blockers = [
    status.modelRegisteredWithMediaManager ? null : "the candidate editing model is not registered for managed use",
    status.runtimeBundleAvailable ? null : "no compatible editing runtime is available",
    status.editCommandAvailable ? null : "no image-editing inference command is available",
  ].filter((item): item is string => item !== null);
  const lifecycle = [
    `download ${status.modelDownloaded === null || status.modelDownloaded === undefined ? "not tracked by the manager" : status.modelDownloaded ? "complete" : "not downloaded"}`,
    `integrity validation ${status.modelIntegrityValidated === null || status.modelIntegrityValidated === undefined ? "not tracked by the manager" : status.modelIntegrityValidated ? "complete" : "not validated"}`,
    `loaded ${status.modelLoaded === undefined ? "not reported" : status.modelLoaded ? "yes" : "no"}`,
    `editing usable ${status.usableForInstructionEditing === undefined ? "not reported" : status.usableForInstructionEditing ? "yes" : "no"}`,
  ].join(", ");
  const source = status.sourceImageImportAvailable
    ? "Secure PNG-byte import and the local image-reference preparation flow are available, but neither provides image-editing inference."
    : "No supported source-image import is available.";
  const inputSupport = status.supportsInstructionGuidedEditing
    ? `The candidate supports instruction-guided editing upstream; mask input ${status.supportsMaskInput ? "is documented" : "is not supported by the reviewed interface"}.`
    : "Instruction-guided editing support is not established.";
  return `Image editing (separate from image generation) is unavailable: ${blockers.join("; ") || "the chat does not expose a supported editing operation"}. Candidate state: ${lifecycle}; model/runtime compatibility ${status.modelRuntimeCompatibilityConfirmed ? "confirmed" : "unconfirmed"}; hardware ${status.hardwareCompatibility}. ${inputSupport} ${source} This request did not edit an image and was not sent to a coding model.`;
}

function videoStatusExplanation(status: VideoGenerationCapabilityStatus): string {
  const registration = status.modelRegisteredWithMediaManager
    ? "The Wan2.1 T2V candidate is registered separately from coding and image models."
    : "No video model is registered for managed use.";
  const downloaded = status.managerReportedDownloaded === true
    ? "complete in the manager record"
    : status.managerReportedDownloaded === false
      ? "not complete in the manager record"
      : "unknown / not tracked";
  const integrity = status.managerReportedIntegrityValidated === true
    ? "manager records validated (not re-hashed by this status query)"
    : status.managerReportedIntegrityValidated === false
      ? "not manager-recorded as validated"
      : "unknown / not tracked";
  const lifecycle = `Managed state ${status.modelLifecycleState ?? "unknown"}; download ${downloaded}; integrity ${integrity}; loaded ${status.modelLoaded === undefined ? "not reported" : status.modelLoaded ? "yes" : "no"}; runtime package pinned ${status.runtimePackageVersionPinned === undefined ? "not reported" : status.runtimePackageVersionPinned ? "yes" : "no"}; runtime compatibility ${status.runtimeCompatibilityStatus ?? "unknown"}; hardware ${status.hardwareCompatibility}; model-specific resource fit ${status.modelSpecificResourceFitConfirmed ? "confirmed" : "not confirmed"}; usable for text-to-video ${status.usableForTextToVideo === undefined ? "not reported" : status.usableForTextToVideo ? "yes" : "no"}.`;
  const blockers = [
    status.runtimeBundleAvailable ? null : "no compatible local runtime bundle is available",
    status.modelRuntimeCompatibilityConfirmed ? null : "model/runtime compatibility is not confirmed",
    status.generationCommandAvailable ? null : "no Nexora video-generation command is available",
    status.nexoraOutputPipelineAvailable ? null : "no validated video output pipeline is available",
  ].filter((item): item is string => item !== null);
  return `Local video generation is unavailable: ${registration} ${lifecycle} ${blockers.join("; ") || "a supported chat generation path is not available"}. Installed assets or a manager validation record alone do not establish inference compatibility. ${status.managerStateNote ?? ""} This request did not generate a video and was not sent to a coding model.`;
}

function videoEditingStatusExplanation(status: VideoEditingCapabilityStatus): string {
  const conventional = status.conventionalProcessingAvailable
    ? `${status.supportedOperationCount} conventional operation(s) are reported`
    : `conventional processing is unavailable (${status.supportedOperationCount} supported operations)`;
  const source = status.sourceVideoImportAvailable
    ? "a secure source-video import is reported"
    : "no secure source-video import or attachment flow exists";
  const output = status.outputArtifactStoreAvailable && status.outputValidationAvailable
    ? "managed output storage and validation are reported"
    : "managed video output storage/validation is unavailable";
  const enhancement = status.aiEnhancementUsable
    ? `AI enhancement is reported usable (${status.aiEnhancementCandidateModelId ?? "candidate unspecified"})`
    : "AI video enhancement is unavailable (no registered model/runtime is usable)";
  return `Local video editing and enhancement status: ${conventional}; ${source}; input format contract ${status.inputFormatContractEstablished ? "established" : "not established"}; media runtime ${status.externalMediaRuntimeConfigured ? "configured" : "not configured"} (compatibility ${status.runtimeCompatibilityStatus}; Windows ${status.windowsCompatibility}; probe performed ${status.runtimeProbePerformed ? "yes" : "no"}); ${output}; ${enhancement}; hardware/resource requirements ${status.hardwareCompatibility}. This read-only status did not process a video and was not sent to a coding model.`;
}

const UNKNOWN_MEDIA_STATUS =
  "Local media capability status could not be checked, so availability is unknown. No media inference was attempted, and this request was not sent to a coding model.";

async function describeMediaTask(task: MediaTask): Promise<string> {
  try {
    if (task === "image_generation") {
      const [mini, sonnet] = await Promise.all([
        fetchImageGenerationStatus()
          .then(miniImageStatusExplanation)
          .catch(() => "Mini / SD-Turbo capability status is unknown; no inference was started."),
        fetchSonnetImageGenerationStatus()
          .then(sonnetImageStatusExplanation)
          .catch(() => "Sonnet image capability status is unknown; no inference was started."),
      ]);
      return `Image generation capability status (separate from editing and the selected coding model): ${mini} ${sonnet} This request did not generate an image and was not sent to a coding model.`;
    }
    if (task === "image_editing") {
      return editingStatusExplanation(await fetchImageEditingStatus());
    }
    if (task === "video_editing") {
      return videoEditingStatusExplanation(await fetchVideoEditingStatus());
    }
    return videoStatusExplanation(await fetchVideoGenerationStatus());
  } catch {
    return UNKNOWN_MEDIA_STATUS;
  }
}

/** One place that turns engine state into a label and a tone. */
function describeEngineChip(
  status: AppStatus | null,
  statusError: string | null,
  ready: boolean,
): { label: string; tone: string } {
  if (statusError) {
    return { label: "Engine status unknown", tone: "chip--unknown" };
  }

  switch (status?.engine.state) {
    case "generating":
      return { label: "Generating", tone: "chip--live" };
    case "loading":
      return { label: "Loading model", tone: "chip--live" };
    case "ready":
      return ready
        ? { label: "Engine ready", tone: "chip--ok" }
        : { label: "Engine starting", tone: "chip--live" };
    case "error":
      return { label: "Engine failed", tone: "chip--unknown" };
    case "stopped":
      return { label: "Engine stopped", tone: "chip--warning" };
    default:
      return { label: "No model loaded", tone: "chip--warning" };
  }
}

/**
 * Main ChatGPT-inspired AI Chat view: top conversation bar, centered message
 * canvas, local grounding controls, shared ModelSelector, and bottom Composer.
 */
export function ChatPage({
  conversation,
  workspace,
  engineStatus,
  statusError,
  engineReady,
  desktop,
  textareaRef,
  focusNonce,
  onSubmit,
  usage,
  serverUsage = null,
  models,
  onModelLoaded,
}: ChatPageProps) {
  const {
    conversationLoading,
    messages,
    draft,
    error,
    runtimeError,
    streaming,
    setDraft,
    dismissError,
    dismissRuntimeError,
    dismissSaveError,
    dismissLoadError,
    saveStatus,
    saveError,
    loadError,
    activeId,
    activeTitle,
    renameActive,
    deleteActive,
    stop,
    newChat,
  } = conversation;

  const [renameEditing, setRenameEditing] = useState(false);
  const [renameValue, setRenameValue] = useState("");
  const [composerMode, setComposerMode] = useState<ComposerMode>("auto");
  const [checkingMedia, setCheckingMedia] = useState(false);
  const [imageReference, setImageReference] = useState<{ metadata: ImageReferenceMetadata; previewUrl: string } | null>(null);
  const [imageReferenceBusy, setImageReferenceBusy] = useState(false);
  const [imageReferenceError, setImageReferenceError] = useState<string | null>(null);
  const imageReferenceRef = useRef<{ metadata: ImageReferenceMetadata; previewUrl: string } | null>(null);
  const imageReferenceSequenceRef = useRef(0);
  const mediaSequenceRef = useRef(0);
  const mediaRequestPendingRef = useRef(false);
  const mediaModeRef = useRef<ComposerMode>("auto");
  const activeIdRef = useRef(activeId);
  const loadingRef = useRef(conversationLoading);
  const mountedRef = useRef(true);
  const scrollRef = useRef<HTMLDivElement>(null);
  const stickToBottomRef = useRef(true);
  mediaModeRef.current = composerMode;
  activeIdRef.current = activeId;
  loadingRef.current = conversationLoading;

  const hasMessages = messages.length > 0;
  const engineChip = describeEngineChip(engineStatus, statusError, engineReady);
  const allowanceExhausted = usage.status?.exhausted ?? false;
  const missingApprovedSelection =
    !models.selected || !isApprovedProfileId(models.selected.profile.id);
  const showIntro = !hasMessages && !draft.trim() && !conversationLoading && !streaming;
  const conversationTitle = activeId
    ? activeTitle
    : hasMessages
      ? "Local conversation"
      : "New conversation";

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
      ++imageReferenceSequenceRef.current;
      ++mediaSequenceRef.current;
      mediaRequestPendingRef.current = false;
      const pendingReference = imageReferenceRef.current;
      imageReferenceRef.current = null;
      if (pendingReference) {
        URL.revokeObjectURL(pendingReference.previewUrl);
        void deleteImageEditSource(pendingReference.metadata.referenceId).catch(() => undefined);
      }
    };
  }, []);

  useEffect(() => {
    ++imageReferenceSequenceRef.current;
    ++mediaSequenceRef.current;
    mediaRequestPendingRef.current = false;
    setCheckingMedia(false);
    setImageReferenceBusy(false);
    setComposerMode("auto");
    const pendingReference = imageReferenceRef.current;
    imageReferenceRef.current = null;
    setImageReference(null);
    setImageReferenceError(null);
    if (pendingReference) {
      URL.revokeObjectURL(pendingReference.previewUrl);
      void deleteImageEditSource(pendingReference.metadata.referenceId).catch(() => undefined);
    }
  }, [activeId]);

  useEffect(() => {
    if (!conversationLoading) return;
    ++mediaSequenceRef.current;
    mediaRequestPendingRef.current = false;
    setCheckingMedia(false);
  }, [conversationLoading]);

  // Focus the composer on mount and whenever the shell asks (Ctrl+L).
  useEffect(() => {
    textareaRef.current?.focus();
  }, [textareaRef, focusNonce]);

  useEffect(() => {
    if (messages.length === 0 || !stickToBottomRef.current) {
      return;
    }

    const container = scrollRef.current;

    if (container) {
      container.scrollTop = container.scrollHeight;
    }
  }, [messages]);

  const handleScroll = useCallback(() => {
    const container = scrollRef.current;

    if (container) {
      stickToBottomRef.current = isNearBottom({
        scrollTop: container.scrollTop,
        scrollHeight: container.scrollHeight,
        clientHeight: container.clientHeight,
      });
    }
  }, []);

  const handleRenameSubmit = useCallback(async () => {
    if (!renameValue.trim() || !activeId) return;
    try {
      await renameActive(renameValue.trim());
      setRenameEditing(false);
    } catch {
      // Error surfaced via saveError.
    }
  }, [renameValue, activeId, renameActive]);

  const handleDeleteWithConfirm = useCallback(async () => {
    if (!activeId) return;
    const confirmed = window.confirm(`Delete conversation "${activeTitle}"? This cannot be undone.`);
    if (!confirmed) return;
    try {
      await deleteActive();
    } catch {
      // Error surfaced via saveError.
    }
  }, [activeId, activeTitle, deleteActive]);

  const handlePickImageReference = useCallback(async (file: File) => {
    const preflightError = preflightReferenceFile(file);
    if (preflightError) {
      setImageReferenceError(preflightError);
      return;
    }
    if (!desktop) {
      setImageReferenceError("Image reference validation requires the Nexora desktop core; no image was retained.");
      return;
    }
    const sequence = ++imageReferenceSequenceRef.current;
    setImageReferenceBusy(true);
    setImageReferenceError(null);
    try {
      // The size preflight bounds this single read to 8 MiB. Rust validates the
      // PNG signature, chunk CRCs, metadata structure, dimensions, and decode.
      const bytes = new Uint8Array(await file.arrayBuffer());
      const artifact = await importImageEditSource(bytes);
      if (!mountedRef.current || sequence !== imageReferenceSequenceRef.current) {
        await deleteImageEditSource(artifact.sourceId).catch(() => undefined);
        return;
      }
      const next = { metadata: imageReferenceFromArtifact(artifact), previewUrl: URL.createObjectURL(file) };
      const previous = imageReferenceRef.current;
      if (previous) {
        try {
          await deleteImageEditSource(previous.metadata.referenceId);
        } catch {
          URL.revokeObjectURL(next.previewUrl);
          await deleteImageEditSource(artifact.sourceId).catch(() => undefined);
          throw new Error("The previous managed reference could not be removed, so it remains attached.");
        }
        URL.revokeObjectURL(previous.previewUrl);
      }
      if (!mountedRef.current || sequence !== imageReferenceSequenceRef.current) {
        URL.revokeObjectURL(next.previewUrl);
        await deleteImageEditSource(artifact.sourceId).catch(() => undefined);
        return;
      }
      imageReferenceRef.current = next;
      setImageReference(next);
    } catch (cause) {
      const message = typeof cause === "object" && cause !== null && "message" in cause && typeof cause.message === "string"
        ? cause.message
        : "The image could not be validated by the local image importer.";
      if (mountedRef.current) setImageReferenceError(message);
    } finally {
      if (mountedRef.current) setImageReferenceBusy(false);
    }
  }, [desktop]);

  const handleRemoveImageReference = useCallback(async () => {
    const current = imageReferenceRef.current;
    if (!current || imageReferenceBusy) return;
    setImageReferenceBusy(true);
    setImageReferenceError(null);
    try {
      await deleteImageEditSource(current.metadata.referenceId);
      if (!mountedRef.current) return;
      URL.revokeObjectURL(current.previewUrl);
      imageReferenceRef.current = null;
      setImageReference(null);
    } catch {
      if (mountedRef.current) setImageReferenceError("The managed image reference could not be removed. It remains attached.");
    } finally {
      if (mountedRef.current) setImageReferenceBusy(false);
    }
  }, [imageReferenceBusy]);

  const mediaTask = resolveMediaTask(composerMode, draft);
  const codingModelReady =
    engineReady &&
    !conversationLoading &&
    !allowanceExhausted &&
    !models.selectionError &&
    !missingApprovedSelection;
  const mediaSubmitAllowed = Boolean(mediaTask) && !conversationLoading && !streaming;
  const imageTaskSubmitAllowed = Boolean(imageReference && draft.trim()) && !conversationLoading && !streaming && !imageReferenceBusy;
  const blockedReason = allowanceExhausted
    ? "Daily free allowance reached — it resets at 00:00 UTC"
    : models.selectionError
      ? models.selectionError
      : missingApprovedSelection
        ? models.loading
          ? "Waiting for an approved local model profile"
          : "Select an approved local model before sending"
        : null;

  const submitComposer = useCallback(() => {
    if (imageReference) {
      if (imageReferenceBusy || loadingRef.current || streaming) return;
      const preparedTask = prepareImageTo3dTask(draft, imageReference.metadata);
      const preparedReference = preparedTask.references[0];
      if (!preparedReference) return;
      const confirmation = [
        "Image attached: yes. Reference validated: yes (PNG content and decoded image were checked locally).",
        "Analysis available: no. No vision-capable model has been verified; the image was not sent to a model or provider.",
        "Blender execution available: no. The Phase 6 executor remains disabled (connector_unavailable).",
        "A versioned image-to-3D task description and opaque local reference have been saved with this conversation for a future supported workflow. This is preparation only; no geometry, Blender action, or script was generated.",
      ].join("\n\n");
      if (conversation.recordLocalExchange(draft, confirmation, { reference: preparedReference, task: preparedTask })) {
        stickToBottomRef.current = true;
        imageReferenceSequenceRef.current += 1;
        imageReferenceRef.current = null;
        URL.revokeObjectURL(imageReference.previewUrl);
        setImageReference(null);
        setImageReferenceError(null);
      }
      return;
    }

    const task = resolveMediaTask(composerMode, draft);
    if (task) {
      if (mediaRequestPendingRef.current || loadingRef.current || streaming) return;
      const prompt = draft;
      const requestMode = composerMode;
      const requestConversationId = activeId;
      const sequence = ++mediaSequenceRef.current;
      mediaRequestPendingRef.current = true;
      setCheckingMedia(true);
      void (async () => {
        try {
          const explanation = await describeMediaTask(task);
          if (
            !mountedRef.current ||
            sequence !== mediaSequenceRef.current ||
            mediaModeRef.current !== requestMode ||
            activeIdRef.current !== requestConversationId ||
            loadingRef.current
          ) {
            return;
          }
          if (conversation.recordLocalExchange(prompt, explanation)) {
            stickToBottomRef.current = true;
          }
        } finally {
          if (sequence === mediaSequenceRef.current) {
            mediaRequestPendingRef.current = false;
            if (mountedRef.current) setCheckingMedia(false);
          }
        }
      })();
      return;
    }

    if (
      !models.selected ||
      !isApprovedProfileId(models.selected.profile.id) ||
      models.selectionError
    ) {
      return;
    }
    stickToBottomRef.current = true;
    onSubmit();
  }, [
    activeId,
    composerMode,
    conversation,
    draft,
    imageReference,
    imageReferenceBusy,
    models.selected,
    models.selectionError,
    onSubmit,
    streaming,
  ]);

  return (
    <>
      <header className="conversation-header">
        <div className="conversation-header__left">
          {!showIntro ? <div className="conversation-header__titles">
            {renameEditing && activeId ? (
              <div className="conversation-header__rename">
                <input
                  className="conversation-header__rename-input"
                  value={renameValue}
                  maxLength={200}
                  aria-label="Conversation title"
                  onChange={(e) => setRenameValue(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") {
                      e.preventDefault();
                      void handleRenameSubmit();
                    }
                    if (e.key === "Escape") {
                      e.preventDefault();
                      setRenameEditing(false);
                      setRenameValue(activeTitle);
                    }
                  }}
                  autoFocus
                />
                <button
                  type="button"
                  className="button button--ghost button--sm"
                  onClick={() => void handleRenameSubmit()}
                >
                  Save
                </button>
                <button
                  type="button"
                  className="button button--ghost button--sm"
                  onClick={() => {
                    setRenameEditing(false);
                    setRenameValue(activeTitle);
                  }}
                >
                  Cancel
                </button>
              </div>
            ) : (
              <h1 className="conversation-header__title">{conversationTitle}</h1>
            )}
          </div> : null}
        </div>

        <div className="conversation-header__chips">
          {saveStatus === "saving" ? (
            <span className="chip chip--live" title="Saving to local storage">
              <span className="pulse" aria-hidden="true" />
              Saving…
            </span>
          ) : saveStatus === "saved" ? (
            <span className="chip chip--ok" title="All changes saved locally">
              Saved
            </span>
          ) : saveStatus === "error" ? (
            <span className="chip chip--unknown" title={saveError ?? "Save failed"}>
              Save failed
            </span>
          ) : null}

          <span className="chip" title="Inference runs on this machine">
            <Icon name="cloud-off" size={13} />
            Local-only
          </span>

          <span
            className={`chip ${engineChip.tone}`}
            title={statusError ?? engineStatus?.engine.detail}
          >
            <span className="status-dot status-dot--sm" aria-hidden="true" />
            {engineChip.label}
          </span>

          {serverUsage?.quotaConfigured && serverUsage.planName ? (
            <span className="chip" aria-label="Daily plan allowance">
              {serverUsage.planName}
            </span>
          ) : null}

          {activeId ? (
            <>
              <button
                type="button"
                className="chip chip--action"
                aria-label="Rename conversation"
                title="Rename conversation"
                onClick={() => {
                  setRenameValue(activeTitle);
                  setRenameEditing(true);
                }}
              >
                <Icon name="edit" size={12} /> Rename
              </button>
              <button
                type="button"
                className="chip chip--action chip--danger"
                aria-label="Delete conversation"
                title="Delete conversation"
                onClick={() => void handleDeleteWithConfirm()}
              >
                <Icon name="trash" size={12} /> Delete
              </button>
            </>
          ) : null}

          {hasMessages || activeId ? (
            <button
              type="button"
              className="chip chip--action"
              aria-label="Start new chat"
              title="Start new chat (Ctrl+N)"
              onClick={newChat}
            >
              <Icon name="message" size={12} /> New chat
            </button>
          ) : null}
        </div>
      </header>

      {runtimeError ? (
        <div className="notice notice--danger" role="alert">
          <Icon name="alert" size={14} />
          <span>{runtimeError}</span>
          <button
            type="button"
            className="notice__action"
            onClick={dismissRuntimeError}
          >
            Dismiss
          </button>
        </div>
      ) : null}

      {saveError ? (
        <div className="notice notice--danger" role="alert">
          <Icon name="alert" size={14} />
          <span>Failed to save conversation: {saveError}</span>
          <button type="button" className="notice__action" onClick={dismissSaveError}>
            Retry save
          </button>
        </div>
      ) : null}

      {loadError ? (
        <div className="notice notice--danger" role="alert">
          <Icon name="alert" size={14} />
          <span>Could not load conversation: {loadError}</span>
          <button type="button" className="notice__action" onClick={dismissLoadError}>
            Dismiss
          </button>
        </div>
      ) : null}

      <div
        className={`conversation${showIntro ? " conversation--intro" : ""}`}
        ref={scrollRef}
        role="region"
        aria-label="Conversation messages"
        tabIndex={0}
        onScroll={handleScroll}
      >
        {conversationLoading ? (
          <p role="status">Loading conversation…</p>
        ) : hasMessages ? (
          <MessageList messages={messages} />
        ) : showIntro ? (
          <EmptyState
            showModelLink={desktop && !engineReady && !statusError}
            note={statusError ? "Engine status unavailable. Retry the connection above." : !desktop ? "Local inference requires the Nexora desktop app." : !engineReady ? "No model loaded. Set up a model in Local Models to start chatting." : null}
            onPick={(prompt) => {
              setDraft(prompt);
              dismissError();
              textareaRef.current?.focus();
            }}
          />
        ) : null}
      </div>

      {workspace.contextFiles.length > 0 ? (
        <div className="context-bar" role="status" aria-label="Selected project context">
          <div className="context-bar__label">
            <Icon name="file" size={12} />
            <span>
              {workspace.contextFiles.length} selected file
              {workspace.contextFiles.length !== 1 ? "s" : ""}
            </span>
          </div>
          <ul className="context-bar__list">
            {workspace.contextFiles.map((f) => (
              <li key={f.path} className="context-bar__item" title={f.path}>
                <span>{f.path}</span>
                <button
                  type="button"
                  className="context-bar__remove"
                  aria-label={`Remove ${f.path} from context`}
                  onClick={() => void workspace.toggleContext(f.path)}
                >
                  <Icon name="x" size={10} />
                </button>
              </li>
            ))}
          </ul>
          <button type="button" className="context-bar__clear" onClick={workspace.clearContext}>
            Clear
          </button>
        </div>
      ) : null}

      {allowanceExhausted && usage.status?.policyConfigured ? (
        <div className="chat-limit" role="alert">
          <p>Your today's limit is over.</p>
          <RouterLink to="plans" className="button button--ghost button--sm">Upgrade to Plus</RouterLink>
        </div>
      ) : usage.error ? (
        <p className="chat-usage-warning" role="alert">Local usage is unavailable. Check the desktop engine before sending.</p>
      ) : null}

      <Composer
        modelSelector={desktop ? (
          <ModelSelector
            catalog={models}
            disabled={streaming || checkingMedia}
            activeConversationId={activeId}
            onModelLoaded={onModelLoaded}
          />
        ) : null}
        mode={composerMode}
        onModeChange={(mode) => {
          if (streaming || checkingMedia) return;
          setComposerMode(mode);
        }}
        checkingMedia={checkingMedia || imageReferenceBusy}
        allowSubmitWithoutModel={mediaSubmitAllowed || imageTaskSubmitAllowed}
        imageReference={imageReference}
        imageReferenceBusy={imageReferenceBusy}
        imageReferenceError={imageReferenceError}
        onPickImageReference={(file) => void handlePickImageReference(file)}
        onRemoveImageReference={() => void handleRemoveImageReference()}
        value={draft}
        error={error}
        textareaRef={textareaRef}
        busy={streaming}
        canGenerate={codingModelReady}
        blockedReason={blockedReason}
        onOpenProjects={() => navigateTo("projects")}
        onChange={setDraft}
        onSubmit={submitComposer}
        onStop={stop}
        onDismissError={dismissError}
      />
    </>
  );
}
