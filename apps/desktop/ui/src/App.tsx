import { useCallback, useEffect, useRef, useState } from "react";
import { Composer } from "./components/Composer";
import { EmptyState } from "./components/EmptyState";
import { Icon } from "./components/Icon";
import { MessageList } from "./components/MessageList";
import { ModelSetup } from "./components/ModelSetup";
import { Sidebar } from "./components/Sidebar";
import { TitleBar } from "./components/TitleBar";
import { usePersistentConversation } from "./features/chat/usePersistentConversation";
import {
  unloadModel,
  toInferenceError as describeFailure,
} from "./lib/inference";
import { isDesktopShell, isEngineReady, type AppStatus } from "./lib/ipc";
import { isNearBottom } from "./lib/scroll";
import { useAppStatus } from "./lib/useAppStatus";

export default function App() {
  const { status, error: statusError, loading, reload } = useAppStatus();

  const handleSettled = useCallback(() => {
    reload();
  }, [reload]);

  const {
    conversations,
    activeId,
    activeTitle,
    listLoading,
    listError,
    saveStatus,
    saveError,
    loadError,
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
    refreshList,
    submit,
    stop,
    newChat,
    selectConversation,
    renameActive,
    deleteActive,
    deleteConversationById,
  } = usePersistentConversation({ onSettled: handleSettled });

  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);
  const [loadedStatus, setLoadedStatus] = useState<AppStatus | null>(null);
  const [unloading, setUnloading] = useState(false);
  const [engineError, setEngineError] = useState<string | null>(null);
  const [renameEditing, setRenameEditing] = useState(false);
  const [renameValue, setRenameValue] = useState("");

  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const scrollRef = useRef<HTMLDivElement>(null);
  const stickToBottomRef = useRef(true);

  const engineReady = isEngineReady(loadedStatus ?? status);
  const engineStatus = loadedStatus ?? status;

  const focusComposer = useCallback(() => {
    textareaRef.current?.focus();
  }, []);

  const startNewChat = useCallback(() => {
    newChat();
    stickToBottomRef.current = true;
    requestAnimationFrame(focusComposer);
  }, [focusComposer, newChat]);

  const handleSubmit = useCallback(() => {
    stickToBottomRef.current = true;
    submit();
  }, [submit]);

  const handleStatus = useCallback(
    (next: AppStatus) => {
      setLoadedStatus(next);
      reload();
    },
    [reload],
  );

  const handleUnload = useCallback(() => {
    setUnloading(true);
    setEngineError(null);

    void unloadModel()
      .catch((cause: unknown) => {
        setEngineError(describeFailure(cause).message);
      })
      .finally(() => {
        setUnloading(false);
        reload();
      });
  }, [reload]);

  useEffect(() => {
    focusComposer();
  }, [focusComposer]);

  // The status returned by `load_model` is only a bridge until the next
  // authoritative read; never let it outlive a fresh snapshot.
  useEffect(() => {
    setLoadedStatus(null);
  }, [status]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (!event.ctrlKey || event.altKey || event.metaKey) {
        return;
      }

      switch (event.key.toLowerCase()) {
        case "l":
          event.preventDefault();
          focusComposer();
          break;
        case "n":
          event.preventDefault();
          startNewChat();
          break;
        case "b":
          event.preventDefault();
          setSidebarCollapsed((current) => !current);
          break;
        default:
          break;
      }
    };

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [focusComposer, startNewChat]);

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

  const desktop = isDesktopShell();
  const hasMessages = messages.length > 0;
  const engineChip = describeEngineChip(engineStatus, statusError, engineReady);
  // TitleBar keeps a generic label to avoid duplicating the prompt in a second
  // heading (accessibility / test stability). The conversation header shows the
  // derived title.
  const titleBarLabel = hasMessages ? "Local conversation" : "New conversation";
  const conversationTitle = activeId ? activeTitle : hasMessages ? "Local conversation" : "New conversation";
  // An unreadable status is "unknown", not "no model": never show setup then.
  const showSetup = !engineReady && !hasMessages && statusError === null;

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
    <div className={`app ${sidebarCollapsed ? "app--collapsed" : ""}`}>
      <TitleBar
        conversationTitle={titleBarLabel}
        sidebarCollapsed={sidebarCollapsed}
        onToggleSidebar={() => setSidebarCollapsed((current) => !current)}
      />

      <Sidebar
        status={engineStatus}
        statusFailed={statusError !== null}
        collapsed={sidebarCollapsed}
        onNewChat={startNewChat}
        onUnload={desktop ? handleUnload : undefined}
        unloading={unloading}
        conversations={conversations}
        activeId={activeId}
        historyLoading={listLoading}
        historyError={listError}
        onSelectConversation={selectConversation}
        onRenameConversation={async (id, title) => {
          // If renaming active, use renameActive; otherwise call generic
          if (id === activeId) {
            await renameActive(title);
          } else {
            // For non-active we can still call renameActive path via history lib directly
            // Use the same store path: call deleteConversationById's sibling — we need to handle generically
            // For now delegate to renameActive if active, else use direct invoke via historyList's own logic
            // HistoryList calls onRename which should handle any id; we implement fallback here
            const { renameConversation } = await import("./lib/history");
            await renameConversation(id, title);
            refreshList();
          }
        }}
        onDeleteConversation={deleteConversationById}
        onRetryHistory={refreshList}
      />

      <main className="workspace" aria-label="Conversation workspace">
        <header className="conversation-header">
          <div className="conversation-header__titles">
            <p className="eyebrow">Conversation</p>
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
                <button type="button" className="button button--ghost button--sm" onClick={() => void handleRenameSubmit()}>
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
          </div>
        </header>

        {statusError ? (
          <div className="notice" role="status">
            <Icon name="alert" size={14} />
            <span>
              Could not read the engine status from the desktop core. It is not
              known whether a model is available.
            </span>
            <button
              type="button"
              className="notice__action"
              onClick={reload}
              disabled={loading}
            >
              {loading ? "Retrying…" : "Retry"}
            </button>
          </div>
        ) : null}

        {engineError ? (
          <div className="notice notice--danger" role="alert">
            <Icon name="alert" size={14} />
            <span>{engineError}</span>
            <button
              type="button"
              className="notice__action"
              onClick={() => setEngineError(null)}
            >
              Dismiss
            </button>
          </div>
        ) : null}

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
              Dismiss
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

        <div className="conversation" ref={scrollRef} onScroll={handleScroll}>
          {hasMessages ? (
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

                  <ModelSetup
                    status={engineStatus}
                    onStatus={handleStatus}
                    onRefresh={reload}
                  />
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
                focusComposer();
              }}
            />
          )}
        </div>

        <Composer
          value={draft}
          error={error}
          textareaRef={textareaRef}
          busy={streaming}
          canGenerate={engineReady}
          onChange={setDraft}
          onSubmit={handleSubmit}
          onStop={stop}
          onDismissError={dismissError}
        />
      </main>
    </div>
  );
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
