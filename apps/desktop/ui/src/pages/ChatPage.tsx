import { useCallback, useEffect, useRef, useState, type RefObject } from "react";
import { Composer } from "../components/Composer";
import { ContextBudgetBar } from "../components/ContextBudgetBar";
import { EmptyState } from "../components/EmptyState";
import { EvidencePanel } from "../components/EvidencePanel";
import { Icon } from "../components/Icon";
import { MessageList } from "../components/MessageList";
import { RetrievalBar } from "../components/RetrievalBar";
import { RouterLink } from "../components/RouterLink";
import { planContextBudget } from "../features/workspace/contextBudget";
import type { RetrievalResult } from "../features/workspace/retrieval";
import type { buildEvidenceReport } from "../lib/evidence";
import type { AppStatus } from "../lib/ipc";
import { isNearBottom } from "../lib/scroll";
import type { usePersistentConversation } from "../features/chat/usePersistentConversation";
import type { useWorkspace } from "../features/workspace/useWorkspace";

type ConversationState = ReturnType<typeof usePersistentConversation>;
type WorkspaceState = ReturnType<typeof useWorkspace>;
type ContextPlan = ReturnType<typeof planContextBudget>;
type EvidenceReport = ReturnType<typeof buildEvidenceReport>;

interface ChatPageProps {
  conversation: ConversationState;
  workspace: WorkspaceState;
  engineStatus: AppStatus | null;
  statusError: string | null;
  engineReady: boolean;
  desktop: boolean;
  /** Prompt-grounding tools computed once in the shell so other pages can reuse them. */
  contextPlan: ContextPlan | null;
  retrievalResult: RetrievalResult | null;
  evidenceReport: EvidenceReport | null;
  /** Submitted from the shell (Ctrl+L) after navigating here. */
  textareaRef: RefObject<HTMLTextAreaElement | null>;
  focusNonce: number;
  onSubmit: () => void;
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
 * The AI Chat page: conversation transcript, composer, and prompt-grounding
 * tools only. Project browsing and patch work live on the Projects page.
 */
export function ChatPage({
  conversation,
  workspace,
  engineStatus,
  statusError,
  engineReady,
  desktop,
  contextPlan,
  retrievalResult,
  evidenceReport,
  textareaRef,
  focusNonce,
  onSubmit,
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
  const conversationTitle = activeId
    ? activeTitle
    : hasMessages
      ? "Local conversation"
      : "New conversation";
  // An unreadable status is "unknown", not "no model": never show setup then.
  const showSetup = !engineReady && !hasMessages && statusError === null;

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
      // error surfaced via saveError
    }
  }, [renameValue, activeId, renameActive]);

  const handleDeleteWithConfirm = useCallback(async () => {
    if (!activeId) return;
    // Simple confirm dialog — explicit UI confirmation required
    const confirmed = window.confirm(`Delete conversation "${activeTitle}"? This cannot be undone.`);
    if (!confirmed) return;
    try {
      await deleteActive();
    } catch {
      // surfaced
    }
  }, [activeId, activeTitle, deleteActive]);

  return (
    <>
      <header className="conversation-header">
        <div className="conversation-header__titles">
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
        className="conversation"
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
        ) : showSetup ? (
          <div className="empty">
            <div className="empty__mark" aria-hidden="true">
              <Icon name="cloud-off" size={30} />
            </div>

            {desktop ? (
              <>
                <h1 className="empty__title">Bring your own model.</h1>
                <p className="empty__copy">
                  Nexora runs entirely on this PC. Point it at a llama.cpp build
                  and a GGUF model you already have. Nothing is downloaded and no
                  prompt leaves the machine.
                </p>
                <p className="empty__note">
                  Model setup lives on the Local Models page.
                </p>
                <RouterLink to="models" className="button button--primary empty__cta">
                  <Icon name="chip" size={14} />
                  Open local model manager
                </RouterLink>
              </>
            ) : (
              <>
                <h1 className="empty__title">Desktop app required.</h1>
                <p className="empty__copy">
                  This is the browser preview of the Nexora interface. Local
                  inference needs the Tauri desktop build, because the engine is
                  a separate process on your machine that a browser tab cannot
                  start or reach.
                </p>
                <p className="empty__note">
                  The window itself is the real UI — same components, same
                  layout, and the same honest engine reporting.
                </p>
              </>
            )}
          </div>
        ) : statusError ? (
          <div className="empty">
            <div className="empty__mark" aria-hidden="true">
              <Icon name="alert" size={28} />
            </div>
            <h1 className="empty__title">Engine state unavailable.</h1>
            <p className="empty__copy">
              The desktop core did not answer, so Nexora cannot tell whether a
              model is loaded. It will not guess — retry the status read above.
            </p>
          </div>
        ) : (
          <EmptyState
            onPick={(prompt) => {
              setDraft(prompt);
              dismissError();
              textareaRef.current?.focus();
            }}
          />
        )}
      </div>

      {retrievalResult ? (
        <RetrievalBar
          retrieval={retrievalResult}
          selectedPaths={workspace.selectedPaths}
          onAdd={(p) => void workspace.toggleContext(p)}
        />
      ) : null}
      {contextPlan ? <ContextBudgetBar plan={contextPlan} /> : null}

      {workspace.contextFiles.length > 0 ? (
        <div className="context-bar" role="status" aria-label="Selected project context">
          <div className="context-bar__label">
            <Icon name="file" size={12} />
            <span>
              Context: {workspace.contextFiles.length} file{workspace.contextFiles.length !== 1 ? "s" : ""} — read-only, not modifying files
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

      {/* Phase 17: concise evidence transparency near composer — reuses tokens, not noisy, shows for current draft */}
      {evidenceReport && (draft.trim().length > 0 || workspace.contextFiles.length > 0 || streaming) ? (
        <div className="composer__evidence">
          <EvidencePanel report={evidenceReport} isStreaming={streaming} />
        </div>
      ) : null}

      <Composer
        value={draft}
        error={error}
        textareaRef={textareaRef}
        busy={streaming}
        canGenerate={engineReady && !conversationLoading}
        onChange={setDraft}
        onSubmit={() => {
          stickToBottomRef.current = true;
          onSubmit();
        }}
        onStop={stop}
        onDismissError={dismissError}
      />
    </>
  );
}
