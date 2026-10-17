import { useCallback, useEffect, useRef, useState } from "react";
import { Composer } from "./components/Composer";
import { EmptyState } from "./components/EmptyState";
import { Icon } from "./components/Icon";
import { MessageList } from "./components/MessageList";
import { Sidebar } from "./components/Sidebar";
import { TitleBar } from "./components/TitleBar";
import {
  submitDraft,
  type ChatMessage,
  type ComposerError,
} from "./features/chat/chat";
import {
  browserPreviewStatus,
  loadAppStatus,
  type AppStatus,
} from "./lib/ipc";

let fallbackId = 0;

function createId(): string {
  if (typeof crypto !== "undefined" && "randomUUID" in crypto) {
    return crypto.randomUUID();
  }

  fallbackId += 1;
  return `message-${Date.now()}-${fallbackId}`;
}

function unavailableStatus(): AppStatus {
  return {
    ...browserPreviewStatus(),
    engine: {
      ...browserPreviewStatus().engine,
      detail:
        "The desktop core did not return a status. The interface is running, but the engine state is unknown.",
    },
  };
}

export default function App() {
  const [status, setStatus] = useState<AppStatus | null>(null);
  const [statusFailed, setStatusFailed] = useState(false);
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [draft, setDraft] = useState("");
  const [error, setError] = useState<ComposerError | null>(null);

  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const endRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    let active = true;

    loadAppStatus()
      .then((next) => {
        if (active) {
          setStatus(next);
        }
      })
      .catch(() => {
        if (active) {
          setStatus(unavailableStatus());
          setStatusFailed(true);
        }
      });

    return () => {
      active = false;
    };
  }, []);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.ctrlKey && event.key.toLowerCase() === "l") {
        event.preventDefault();
        textareaRef.current?.focus();
      }
    };

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  useEffect(() => {
    if (messages.length === 0) {
      return;
    }

    const reducedMotion = window.matchMedia(
      "(prefers-reduced-motion: reduce)",
    ).matches;

    endRef.current?.scrollIntoView({
      block: "end",
      behavior: reducedMotion ? "auto" : "smooth",
    });
  }, [messages]);

  const focusComposer = useCallback(() => {
    requestAnimationFrame(() => textareaRef.current?.focus());
  }, []);

  const handleSubmit = useCallback(() => {
    const outcome = submitDraft(messages, draft, {
      createId,
      now: () => Date.now(),
    });

    if (!outcome.ok) {
      setError(outcome.error);
      return;
    }

    setMessages(outcome.messages);
    setDraft("");
    setError(null);
    focusComposer();
  }, [draft, focusComposer, messages]);

  const handleNewChat = useCallback(() => {
    setMessages([]);
    setDraft("");
    setError(null);
    focusComposer();
  }, [focusComposer]);

  const engineConnected = status?.engine.inferenceAvailable === true;
  const hasMessages = messages.length > 0;

  return (
    <div className="app">
      <TitleBar />

      <Sidebar status={status} onNewChat={handleNewChat} />

      <main className="workspace" aria-label="Conversation workspace">
        <header className="conversation-header">
          <div>
            <p className="eyebrow">Conversation</p>
            <h1 className="conversation-header__title">New conversation</h1>
          </div>

          <div className="conversation-header__chips">
            <span className="chip">
              <Icon name="cloud-off" size={13} />
              Local-only
            </span>
            <span
              className={`chip ${engineConnected ? "chip--ok" : "chip--warning"}`}
            >
              <span className="status-dot status-dot--sm" aria-hidden="true" />
              {engineConnected ? "Engine ready" : "Engine not connected"}
            </span>
          </div>
        </header>

        {statusFailed ? (
          <div className="notice" role="status">
            <Icon name="alert" size={14} />
            The desktop core did not return a status. Engine state is unknown.
          </div>
        ) : null}

        <div className={`conversation ${hasMessages ? "conversation--active" : ""}`}>
          {hasMessages ? (
            <MessageList messages={messages} />
          ) : (
            <EmptyState
              onPick={(prompt) => {
                setDraft(prompt);
                setError(null);
                focusComposer();
              }}
            />
          )}
          <div ref={endRef} />
        </div>

        <Composer
          value={draft}
          error={error}
          textareaRef={textareaRef}
          onChange={setDraft}
          onSubmit={handleSubmit}
          onDismissError={() => setError(null)}
        />
      </main>
    </div>
  );
}
