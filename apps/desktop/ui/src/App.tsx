import { useCallback, useEffect, useRef, useState } from "react";
import { Composer } from "./components/Composer";
import { EmptyState } from "./components/EmptyState";
import { Icon } from "./components/Icon";
import { MessageList } from "./components/MessageList";
import { ModelSetup } from "./components/ModelSetup";
import { Sidebar } from "./components/Sidebar";
import { TitleBar } from "./components/TitleBar";
import { useConversation } from "./features/chat/useConversation";
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
    messages,
    draft,
    error,
    runtimeError,
    streaming,
    setDraft,
    dismissError,
    dismissRuntimeError,
    submit,
    stop,
    newChat,
  } = useConversation({ onSettled: handleSettled });

  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);
  const [loadedStatus, setLoadedStatus] = useState<AppStatus | null>(null);
  const [unloading, setUnloading] = useState(false);
  const [engineError, setEngineError] = useState<string | null>(null);

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
  const conversationTitle = hasMessages ? "Local conversation" : "New conversation";
  // An unreadable status is "unknown", not "no model": never show setup then.
  const showSetup = !engineReady && !hasMessages && statusError === null;

  return (
    <div className={`app ${sidebarCollapsed ? "app--collapsed" : ""}`}>
      <TitleBar
        conversationTitle={conversationTitle}
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
      />

      <main className="workspace" aria-label="Conversation workspace">
        <header className="conversation-header">
          <div className="conversation-header__titles">
            <p className="eyebrow">Conversation</p>
            <h1 className="conversation-header__title">{conversationTitle}</h1>
          </div>

          <div className="conversation-header__chips">
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

                  <ModelSetup status={engineStatus} onStatus={handleStatus} />
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
