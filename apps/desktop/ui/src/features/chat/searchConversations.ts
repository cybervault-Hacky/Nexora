import {
  loadConversation,
  type ConversationSummary,
} from "../../lib/history";

export interface ConversationSearchHit {
  conversation: ConversationSummary;
  matchSource: "recent" | "title" | "preview" | "message";
  snippet: string;
}

const MAX_SNIPPET_LENGTH = 140;

/**
 * Extracts a short, single-line excerpt centered around the first match in `text`.
 */
export function buildMatchSnippet(text: string, query: string): string {
  const collapsed = text.replace(/\s+/g, " ").trim();
  if (collapsed.length === 0) return "";
  if (collapsed.length <= MAX_SNIPPET_LENGTH) return collapsed;

  const lower = collapsed.toLowerCase();
  const needle = query.trim().toLowerCase();
  const idx = needle.length > 0 ? lower.indexOf(needle) : 0;

  if (idx <= 0) {
    return `${collapsed.slice(0, MAX_SNIPPET_LENGTH - 1)}…`;
  }

  const half = Math.floor((MAX_SNIPPET_LENGTH - needle.length) / 2);
  const start = Math.max(0, idx - Math.max(16, half));
  const end = Math.min(collapsed.length, start + MAX_SNIPPET_LENGTH);
  const prefix = start > 0 ? "…" : "";
  const suffix = end < collapsed.length ? "…" : "";
  return `${prefix}${collapsed.slice(start, end).trim()}${suffix}`;
}

/**
 * Computes fast synchronous matches from local conversation summaries (`title` + `preview`).
 */
export function filterSummariesLocally(
  conversations: readonly ConversationSummary[],
  rawQuery: string,
): ConversationSearchHit[] {
  const query = rawQuery.trim().toLowerCase();
  if (query.length === 0) {
    return conversations.map((conversation) => ({
      conversation,
      matchSource: "recent",
      snippet: conversation.preview,
    }));
  }

  const hits: ConversationSearchHit[] = [];
  for (const conversation of conversations) {
    const inTitle = conversation.title.toLowerCase().includes(query);
    const inPreview = conversation.preview.toLowerCase().includes(query);
    if (inTitle) {
      hits.push({
        conversation,
        matchSource: "title",
        snippet: conversation.preview,
      });
    } else if (inPreview) {
      hits.push({
        conversation,
        matchSource: "preview",
        snippet: buildMatchSnippet(conversation.preview, query),
      });
    }
  }
  return hits;
}

/**
 * Searches all local conversations (titles, previews, and locally persisted
 * message content via `loadConversation`) without making any network requests.
 *
 * Callers pass `isCancelled` so superseded queries stop reading remaining
 * conversations and never overwrite newer results.
 */
export async function searchLocalConversations(
  conversations: readonly ConversationSummary[],
  rawQuery: string,
  isCancelled: () => boolean = () => false,
  loadById: typeof loadConversation = loadConversation,
): Promise<ConversationSearchHit[]> {
  const query = rawQuery.trim().toLowerCase();
  if (query.length === 0) {
    return filterSummariesLocally(conversations, "");
  }

  const titleHits: ConversationSearchHit[] = [];
  const messageHits: ConversationSearchHit[] = [];

  for (const conversation of conversations) {
    if (isCancelled()) {
      return [];
    }

    const inTitle = conversation.title.toLowerCase().includes(query);
    const inPreview = conversation.preview.toLowerCase().includes(query);

    let matchedMessageSnippet: string | null = null;
    try {
      const full = await loadById(conversation.id);
      if (full && Array.isArray(full.messages)) {
        for (let i = full.messages.length - 1; i >= 0; i -= 1) {
          const msg = full.messages[i];
          if (msg && typeof msg.content === "string" && msg.content.toLowerCase().includes(query)) {
            matchedMessageSnippet = buildMatchSnippet(msg.content, query);
            break;
          }
        }
      }
    } catch {
      // Individual unreadable or missing records fall back safely to summary fields.
    }

    if (inTitle) {
      titleHits.push({
        conversation,
        matchSource: "title",
        snippet: matchedMessageSnippet ?? conversation.preview,
      });
    } else if (matchedMessageSnippet !== null) {
      messageHits.push({
        conversation,
        matchSource: "message",
        snippet: matchedMessageSnippet,
      });
    } else if (inPreview) {
      messageHits.push({
        conversation,
        matchSource: "preview",
        snippet: buildMatchSnippet(conversation.preview, query),
      });
    }
  }

  return [...titleHits, ...messageHits];
}
