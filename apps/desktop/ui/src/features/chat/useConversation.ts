import { useCallback, useRef, useState } from "react";
import {
  submitDraft,
  type ChatMessage,
  type ComposerError,
} from "./chat";
import { appendForConversation, shouldIgnoreSubmission } from "./conversation";

let fallbackId = 0;

function createMessageId(): string {
  if (typeof crypto !== "undefined" && "randomUUID" in crypto) {
    return crypto.randomUUID();
  }

  fallbackId += 1;
  return `message-${Date.now()}-${fallbackId}`;
}

export interface ConversationController {
  messages: ChatMessage[];
  draft: string;
  error: ComposerError | null;
  setDraft: (value: string) => void;
  dismissError: () => void;
  submit: () => void;
  newChat: () => void;
}

/**
 * Session-scoped conversation state.
 *
 * - Messages carry stable ids generated once, at creation.
 * - A ref-based guard rejects a second submit before React has re-rendered,
 *   which is what a double-click or key repeat would otherwise cause.
 * - Every new chat rotates the conversation id, so a turn produced for the
 *   previous conversation can never be appended to the new one.
 *
 * Nothing is persisted; this is deliberately in-memory only.
 */
export function useConversation(): ConversationController {
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [draft, setDraftState] = useState("");
  const [error, setError] = useState<ComposerError | null>(null);

  const messagesRef = useRef<ChatMessage[]>([]);
  const draftRef = useRef("");
  const conversationRef = useRef(createMessageId());
  const lastSubmitAtRef = useRef(0);

  const setDraft = useCallback((value: string) => {
    draftRef.current = value;
    setDraftState(value);
  }, []);

  const dismissError = useCallback(() => {
    setError(null);
  }, []);

  const submit = useCallback(() => {
    const now = Date.now();

    if (shouldIgnoreSubmission(lastSubmitAtRef.current, now)) {
      return;
    }

    const capturedConversationId = conversationRef.current;
    const outcome = submitDraft(messagesRef.current, draftRef.current, {
      createId: createMessageId,
      now: () => now,
    });

    if (!outcome.ok) {
      setError(outcome.error);
      return;
    }

    const additions = outcome.messages.slice(messagesRef.current.length);
    const next = appendForConversation(
      messagesRef.current,
      additions,
      capturedConversationId,
      conversationRef.current,
    );

    lastSubmitAtRef.current = now;
    messagesRef.current = next;
    draftRef.current = "";

    setMessages(next);
    setDraftState("");
    setError(null);
  }, []);

  const newChat = useCallback(() => {
    conversationRef.current = createMessageId();
    messagesRef.current = [];
    draftRef.current = "";
    lastSubmitAtRef.current = 0;

    setMessages([]);
    setDraftState("");
    setError(null);
  }, []);

  return {
    messages,
    draft,
    error,
    setDraft,
    dismissError,
    submit,
    newChat,
  };
}
