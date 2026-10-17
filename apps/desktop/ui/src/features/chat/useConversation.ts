import { useCallback, useRef, useState } from "react";
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
}

export interface ConversationController {
  messages: ChatMessage[];
  draft: string;
  error: ComposerError | null;
  runtimeError: string | null;
  streaming: boolean;
  setDraft: (value: string) => void;
  dismissError: () => void;
  dismissRuntimeError: () => void;
  submit: () => void;
  stop: () => void;
  newChat: () => void;
}

interface Options {
  /** Called whenever a generation settles, so the caller can refresh status. */
  onSettled?: () => void;
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
export function useConversation({ onSettled }: Options = {}): ConversationController {
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

  const commit = useCallback((next: ChatMessage[]) => {
    messagesRef.current = next;
    setMessages(next);
  }, []);

  const setDraft = useCallback((value: string) => {
    draftRef.current = value;
    setDraftState(value);
  }, []);

  const dismissError = useCallback(() => setError(null), []);
  const dismissRuntimeError = useCallback(() => setRuntimeError(null), []);

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

    const session: StreamSession = {
      requestId: createId(),
      assistantId: createId(),
      conversationId: conversationRef.current,
    };

    const userMessage = createUserMessage(validation.text, createId(), now);
    const assistantMessage = createAssistantMessage(
      session.assistantId,
      session.requestId,
      now,
    );
    const history = [...messagesRef.current, userMessage];

    lastSubmitAtRef.current = now;
    draftRef.current = "";
    setDraftState("");
    setError(null);
    setRuntimeError(null);
    commit([...history, assistantMessage]);

    sessionRef.current = session;
    setStreaming(true);

    const turns = toChatTurns(history);

    const isLive = () => sessionRef.current === session;

    const settle = (outcome: GenerationOutcome) => {
      if (!isLive()) {
        return;
      }

      if (!isCurrentConversation(session.conversationId, conversationRef.current)) {
        return;
      }

      const current = messagesRef.current.find(
        (message) => message.id === session.assistantId,
      );

      if (!current) {
        return;
      }

      if (outcome.cancelled) {
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
      const failure = toInferenceError(cause);

      if (!isLive()) {
        return;
      }

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
          },
          (event) => {
            if (event.type !== "delta" || !event.text) {
              return;
            }

            if (!isLive() || event.requestId !== session.requestId) {
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
          setStreaming(false);
          onSettled?.();
        }
      }
    })();
  }, [commit, onSettled]);

  const stop = useCallback(() => {
    const session = sessionRef.current;

    if (session) {
      void cancelGeneration(session.requestId);
    }
  }, []);

  const newChat = useCallback(() => {
    const session = sessionRef.current;

    if (session) {
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

  return {
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
  };
}
