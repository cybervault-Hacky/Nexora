import { useCallback, useEffect, useRef, useState } from "react";
import {
  cancelGeneration,
  generate,
  toInferenceError,
} from "../../lib/inference";
import type { GenerationOutcome } from "../../lib/ipc";
import {
  BUSY_ERROR,
  createAssistantMessage,
  createUserMessage,
  toChatTurns,
  validateDraft,
  type ChatMessage,
  type ComposerError,
} from "./chat";
import {
  appendDelta,
  isCurrentConversation,
  patchMessage,
  planRetry,
  removeMessage,
  shouldIgnoreSubmission,
} from "./conversation";

let fallbackId = 0;

function createId(): string {
  if (typeof crypto !== "undefined" && "randomUUID" in crypto) {
    return crypto.randomUUID();
  }

  fallbackId += 1;
  return `id-${Date.now()}-${fallbackId}`;
}

interface StreamSession {
  requestId: string;
  assistantId: string;
  conversationId: string;
  cancelled: boolean;
  settled: boolean;
}

export interface ConversationController {
  messages: ChatMessage[];
  draft: string;
  error: ComposerError | null;
  runtimeError: string | null;
  streaming: boolean;
  canRetry: boolean;
  setDraft: (value: string) => void;
  dismissError: () => void;
  dismissRuntimeError: () => void;
  submit: () => void;
  retry: () => void;
  stop: () => void;
  newChat: () => void;
}

interface Options {
  /** Called whenever a generation settles, so the caller can refresh status. */
  onSettled?: () => void;
  /** The profile selected in the core; every request names it. */
  profileId?: string | null;
}

/**
 * Session-scoped conversation with real streaming inference.
 *
 * - One generation at a time; a second submit is refused, not queued.
 * - Every stream carries a request id and a conversation id. A delta or a
 *   completion that does not match the live session is dropped, so a reply can
 *   never land in a conversation the user has already left.
 * - Nothing is persisted.
 */
export function useConversation({
  onSettled,
  profileId = null,
}: Options = {}): ConversationController {
  // Read at send time, so a message carries the profile that will answer it.
  const profileRef = useRef<string | null>(profileId);
  profileRef.current = profileId;
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [draft, setDraftState] = useState("");
  const [error, setError] = useState<ComposerError | null>(null);
  const [runtimeError, setRuntimeError] = useState<string | null>(null);
  const [streaming, setStreaming] = useState(false);

  const messagesRef = useRef<ChatMessage[]>([]);
  const draftRef = useRef("");
  const conversationRef = useRef(createId());
  const lastSubmitAtRef = useRef(0);
  const sessionRef = useRef<StreamSession | null>(null);
  const mountedRef = useRef(true);

  const commit = useCallback((next: ChatMessage[]) => {
    messagesRef.current = next;
    setMessages(next);
  }, []);

  useEffect(() => {
    mountedRef.current = true;

    return () => {
      mountedRef.current = false;
      const active = sessionRef.current;
      if (active) {
        active.cancelled = true;
        sessionRef.current = null;
        void cancelGeneration(active.requestId);
      }
    };
  }, []);

  const setDraft = useCallback((value: string) => {
    draftRef.current = value;
    setDraftState(value);
  }, []);

  const dismissError = useCallback(() => setError(null), []);
  const dismissRuntimeError = useCallback(() => setRuntimeError(null), []);

  const launchGeneration = useCallback(
    (history: ChatMessage[], now: number) => {
      const selectedProfile = profileRef.current;
      const session: StreamSession = {
        requestId: createId(),
        assistantId: createId(),
        conversationId: conversationRef.current,
        cancelled: false,
        settled: false,
      };

      const assistantMessage = createAssistantMessage(
        session.assistantId,
        session.requestId,
        now,
        selectedProfile,
      );

      setError(null);
      setRuntimeError(null);
      commit([...history, assistantMessage]);

      sessionRef.current = session;
      setStreaming(true);

      const turns = toChatTurns(history);

      const isLive = () => sessionRef.current === session;

      const settle = (outcome: GenerationOutcome) => {
        if (session.settled || !mountedRef.current || !isLive()) {
          return;
        }
        session.settled = true;

        if (!isCurrentConversation(session.conversationId, conversationRef.current)) {
          return;
        }

        const current = messagesRef.current.find(
          (message) => message.id === session.assistantId,
        );

        if (!current) {
          return;
        }

        if (outcome.cancelled || session.cancelled) {
          // Never invent an assistant turn: an empty stop stays empty so it is
          // excluded from the next request and shown as a system note instead.
          commit(
            patchMessage(messagesRef.current, session.assistantId, {
              state: "stopped",
            }),
          );
          return;
        }

        if (current.content.trim().length === 0) {
          commit(removeMessage(messagesRef.current, session.assistantId));
          setRuntimeError(
            "The model finished without producing any text. Check the model and try again.",
          );
          return;
        }

        commit(
          patchMessage(messagesRef.current, session.assistantId, { state: "complete" }),
        );
      };

      const fail = (cause: unknown) => {
        if (session.settled || !mountedRef.current || !isLive()) {
          return;
        }
        session.settled = true;

        const failure = toInferenceError(cause);

        if (!isCurrentConversation(session.conversationId, conversationRef.current)) {
          return;
        }

        const current = messagesRef.current.find(
          (message) => message.id === session.assistantId,
        );

        if (!current || current.content.trim().length === 0) {
          commit(removeMessage(messagesRef.current, session.assistantId));
        } else {
          commit(
            patchMessage(messagesRef.current, session.assistantId, {
              state: "error",
              failure: failure.message,
            }),
          );
        }

        setRuntimeError(failure.message);
      };

      void (async () => {
        try {
          const outcome = await generate(
            {
              requestId: session.requestId,
              messages: turns,
              maxTokens: 1024,
              temperature: 0.2,
              profileId: selectedProfile,
            },
            (event) => {
              if (event.type !== "delta" || !event.text) {
                return;
              }

              if (
                session.cancelled ||
                session.settled ||
                !isLive() ||
                event.requestId !== session.requestId
              ) {
                return;
              }

              if (!isCurrentConversation(session.conversationId, conversationRef.current)) {
                return;
              }

              commit(appendDelta(messagesRef.current, session.assistantId, event.text));
            },
          );

          settle(outcome);
        } catch (cause) {
          fail(cause);
        } finally {
          if (sessionRef.current === session) {
            sessionRef.current = null;

            // A generation can outlive the component that started it. Writing
            // state is a no-op after unmount, but `onSettled` would still spend an
            // IPC round trip on a tree that no longer exists.
            if (mountedRef.current) {
              setStreaming(false);
              onSettled?.();
            }
          }
        }
      })();
    },
    [commit, onSettled],
  );

  const submit = useCallback(() => {
    if (sessionRef.current) {
      setError(BUSY_ERROR);
      return;
    }

    const now = Date.now();

    if (shouldIgnoreSubmission(lastSubmitAtRef.current, now)) {
      return;
    }

    const validation = validateDraft(draftRef.current);

    if (!validation.ok) {
      setError(validation.error);
      return;
    }

    const userMessage = createUserMessage(validation.text, createId(), now);
    const history = [...messagesRef.current, userMessage];

    lastSubmitAtRef.current = now;
    draftRef.current = "";
    setDraftState("");
    launchGeneration(history, now);
  }, [launchGeneration]);

  const retry = useCallback(() => {
    if (sessionRef.current) {
      setError(BUSY_ERROR);
      return;
    }

    const retryHistory = planRetry(messagesRef.current);
    if (!retryHistory) {
      return;
    }

    const now = Date.now();
    lastSubmitAtRef.current = now;
    launchGeneration(retryHistory, now);
  }, [launchGeneration]);

  const stop = useCallback(() => {
    const session = sessionRef.current;

    if (session) {
      session.cancelled = true;
      commit(
        patchMessage(messagesRef.current, session.assistantId, {
          state: "stopped",
        }),
      );
      void cancelGeneration(session.requestId);
    }
  }, [commit]);

  const newChat = useCallback(() => {
    const session = sessionRef.current;

    if (session) {
      session.cancelled = true;
      void cancelGeneration(session.requestId);
    }

    sessionRef.current = null;
    conversationRef.current = createId();
    messagesRef.current = [];
    draftRef.current = "";
    lastSubmitAtRef.current = 0;

    setMessages([]);
    setDraftState("");
    setError(null);
    setRuntimeError(null);
    setStreaming(false);
  }, []);

  const canRetry = !streaming && planRetry(messages) !== null;

  return {
    messages,
    draft,
    error,
    runtimeError,
    streaming,
    canRetry,
    setDraft,
    dismissError,
    dismissRuntimeError,
    submit,
    retry,
    stop,
    newChat,
  };
}
