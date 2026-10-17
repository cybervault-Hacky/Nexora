import type { ChatMessage } from "./chat";

/**
 * A rapid second submit (double-click, key repeat, or a second Enter before
 * React has re-rendered with an empty draft) is ignored inside this window.
 */
export const SUBMISSION_GUARD_MS = 350;

export function shouldIgnoreSubmission(
  lastSubmitAt: number,
  now: number,
  guardMs: number = SUBMISSION_GUARD_MS,
): boolean {
  if (lastSubmitAt <= 0) {
    return false;
  }

  const elapsed = now - lastSubmitAt;

  if (elapsed < 0) {
    return false;
  }

  return elapsed < guardMs;
}

/** Immutably replaces the message with `id`, if it exists. */
export function patchMessage(
  messages: ChatMessage[],
  id: string,
  patch: Partial<ChatMessage>,
): ChatMessage[] {
  const index = messages.findIndex((message) => message.id === id);

  if (index === -1) {
    return messages;
  }

  const current = messages[index];
  const next = { ...current, ...patch } as ChatMessage;

  if (next === current) {
    return messages;
  }

  const copy = messages.slice();
  copy[index] = next;

  return copy;
}

/**
 * Appends streamed text to one assistant reply.
 *
 * Returns the same array when the target is gone, so a delta that arrives after
 * a new chat cannot resurrect an old reply.
 */
export function appendDelta(
  messages: ChatMessage[],
  id: string,
  text: string,
): ChatMessage[] {
  const current = messages.find((message) => message.id === id);

  if (!current || current.state !== "streaming") {
    return messages;
  }

  return patchMessage(messages, id, { content: current.content + text });
}

export function removeMessage(
  messages: ChatMessage[],
  id: string,
): ChatMessage[] {
  const next = messages.filter((message) => message.id !== id);

  return next.length === messages.length ? messages : next;
}

/**
 * True when a stream result still belongs to the conversation on screen.
 */
export function isCurrentConversation(
  capturedConversationId: string,
  activeConversationId: string,
): boolean {
  return capturedConversationId === activeConversationId;
}
