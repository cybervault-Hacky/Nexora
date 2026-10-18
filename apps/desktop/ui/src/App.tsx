import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Composer } from "./components/Composer";
import { EmptyState } from "./components/EmptyState";
import { FilePreview } from "./components/FilePreview";
import { Icon } from "./components/Icon";
import { MessageList } from "./components/MessageList";
import { ModelSetup } from "./components/ModelSetup";
import { Sidebar } from "./components/Sidebar";
import { TitleBar } from "./components/TitleBar";
import { WorkspacePanel } from "./components/WorkspacePanel";
import { ContextBudgetBar } from "./components/ContextBudgetBar";
import { PatchEditor } from "./components/PatchEditor";
import { PatchPreview } from "./components/PatchPreview";
import { usePersistentConversation } from "./features/chat/usePersistentConversation";
import { useWorkspace } from "./features/workspace/useWorkspace";
import { usePatch } from "./features/workspace/usePatch";
import { planContextBudget } from "./features/workspace/contextBudget";
import { getWorkspaceSettings, saveWorkspaceSettings } from "./lib/workspaceSettings";
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
    conversationLoading,
    renameById,
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

  const workspace = useWorkspace();
  const patch = usePatch(workspace.workspacePath);

  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);
  const [settingsRestored, setSettingsRestored] = useState(false);
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

  // Context budget: reserve system + output, fit prompt + selected files + recent history
  const contextSize = engineStatus?.contextSize ?? 32768;
  const maxTokens = 1024; // reserved for model output
  const contextPlan = useMemo(() => {
    // History for budget: all messages as role/content, newest trimming
    const hist = messages.map((m) => ({ role: m.role, content: m.content ?? "" }));
    // Prompt for planning is current draft
    const prompt = draft ?? "";
    const files = workspace.contextFiles.map((f) => ({ path: f.path, content: f.content, size: f.size }));
    try {
      return planContextBudget({
        contextSize,
        maxTokens,
        history: hist,
        currentPrompt: prompt,
        selectedFiles: files,
      });
    } catch {
      return null;
    }
  }, [contextSize, messages, draft, workspace.contextFiles]);

  const filteredContextBlock = useMemo(() => {
    if (!contextPlan) return workspace.contextBlock;
    const included = new Set(contextPlan.filePlans.filter((f) => f.included).map((f) => f.path));
    if (included.size === workspace.contextFiles.length) return workspace.contextBlock;
    if (included.size === 0) return null;
    // Rebuild block with only included files, preserving fences/paths
    const files = workspace.contextFiles.filter((f) => included.has(f.path));
    if (files.length === 0) return null;
    const fenceFor = (content: string) => (content.includes("```") ? "````" : "```");
    const inferLang = (path: string) => {
      const ext = path.split(".").pop()?.toLowerCase() ?? "";
      const map: Record<string, string> = {
        rs: "rust",
        ts: "typescript",
        tsx: "tsx",
        js: "javascript",
        jsx: "jsx",
        py: "python",
        json: "json",
        md: "markdown",
        toml: "toml",
        yaml: "yaml",
        yml: "yaml",
        css: "css",
        html: "html",
        sh: "bash",
      };
      return map[ext] ?? "";
    };
    let block = "Consider the following project context (read-only; do not claim to have modified files):\n\n";
    for (const file of files) {
      const lang = inferLang(file.path);
      const fence = fenceFor(file.content);
      block += `\nFile: ${file.path}\n${fence}${lang}\n${file.content}\n${fence}\n`;
    }
    block += "\nUse the context above only as background for your answer.\n";
    return block;
  }, [contextPlan, workspace.contextBlock, workspace.contextFiles]);

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
    const block = filteredContextBlock ?? workspace.contextBlock;
    const hasContext = block !== null && block.length > 0;
    submit(hasContext ? { contextBlock: block } : undefined);
  }, [submit, filteredContextBlock, workspace.contextBlock]);

  // Handle manual path entry from WorkspacePanel (custom event)
  useEffect(() => {
    const handler = (e: Event) => {
      const detail = (e as CustomEvent<string>).detail;
      if (typeof detail === "string" && detail.trim()) {
        void workspace.openWorkspace(detail.trim()).catch(() => {});
      }
    };
    window.addEventListener("workspace:openManual" as never, handler as never);
    return () => window.removeEventListener("workspace:openManual" as never, handler as never);
  }, [workspace]);

  // Workspace settings: restore last path once, then persist on change (no auto-scan)
  useEffect(() => {
    if (settingsRestored) return;
    void getWorkspaceSettings()
      .then((s) => {
        setSettingsRestored(true);
        // Do not auto-scan; only remember for manual open hint if no workspace active
        if (s.lastWorkspacePath && !workspace.workspacePath) {
          // Hint stored; user must explicitly open. We don't auto-open.
        }
      })
      .catch(() => setSettingsRestored(true));
  }, [settingsRestored, workspace.workspacePath]);

  useEffect(() => {
    if (!settingsRestored) return;
    // Persist last path and collapsed state on change
    const pref = { explorerCollapsed: sidebarCollapsed };
    const last = workspace.workspacePath ?? null;
    void saveWorkspaceSettings({ version: 1, lastWorkspacePath: last, preferences: pref }).catch(() => {});
  }, [settingsRestored, workspace.workspacePath, sidebarCollapsed]);

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
        onRenameConversation={renameById}
        onDeleteConversation={deleteConversationById}
        onRetryHistory={refreshList}
      />

      <main className="workspace" aria-label="Conversation workspace">
        <div className="workspace__layout">
          <aside className="workspace__explorer" aria-label="Project explorer">
            <WorkspacePanel
              workspacePath={workspace.workspacePath}
              workspaceName={workspace.workspaceName}
              inventory={workspace.inventory}
              loading={workspace.loading}
              error={workspace.error}
              search={workspace.search}
              onSearchChange={workspace.setSearch}
              filteredEntries={workspace.filteredEntries}
              selectedPaths={workspace.selectedPaths}
              onToggleContext={workspace.toggleContext}
              onClearContext={workspace.clearContext}
              contextError={workspace.contextError}
              onOpen={() => void workspace.openWorkspace().catch(() => {})}
              onRefresh={() => void workspace.refresh().catch(() => {})}
              onClose={() => void workspace.closeWorkspace()}
              onPreview={workspace.loadPreview}
              previewPath={workspace.previewPath}
              collapsed={false}
            />
            <FilePreview
              path={workspace.previewPath}
              content={workspace.preview?.content ?? null}
              size={workspace.preview?.size ?? null}
              loading={workspace.previewLoading}
              error={workspace.previewError}
              onClose={workspace.clearPreview}
            />
            {contextPlan ? (
              <ContextBudgetBar plan={contextPlan} />
            ) : null}
            {/* Patch workflow — read-only until explicit apply */}
            {workspace.previewPath && workspace.preview?.content !== undefined && !workspace.previewLoading && !workspace.previewError ? (
              <div className="workspace__patch" aria-label="Patch workflow">
                {patch.patchState.status === "idle" ? (
                  <button
                    type="button"
                    className="button button--ghost button--sm workspace__propose"
                    disabled={!workspace.workspacePath || workspace.preview?.content?.includes("\0")}
                    onClick={() => {
                      if (workspace.previewPath && workspace.preview?.content !== undefined) {
                        patch.startEditing(workspace.previewPath, workspace.preview!.content);
                      }
                    }}
                    title={!workspace.workspacePath ? "Open a project first" : "Edit a proposal — original file unchanged until you apply"}
                  >
                    <Icon name="edit" size={12} /> Propose change
                  </button>
                ) : patch.patchState.status === "editing" ? (
                  <PatchEditor
                    path={patch.patchState.path}
                    value={patch.editorContent}
                    onChange={patch.updateEditor}
                    onCancel={patch.cancel}
                    onPreview={() => void patch.createProposal()}
                    disabled={patch.editorContent === patch.patchState.originalContent}
                  />
                ) : patch.patchState.status === "creating" ? (
                  <div className="patch-preview" role="status">
                    <p className="patch-preview__status">Creating proposal… Validating single file, checking traversal, sensitive, binary, size…</p>
                  </div>
                ) : patch.patchState.status === "preview" ? (
                  <PatchPreview
                    proposal={patch.patchState.proposal}
                    validation={patch.patchState.validation}
                    onCancel={patch.cancel}
                    onApply={() => void patch.apply()}
                    applying={false}
                    applied={false}
                  />
                ) : patch.patchState.status === "validating" ? (
                  <PatchPreview proposal={patch.patchState.proposal} validation={null} loading onCancel={patch.cancel} onApply={() => {}} />
                ) : patch.patchState.status === "applying" ? (
                  <PatchPreview
                    proposal={patch.patchState.proposal}
                    validation={{ ok: true, errors: [], warnings: [], diffSummary: { added: 0, removed: 0, unchanged: 0, originalLines: 0, proposedLines: 0, truncated: false } }}
                    loading={false}
                    applying
                    onCancel={patch.cancel}
                    onApply={() => {}}
                  />
                ) : patch.patchState.status === "applied" ? (
                  <PatchPreview
                    proposal={patch.patchState.proposal}
                    validation={null}
                    applied
                    onCancel={() => {
                      patch.cancel();
                      void workspace.refresh();
                      if (workspace.previewPath) void workspace.loadPreview(workspace.previewPath);
                    }}
                    onApply={() => {}}
                  />
                ) : patch.patchState.status === "error" ? (
                  <PatchPreview
                    proposal={patch.patchState.proposal ?? null}
                    validation={null}
                    error={patch.patchState.message}
                    onCancel={patch.cancel}
                    onApply={() => {}}
                  />
                ) : null}
                {patch.patchState.status !== "idle" && patch.patchState.status !== "editing" && patch.patchState.status !== "error" ? (
                  <p className="workspace__patch-note">Reviewing a diff does not modify the file. Only Apply changes the intended file. Never auto-applies on generate or switch.</p>
                ) : null}
              </div>
            ) : null}
          </aside>
          <section className="workspace__chat" aria-label="Conversation">
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

        <div className="conversation" ref={scrollRef} onScroll={handleScroll}>
          {conversationLoading ? <p role="status">Loading conversation…</p> : hasMessages ? (
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

            <Composer
              value={draft}
              error={error}
              textareaRef={textareaRef}
              busy={streaming}
              canGenerate={engineReady && !conversationLoading}
              onChange={setDraft}
              onSubmit={handleSubmit}
              onStop={stop}
              onDismissError={dismissError}
            />
          </section>
        </div>
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
