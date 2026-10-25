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
import type { AppStatus } from "../lib/ipc";
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
  const scrollRef = useRef<HTMLDivElement>(null);
  const stickToBottomRef = useRef(true);

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
            disabled={streaming}
            activeConversationId={activeId}
            onModelLoaded={onModelLoaded}
          />
        ) : null}
        value={draft}
        error={error}
        textareaRef={textareaRef}
        busy={streaming}
        canGenerate={
          engineReady &&
          !conversationLoading &&
          !allowanceExhausted &&
          !models.selectionError &&
          !missingApprovedSelection
        }
        blockedReason={
          allowanceExhausted
            ? "Daily free allowance reached — it resets at 00:00 UTC"
            : models.selectionError
              ? models.selectionError
              : missingApprovedSelection
                ? models.loading
                  ? "Waiting for an approved local model profile"
                  : "Select an approved local model before sending"
                : null
        }
        onOpenProjects={() => navigateTo("projects")}
        onChange={setDraft}
        onSubmit={() => {
          if (
            !models.selected ||
            !isApprovedProfileId(models.selected.profile.id) ||
            models.selectionError
          ) {
            return;
          }
          stickToBottomRef.current = true;
          onSubmit();
        }}
        onStop={stop}
        onDismissError={dismissError}
      />
    </>
  );
}
