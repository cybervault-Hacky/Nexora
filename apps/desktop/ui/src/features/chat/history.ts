import type { ChatMessage } from "./chat";

export const MAX_TITLE_LENGTH = 60;
export const MAX_TITLE_CHARS = 200;

/**
 * Derives a conversation title from the first user prompt.
 * Falls back to "New conversation" if no user message exists.
 * Safe truncation: takes first line, trims, limits to MAX_TITLE_LENGTH with ellipsis.
 */
export function deriveTitle(messages: ChatMessage[], fallback = "New conversation"): string {
  const firstUser = messages.find((m) => m.role === "user" && m.content.trim().length > 0);
  if (!firstUser) return fallback;

  // First line only, collapse whitespace
  const firstLine = firstUser.content.trim().split("\n")[0] ?? "";
  const collapsed = firstLine.replace(/\s+/g, " ").trim();
  if (collapsed.length === 0) return fallback;

  return truncateTitle(collapsed);
}

export function truncateTitle(title: string, maxLen: number = MAX_TITLE_LENGTH): string {
  const trimmed = title.trim();
  if (trimmed.length === 0) return "New conversation";
  if (trimmed.length <= maxLen) return trimmed;
  return trimmed.slice(0, maxLen - 1).trimEnd() + "…";
}

export function sanitizeTitle(title: string): string {
  const t = title.trim().replace(/[\r\n]+/g, " ").replace(/\s+/g, " ").trim();
  if (t.length === 0) return "New conversation";
  return t.slice(0, MAX_TITLE_CHARS);
}

export function isValidTitle(title: string): boolean {
  const t = title.trim();
  return t.length > 0 && t.length <= MAX_TITLE_CHARS;
}

export function formatPreview(content: string, maxLen = 120): string {
  if (!content || content.trim().length === 0) return "";
  const firstLine = content.trim().split("\n")[0] ?? "";
  const collapsed = firstLine.replace(/\s+/g, " ").trim();
  if (collapsed.length <= maxLen) return collapsed;
  return collapsed.slice(0, maxLen - 1).trimEnd() + "…";
}

export function formatTimestamp(timestamp: number): string {
  try {
    const date = new Date(timestamp);
    const now = new Date();
    const isToday = date.toDateString() === now.toDateString();
    if (isToday) {
      return new Intl.DateTimeFormat(undefined, { hour: "2-digit", minute: "2-digit" }).format(date);
    }
    return new Intl.DateTimeFormat(undefined, { month: "short", day: "numeric" }).format(date);
  } catch {
    return "";
  }
}
