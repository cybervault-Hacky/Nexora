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

  // A backwards clock is not a reason to lock the composer forever.
  if (elapsed < 0) {
    return false;
  }

  return elapsed < guardMs;
}

/**
 * Appends a completed turn only when it belongs to the conversation that is
 * still on screen. A reply produced for an earlier conversation can never
 * leak into a new one.
 */
export function appendForConversation(
  current: ChatMessage[],
  additions: ChatMessage[],
  capturedConversationId: string,
  activeConversationId: string,
): ChatMessage[] {
  if (capturedConversationId !== activeConversationId || additions.length === 0) {
    return current;
  }

  return [...current, ...additions];
}
