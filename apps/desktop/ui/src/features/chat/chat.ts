import type { ChatTurn } from "../../lib/ipc";

export const MAX_MESSAGE_LENGTH = 32_000;
export const WARN_MESSAGE_LENGTH = 4_000;

export type ChatRole = "user" | "assistant";

export type MessageState = "streaming" | "complete" | "stopped" | "error";

export interface ChatMessage {
  id: string;
  role: ChatRole;
  content: string;
  createdAt: number;
  state: MessageState;
  /** Present on assistant replies so a stale stream can be matched. */
  requestId?: string;
  /** Explains a failed reply without a second source of truth. */
  failure?: string;
}

export type ComposerErrorCode = "empty" | "too_long" | "busy";

export interface ComposerError {
  code: ComposerErrorCode;
  message: string;
}

const EMPTY_ERROR: ComposerError = {
  code: "empty",
  message: "Enter a message before sending.",
};

const TOO_LONG_ERROR: ComposerError = {
  code: "too_long",
  message: `Messages are limited to ${MAX_MESSAGE_LENGTH.toLocaleString(
    "en-US",
  )} characters.`,
};

export const BUSY_ERROR: ComposerError = {
  code: "busy",
  message: "A reply is still being generated. Stop it before sending again.",
};

export function validateDraft(draft: string):
  | { ok: true; text: string }
  | { ok: false; error: ComposerError } {
  const text = draft.replace(/\r\n/g, "\n").trim();

  if (text.length === 0) {
    return { ok: false, error: EMPTY_ERROR };
  }

  if (text.length > MAX_MESSAGE_LENGTH) {
    return { ok: false, error: TOO_LONG_ERROR };
  }

  return { ok: true, text };
}

export function createUserMessage(
  text: string,
  id: string,
  createdAt: number,
): ChatMessage {
  return { id, role: "user", content: text, createdAt, state: "complete" };
}

export function createAssistantMessage(
  id: string,
  requestId: string,
  createdAt: number,
): ChatMessage {
  return {
    id,
    role: "assistant",
    content: "",
    createdAt,
    state: "streaming",
    requestId,
  };
}

/** Reduces the visible conversation to the turns the engine should see. */
export function toChatTurns(messages: readonly ChatMessage[]): ChatTurn[] {
  return messages
    .filter(
      (message) =>
        (message.role === "user" || message.role === "assistant") &&
        message.content.trim().length > 0 &&
        message.state !== "error",
    )
    .map((message) => ({ role: message.role, content: message.content }));
}
