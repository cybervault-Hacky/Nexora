import { useCallback, useEffect, useRef, useState } from "react";
import { Composer } from "./components/Composer";
import { EmptyState } from "./components/EmptyState";
import { Icon } from "./components/Icon";
import { MessageList } from "./components/MessageList";
import { Sidebar } from "./components/Sidebar";
import { TitleBar } from "./components/TitleBar";
import { useConversation } from "./features/chat/useConversation";
import { isEngineReady } from "./lib/ipc";
import { isNearBottom } from "./lib/scroll";
import { useAppStatus } from "./lib/useAppStatus";

function engineChipLabel(
  ready: boolean,
  failed: boolean,
): string {
  if (failed) {
    return "Engine status unknown";
  }

  return ready ? "Engine ready" : "Engine not connected";
}

export default function App() {
  const {
    messages,
    draft,
    error,
    setDraft,
    dismissError,
    submit,
    newChat,
  } = useConversation();
  const { status, error: statusError, loading, reload } = useAppStatus();

  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);

  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const scrollRef = useRef<HTMLDivElement>(null);
  const stickToBottomRef = useRef(true);

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

  useEffect(() => {
    focusComposer();
  }, [focusComposer]);

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

  const engineReady = isEngineReady(status);
  const hasMessages = messages.length > 0;
  const conversationTitle = hasMessages ? "Scaffold conversation" : "New conversation";

  return (
    <div className={`app ${sidebarCollapsed ? "app--collapsed" : ""}`}>
      <TitleBar
        conversationTitle={conversationTitle}
        sidebarCollapsed={sidebarCollapsed}
        onToggleSidebar={() => setSidebarCollapsed((current) => !current)}
      />

      <Sidebar
        status={status}
        statusFailed={statusError !== null}
        collapsed={sidebarCollapsed}
        onNewChat={startNewChat}
      />

      <main className="workspace" aria-label="Conversation workspace">
        <header className="conversation-header">
          <div className="conversation-header__titles">
            <p className="eyebrow">Conversation</p>
            <h1 className="conversation-header__title">{conversationTitle}</h1>
          </div>

          <div className="conversation-header__chips">
            <span className="chip" title="Inference is designed to run on this machine">
              <Icon name="cloud-off" size={13} />
              Local-only
            </span>
            <span
              className={`chip ${
                statusError
                  ? "chip--unknown"
                  : engineReady
                    ? "chip--ok"
                    : "chip--warning"
              }`}
              title={statusError ?? undefined}
            >
              <span className="status-dot status-dot--sm" aria-hidden="true" />
              {engineChipLabel(engineReady, statusError !== null)}
            </span>
          </div>
        </header>

        {statusError ? (
          <div className="notice" role="status">
            <Icon name="alert" size={14} />
            <span>
              Could not read the engine status from the desktop core. It is not
              known whether an engine is available.
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

        <div
          className="conversation"
          ref={scrollRef}
          onScroll={handleScroll}
        >
          {hasMessages ? (
            <MessageList messages={messages} />
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
          onChange={setDraft}
          onSubmit={handleSubmit}
          onDismissError={dismissError}
        />
      </main>
    </div>
  );
}
